//! Server-side CMS glue (compiled only with the `server` feature). Lazily opens
//! the SQLite database on first use, runs the schema, ensures a first admin, and
//! seeds the compile-time `content.rs` articles into the DB. The `#[server]`
//! endpoints in `api.rs` call the helpers here; nothing in this file is ever
//! compiled into the wasm/web bundle.

use ph_cms::{ArticleSeed, Db};
use tokio::sync::OnceCell;

static DB: OnceCell<Db> = OnceCell::const_new();

/// Lazily open + set up the database. Config via env:
///   PH_DB         sqlite url (default sqlite:/data/ph-press.db?mode=rwc)
///   PH_ADMIN_USER first admin username (default "admin")
///   PH_ADMIN_PASS first admin password (default: generated + logged once)
async fn db() -> Result<&'static Db, ph_cms::CmsError> {
    DB.get_or_try_init(|| async {
        let url = std::env::var("PH_DB")
            .unwrap_or_else(|_| "sqlite:/data/ph-press.db?mode=rwc".to_string());
        let admin_user = std::env::var("PH_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let admin_pass = std::env::var("PH_ADMIN_PASS").unwrap_or_else(|_| {
            let p = generated_password();
            eprintln!("[ph-press] PH_ADMIN_PASS not set; generated first-admin password: {p}");
            p
        });
        let owned = seed_data();
        let seeds: Vec<ArticleSeed> = owned.iter().map(OwnedSeed::as_seed).collect();
        ph_cms::open_and_setup(&url, &admin_user, "Administrator", &admin_pass, &seeds).await
    })
    .await
}

/// Number of publicly visible (published/corrected) articles in the DB. Used by
/// the status endpoint to confirm the live CMS is wired + seeded.
pub async fn published_count() -> Result<i64, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::published_articles(pool)
        .await
        .map(|v| v.len() as i64)
        .map_err(|e| e.to_string())
}

struct OwnedSeed {
    slug: &'static str,
    title: &'static str,
    summary: &'static str,
    body: String, // JSON array of paragraphs
    byline: &'static str,
    kind: &'static str,
    published_at: i64,
}
impl OwnedSeed {
    fn as_seed(&self) -> ArticleSeed<'_> {
        ArticleSeed {
            slug: self.slug,
            title: self.title,
            summary: self.summary,
            body: &self.body,
            byline: self.byline,
            kind: self.kind,
            published_at: self.published_at,
        }
    }
}

fn seed_data() -> Vec<OwnedSeed> {
    crate::content::ARTICLES
        .iter()
        .map(|a| OwnedSeed {
            slug: a.slug,
            title: a.title,
            summary: a.summary,
            body: serde_json::to_string(a.body).unwrap_or_else(|_| "[]".to_string()),
            byline: a.byline,
            kind: a.kind,
            published_at: iso_to_unix(a.iso_date),
        })
        .collect()
}

/// Parse "YYYY-MM-DD" to unix seconds (days_from_civil; no chrono dependency).
fn iso_to_unix(iso: &str) -> i64 {
    let p: Vec<i64> = iso.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 3 {
        return 0;
    }
    let (mut y, m, d) = (p[0], p[1], p[2]);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400
}

fn generated_password() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("ph-{n:x}")
}
