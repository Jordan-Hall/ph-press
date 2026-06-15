//! Server-side CMS glue (compiled only with the `server` feature). Lazily opens
//! the SQLite database on first use, runs the schema, ensures a first admin, and
//! seeds the compile-time `content.rs` articles into the DB. The `#[server]`
//! endpoints in `api.rs` call the helpers here; nothing in this file is ever
//! compiled into the wasm/web bundle.

use ph_cms::{ArticleSeed, Db};
use tokio::sync::OnceCell;

static DB: OnceCell<Db> = OnceCell::const_new();

/// Known default for the first admin when PH_ADMIN_PASS isn't set. Meant to be
/// changed immediately via /desk → Settings (logged as a warning at boot).
const DEFAULT_ADMIN_PASS: &str = "PH-med!a1";

/// Lazily open + set up the database. Config via env:
///   PH_DB         sqlite url (default sqlite:/data/ph-press.db?mode=rwc)
///   PH_ADMIN_USER first admin username (default "admin")
///   PH_ADMIN_PASS first admin password (default: generated + logged once)
async fn db() -> Result<&'static Db, ph_cms::CmsError> {
    DB.get_or_try_init(|| async {
        let url = std::env::var("PH_DB")
            .unwrap_or_else(|_| "sqlite:/data/ph-press.db?mode=rwc".to_string());
        let admin_user = std::env::var("PH_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        // PH_ADMIN_PASS is an OPTIONAL deploy override. When it isn't set (empty =
        // unset, since GitHub renders an unset secret as ""), the first admin gets
        // a KNOWN default so you can always log in — then change it in /desk →
        // Settings. The default is only meaningful on the very first deploy
        // (bootstrap is create-once); it never overwrites an existing admin.
        let admin_pass = std::env::var("PH_ADMIN_PASS")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                eprintln!(
                    "[ph-press] PH_ADMIN_PASS not set; first admin uses the default password. \
                     Sign in and change it in /desk → Settings."
                );
                DEFAULT_ADMIN_PASS.to_string()
            });
        // PH_ADMIN_RESET=1 (or true) on a deploy deliberately resets the admin
        // password to PH_ADMIN_PASS (operator takeover). Otherwise the existing
        // admin is never touched on an update.
        let reset = std::env::var("PH_ADMIN_RESET")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let owned = seed_data();
        let seeds: Vec<ArticleSeed> = owned.iter().map(OwnedSeed::as_seed).collect();
        ph_cms::open_and_setup(
            &url,
            &admin_user,
            "Administrator",
            &admin_pass,
            reset,
            &seeds,
        )
        .await
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

/// Authenticate a staff member + mint a session. Returns the RAW token, which the
/// API layer puts in an HttpOnly cookie. A login is recorded in the audit chain.
pub async fn login(username: &str, password: &str) -> Result<String, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = ph_cms::authenticate(pool, username, password)
        .await
        .map_err(|_| "invalid username or password".to_string())?;
    let token = ph_cms::create_session(pool, &user, ph_cms::SESSION_TTL_SECS)
        .await
        .map_err(|e| e.to_string())?;
    let _ = ph_cms::append_audit(pool, &user.username, "staff.login", &user.username, "").await;
    Ok(token)
}

/// Resolve a raw session token to a validated session (None if absent/expired).
pub async fn session_for(token: &str) -> Result<Option<ph_cms::Session>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::validate_session(pool, token)
        .await
        .map_err(|e| e.to_string())
}

/// Destroy a session (logout). Idempotent.
pub async fn logout(token: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::destroy_session(pool, token)
        .await
        .map_err(|e| e.to_string())
}

/// Every article regardless of state — the editorial dashboard listing.
pub async fn all_articles() -> Result<Vec<ph_cms::Article>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::all_articles(pool).await.map_err(|e| e.to_string())
}

/// Publicly visible articles (published/corrected), newest first — the LIVE feed
/// that surfaces stories published through /desk on the public site.
pub async fn public_feed() -> Result<Vec<ph_cms::Article>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::published_articles(pool)
        .await
        .map_err(|e| e.to_string())
}

/// A single publicly visible article by slug, for the public detail page.
pub async fn public_article(slug: &str) -> Result<Option<ph_cms::Article>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::published_by_slug(pool, slug)
        .await
        .map_err(|e| e.to_string())
}

