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
///   PH_ADMIN_PASS optional. UNSET → no admin is seeded; a fresh deploy creates the
///                 first admin via the /desk install screen (no default password).
///                 SET → env-seeds the first admin (+ resets it with PH_ADMIN_RESET).
async fn db() -> Result<&'static Db, ph_cms::CmsError> {
    DB.get_or_try_init(|| async {
        let url = std::env::var("PH_DB")
            .unwrap_or_else(|_| "sqlite:/data/ph-press.db?mode=rwc".to_string());
        let admin_user = std::env::var("PH_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        // PH_ADMIN_PASS is OPTIONAL. Unset (empty — GitHub renders an unset secret
        // as "") → None: no admin is auto-created, and a fresh deploy is finished
        // via the /desk first-run install screen. Set → env-seed the first admin.
        // There is no default password anywhere.
        let admin_pass: Option<String> =
            std::env::var("PH_ADMIN_PASS").ok().filter(|s| !s.is_empty());
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
            admin_pass.as_deref(),
            reset,
            &seeds,
        )
        .await?;
        // Link the admin account to a recovery email (PH_ADMIN_EMAIL). Runs every
        // deploy and is idempotent, so an admin created before this feature still
        // gets its email set — bootstrap_admin only ever *creates*, never updates.
        if let Ok(email) = std::env::var("PH_ADMIN_EMAIL") {
            if !email.trim().is_empty() {
                match ph_cms::set_user_email(&pool, &admin_user, email.trim()).await {
                    Ok(true) => eprintln!("[ph-press] admin '{admin_user}' recovery email set"),
                    Ok(false) => {
                        eprintln!("[ph-press] PH_ADMIN_EMAIL set but no user '{admin_user}' to update")
                    }
                    Err(e) => eprintln!("[ph-press] failed to set admin recovery email: {e}"),
                }
            }
        }
        // Start the crawler once the DB is ready (no-op unless PH_CRAWL_ENABLED).
        maybe_start_crawler(pool.clone());
        Ok(pool)
    })
    .await
}

/// Does the site still need first-run setup (no staff users yet)? Drives the
/// `/desk` install screen.
pub async fn needs_install() -> Result<bool, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::needs_install(pool).await.map_err(|e| e.to_string())
}

/// First-run install: create the very first administrator account and mint a
/// session for them. Valid ONLY while no users exist (re-checked here to close the
/// race), so it can never create a second account or run after setup. Returns the
/// raw session token for the API layer to put in the cookie.
pub async fn install_admin(
    username: &str,
    display_name: &str,
    email: &str,
    password: &str,
) -> Result<String, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    if !ph_cms::needs_install(pool).await.map_err(|e| e.to_string())? {
        return Err("setup has already been completed".to_string());
    }
    if username.trim().is_empty() || display_name.trim().is_empty() {
        return Err("username and display name are required".to_string());
    }
    ph_cms::create_user(
        pool,
        username.trim(),
        display_name.trim(),
        ph_cms::Role::Admin,
        password,
        email,
    )
    .await
    .map_err(|e| match e {
        ph_cms::CmsError::Db(_) => "that username is already taken".to_string(),
        other => other.to_string(),
    })?;
    let user = ph_cms::authenticate(pool, username.trim(), password)
        .await
        .map_err(|e| e.to_string())?;
    let token = ph_cms::create_session(pool, &user, ph_cms::SESSION_TTL_SECS)
        .await
        .map_err(|e| e.to_string())?;
    let _ = ph_cms::append_audit(
        pool,
        &user.username,
        "install.admin",
        &user.username,
        "first-run install",
    )
    .await;
    Ok(token)
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

/// Build the absolute reset link for a raw token. The base URL comes from
/// PH_PUBLIC_BASE_URL (default the production apex) so the link works in an email.
fn reset_link(token: &str) -> String {
    let base = std::env::var("PH_PUBLIC_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://predatorhunters.co.uk".to_string());
    format!("{}/desk/reset/{token}", base.trim_end_matches('/'))
}

