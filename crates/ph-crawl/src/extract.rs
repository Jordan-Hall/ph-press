//! Rules-first, conventional extraction + classification. No ML / LLM (matches
//! the org's rules-first posture and keeps every decision explainable). All
//! output is best-effort and treated as UNVERIFIED downstream; the human legal
//! gate is what actually clears a case for publication.

/// Offence relevance bucket. `Child` covers crimes against children broadly
/// (including child sexual offences); `Sexual` covers sexual offences against
/// adults. Only these two are within the remit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffenceCategory {
    Sexual,
    Child,
    Other,
    Unknown,
}

impl OffenceCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            OffenceCategory::Sexual => "sexual",
            OffenceCategory::Child => "child",
            OffenceCategory::Other => "other",
            OffenceCategory::Unknown => "unknown",
        }
    }
    /// Within our remit: sexual offences OR crimes against children.
    pub fn is_relevant(self) -> bool {
        matches!(self, OffenceCategory::Sexual | OffenceCategory::Child)
    }
}

/// Where a case appears to be in its life. Drives the active-proceedings
/// firewall: the public adapters keep only `Concluded` matters; anything that
/// reads as live/upcoming is kept out of the public queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStatus {
    Concluded,
    Upcoming,
    Unknown,
}

impl CaseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CaseStatus::Concluded => "concluded",
            CaseStatus::Upcoming => "upcoming",
            CaseStatus::Unknown => "unknown",
        }
    }
}

// Crimes against children (incl. child sexual offences).
const CHILD_TERMS: &[&str] = &[
    "child",
    "children",
    "indecent image",
    "indecent photograph",
    "child abuse",
    "child sex",
    "child cruelty",
    "cruelty to a child",
    "neglect of a child",
    "child neglect",
    "grooming a child",
    "sexual communication with a child",
    "making indecent photographs",
    "schoolgirl",
    "schoolboy",
    "abuse of a child",
    "causing or allowing the death of a child",
    "child abduction",
    "child destruction",
];

// Sexual offences (against adults — child sexual offences fall under CHILD_TERMS).
const SEXUAL_TERMS: &[&str] = &[
    "sexual",
    "sex offence",
    "sexual offence",
    "sex offender",
    "rape",
    "sexual assault",
    "indecent assault",
    "sexual activity",
    "voyeurism",
    "upskirt",
    "sexual harm prevention",
    "sexual abuse",
    "sexual exploitation",
    "bestiality",
    "intercourse with an animal",
    "buggery",
    "grooming",
];

// Hard pre-conviction / active-proceedings markers. ANY of these forces a case
// out of the public (post-conviction) pipeline — the firewall.
const PRE_CONVICTION_TERMS: &[&str] = &[
    "charged with",
    "to stand trial",
    "will stand trial",
    "stand trial",
    "on trial",
    "trial date",
    "awaiting trial",
    "will appear",
    "to appear",
    "appeared in court charged",
    "denies",
    "denied the",
    "alleged",
    "accused of",
    "remanded",
    "bailed",
    "jury",
    "prosecution alleges",
    "to be sentenced",
    "due to be sentenced",
];

// Concluded markers (post-conviction).
const CONCLUDED_TERMS: &[&str] = &[
    "sentenced",
    "jailed",
    "convicted",
    "pleaded guilty",
    "pleads guilty",
    "found guilty",
    "guilty of",
    "admitted",
    "suspended sentence",
    "imprisoned",
    "locked up",
    "sentencing remarks",
    "handed a",
    "given a",
];

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Classify the offence bucket from free text (title + summary). Lower-cased
/// keyword match; child terms take precedence (a child sexual offence is filed
/// under children).
pub fn classify_offence(text: &str) -> OffenceCategory {
    if text.trim().is_empty() {
        return OffenceCategory::Unknown;
    }
    let lower = text.to_lowercase();
    if contains_any(&lower, CHILD_TERMS) {
        OffenceCategory::Child
    } else if contains_any(&lower, SEXUAL_TERMS) {
        OffenceCategory::Sexual
    } else {
        OffenceCategory::Other
    }
}

/// Classify case status. Any hard pre-conviction / active marker wins (kept out
/// of the public pipeline); otherwise a concluded marker means concluded.
pub fn case_status(text: &str) -> CaseStatus {
    if text.trim().is_empty() {
        return CaseStatus::Unknown;
    }
    let lower = text.to_lowercase();
    if contains_any(&lower, PRE_CONVICTION_TERMS) {
        CaseStatus::Upcoming
    } else if contains_any(&lower, CONCLUDED_TERMS) {
        CaseStatus::Concluded
    } else {
        CaseStatus::Unknown
    }
}

/// Best-effort hearing type for a court-watch listing.
pub fn hearing_type(text: &str) -> &'static str {
    let lower = text.to_lowercase();
    if lower.contains("appeal") {
        "appeal"
    } else if lower.contains("sentenc") {
        "sentencing"
    } else if lower.contains("trial") {
        "trial"
    } else {
        "listing"
    }
}

/// A compact JSON blob of the unverified machine classification, stored on the
/// lead so the desk can show provenance.
pub fn extracted_json(cat: OffenceCategory, status: CaseStatus) -> String {
    serde_json::json!({
        "unverified": true,
        "offence_category": cat.as_str(),
        "case_status": status.as_str(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offence_classification() {
        assert_eq!(
            classify_offence("Man sentenced for making indecent photographs of a child"),
            OffenceCategory::Child
        );
        assert_eq!(
            classify_offence("Convicted of rape and sexual assault"),
            OffenceCategory::Sexual
        );
        assert_eq!(
            classify_offence("Jailed for fraud and theft"),
            OffenceCategory::Other
        );
        assert_eq!(classify_offence(""), OffenceCategory::Unknown);
        assert!(OffenceCategory::Child.is_relevant());
        assert!(!OffenceCategory::Other.is_relevant());
    }

    #[test]
    fn status_firewall_blocks_pre_conviction() {
        assert_eq!(
            case_status("X has been jailed for 9 months, suspended"),
            CaseStatus::Concluded
        );
        // pre-conviction / live markers force Upcoming even with a guilty word
        assert_eq!(
            case_status("Man charged with sexual assault to stand trial next month"),
            CaseStatus::Upcoming
        );
        assert_eq!(
            case_status("Defendant remanded in custody ahead of hearing"),
            CaseStatus::Upcoming
        );
        // a decided appeal judgment is NOT treated as live (Find Case Law)
        assert_eq!(
            case_status("Convicted of sexual assault; appeal dismissed"),
            CaseStatus::Concluded
        );
        assert_eq!(case_status("Court report"), CaseStatus::Unknown);
    }

    #[test]
    fn hearing_type_detection() {
        assert_eq!(hearing_type("Court of Appeal hearing"), "appeal");
        assert_eq!(hearing_type("Sentencing listed for Friday"), "sentencing");
        assert_eq!(hearing_type("Trial of ..."), "trial");
        assert_eq!(hearing_type("Plea and case management"), "listing");
    }
}
