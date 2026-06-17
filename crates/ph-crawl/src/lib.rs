//! ph-crawl — the court/news crawler for PH Press.
//!
//! Polls primary court sources and UK news for **sex-offence and
//! crimes-against-children** cases and files them as approval-gated **leads**
//! (via [`ph_cms::ingest`]) for an editor to turn into our own legal-gated
//! report — nothing is ever auto-published. A separate, private court-watch path
//! (via [`ph_cms::courtwatch`]) tracks upcoming / appeal hearings the newsroom
//! wants to attend.
//!
//! ## Active-proceedings firewall
//! Public ingest ([`adapters::caselaw`], [`adapters::news`]) keeps only
//! post-conviction matters; the private court-watch ([`adapters::courtwatch`])
//! holds live/upcoming hearings. The two never cross: [`runner`] writes leads
//! and watch entries through the separate `ph_cms` modules, and there is no path
//! that turns a watch entry into a public lead or conviction. (Contempt of Court
//! Act 1981.)
//!
//! SERVER-ONLY: pulled by ph-press behind its `server` feature; never in wasm.

pub mod adapters;
pub mod dedupe;
pub mod extract;
pub mod feed;
pub mod fetch;
pub mod presets;
pub mod runner;
pub mod source;

pub use fetch::{Fetcher, DEFAULT_USER_AGENT};
pub use runner::{run_once, seed_sources, spawn, RunReport};
pub use source::{RawLead, RawWatch, SourceConfig};

/// Errors a crawl can produce. CMS errors are flattened to strings to avoid
/// leaking the `ph_cms` error type across the adapter boundary.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("blocked by robots.txt: {0}")]
    Disallowed(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("cms error: {0}")]
    Cms(String),
}

pub type Result<T> = std::result::Result<T, Error>;
