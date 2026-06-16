//! Polite HTTP: an identifying User-Agent, per-host rate limiting, and a
//! best-effort robots.txt gate. Used by the runner; the parse logic in the
//! adapters is pure and tested without any network.

use crate::{Error, Result};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Identifying, contactable User-Agent (politeness — a crawler should say who it
/// is and how to reach us).
pub const DEFAULT_USER_AGENT: &str =
    "PHPressBot/0.1 (+https://predatorhunters.co.uk/standards; court-reporting crawler; \
     contact editor@predatorhunters.co.uk)";

/// A polite HTTP client. One per crawl run.
pub struct Fetcher {
    client: reqwest::Client,
    user_agent: String,
    min_interval: Duration,
    last_hit: Mutex<HashMap<String, Instant>>,
    robots: Mutex<HashMap<String, RobotsRules>>,
}

impl Fetcher {
    pub fn new(user_agent: impl Into<String>) -> Result<Self> {
        let user_agent = user_agent.into();
        let client = reqwest::Client::builder()
            .user_agent(&user_agent)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            user_agent,
            min_interval: Duration::from_secs(2),
            last_hit: Mutex::new(HashMap::new()),
            robots: Mutex::new(HashMap::new()),
        })
    }

    /// Override the minimum gap between requests to the same host.
    pub fn with_min_interval(mut self, d: Duration) -> Self {
        self.min_interval = d;
        self
    }

    /// GET a URL as text, honouring robots.txt and per-host rate limiting.
    pub async fn get_text(&self, url: &str) -> Result<String> {
        let parsed = url::Url::parse(url).map_err(|e| Error::Parse(e.to_string()))?;
        if !self.robots_allows(&parsed).await {
            return Err(Error::Disallowed(url.to_string()));
        }
        let host = parsed.host_str().unwrap_or_default().to_string();
        self.rate_limit(&host).await;
        let resp = self.client.get(url).send().await?.error_for_status()?;
        Ok(resp.text().await?)
    }

    /// Sleep so this host is hit at most once per `min_interval`. Reserves the
    /// slot under the lock so concurrent callers serialise.
    async fn rate_limit(&self, host: &str) {
        let wait = {
            let mut last = self.last_hit.lock().await;
            let now = Instant::now();
            let until = last.get(host).map(|t| *t + self.min_interval);
            let wait = until
                .and_then(|u| u.checked_duration_since(now))
                .unwrap_or_default();
            last.insert(host.to_string(), now + wait);
            wait
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }

    /// Best-effort robots.txt check (cached per host). A fetch failure defaults
    /// to allow; an explicit Disallow that matches is honoured.
    async fn robots_allows(&self, url: &url::Url) -> bool {
        let Some(host) = url.host_str().map(str::to_string) else {
            return false;
        };
        if let Some(rules) = self.robots.lock().await.get(&host) {
            return rules.allows(url.path());
        }
        let robots_url = format!("{}://{}/robots.txt", url.scheme(), host);
        let body = match self.client.get(&robots_url).send().await {
            Ok(r) => r.text().await.unwrap_or_default(),
            Err(_) => String::new(),
        };
        let rules = RobotsRules::parse(&body, &self.user_agent);
        let allowed = rules.allows(url.path());
        self.robots.lock().await.insert(host, rules);
        allowed
    }
}

/// A small robots.txt rule set for the group that applies to us (our token, else
/// `*`). Longest-prefix match wins; an empty `Disallow` is no constraint.
struct RobotsRules {
    rules: Vec<(bool, String)>, // (is_allow, path_prefix)
}

impl RobotsRules {
    fn parse(txt: &str, ua: &str) -> Self {
        let ua_token = ua.split('/').next().unwrap_or(ua).trim().to_lowercase();
        let mut star: Vec<(bool, String)> = Vec::new();
        let mut mine: Vec<(bool, String)> = Vec::new();
        let mut cur_agents: Vec<String> = Vec::new();
        let mut last_was_agent = false;
        for raw in txt.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            let key = k.trim().to_lowercase();
            let val = v.trim().to_string();
            match key.as_str() {
                "user-agent" => {
                    if !last_was_agent {
                        cur_agents.clear();
                    }
                    cur_agents.push(val.to_lowercase());
                    last_was_agent = true;
                }
                "disallow" | "allow" => {
                    last_was_agent = false;
                    let is_allow = key == "allow";
                    for a in &cur_agents {
                        if a == "*" {
                            star.push((is_allow, val.clone()));
                        } else if ua_token.contains(a.as_str()) || a.contains(&ua_token) {
                            mine.push((is_allow, val.clone()));
                        }
                    }
                }
                _ => last_was_agent = false,
            }
        }
        let rules = if mine.is_empty() { star } else { mine };
        RobotsRules { rules }
    }

    fn allows(&self, path: &str) -> bool {
        let mut best: Option<(bool, usize)> = None;
        for (is_allow, prefix) in &self.rules {
            if prefix.is_empty() {
                continue; // empty Disallow = no constraint
            }
            if path.starts_with(prefix.as_str()) {
                let len = prefix.len();
                if best.map(|(_, l)| len > l).unwrap_or(true) {
                    best = Some((*is_allow, len));
                }
            }
        }
        best.map(|(is_allow, _)| is_allow).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_longest_match_wins() {
        let txt = "User-agent: *\nDisallow: /private\nAllow: /private/feed\n";
        let r = RobotsRules::parse(txt, DEFAULT_USER_AGENT);
        assert!(!r.allows("/private/secret"));
        assert!(r.allows("/private/feed/atom.xml"));
        assert!(r.allows("/public"));
    }

    #[test]
    fn robots_empty_disallow_allows_all() {
        let r = RobotsRules::parse("User-agent: *\nDisallow:\n", DEFAULT_USER_AGENT);
        assert!(r.allows("/anything"));
    }

    #[test]
    fn robots_specific_group_overrides_star() {
        let txt = "User-agent: *\nDisallow: /\n\nUser-agent: PHPressBot\nDisallow: /admin\n";
        let r = RobotsRules::parse(txt, DEFAULT_USER_AGENT);
        // our group allows everything except /admin (the '*' blanket does not apply)
        assert!(r.allows("/news"));
        assert!(!r.allows("/admin/panel"));
    }
}
