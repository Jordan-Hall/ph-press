//! National Archives **Find Case Law** adapter. Parses the sanctioned Atom feed
//! of handed-down judgments (caselaw.nationalarchives.gov.uk) into post-conviction
//! leads. Find Case Law publishes decided judgments, so the firewall here only
//! drops anything that still reads as live (defence in depth); concluded and
//! unknown matters within the remit are kept.

use crate::extract::{self, CaseStatus};
use crate::feed;
use crate::source::RawLead;

pub fn parse(xml: &str) -> Vec<RawLead> {
    feed::collect(xml)
        .into_iter()
        .filter_map(record_to_lead)
        .collect()
}

fn record_to_lead(r: feed::FeedRecord) -> Option<RawLead> {
    let text = format!("{} {}", r.title, r.summary);
    let cat = extract::classify_offence(&text);
    if !cat.is_relevant() {
        return None;
    }
    let status = extract::case_status(&text);
    if status == CaseStatus::Upcoming {
        return None;
    }
    let external_id = if r.id.is_empty() {
        r.link.clone()
    } else {
        r.id.clone()
    };
    if external_id.is_empty() {
        return None;
    }
    Some(RawLead {
        external_id,
        url: r.link,
        title: r.title,
        snippet: feed::snippet(&r.summary, 300),
        offence_category: cat.as_str().to_string(),
        extracted_json: extract::extracted_json(cat, status),
        image_url: r.image_url,
        image_attribution: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_relevant_judgments_drops_irrelevant() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title>R v Smith</title>
            <id>urn:judgment:crim:1</id>
            <link rel="alternate" href="https://caselaw.example/crim/1"/>
            <summary>Appellant convicted of sexual assault; appeal dismissed.</summary>
          </entry>
          <entry>
            <title>Acme Ltd v HMRC</title>
            <id>urn:judgment:tax:9</id>
            <link rel="alternate" href="https://caselaw.example/tax/9"/>
            <summary>A VAT dispute.</summary>
          </entry>
        </feed>"#;
        let leads = parse(xml);
        assert_eq!(leads.len(), 1);
        assert_eq!(leads[0].external_id, "urn:judgment:crim:1");
        assert_eq!(leads[0].offence_category, "sexual");
        assert!(leads[0].extracted_json.contains("\"unverified\":true"));
    }
}
