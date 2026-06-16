//! Court-listing adapter (PRIVATE court-watch). Parses table-structured hearing
//! lists (CourtServe free lists, gov.uk court & tribunal hearing lists) into
//! upcoming/appeal entries for the newsroom's private watch list.
//!
//! Best-effort + heuristic: court lists are not standardised, so this extracts
//! data rows from HTML tables, classifies them, and keeps only relevant matters
//! that read as live/upcoming (or appeals). It NEVER produces a public lead or
//! conviction — its output goes only to the private `court_watch` store.

use crate::extract::{self, CaseStatus};
use crate::{dedupe, source::RawWatch};
use scraper::{Html, Selector};

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// A confident date cell: a month name, or a slashed numeric date. Preferred so
/// we don't mistake a case number that embeds a year (e.g. T20260099) for a date.
fn strong_date(s: &str) -> bool {
    let l = s.to_lowercase();
    if MONTHS.iter().any(|m| l.contains(m)) {
        return true;
    }
    s.contains('/') && s.chars().filter(char::is_ascii_digit).count() >= 4
}

/// A weaker date heuristic (also matches a bare 19xx/20xx year) — only used as a
/// fallback when no strong date cell is present.
fn looks_like_date(s: &str) -> bool {
    if strong_date(s) {
        return true;
    }
    s.as_bytes()
        .windows(4)
        .any(|w| (w.starts_with(b"19") || w.starts_with(b"20")) && w.iter().all(u8::is_ascii_digit))
}

/// Parse hearing rows from an HTML court list. `base_url` is stored as the
/// link-back. Returns one entry per relevant, upcoming/appeal row.
pub fn parse(html: &str, base_url: &str) -> Vec<RawWatch> {
    let doc = Html::parse_document(html);
    let row_sel = Selector::parse("tr").expect("static selector");
    let cell_sel = Selector::parse("td, th").expect("static selector");

    let mut out = Vec::new();
    for row in doc.select(&row_sel) {
        let cells: Vec<String> = row
            .select(&cell_sel)
            .map(|c| {
                c.text()
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|s| !s.is_empty())
            .collect();
        if cells.is_empty() {
            continue;
        }
        let row_text = cells.join(" ");

        let cat = extract::classify_offence(&row_text);
        if !cat.is_relevant() {
            continue;
        }
        let status = extract::case_status(&row_text);
        let htype = extract::hearing_type(&row_text);
        // Court-watch wants live/upcoming matters (and appeals). Skip anything
        // that reads as already concluded unless it is an appeal.
        if status == CaseStatus::Concluded && htype != "appeal" {
            continue;
        }

        let hearing_date = cells
            .iter()
            .find(|c| strong_date(c))
            .or_else(|| cells.iter().find(|c| looks_like_date(c)))
            .cloned()
            .unwrap_or_default();
        let case_ref = cells
            .iter()
            .find(|c| {
                c.as_str() != hearing_date.as_str() && c.chars().any(|ch| ch.is_ascii_digit())
            })
            .cloned()
            .unwrap_or_default();

        out.push(RawWatch {
            court: String::new(), // filled from the source label by the runner
            case_ref,
            hearing_date,
            hearing_type: htype.to_string(),
            offence_category: cat.as_str().to_string(),
            external_id: dedupe::stable_id(&row_text),
            source_url: base_url.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_relevant_upcoming_rows_only() {
        let html = r#"<html><body>
        <table>
          <tr><th>Case</th><th>Date</th><th>Matter</th></tr>
          <tr><td>T20260099</td><td>01 Jul 2026</td><td>Trial — sexual assault</td></tr>
          <tr><td>A20260012</td><td>15 Aug 2026</td><td>Appeal against conviction (child cruelty)</td></tr>
          <tr><td>C20260050</td><td>02 Jul 2026</td><td>Sentencing — fraud</td></tr>
        </table></body></html>"#;
        let watches = parse(html, "https://courts.example/list");
        // sexual-assault trial + child-cruelty appeal kept; fraud dropped (off-remit)
        assert_eq!(watches.len(), 2);
        assert_eq!(watches[0].hearing_type, "trial");
        assert_eq!(watches[0].offence_category, "sexual");
        assert_eq!(watches[0].case_ref, "T20260099");
        assert_eq!(watches[0].hearing_date, "01 Jul 2026");
        assert_eq!(watches[1].hearing_type, "appeal");
        assert_eq!(watches[1].offence_category, "child");
        // stable id is deterministic
        assert_eq!(parse(html, "https://courts.example/list"), watches);
    }
}
