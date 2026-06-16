//! UK news adapter. Parses an outlet's RSS/Atom feed into post-conviction leads.
//! STRICTER than the caselaw adapter: news covers pre-charge and ongoing matters
//! too, so only items that read as concluded (post-conviction) are kept — the
//! active-proceedings firewall. Only a short snippet + a link-back are stored;
//! the outlet's full text is never copied, and images are references only.

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
    // News: keep only clearly concluded matters.
    let status = extract::case_status(&text);
    if status != CaseStatus::Concluded {
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
    fn keeps_concluded_child_case_drops_pre_charge_and_offtopic() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0"><channel>
          <item>
            <title>Man jailed for making indecent images of children</title>
            <link>https://outlet.example/a</link>
            <guid>a</guid>
            <description>He was sentenced to two years.</description>
          </item>
          <item>
            <title>Man charged with sexual assault to stand trial</title>
            <link>https://outlet.example/b</link>
            <guid>b</guid>
            <description>He denies the offence.</description>
          </item>
          <item>
            <title>Council approves new car park</title>
            <link>https://outlet.example/c</link>
            <guid>c</guid>
            <description>Planning news.</description>
          </item>
        </channel></rss>"#;
        let leads = parse(xml);
        assert_eq!(leads.len(), 1);
        assert_eq!(leads[0].external_id, "a");
        assert_eq!(leads[0].offence_category, "child");
    }
}