/// Begin password recovery for `email`. Mints a single-use 1-hour reset link when
/// the email matches an account; does nothing (but still succeeds) otherwise — the
/// caller MUST report the same message either way so registered emails can't be
/// probed. The link is always logged so an operator can retrieve it from the
/// container logs until email delivery is configured; once a provider is wired up
/// it is also emailed. Only DB failures surface as an error.
pub async fn request_password_reset(email: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let issued = ph_cms::create_password_reset(pool, email, ph_cms::RESET_TTL_SECS)
        .await
        .map_err(|e| e.to_string())?;
    if let Some((token, user)) = issued {
        let link = reset_link(&token);
        // Always log the link: it's the operator fallback (read from the container
        // logs via SSM) when email delivery is off or fails.
        eprintln!(
            "[ph-press] password-reset link for {} <{}>: {link}",
            user.username,
            user.email.as_deref().unwrap_or("")
        );
        // Deliver by email when a provider is configured (PH_EMAIL_BACKEND).
        if let (Some(cfg), Some(to)) = (ph_email::EmailConfig::from_env(), user.email.as_deref()) {
            send_reset_email(&cfg, to, &link).await;
        }
    }
    Ok(())
}

/// Send a password-reset email via the configured provider. Failures are logged,
/// never propagated — the link is always in the server log as a fallback, and the
/// endpoint must stay non-revealing about whether the email exists.
async fn send_reset_email(cfg: &ph_email::EmailConfig, to: &str, link: &str) {
    let subject = "Reset your Predator Hunters editorial password";
    let text = format!(
        "We received a request to reset your Predator Hunters editorial password.\n\n\
         Open this link to choose a new one (it expires in one hour):\n{link}\n\n\
         If you didn't request this you can ignore this email — your password won't change."
    );
    let html = format!(
        "<p>We received a request to reset your Predator Hunters editorial password.</p>\
         <p><a href=\"{link}\">Choose a new password</a> — this link expires in one hour.</p>\
         <p>If you didn't request this you can ignore this email; your password won't change.</p>"
    );
    let msg = ph_email::Email { to, subject, text: &text, html: &html };
    match ph_email::send(cfg, &msg).await {
        Ok(id) => eprintln!("[ph-press] reset email sent to {to} (id {id})"),
        Err(e) => eprintln!("[ph-press] reset email to {to} failed: {e}"),
    }
}

/// Minimal HTML escaping for staff/complainant text placed in an HTML email part.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Email the complainant an acknowledgement (IMPRESS: prompt acknowledgement).
/// Best-effort — logged, never fatal to the submission; no-op without an email
/// backend (PH_EMAIL_BACKEND) configured.
async fn send_complaint_ack(to: &str, reference: &str, article_slug: &str) {
    let Some(cfg) = ph_email::EmailConfig::from_env() else {
        return;
    };
    if to.is_empty() {
        return;
    }
    let subject = format!("We've received your complaint ({reference})");
    let text = format!(
        "Thank you for your complaint about our article \"{article_slug}\".\n\n\
         Your reference is {reference}. In line with the IMPRESS Standards Code we acknowledge \
         complaints promptly and aim to give a final response within 21 days. We may contact you \
         for more detail, and will let you know the outcome.\n\n\
         If you are unhappy with our final response you can refer your complaint to IMPRESS, our \
         independent regulator: https://impress.press/complaints/\n\n\
         \u{2014} Predator Hunters"
    );
    let html = format!(
        "<p>Thank you for your complaint about our article \u{201c}{}\u{201d}.</p>\
         <p>Your reference is <strong>{reference}</strong>. In line with the IMPRESS Standards Code \
         we acknowledge complaints promptly and aim to give a final response within 21 days. We may \
         contact you for more detail, and will let you know the outcome.</p>\
         <p>If you are unhappy with our final response you can refer your complaint to \
         <a href=\"https://impress.press/complaints/\">IMPRESS</a>, our independent regulator.</p>\
         <p>\u{2014} Predator Hunters</p>",
        html_escape(article_slug)
    );
    let msg = ph_email::Email { to, subject: &subject, text: &text, html: &html };
    match ph_email::send(&cfg, &msg).await {
        Ok(id) => eprintln!("[ph-press] complaint ack emailed to {to} ({reference}, id {id})"),
        Err(e) => eprintln!("[ph-press] complaint ack to {to} failed: {e}"),
    }
}

