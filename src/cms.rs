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
        let pool = ph_cms::open_and_setup(
            &url,
            &admin_user,
            "Administrator",
            &admin_pass,
            reset,
            &seeds,
        )
        .await?;
        // Start the crawler once the DB is ready (no-op unless PH_CRAWL_ENABLED).
        maybe_start_crawler(pool.clone());
        Ok(pool)
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

/// All staff users (for the admin Staff tab + the public team page).
pub async fn list_staff() -> Result<Vec<ph_cms::StaffUser>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::list_staff(pool).await.map_err(|e| e.to_string())
}

/// The hash-chained audit trail: (chain-verifies-ok, entries oldest-first as
/// (ts, actor, action, subject, detail)). Powers the admin audit viewer.
pub async fn audit_log() -> Result<(bool, Vec<(i64, String, String, String, String)>), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let chain = ph_cms::audit_chain(pool).await.map_err(|e| e.to_string())?;
    let verified = chain.verify().is_ok();
    let rows = chain
        .entries()
        .iter()
        .map(|e| {
            (
                e.ts,
                e.actor.clone(),
                e.action.clone(),
                e.subject.clone(),
                e.detail.clone(),
            )
        })
        .collect();
    Ok((verified, rows))
}

/// Create a staff user at the given role. (API layer gates this to admins.)
pub async fn create_staff(
    username: &str,
    display_name: &str,
    role: &str,
    password: &str,
) -> Result<i64, String> {
    if username.trim().is_empty() || display_name.trim().is_empty() {
        return Err("username and display name are required".to_string());
    }
    if password.chars().count() < 8 {
        return Err("the password must be at least 8 characters".to_string());
    }
    let role = ph_cms::Role::parse(role).map_err(|_| "unknown role".to_string())?;
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::create_user(pool, username.trim(), display_name.trim(), role, password)
        .await
        .map_err(|e| match e {
            ph_cms::CmsError::Db(_) => "that username is already taken".to_string(),
            other => other.to_string(),
        })
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
pub async fn transition(username: &str, id: i64, to_state: &str, note: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = ph_cms::find_user(pool, username)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "user not found".to_string())?;
    let to = ph_cms::State::parse(to_state).map_err(|_| "unknown target state".to_string())?;
    let note = if note.trim().is_empty() {
        "via /desk"
    } else {
        note.trim()
    };
    ph_cms::transition(pool, id, to, &user, note)
        .await
        .map_err(|e| e.to_string())
}

/// Create a Draft authored by the current user (body starts empty). `byline` is
/// the article credit (display name); `username` is the stable audit actor.
#[allow(clippy::too_many_arguments)]
pub async fn create_draft(
    username: &str,
    byline: &str,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
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
        meta_description.trim(),
        og_image_url.trim(),
        tags,
    )
    .await
    .map_err(|e| e.to_string())
}

/// Update an editable article's content (title/summary/body/kind/section). The
/// engine rejects editing a published article (use corrections instead).
#[allow(clippy::too_many_arguments)]
pub async fn update_article(
    username: &str,
    id: i64,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
    slug: &str,
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
        meta_description.trim(),
        og_image_url.trim(),
        tags,
        slug.trim(),
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

// ===================== crawler ingest + court-watch =====================
// Glue over ph_cms::ingest (PUBLIC leads + conviction database) and
// ph_cms::courtwatch (PRIVATE upcoming/appeal hearings). The two stores never
// cross (the active-proceedings firewall lives in ph-cms).

/// Reload the actor as a StaffUser so the engine's role gate uses the current role.
async fn actor_user(pool: &ph_cms::Db, actor: &str) -> Result<ph_cms::StaffUser, String> {
    ph_cms::find_user(pool, actor)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "user not found".to_string())
}

/// Crawled leads (optionally filtered by status), newest first — the Intake desk.
pub async fn leads(status: Option<&str>) -> Result<Vec<ph_cms::ingest::IngestItem>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::list_leads(pool, status)
        .await
        .map_err(|e| e.to_string())
}

