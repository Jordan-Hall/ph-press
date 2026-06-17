//! Default source presets, so the crawler can be switched on with just
//! `PH_CRAWL_ENABLED=1`. Each kind is overridable via its `PH_CRAWL_*_FEEDS`
//! env var; when that var is unset/empty these defaults are used (court-watch has
//! no default — it is opt-in because listing sources are site-specific / paid).

use crate::source::SourceConfig;

/// National Archives **Find Case Law** — the sanctioned Atom feed, queried for our
/// remit. Judgments are concluded matters; the extractor still filters relevance.
pub fn caselaw() -> Vec<SourceConfig> {
    vec![
        SourceConfig::new(
            "caselaw-sexual",
            "caselaw",
            "Find Case Law — sexual offences",
            "https://caselaw.nationalarchives.gov.uk/atom.xml?query=sexual+offences&order=-date",
        ),
        SourceConfig::new(
            "caselaw-child",
            "caselaw",
            "Find Case Law — offences against children",
            "https://caselaw.nationalarchives.gov.uk/atom.xml?query=indecent+images+of+children&order=-date",
        ),
    ]
}

/// UK regional news (the newsroom's local patch) — BBC England regional RSS.
pub fn news() -> Vec<SourceConfig> {
    ["leicester", "nottingham", "derbyshire"]
        .into_iter()
        .map(|r| {
            let cap = {
                let mut c = r.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            };
            SourceConfig::new(
                &format!("bbc-{r}"),
                "news",
                &format!("BBC News — {cap}"),
                &format!("https://feeds.bbci.co.uk/news/england/{r}/rss.xml"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_well_formed() {
        for s in caselaw() {
            assert_eq!(s.kind, "caselaw");
            assert!(s.url.starts_with("https://caselaw.nationalarchives.gov.uk/atom.xml"));
        }
        let news = news();
        assert_eq!(news.len(), 3);
        assert!(news.iter().all(|s| s.kind == "news" && s.url.contains("bbci.co.uk")));
        assert_eq!(news[0].label, "BBC News — Leicester");
    }
}