/// Email a staff reply to the complainant. Best-effort; logged. When no email
/// backend is configured the reply is still recorded (the caller did that).
async fn send_complaint_reply_email(to: &str, reference: &str, body: &str) {
    let Some(cfg) = ph_email::EmailConfig::from_env() else {
        eprintln!("[ph-press] complaint reply {reference} recorded but NOT emailed (no email backend)");
        return;
    };
    if to.is_empty() {
        eprintln!("[ph-press] complaint reply {reference} recorded but no complainant email on file");
        return;
    }
    let subject = format!("Re: your complaint ({reference})");
    let html = body
        .lines()
        .map(|l| format!("<p>{}</p>", html_escape(l)))
        .collect::<String>();
    let msg = ph_email::Email { to, subject: &subject, text: body, html: &html };
    match ph_email::send(&cfg, &msg).await {
        Ok(id) => eprintln!("[ph-press] complaint reply emailed to {to} ({reference}, id {id})"),
        Err(e) => eprintln!("[ph-press] complaint reply to {to} failed: {e}"),
    }
}

/// Complete password recovery: redeem the reset token and set the new password
/// (which destroys the account's existing sessions). Password-strength validation
/// is the API layer's responsibility, as elsewhere. A bad/expired/used token gives
/// a generic error so a guessed token can't be distinguished from an expired one.
pub async fn complete_password_reset(token: &str, new_password: &str) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::consume_password_reset(pool, token, new_password)
        .await
        .map(|_| ())
        .map_err(|_| "this reset link is invalid or has expired".to_string())
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
    email: &str,
) -> Result<i64, String> {
    if username.trim().is_empty() || display_name.trim().is_empty() {
        return Err("username and display name are required".to_string());
    }
    if password.chars().count() < 8 {
        return Err("the password must be at least 8 characters".to_string());
    }
    let role = ph_cms::Role::parse(role).map_err(|_| "unknown role".to_string())?;
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::create_user(pool, username.trim(), display_name.trim(), role, password, email)
        .await
        .map_err(|e| match e {
            ph_cms::CmsError::Db(_) => "that username is already taken".to_string(),
            other => other.to_string(),
        })
}

/// Return the contact email for the given username (None when unset).
pub async fn get_user_email(username: &str) -> Result<Option<String>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = ph_cms::find_user(pool, username)
        .await
        .map_err(|e| e.to_string())?;
    Ok(user.and_then(|u| u.email))
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

/// A human-friendly complaint reference shown to the complainant.
pub fn complaint_reference(id: i64) -> String {
    format!("PH-C{id}")
}

