//! UK police-force news adapter. Scrapes a force's news LISTING page (the GOSS /
//! "Police.UK" CMS, where each item is an `a.c-news-panel`) into post-conviction
//! leads. SAME strict filter as the news adapter: only sex-offence / child cases
//! that read as concluded are kept — appeals, witness/missing-person appeals,
//! ongoing matters and off-remit items are dropped (the active-proceedings
//! firewall). Only a title + short snippet + link-back are stored; the force's
//! text is never copied.
//!
//! The custody image is deliberately NOT auto-attached: on this corpus an editor
//! must confirm from the linked release that it is the *convicted defendant*
//! (never a victim, never someone under reporting restrictions) before any image
//! is used. The lead links back to the release for that human check.

use std::collections::HashSet;

use scraper::{ElementRef, Html, Selector};

use crate::extract::{self, CaseStatus};
use crate::feed;
use crate::source::RawLead;

/// Origin (`scheme://host`) of `url`, for resolving the listing's relative links.
fn origin_of(url: &str) -> String {
    for scheme in ["https://", "http://"] {
        if let Some(rest) = url.strip_prefix(scheme) {
            let host = rest.split('/').next().unwrap_or("");
            return format!("{scheme}{host}");
        }
    }
    String::new()
}

fn absolutize(origin: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if let Some(path) = href.strip_prefix('/') {
        format!("{origin}/{path}")
    } else {
        format!("{origin}/{href}")
    }
}

fn inner_text(el: &ElementRef<'_>, sel: &Selector) -> String {
    el.select(sel)
        .next()
        .map(|t| {
            t.text()
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

/// Scrape post-conviction sex/child leads from a force news listing page.
/// `base_url` resolves the relative article links and is the link-back origin.
pub fn parse(html: &str, base_url: &str) -> Vec<RawLead> {
    let origin = origin_of(base_url);
    let doc = Html::parse_document(html);
    let panel = Selector::parse("a.c-news-panel").expect("static selector");
    let title_sel = Selector::parse(".c-news-panel_title").expect("static selector");
    let text_sel = Selector::parse(".c-news-panel_text").expect("static selector");

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in doc.select(&panel) {
        let Some(href) = p.value().attr("href") else {
            continue;
        };
        let title = inner_text(&p, &title_sel);
        if title.is_empty() {
            continue;
        }
        let summary = inner_text(&p, &text_sel);
        let text = format!("{title} {summary}");

        let cat = extract::classify_offence(&text);
        if !cat.is_relevant() {
            continue;
        }
        // Strict: only matters that read as concluded (post-conviction).
        if extract::case_status(&text) != CaseStatus::Concluded {
            continue;
        }

        let url = absolutize(&origin, href);
        if url.is_empty() || !seen.insert(url.clone()) {
            continue;
        }
        out.push(RawLead {
            external_id: url.clone(),
            url,
            title,
            snippet: feed::snippet(&summary, 300),
            offence_category: cat.as_str().to_string(),
            extracted_json: extract::extracted_json(cat, CaseStatus::Concluded, &text),
            image_url: String::new(),
            image_attribution: String::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_concluded_child_case_drops_appeals_and_offtopic() {
        let html = r#"<html><body>
          <a href="/news/x/news/2026/june/man-jailed-indecent-images/" class="c-news-panel">
            <h3 class="c-news-panel_title">Man jailed for making indecent images of children</h3>
            <div class="c-news-panel_text">He was sentenced to three years.</div>
          </a>
          <a href="/news/x/news/2026/june/appeal-missing-teen/" class="c-news-panel">
            <h3 class="c-news-panel_title">Appeal to find missing teenager</h3>
            <div class="c-news-panel_text">Have you seen her?</div>
          </a>
          <a href="/news/x/news/2026/june/new-car-park/" class="c-news-panel">
            <h3 class="c-news-panel_title">New station car park opens</h3>
            <div class="c-news-panel_text">Community news.</div>
          </a>
        </body></html>"#;
        let leads = parse(html, "https://www.example.police.uk/news/x/news/");
        assert_eq!(leads.len(), 1);
        assert_eq!(leads[0].offence_category, "child");
        assert_eq!(
            leads[0].url,
            "https://www.example.police.uk/news/x/news/2026/june/man-jailed-indecent-images/"
        );
    }
}
