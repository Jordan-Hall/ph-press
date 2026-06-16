//! Shared crawler types: the lightweight, DB-agnostic shapes adapters produce.
//! The [`crate::runner`] maps these into `ph_cms` rows (leads / court-watch),
//! attaching the source id and applying the firewall before any write.

/// A configured source, seeded into `ingest_source`. `kind` selects the adapter
/// the runner dispatches to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceConfig {
    pub key: String,
    /// "caselaw" | "news" | "courtwatch".
    pub kind: String,
    pub label: String,
    pub url: String,
}

impl SourceConfig {
    pub fn new(key: &str, kind: &str, label: &str, url: &str) -> Self {
        Self {
            key: key.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            url: url.to_string(),
        }
    }
}

/// A post-conviction / news LEAD produced by a PUBLIC-ingest adapter. Everything
/// here is UNVERIFIED machine output; `snippet` is a short extract only (never
/// the source's full body). `image_url` is a reference for the editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawLead {
    pub external_id: String,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub offence_category: String,
    pub extracted_json: String,
    pub image_url: String,
    pub image_attribution: String,
}

/// An upcoming / appeal hearing produced by the PRIVATE court-watch adapter.
/// Lives only in the private store; never enters the public pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawWatch {
    pub court: String,
    pub case_ref: String,
    pub hearing_date: String,
    pub hearing_type: String,
    pub offence_category: String,
    pub external_id: String,
    pub source_url: String,
}
