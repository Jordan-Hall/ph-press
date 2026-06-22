//! Sitemap + RSS feed endpoints.
//!
//! Two `#[get]` server fns that build and return raw XML, served via Caddy at
//! `/sitemap.xml` and `/feed.xml`.  Both merge the compile-time seeds
//! (`content::ARTICLES`) with any CMS-published articles, mirroring the same
//! logic as the public news pages.
//!
//! ## Custom response type — `Xml`
//!
//! Dioxus 0.8 fullstack uses `axum::response::IntoResponse` + `FromResponse`
//! to encode server function outputs.  The default path serialises the return
//! type as JSON, which is wrong for raw XML.  We define a small `Xml(String)`
//! newtype that:
//!
//! - On the **server**: implements `IntoResponse` with
//!   `Content-Type: application/xml; charset=utf-8` so axum emits the XML
//!   bytes verbatim.
//! - On the **client** (wasm): implements `FromResponse` by reading the
//!   response body as a plain string.  These endpoints are consumed by Caddy /
//!   crawlers, not by the wasm client, so this code path is rarely exercised —
//!   but it must compile on both targets.

use dioxus::prelude::*;

// ---- custom XML response type -----------------------------------------------

/// A raw XML response.  Wraps a `String` and sets the correct Content-Type
/// when returned from an axum handler (i.e. a Dioxus 0.8 `#[server]` fn).
pub struct Xml(pub String);

// Server side: emit as application/xml.
// `dioxus::fullstack` re-exports axum's `body`, `response`, and routing
// modules at its top level, so we use those paths instead of `axum::` directly.
impl dioxus::fullstack::response::IntoResponse for Xml {
    fn into_response(self) -> dioxus::fullstack::response::Response {
        dioxus::fullstack::response::Response::builder()
            .header(
                dioxus::fullstack::http::header::CONTENT_TYPE,
                "application/xml; charset=utf-8",
            )
            .body(dioxus::fullstack::body::Body::from(self.0))
            .unwrap()
    }
}

// Client side: read the response body as a plain string.  The wasm client
// never actually calls these endpoints (they're sitemap / feed for crawlers),
// but the trait bound must be satisfied so the crate compiles on the web target.
impl dioxus::fullstack::FromResponse for Xml {
    fn from_response(
        res: dioxus::fullstack::ClientResponse,
    ) -> impl std::future::Future<Output = Result<Self, ServerFnError>> {
        async move {
            let text = res.text().await?;
            Ok(Xml(text))
        }
    }
}

// ---- helpers ----------------------------------------------------------------

/// Unix epoch seconds → `"YYYY-MM-DD"` (no external date crate).
/// Identical to the private `ymd` in `api.rs` — replicated here because that
/// helper is module-private.
#[cfg(feature = "server")]
fn ymd(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// XML-escape the five predefined XML character entities in a string.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---- unified article record used by both feed fns ---------------------------

/// A minimal, unified article record built from either a compile-time seed or a
/// live CMS item.  Only the fields needed by the two XML endpoints.
#[cfg(feature = "server")]
struct FeedEntry {
    slug: String,
    title: String,
    summary: String,
    iso_date: String,
}

/// Build the merged, deduplicated, newest-first list of published articles.
///
/// Logic mirrors `src/pages/news.rs`:
///  1. Start with `content::ARTICLES` (compile-time seeds).
///  2. Fold in CMS-published items whose slug is NOT already in the seed list.
///  3. Sort the combined list by `iso_date` descending.
#[cfg(feature = "server")]
async fn merged_feed() -> Result<Vec<FeedEntry>, ServerFnError> {
    use crate::content::ARTICLES;

    // Compile-time seeds.
    let mut entries: Vec<FeedEntry> = ARTICLES
        .iter()
        .map(|a| FeedEntry {
            slug: a.slug.to_string(),
            title: a.title.to_string(),
            summary: a.summary.to_string(),
            iso_date: a.iso_date.to_string(),
        })
        .collect();

    // Live CMS articles — fold in only those not already covered by a seed.
    let cms = crate::cms::public_feed()
        .await
        .map_err(ServerFnError::new)?;
    for a in cms {
        if !ARTICLES.iter().any(|s| s.slug == a.slug) {
            let iso_date = ymd(a.published_at.unwrap_or(a.updated_at));
            entries.push(FeedEntry {
                slug: a.slug,
                title: a.title,
                summary: a.summary,
                iso_date,
            });
        }
    }

    // Newest first.
    entries.sort_by(|a, b| b.iso_date.cmp(&a.iso_date));
    Ok(entries)
}

// ---- server functions -------------------------------------------------------

/// `GET /api/sitemap_xml` — returns a valid `sitemap.xml` (sitemaps.org 0.9)
/// containing the home page, key static pages, and every published article.
///
/// Caddy rewrites `GET /sitemap.xml` to this endpoint.
#[get("/api/sitemap_xml")]
pub async fn sitemap_xml() -> Result<Xml, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use crate::config::BASE_URL;

        let entries = merged_feed().await?;

        let mut urls = String::new();

        // Static pages (home + key public routes).
        let static_pages: &[&str] = &[
            "",
            "/news",
            "/about",
            "/team",
            "/standards",
            "/contact",
            "/privacy",
            "/watch",
            "/podcast",
        ];
        for page in static_pages {
            urls.push_str(&format!(
                "  <url>\n    <loc>{}{}</loc>\n  </url>\n",
                xml_escape(BASE_URL),
                xml_escape(page),
            ));
        }

        // Published articles.
        for entry in &entries {
            urls.push_str(&format!(
                "  <url>\n    <loc>{}/news/{}</loc>\n    <lastmod>{}</lastmod>\n  </url>\n",
                xml_escape(BASE_URL),
                xml_escape(&entry.slug),
                xml_escape(&entry.iso_date),
            ));
        }

        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{urls}</urlset>"#
        );

        Ok(Xml(xml))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// `GET /api/feed_xml` — returns an RSS 2.0 feed of the most recent 30
/// published articles, suitable for Google News and feed readers.
///
/// Caddy rewrites `GET /feed.xml` to this endpoint.
#[get("/api/feed_xml")]
pub async fn feed_xml() -> Result<Xml, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use crate::config::{BASE_URL, SITE_NAME};

        let entries = merged_feed().await?;
        let recent: Vec<_> = entries.into_iter().take(30).collect();

        let mut items = String::new();
        for entry in &recent {
            let link = format!("{}/news/{}", BASE_URL, entry.slug);
            items.push_str(&format!(
                "    <item>\n\
                       <title>{}</title>\n\
                       <link>{}</link>\n\
                       <guid isPermaLink=\"true\">{}</guid>\n\
                       <pubDate>{}</pubDate>\n\
                       <description>{}</description>\n\
                     </item>\n",
                xml_escape(&entry.title),
                xml_escape(&link),
                xml_escape(&link),
                xml_escape(&entry.iso_date),
                xml_escape(&entry.summary),
            ));
        }

        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>{site_name}</title>
    <link>{base_url}</link>
    <description>{site_name} — independent court-reporting journalism.</description>
    <language>en-gb</language>
    <atom:link href="{base_url}/feed.xml" rel="self" type="application/rss+xml"/>
{items}  </channel>
</rss>"#,
            site_name = xml_escape(SITE_NAME),
            base_url = xml_escape(BASE_URL),
        );

        Ok(Xml(xml))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}