/// Promote a lead into a Draft article — AI-drafted when enabled, banner otherwise.
pub async fn promote_lead(actor: &str, id: i64, kind: &str, section: &str) -> Result<i64, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let lead = ph_cms::ingest::get_lead(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no lead {id}"))?;
    // Authorize + dedupe BEFORE any outbound AI generation (no surprise spend).
    if lead.status == "promoted" {
        return Err("this lead is already promoted".to_string());
    }
    let role = user.role().map_err(|e| e.to_string())?;
    if !matches!(
        role,
        ph_cms::Role::Writer | ph_cms::Role::SubEditor | ph_cms::Role::Editor | ph_cms::Role::Admin
    ) {
        return Err("your role cannot promote a lead into a draft".to_string());
    }
    let content = generate_promo_content(&lead, kind, section).await;
    ph_cms::ingest::promote_lead_with_draft(pool, id, &user, kind, section, &content)
        .await
        .map_err(|e| e.to_string())
}

/// Set a lead's triage status (triaged / dismissed).
pub async fn set_lead_status(actor: &str, id: i64, status: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::set_lead_status(pool, id, status, actor)
        .await
        .map_err(|e| e.to_string())
}

/// Conviction-database entries (optionally filtered by status), newest first.
pub async fn convictions(status: Option<&str>) -> Result<Vec<ph_cms::ingest::Conviction>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::list_convictions(pool, status)
        .await
        .map_err(|e| e.to_string())
}

/// Published conviction entries — the PUBLIC `/database` read.
pub async fn published_convictions() -> Result<Vec<ph_cms::ingest::Conviction>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::published_convictions(pool)
        .await
        .map_err(|e| e.to_string())
}

fn build_conviction(
    name: &str,
    area: &str,
    offence: &str,
    outcome: &str,
    date: &str,
    iso_date: &str,
    lat: f64,
    lng: f64,
    article_id: Option<i64>,
    article_slug: &str,
    source_url: &str,
    source_name: &str,
) -> ph_cms::ingest::NewConviction {
    ph_cms::ingest::NewConviction {
        name: name.trim().to_string(),
        area: area.trim().to_string(),
        offence: offence.trim().to_string(),
        outcome: outcome.trim().to_string(),
        date: date.trim().to_string(),
        iso_date: iso_date.trim().to_string(),
        lat,
        lng,
        article_id,
        article_slug: article_slug.trim().to_string(),
        source_url: source_url.trim().to_string(),
        source_name: source_name.trim().to_string(),
    }
}

/// Create a draft conviction entry.
#[allow(clippy::too_many_arguments)]
pub async fn create_conviction(
    actor: &str,
    name: &str,
    area: &str,
    offence: &str,
    outcome: &str,
    date: &str,
    iso_date: &str,
    lat: f64,
    lng: f64,
    article_id: Option<i64>,
    article_slug: &str,
    source_url: &str,
    source_name: &str,
) -> Result<i64, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let c = build_conviction(
        name,
        area,
        offence,
        outcome,
        date,
        iso_date,
        lat,
        lng,
        article_id,
        article_slug,
        source_url,
        source_name,
    );
    ph_cms::ingest::create_conviction(pool, &c, &user)
        .await
        .map_err(|e| e.to_string())
}

/// Edit a draft conviction entry.
#[allow(clippy::too_many_arguments)]
pub async fn update_conviction(
    actor: &str,
    id: i64,
    name: &str,
    area: &str,
    offence: &str,
    outcome: &str,
    date: &str,
    iso_date: &str,
    lat: f64,
    lng: f64,
    article_id: Option<i64>,
    article_slug: &str,
    source_url: &str,
    source_name: &str,
) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let c = build_conviction(
        name,
        area,
        offence,
        outcome,
        date,
        iso_date,
        lat,
        lng,
        article_id,
        article_slug,
        source_url,
        source_name,
    );
    ph_cms::ingest::update_conviction(pool, &c, id, &user)
        .await
        .map_err(|e| e.to_string())
}

