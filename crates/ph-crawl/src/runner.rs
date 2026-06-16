//! The crawl runner: seeds configured sources, polls each enabled source once,
//! and persists results through `ph_cms`. Public-ingest sources (caselaw, news)
//! write LEADS; court-watch sources write to the private store. Per-source errors
//! are collected, never fatal.

use crate::adapters;
use crate::fetch::Fetcher;
use crate::source::SourceConfig;
use crate::{Error, Result};
use ph_cms::{courtwatch, ingest, Db};
use std::time::Duration;

/// What a single crawl pass did.
#[derive(Debug, Default, Clone)]
pub struct RunReport {
    pub leads_added: u64,
    pub watch_added: u64,
    pub sources_polled: u64,
    pub errors: Vec<String>,
}

/// Register/refresh the configured sources in `ingest_source` (idempotent).
pub async fn seed_sources(pool: &Db, sources: &[SourceConfig]) -> Result<()> {
    for s in sources {
        ingest::upsert_source(pool, &s.key, &s.kind, &s.label, &s.url)
            .await
            .map_err(|e| Error::Cms(e.to_string()))?;
    }
    Ok(())
}

/// Poll every enabled source once.
pub async fn run_once(pool: &Db, fetcher: &Fetcher) -> RunReport {
    let mut report = RunReport::default();

    for src in enabled(pool, "caselaw", &mut report).await {
        match poll_feed(pool, fetcher, &src, true).await {
            Ok(n) => report.leads_added += n,
            Err(e) => report.errors.push(format!("{}: {e}", src.key)),
        }
        let _ = ingest::mark_source_polled(pool, src.id).await;
        report.sources_polled += 1;
    }
    for src in enabled(pool, "news", &mut report).await {
        match poll_feed(pool, fetcher, &src, false).await {
            Ok(n) => report.leads_added += n,
            Err(e) => report.errors.push(format!("{}: {e}", src.key)),
        }
        let _ = ingest::mark_source_polled(pool, src.id).await;
        report.sources_polled += 1;
    }
    for src in enabled(pool, "courtwatch", &mut report).await {
        match poll_courtwatch(pool, fetcher, &src).await {
            Ok(n) => report.watch_added += n,
            Err(e) => report.errors.push(format!("{}: {e}", src.key)),
        }
        let _ = ingest::mark_source_polled(pool, src.id).await;
        report.sources_polled += 1;
    }
    report
}

async fn enabled(pool: &Db, kind: &str, report: &mut RunReport) -> Vec<ingest::IngestSource> {
    match ingest::enabled_sources(pool, kind).await {
        Ok(v) => v,
        Err(e) => {
            report.errors.push(format!("list {kind}: {e}"));
            Vec::new()
        }
    }
}

/// Poll a feed source. `caselaw` selects the (lenient) Find Case Law adapter;
/// otherwise the (strict, concluded-only) news adapter.
async fn poll_feed(
    pool: &Db,
    fetcher: &Fetcher,
    src: &ingest::IngestSource,
    caselaw: bool,
) -> Result<u64> {
    let body = fetcher.get_text(&src.url).await?;
    let leads = if caselaw {
        adapters::caselaw::parse(&body)
    } else {
        adapters::news::parse(&body)
    };
    let mut added = 0;
    for raw in leads {
        // Attribute the image reference to the source if the adapter didn't.
        let image_attribution = if raw.image_attribution.is_empty() && !raw.image_url.is_empty() {
            src.label.clone()
        } else {
            raw.image_attribution
        };
        let lead = ingest::NewLead {
            source_id: src.id,
            source_key: src.key.clone(),
            external_id: raw.external_id,
            url: raw.url,
            title: raw.title,
            snippet: raw.snippet,
            offence_category: raw.offence_category,
            extracted_json: raw.extracted_json,
            image_url: raw.image_url,
            image_attribution,
        };
        match ingest::insert_lead(pool, &lead).await {
            Ok(Some(_)) => added += 1,
            Ok(None) => {}
            Err(e) => return Err(Error::Cms(e.to_string())),
        }
    }
    Ok(added)
}

async fn poll_courtwatch(pool: &Db, fetcher: &Fetcher, src: &ingest::IngestSource) -> Result<u64> {
    let body = fetcher.get_text(&src.url).await?;
    let watches = adapters::courtwatch::parse(&body, &src.url);
    let mut added = 0;
    for w in watches {
        let nw = courtwatch::NewWatch {
            court: if w.court.is_empty() {
                src.label.clone()
            } else {
                w.court
            },
            case_ref: w.case_ref,
            hearing_date: w.hearing_date,
            hearing_type: w.hearing_type,
            offence_category: w.offence_category,
            source_key: src.key.clone(),
            external_id: w.external_id,
            source_url: w.source_url,
            notes: String::new(),
        };
        match courtwatch::insert_watch(pool, &nw).await {
            Ok(Some(_)) => added += 1,
            Ok(None) => {}
            Err(e) => return Err(Error::Cms(e.to_string())),
        }
    }
    Ok(added)
}

/// Spawn the background poll loop on the current Tokio runtime. Seeds sources
/// once, then polls every `interval`. Errors are logged, not fatal.
pub fn spawn(pool: Db, sources: Vec<SourceConfig>, interval: Duration, user_agent: String) {
    tokio::spawn(async move {
        let fetcher = match Fetcher::new(user_agent) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("[ph-crawl] fetcher init failed: {e}");
                return;
            }
        };
        if let Err(e) = seed_sources(&pool, &sources).await {
            eprintln!("[ph-crawl] seeding sources failed: {e}");
        }
        let mut tick = tokio::time::interval(interval);
        loop {
            tick.tick().await;
            let report = run_once(&pool, &fetcher).await;
            eprintln!(
                "[ph-crawl] poll: {} leads, {} watch, {} sources, {} errors",
                report.leads_added,
                report.watch_added,
                report.sources_polled,
                report.errors.len()
            );
            for e in &report.errors {
                eprintln!("[ph-crawl]   {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_USER_AGENT;

    #[tokio::test]
    async fn run_once_with_no_sources_does_no_network() {
        let pool = ph_cms::connect("sqlite::memory:").await.unwrap();
        ph_cms::init(&pool).await.unwrap();
        let fetcher = Fetcher::new(DEFAULT_USER_AGENT).unwrap();
        let report = run_once(&pool, &fetcher).await;
        assert_eq!(report.sources_polled, 0);
        assert_eq!(report.leads_added, 0);
        assert_eq!(report.watch_added, 0);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn seed_sources_is_idempotent() {
        let pool = ph_cms::connect("sqlite::memory:").await.unwrap();
        ph_cms::init(&pool).await.unwrap();
        let srcs = vec![
            SourceConfig::new("caselaw", "caselaw", "Find Case Law", "https://x/atom.xml"),
            SourceConfig::new("bbc", "news", "BBC", "https://bbc/rss.xml"),
        ];
        seed_sources(&pool, &srcs).await.unwrap();
        seed_sources(&pool, &srcs).await.unwrap();
        assert_eq!(ingest::list_sources(&pool).await.unwrap().len(), 2);
    }
}
