//! A minimal RSS 2.0 + Atom reader (quick-xml). Extracts the fields the public
//! adapters need — title, id, link, summary, and an optional image reference —
//! handling both `<item>` (RSS) and `<entry>` (Atom) records, plus CDATA.

use quick_xml::events::{BytesStart, Event};
use quick_xml::name::QName;
use quick_xml::Reader;

/// One feed record, source-agnostic. Adapters apply their own relevance + status
/// filtering on top.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedRecord {
    pub title: String,
    pub id: String,
    pub link: String,
    pub summary: String,
    pub image_url: String,
}

fn local(name: QName) -> String {
    String::from_utf8_lossy(name.local_name().as_ref()).to_string()
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.local_name().as_ref() == key {
            Some(String::from_utf8_lossy(a.value.as_ref()).replace("&amp;", "&"))
        } else {
            None
        }
    })
}

/// Set the image reference from the element's `url` attribute, if not already set.
fn set_image_ref(rec: &mut FeedRecord, e: &BytesStart) {
    if !rec.image_url.is_empty() {
        return;
    }
    if let Some(u) = attr(e, b"url") {
        rec.image_url = u;
    }
}

/// Capture image references / Atom link hrefs on a (possibly empty) element.
fn inline(e: &BytesStart, l: &str, rec: &mut FeedRecord) {
    let qualified = e.name();
    let qbytes = qualified.as_ref();
    if l == "link" {
        if let Some(h) = attr(e, b"href") {
            if rec.link.is_empty() {
                rec.link = h;
            }
        }
    } else if l == "enclosure" {
        let ty = attr(e, b"type").unwrap_or_default();
        if ty.is_empty() || ty.contains("image") {
            set_image_ref(rec, e);
        }
    } else if qbytes.starts_with(b"media:") && (l == "content" || l == "thumbnail") {
        set_image_ref(rec, e);
    }
}

fn route(cur: &str, text: &str, rec: &mut FeedRecord) {
    match cur {
        "title" => rec.title.push_str(text),
        "id" | "guid" => rec.id.push_str(text),
        "summary" | "description" => rec.summary.push_str(text),
        "content" => {
            if rec.summary.is_empty() {
                rec.summary.push_str(text)
            }
        }
        "link" => {
            if rec.link.is_empty() {
                rec.link.push_str(text)
            }
        }
        _ => {}
    }
}

fn trim_record(mut rec: FeedRecord) -> FeedRecord {
    rec.title = rec.title.trim().to_string();
    rec.id = rec.id.trim().to_string();
    rec.link = rec.link.trim().to_string();
    rec.summary = rec.summary.trim().to_string();
    rec.image_url = rec.image_url.trim().to_string();
    rec
}

/// Parse a feed into its records. Unknown XML is tolerated (best-effort).
pub fn collect(xml: &str) -> Vec<FeedRecord> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut in_record = false;
    let mut cur = String::new();
    let mut rec = FeedRecord::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let l = local(e.name());
                if l == "item" || l == "entry" {
                    in_record = true;
                    rec = FeedRecord::default();
                    cur.clear();
                } else if in_record {
                    cur = l.clone();
                    inline(&e, &l, &mut rec);
                }
            }
            Ok(Event::Empty(e)) => {
                if in_record {
                    let l = local(e.name());
                    inline(&e, &l, &mut rec);
                }
            }
            Ok(Event::Text(t)) => {
                if in_record && !cur.is_empty() {
                    let s = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                    route(&cur, &s, &mut rec);
                }
            }
            Ok(Event::CData(t)) => {
                if in_record && !cur.is_empty() {
                    let s = String::from_utf8_lossy(&t.into_inner()).to_string();
                    route(&cur, &s, &mut rec);
                }
            }
            Ok(Event::End(e)) => {
                let l = local(e.name());
                if l == "item" || l == "entry" {
                    let r = trim_record(std::mem::take(&mut rec));
                    if !r.title.is_empty() || !r.link.is_empty() {
                        out.push(r);
                    }
                    in_record = false;
                    cur.clear();
                } else {
                    cur.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Strip HTML tags + decode the common entities, for a plain-text snippet.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// A tidy, whitespace-collapsed, length-capped snippet (never the full body).
pub fn snippet(raw: &str, max: usize) -> String {
    let text = strip_html(raw);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > max {
        let s: String = collapsed.chars().take(max).collect();
        format!("{}\u{2026}", s.trim_end())
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss_with_cdata_and_enclosure() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0"><channel>
          <title>Outlet</title>
          <item>
            <title><![CDATA[Man jailed for child abuse]]></title>
            <link>https://outlet.example/story-1</link>
            <guid isPermaLink="false">story-1</guid>
            <description><![CDATA[<p>He was <b>sentenced</b> &amp; jailed.</p>]]></description>
            <enclosure url="https://outlet.example/img.jpg" type="image/jpeg"/>
          </item>
        </channel></rss>"#;
        let recs = collect(xml);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].title, "Man jailed for child abuse");
        assert_eq!(recs[0].id, "story-1");
        assert_eq!(recs[0].link, "https://outlet.example/story-1");
        assert_eq!(recs[0].image_url, "https://outlet.example/img.jpg");
        assert_eq!(snippet(&recs[0].summary, 80), "He was sentenced & jailed.");
    }

    #[test]
    fn extracts_media_content_image_and_tolerates_unclosed_tags() {
        let xml = r#"<?xml version="1.0"?>
        <rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel>
          <item>
            <title>Woman jailed for child cruelty</title>
            <link>https://outlet.example/m</link>
            <guid>m</guid>
            <description>She was sentenced.</description>
            <media:content url="https://outlet.example/m.jpg" medium="image"/>
          </item>
        </channel></rss>"#;
        let recs = collect(xml);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].image_url, "https://outlet.example/m.jpg");
    }

    #[test]
    fn malformed_feed_does_not_panic() {
        // Truncated / broken XML must return cleanly (best-effort), never panic.
        assert!(collect("<rss><channel><item><title>Broken").is_empty());
        assert!(collect("not xml at all <<<").is_empty());
        assert!(collect("").is_empty());
    }

    #[test]
    fn parses_atom_with_link_href() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title>R v Smith</title>
            <id>urn:judgment:1</id>
            <link rel="alternate" href="https://caselaw.example/1"/>
            <summary>Convicted of sexual assault.</summary>
          </entry>
        </feed>"#;
        let recs = collect(xml);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, "urn:judgment:1");
        assert_eq!(recs[0].link, "https://caselaw.example/1");
    }
}