/// Publish or retract a conviction (publish requires a linked, published report).
pub async fn set_conviction_status(actor: &str, id: i64, status: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    ph_cms::ingest::set_conviction_status(pool, id, status, &user)
        .await
        .map_err(|e| e.to_string())
}

/// Private court-watch entries (optionally filtered by status), soonest first.
pub async fn court_watch(
    status: Option<&str>,
) -> Result<Vec<ph_cms::courtwatch::CourtWatch>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::courtwatch::list_watch(pool, status)
        .await
        .map_err(|e| e.to_string())
}

/// Add a court-watch entry by hand (e.g. a tip). Synthesises a unique id so the
/// dedupe key never collides with crawled rows.
#[allow(clippy::too_many_arguments)]
pub async fn add_watch(
    actor: &str,
    court: &str,
    case_ref: &str,
    hearing_date: &str,
    hearing_type: &str,
    offence_category: &str,
    source_url: &str,
    notes: &str,
) -> Result<i64, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let _ = actor_user(pool, actor).await?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let w = ph_cms::courtwatch::NewWatch {
        court: court.trim().to_string(),
        case_ref: case_ref.trim().to_string(),
        hearing_date: hearing_date.trim().to_string(),
        hearing_type: hearing_type.trim().to_string(),
        offence_category: offence_category.trim().to_string(),
        source_key: "manual".to_string(),
        external_id: format!("manual-{nanos}"),
        source_url: source_url.trim().to_string(),
        notes: notes.trim().to_string(),
    };
    ph_cms::courtwatch::insert_watch(pool, &w)
        .await
        .map(|o| o.unwrap_or(0))
        .map_err(|e| e.to_string())
}

/// Update a court-watch entry's status (attending / transcript requested / closed).
pub async fn set_watch_status(
    actor: &str,
    id: i64,
    status: &str,
    note: &str,
) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    ph_cms::courtwatch::set_watch_status(pool, id, status, note, &user)
        .await
        .map_err(|e| e.to_string())
}