/// Any article by id, ANY state — for an authenticated staff draft preview.
pub async fn preview_article(id: i64) -> Result<Option<ph_cms::Article>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::get_article(pool, id)
        .await
        .map_err(|e| e.to_string())
}

/// Apply a lifecycle transition as `username`. The actor is reloaded from the DB
/// so the role gate uses the CURRENT role; ph_cms::transition enforces the gate
/// (publish only via legal sign-off), logs the review, and audits.
pub async fn transition(username: &str, id: i64, to_state: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = ph_cms::find_user(pool, username)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "user not found".to_string())?;
    let to = ph_cms::State::parse(to_state).map_err(|_| "unknown target state".to_string())?;
    ph_cms::transition(pool, id, to, &user, "via /desk")
        .await
        .map_err(|e| e.to_string())
}

/// Create a Draft authored by the current user (body starts empty). `byline` is
/// the article credit (display name); `username` is the stable audit actor.
pub async fn create_draft(
    username: &str,
    byline: &str,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
) -> Result<i64, String> {
    if title.trim().is_empty() {
        return Err("a title is required".to_string());
    }
    // Each non-empty line of the editor becomes one body paragraph (the shape the
    // public renderer reads). Stored as a JSON array of strings.
    let paras: Vec<&str> = body_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::create_draft(
        pool,
        title.trim(),
        summary.trim(),
        &body_json,
        byline,
        kind,
        section,
        username,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Update an editable article's content (title/summary/body/kind/section). The
/// engine rejects editing a published article (use corrections instead).
pub async fn update_article(
    username: &str,
    id: i64,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("a title is required".to_string());
    }
    let paras: Vec<&str> = body_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::update_article(
        pool,
        id,
        title.trim(),
        summary.trim(),
        &body_json,
        kind,
        section,
        username,
    )
    .await
    .map_err(|e| e.to_string())
}

/// The published corrections archive (newest first).
pub async fn corrections() -> Result<Vec<ph_cms::Correction>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::list_corrections(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Record a correction against an article (both versions kept), audited as `actor`.
/// The engine gates this to Editor/Admin and review-logs the Published→Corrected flip.
pub async fn add_correction(
    actor: &str,
    article_id: i64,
    original: &str,
    corrected: &str,
    reason: &str,
) -> Result<i64, String> {
    if corrected.trim().is_empty() {
        return Err("the corrected text is required".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = ph_cms::find_user(pool, actor)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "user not found".to_string())?;
    ph_cms::add_correction(
        pool,
        article_id,
        original.trim(),
        corrected.trim(),
        reason.trim(),
        &user,
    )
    .await
    .map_err(|e| e.to_string())
}

/// The complaints inbox (newest first).
pub async fn complaints() -> Result<Vec<ph_cms::Complaint>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::list_complaints(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Record a complaint received by any means (currently the public route is email
/// to complaints@predatorhunters.co.uk; staff log it here).
pub async fn log_complaint(
    article_slug: &str,
    complainant: &str,
    body: &str,
) -> Result<i64, String> {
    if body.trim().is_empty() {
        return Err("the complaint details are required".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::log_complaint(pool, article_slug.trim(), complainant.trim(), body.trim())
        .await
        .map_err(|e| e.to_string())
}

/// Advance a complaint's status (received → under_review → upheld/rejected), audited.
pub async fn set_complaint_status(actor: &str, id: i64, status: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::set_complaint_status(pool, id, status, actor)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

struct OwnedSeed {
    slug: &'static str,
    title: &'static str,
    summary: &'static str,
    body: String, // JSON array of paragraphs
    byline: &'static str,
    kind: &'static str,
    section: &'static str,
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
            section: self.section,
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
            section: a.section,
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

/// Self-service password change for the logged-in user. Verifies the current
/// password first; enforces a minimum length on the new one.
pub async fn change_password(username: &str, current: &str, new: &str) -> Result<(), String> {
    if new.chars().count() < 8 {
        return Err("the new password must be at least 8 characters".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::change_password(pool, username, current, new)
        .await
        .map_err(|e| match e {
            ph_cms::CmsError::Auth => "the current password is incorrect".to_string(),
            other => other.to_string(),
        })
}