/// Public per-article complaint submission. Records the complaint, emails the
/// complainant an acknowledgement (IMPRESS: prompt acknowledgement) when email is
/// configured, and returns the reference for the confirmation screen.
pub async fn submit_complaint(
    article_slug: &str,
    complainant: &str,
    email: &str,
    category: &str,
    body: &str,
) -> Result<String, String> {
    if complainant.trim().is_empty() {
        return Err("Please give your name.".to_string());
    }
    if !email.contains('@') {
        return Err("Please give a valid email so we can respond.".to_string());
    }
    if body.trim().is_empty() {
        return Err("Please describe the problem with the article.".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    let id = ph_cms::log_complaint(
        pool,
        article_slug.trim(),
        complainant.trim(),
        email,
        category,
        body.trim(),
    )
    .await
    .map_err(|e| e.to_string())?;
    let reference = complaint_reference(id);
    // The complaint is recorded above no matter what; only the outbound ack is
    // rate-capped, so this public endpoint can't relay mail to arbitrary addresses.
    if ack_send_allowed(email.trim()) {
        send_complaint_ack(email.trim(), &reference, article_slug.trim()).await;
    } else {
        eprintln!("[ph-press] complaint {reference} recorded; ack email rate-capped");
    }
    Ok(reference)
}

/// Rate-gate for acknowledgement emails. A GLOBAL cap bounds total outbound volume
/// regardless of how an attacker varies the email (the per-email cooldown alone is
/// trivially bypassed), plus a per-email cooldown for honest duplicates. The
/// complaint itself is always recorded — only the ack send is gated.
fn ack_send_allowed(email: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};
    const PER_EMAIL_COOLDOWN: u64 = 300; // one ack per email per 5 min
    const GLOBAL_WINDOW: u64 = 60;
    const GLOBAL_MAX: usize = 10; // at most 10 acks/min across everyone
    static PER_EMAIL: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    static GLOBAL: OnceLock<Mutex<Vec<u64>>> = OnceLock::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Global cap first (the abuse bound).
    {
        let mut g = GLOBAL.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
        g.retain(|&t| now.saturating_sub(t) < GLOBAL_WINDOW);
        if g.len() >= GLOBAL_MAX {
            return false;
        }
    }
    // Per-email cooldown.
    let key = email.trim().to_lowercase();
    {
        let mut m = PER_EMAIL.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
        m.retain(|_, &mut t| now.saturating_sub(t) < PER_EMAIL_COOLDOWN);
        if m.get(&key).is_some_and(|&t| now.saturating_sub(t) < PER_EMAIL_COOLDOWN) {
            return false;
        }
        m.insert(key, now);
    }
    GLOBAL
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(now);
    true
}

/// Record a complaint that arrived another way (staff log it on the complainant's
/// behalf). No acknowledgement email is sent.
pub async fn log_complaint(
    article_slug: &str,
    complainant: &str,
    email: &str,
    category: &str,
    body: &str,
) -> Result<i64, String> {
    if body.trim().is_empty() {
        return Err("the complaint details are required".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::log_complaint(
        pool,
        article_slug.trim(),
        complainant.trim(),
        email,
        category,
        body.trim(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// A complaint + its handling thread (for the desk detail view).
pub async fn complaint_detail(
    id: i64,
) -> Result<(ph_cms::Complaint, Vec<ph_cms::ComplaintMessage>), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let complaint = ph_cms::get_complaint(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no complaint {id}"))?;
    let thread = ph_cms::list_complaint_messages(pool, id)
        .await
        .map_err(|e| e.to_string())?;
    Ok((complaint, thread))
}

/// Add a staff-only internal note to a complaint's handling thread.
pub async fn add_complaint_note(actor: &str, id: i64, body: &str) -> Result<(), String> {
    if body.trim().is_empty() {
        return Err("the note is empty".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::add_complaint_message(pool, id, actor, "internal", body.trim())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Reply to the complainant: record it on the thread AND email it (via SES). The
/// record is kept even if email isn't configured / delivery fails.
pub async fn send_complaint_reply(actor: &str, id: i64, body: &str) -> Result<(), String> {
    if body.trim().is_empty() {
        return Err("the reply is empty".to_string());
    }
    let pool = db().await.map_err(|e| e.to_string())?;
    let complaint = ph_cms::get_complaint(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no complaint {id}"))?;
    ph_cms::add_complaint_message(pool, id, actor, "reply", body.trim())
        .await
        .map_err(|e| e.to_string())?;
    send_complaint_reply_email(
        complaint.complainant_email.trim(),
        &complaint_reference(id),
        body.trim(),
    )
    .await;
    Ok(())
}

/// Advance a complaint's status (the IMPRESS workflow), audited; stamps the
/// acknowledged/resolved timestamps in ph-cms.
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

/// Re-run AI generation on a draft promoted from a lead, overwriting its body + SEO.
/// Requires: AI enabled, the article is a `draft`, an authoring-role actor, and the
/// article was promoted from a lead. Never clobbers a submitted/published article.
pub async fn regenerate_draft(actor: &str, article_id: i64) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    // authoring role gate (same set as promote)
    let role = user.role().map_err(|e| e.to_string())?;
    if !matches!(role, ph_cms::Role::Writer | ph_cms::Role::SubEditor | ph_cms::Role::Editor | ph_cms::Role::Admin) {
        return Err("your role cannot regenerate a draft".to_string());
    }
    if ai_config().is_none() {
        return Err("AI drafting is not enabled".to_string());
    }
    let article = ph_cms::get_article(pool, article_id).await.map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no article {article_id}"))?;
    if article.state != "draft" {
        return Err("only a draft can be regenerated".to_string());
    }
    let lead = ph_cms::ingest::lead_by_promoted_article(pool, article_id).await.map_err(|e| e.to_string())?
        .ok_or_else(|| "this draft was not promoted from a lead".to_string())?;
    // Strict: on AI failure this returns Err and we bail out below, leaving the
    // existing draft untouched — never clobber a draft with the banner fallback.
    let content = generate_promo_content_strict(&lead, &article.kind, &article.section).await?;
    // Write the regenerated content directly (body is already a JSON array; keep current slug).
    ph_cms::update_article(
        pool, article_id, &article.title, &content.summary, &content.body_json,
        &article.kind, &article.section, actor,
        &content.meta_description, &content.og_image_url, &content.tags, "",
    ).await.map_err(|e| e.to_string())
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
        "bedrock" => ph_ai::Backend::Bedrock,
        "" | "local" => ph_ai::Backend::Local, // default: local OpenAI-compatible
        other => {
            eprintln!(
                "[ph-press] PH_AI_BACKEND={other:?} is not recognised (expected \"local\", \
                 \"anthropic\", or \"bedrock\"); defaulting to local"
            );
            ph_ai::Backend::Local
        }
    };
    let api_key = std::env::var("PH_AI_API_KEY").ok().unwrap_or_default();
    // Anthropic requires a key; local and Bedrock do not (Bedrock uses the cred chain).
    if backend == ph_ai::Backend::Anthropic && api_key.trim().is_empty() {
        eprintln!("[ph-press] PH_AI_BACKEND=anthropic but PH_AI_API_KEY is empty; AI disabled");
        return None;
    }
    let default_base = match backend {
        ph_ai::Backend::Anthropic => "https://api.anthropic.com",
        ph_ai::Backend::Local => "http://127.0.0.1:8080",
        ph_ai::Backend::Bedrock => "", // unused for Bedrock
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
        ph_ai::Backend::Bedrock => "amazon.nova-lite-v1:0",
    };
    let model = model_env.unwrap_or_else(|| default_model.to_string());
    let timeout_secs = std::env::var("PH_AI_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    let region = std::env::var("PH_AI_REGION")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("AWS_REGION").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "eu-west-2".to_string());
    Some(ph_ai::AiConfig { backend, api_key, model, base_url, max_tokens: 4000, timeout_secs, region })
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
/// Self-host the lead's image at PROMOTE time: download it into the local media
/// dir (`PH_MEDIA_DIR`, default `/data/uploads`) and return a lead clone with
/// `image_url` rewritten to the served `/uploads/<sha>.<ext>` path, so a published
/// report self-hosts the photo. If the image can't be self-hosted (no image,
/// SSRF-blocked, not a real image, oversize, fetch/write error) it is DROPPED, NOT
/// hot-linked — so nothing published ever silently depends on the source staying
/// up. The download is SSRF-hardened in `ph_crawl::image::fetch_image_local`.
async fn localize_lead_image(lead: &ph_cms::ingest::IngestItem) -> ph_cms::ingest::IngestItem {
    if lead.image_url.is_empty() || lead.image_url.starts_with("/uploads/") {
        return lead.clone();
    }
    let dir = std::env::var("PH_MEDIA_DIR").unwrap_or_else(|_| "/data/uploads".to_string());
    let mut l = lead.clone();
    match ph_crawl::image::fetch_image_local(
        &lead.image_url,
        std::path::Path::new(&dir),
        &crawler_user_agent(),
    )
    .await
    {
        Some(name) => l.image_url = format!("/uploads/{name}"),
        None => {
            // Drop rather than hot-link the source.
            l.image_url = String::new();
            l.image_attribution = String::new();
        }
    }
    l
}

async fn generate_promo_content(
    lead: &ph_cms::ingest::IngestItem,
    kind: &str,
    section: &str,
) -> ph_cms::ingest::PromotedDraft {
    let lead = localize_lead_image(lead).await;
    let lead = &lead;
    let banner = ph_cms::ingest::banner_draft(lead);
    let Some(cfg) = ai_config() else {
        return banner;
    };
    let facts = lead_facts(lead, kind, section);
    match ph_ai::draft(&facts, &cfg).await {
        Ok(d) => assemble_promo_draft(d, lead),
        Err(e) => {
            eprintln!("[ph-press] AI draft failed ({e}); using banner draft");
            banner
        }
    }
}

/// Strict AI generation for the "regenerate draft" action: runs the model and
/// returns an `Err` instead of silently falling back to the banner. This is what
/// keeps a transient AI failure from OVERWRITING an existing draft's body, SEO and
/// tags with the generic banner — `regenerate_draft` propagates the error and
/// leaves the current draft untouched. Requires AI to be enabled.
async fn generate_promo_content_strict(
    lead: &ph_cms::ingest::IngestItem,
    kind: &str,
    section: &str,
) -> Result<ph_cms::ingest::PromotedDraft, String> {
    let lead = localize_lead_image(lead).await;
    let lead = &lead;
    let cfg = ai_config().ok_or_else(|| "AI drafting is not enabled".to_string())?;
    let facts = lead_facts(lead, kind, section);
    let d = ph_ai::draft(&facts, &cfg)
        .await
        .map_err(|e| format!("AI draft failed: {e}"))?;
    Ok(assemble_promo_draft(d, lead))
}

/// Assemble a promoted draft from an AI result + the lead: the provenance banner
/// first, then the model's body paragraphs, the lead's own crawled image as a
/// figure (caption priority: lead attribution → AI caption → generic), and a
/// source line; summary/SEO/tags/slug come from the model.
fn assemble_promo_draft(
    d: ph_ai::AiDraft,
    lead: &ph_cms::ingest::IngestItem,
) -> ph_cms::ingest::PromotedDraft {
    // Source-aware banner (shared with the non-AI banner_draft so wording can't drift):
    // official sources skip the "unverified" framing, press keeps the full caution.
    let mut paras = vec![ph_cms::ingest::lead_banner(&lead.source_key).to_string()];
    paras.extend(d.body_paragraphs);
    let og_image_url = if !lead.image_url.is_empty() {
        let cap = if !lead.image_attribution.is_empty() {
            lead.image_attribution.as_str()
        } else if !d.figure_caption.trim().is_empty() {
            d.figure_caption.trim()
        } else {
            "Source image \u{2014} check it shows the right person and verify usage rights"
        };
        paras.push(format!("![{}]({})", cap, lead.image_url));
        lead.image_url.clone()
    } else if !d.figure_caption.trim().is_empty() {
        // No lead image: leave an empty-URL placeholder for the editor to fill.
        paras.push(format!("![{}](  )", d.figure_caption.trim()));
        String::new()
    } else {
        String::new()
    };
    paras.push(format!("Source ({}): {}", lead.source_key, lead.url));
    let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
    let tags = serde_json::to_string(&d.tags).unwrap_or_else(|_| "[]".to_string());
    ph_cms::ingest::PromotedDraft {
        summary: d.summary,
        body_json,
        meta_description: d.meta_description,
        og_image_url,
        tags,
        slug_base: d.slug,
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
    // Run the poll as a task but AWAIT it (bounded) before returning, so the
    // Intake "poll now" caller gets leads that have actually been written instead
    // of racing a detached task. The bound keeps the request well under
    // Cloudflare's ~100s origin timeout: a fast crawl finishes and the UI reloads
    // fresh leads; a slow one keeps running in the background (its leads appear on
    // the next refresh) rather than surfacing a 524 for a crawl that's succeeding.
    let pool2 = pool.clone();
    let handle = tokio::spawn(async move { ph_crawl::run_once(&pool2, &fetcher).await });
    match tokio::time::timeout(std::time::Duration::from_secs(75), handle).await {
        Ok(Ok(r)) => eprintln!(
            "[ph-press] manual poll: {} leads, {} watch, {} sources, {} errors",
            r.leads_added, r.watch_added, r.sources_polled, r.errors.len()
        ),
        Ok(Err(e)) => eprintln!("[ph-press] manual poll task failed: {e}"),
        Err(_) => eprintln!("[ph-press] manual poll exceeded 75s; still running in the background"),
    }
    Ok(())
}

/// Configured sources for the desk Sources view (key, kind, label, last_polled_at).
pub async fn sources() -> Result<Vec<ph_cms::ingest::IngestSource>, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::ingest::list_sources(pool)
        .await
        .map_err(|e| e.to_string())
}