// ===================== crawler boot =====================

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Resolve the AI config from env, or None when disabled / unconfigured.
/// OFF by default — no surprise outbound traffic.
fn ai_config() -> Option<ph_ai::AiConfig> {
    if !env_flag("PH_AI_ENABLED") {
        return None;
    }
    let backend_env = std::env::var("PH_AI_BACKEND").unwrap_or_default();
    let backend = match backend_env.as_str() {
        "anthropic" => ph_ai::Backend::Anthropic,
        "" | "local" => ph_ai::Backend::Local, // default: local OpenAI-compatible
        other => {
            eprintln!(
                "[ph-press] PH_AI_BACKEND={other:?} is not recognised (expected \"local\" or \
                 \"anthropic\"); defaulting to local"
            );
            ph_ai::Backend::Local
        }
    };
    let api_key = std::env::var("PH_AI_API_KEY").ok().unwrap_or_default();
    // Anthropic requires a key; local does not.
    if backend == ph_ai::Backend::Anthropic && api_key.trim().is_empty() {
        eprintln!("[ph-press] PH_AI_BACKEND=anthropic but PH_AI_API_KEY is empty; AI disabled");
        return None;
    }
    let default_base = match backend {
        ph_ai::Backend::Anthropic => "https://api.anthropic.com",
        ph_ai::Backend::Local => "http://127.0.0.1:8080",
    };
    let base_url = std::env::var("PH_AI_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_base.to_string());
    let model_env = std::env::var("PH_AI_MODEL").ok().filter(|s| !s.is_empty());
    if backend == ph_ai::Backend::Local && model_env.is_none() {
        eprintln!(
            "[ph-press] PH_AI_MODEL is not set; using placeholder \"local-model\". \
             Set PH_AI_MODEL to the name of the model your local server is serving."
        );
    }
    let default_model = match backend {
        ph_ai::Backend::Anthropic => "claude-sonnet-4-6",
        ph_ai::Backend::Local => "local-model",
    };
    let model = model_env.unwrap_or_else(|| default_model.to_string());
    let timeout_secs = std::env::var("PH_AI_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    Some(ph_ai::AiConfig { backend, api_key, model, base_url, max_tokens: 4000, timeout_secs })
}

/// Build LeadFacts from a stored lead (pull citation/court/id_risk from extracted_json).
fn lead_facts(lead: &ph_cms::ingest::IngestItem, kind: &str, section: &str) -> ph_ai::LeadFacts {
    let v: serde_json::Value =
        serde_json::from_str(&lead.extracted_json).unwrap_or(serde_json::Value::Null);
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    ph_ai::LeadFacts {
        title: lead.title.clone(),
        snippet: lead.snippet.clone(),
        offence_category: lead.offence_category.clone(),
        source_key: lead.source_key.clone(),
        source_url: lead.url.clone(),
        citation: get("citation"),
        court: get("court"),
        kind: kind.to_string(),
        section: section.to_string(),
        id_risk: v.get("identification_risk").and_then(|b| b.as_bool()).unwrap_or(false),
    }
}

/// Generate the promote content ONCE. When AI is enabled and succeeds, a provenance
/// banner paragraph is prepended to the AI body and a figure placeholder is appended.
/// When AI is disabled or the call fails, the banner draft is returned wholesale.
async fn generate_promo_content(
    lead: &ph_cms::ingest::IngestItem,
    kind: &str,
    section: &str,
) -> ph_cms::ingest::PromotedDraft {
    let banner = ph_cms::ingest::banner_draft(lead);
    let Some(cfg) = ai_config() else {
        return banner;
    };
    let facts = lead_facts(lead, kind, section);
    match ph_ai::draft(&facts, &cfg).await {
        Ok(d) => {
            // Prepend the provenance banner; append a figure placeholder slot.
            let banner_para = "DRAFT FROM AN EXTERNAL LEAD — unverified. Write this report \
                from the public court record; clear reporting restrictions and confirm the \
                conviction before publishing. Source for context only — do not copy its wording.";
            let mut paras = vec![banner_para.to_string()];
            paras.extend(d.body_paragraphs);
            if !d.figure_caption.trim().is_empty() {
                paras.push(format!("![{}](  )", d.figure_caption.trim()));
            }
            paras.push(format!("Source ({}): {}", lead.source_key, lead.url));
            let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
            let tags = serde_json::to_string(&d.tags).unwrap_or_else(|_| "[]".to_string());
            ph_cms::ingest::PromotedDraft {
                summary: d.summary,
                body_json,
                meta_description: d.meta_description,
                og_image_url: String::new(),
                tags,
                slug_base: d.slug,
            }
        }
        Err(e) => {
            eprintln!("[ph-press] AI draft failed ({e}); using banner draft");
            banner
        }
    }
}

/// Parse a `key|label|url;key|label|url` env list into source configs of `kind`.
fn parse_sources(raw: &str, kind: &str) -> Vec<ph_crawl::SourceConfig> {
    raw.split(';')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.split('|').map(str::trim).collect();
            if parts.len() == 3 && !parts[0].is_empty() && !parts[2].is_empty() {
                Some(ph_crawl::SourceConfig::new(
                    parts[0], kind, parts[1], parts[2],
                ))
            } else {
                None
            }
        })
        .collect()
}

/// Sources for a kind: parse the override env var when set+non-empty, else fall
/// back to the built-in presets (court-watch has no preset — opt-in only).
fn sources_for(
    kind: &str,
    env_var: &str,
    presets: fn() -> Vec<ph_crawl::SourceConfig>,
) -> Vec<ph_crawl::SourceConfig> {
    match std::env::var(env_var).ok().map(|v| parse_sources(&v, kind)) {
        Some(v) if !v.is_empty() => v,
        _ => presets(),
    }
}

