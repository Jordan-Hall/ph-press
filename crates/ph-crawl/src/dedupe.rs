//! De-duplication helpers. The database's `UNIQUE(source, external_id)`
//! constraint is the primary guard against re-crawl duplicates; these helpers
//! provide stable ids for sources that don't expose one, and a normalized
//! cross-source key so the same case from two outlets can be recognised.

use std::hash::{Hash, Hasher};

/// Normalise a string for fuzzy matching: keep alphanumerics, lower-case.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// A stable, deterministic id for content that carries no native id (e.g. a row
/// scraped from an HTML court list). `DefaultHasher` uses fixed keys, so the same
/// input yields the same id across runs — keeping de-dup stable on re-crawl.
pub fn stable_id(content: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    normalize(content).hash(&mut h);
    format!("{:016x}", h.finish())
}

/// A normalized cross-source key for a conviction-style record.
pub fn fuzzy_key(name: &str, court: &str, date: &str) -> String {
    format!(
        "{}|{}|{}",
        normalize(name),
        normalize(court),
        normalize(date)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_is_deterministic_and_normalised() {
        assert_eq!(stable_id("R v Smith  "), stable_id("r v  smith"));
        assert_ne!(stable_id("R v Smith"), stable_id("R v Jones"));
    }

    #[test]
    fn fuzzy_key_normalises() {
        assert_eq!(
            fuzzy_key("John Smith", "Leicester Crown Court", "2026-05-01"),
            fuzzy_key("john  smith!", "leicester crown court.", "20260501")
        );
    }
}