/// Start the background crawl loop once, if `PH_CRAWL_ENABLED` is set. Each kind
/// uses its `PH_CRAWL_*_FEEDS` override (`key|label|url;…`) or the built-in
/// presets (Find Case Law + BBC regional news); court-watch is opt-in via
/// `PH_CRAWL_COURTWATCH_FEEDS`. Interval via `PH_CRAWL_INTERVAL_SECS` (default
/// 3600, min 60). OFF by default so there is never surprise outbound traffic.
/// Resolve the crawl sources from env overrides + presets (court-watch opt-in).
fn crawler_sources() -> Vec<ph_crawl::SourceConfig> {
    let mut sources = sources_for(
        "caselaw",
        "PH_CRAWL_CASELAW_FEEDS",
        ph_crawl::presets::caselaw,
    );
    sources.extend(sources_for(
        "news",
        "PH_CRAWL_NEWS_FEEDS",
        ph_crawl::presets::news,
    ));
    sources.extend(sources_for(
        "police",
        "PH_CRAWL_POLICE_FEEDS",
        ph_crawl::presets::police,
    ));
    if let Ok(v) = std::env::var("PH_CRAWL_COURTWATCH_FEEDS") {
        sources.extend(parse_sources(&v, "courtwatch"));
    }
    sources
}

fn crawler_user_agent() -> String {
    std::env::var("PH_CRAWL_USER_AGENT")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ph_crawl::DEFAULT_USER_AGENT.to_string())
}

fn maybe_start_crawler(pool: ph_cms::Db) {
    if !env_flag("PH_CRAWL_ENABLED") {
        return;
    }
    let sources = crawler_sources();
    if sources.is_empty() {
        eprintln!("[ph-press] PH_CRAWL_ENABLED set but no sources resolved; crawler idle");
        return;
    }
    let secs = std::env::var("PH_CRAWL_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(3600)
        .max(60);
    eprintln!(
        "[ph-press] starting crawler: {} source(s), every {secs}s",
        sources.len()
    );
    ph_crawl::spawn(
        pool,
        sources,
        std::time::Duration::from_secs(secs),
        crawler_user_agent(),
    );
}

/// Promote a lead into a draft article + a linked draft conviction (AI or banner).
pub async fn promote_lead_to_conviction(
    actor: &str,
    id: i64,
    kind: &str,
    section: &str,
) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let lead = ph_cms::ingest::get_lead(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no lead {id}"))?;
    // Authorize + dedupe BEFORE any outbound AI generation (no surprise spend).
    if lead.status == "promoted" {
        return Err("this lead is already promoted".to_string());
    }
    let role = user.role().map_err(|e| e.to_string())?;
    if !matches!(
        role,
        ph_cms::Role::Writer | ph_cms::Role::SubEditor | ph_cms::Role::Editor | ph_cms::Role::Admin
    ) {
        return Err("your role cannot promote a lead into a draft".to_string());
    }
    let content = generate_promo_content(&lead, kind, section).await;
    ph_cms::ingest::promote_lead_to_conviction_with_draft(pool, id, &user, kind, section, &content)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Run one crawl pass now, in the background. Seeds sources first so it works even
/// when the scheduled loop is off. Returns as soon as the pass is queued.
pub async fn crawl_now() -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let sources = crawler_sources();
    if sources.is_empty() {
        return Err("no crawler sources configured".to_string());
    }
    let fetcher = ph_crawl::Fetcher::new(crawler_user_agent()).map_err(|e| e.to_string())?;
    ph_crawl::seed_sources(pool, &sources)
        .await
        .map_err(|e| e.to_string())?;
    let pool2 = pool.clone();
    tokio::spawn(async move {
        let r = ph_crawl::run_once(&pool2, &fetcher).await;
        eprintln!(
            "[ph-press] manual poll: {} leads, {} watch, {} sources, {} errors",
            r.leads_added,
            r.watch_added,
            r.sources_polled,
            r.errors.len()
        );
    });
    Ok(())
}

/// Configured sources for the desk Sources view (key, kind, label, last_polled_at).
pub async fn sources() -> Result<Vec<ph_cms::ingest::IngestSource>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::list_sources(pool)
        .await
        .map_err(|e| e.to_string())
}
