//! Server functions: the typed RPC boundary. Each `#[server]` fn is defined for
//! both targets — on the client it becomes an HTTP call, on the server it runs
//! the body. All server-only work (the CMS/SQLite + cookie handling) sits behind
//! `#[cfg(feature = "server")]` so the wasm build never pulls it in.
//!
//! Auth model: a successful `staff_login` mints a 256-bit session token, stores
//! only its SHA-256, and returns the raw token in an HttpOnly `ph_session` cookie
//! scoped `Path=/` so it rides along on the `/api/*` server-fn calls. Protected
//! endpoints read that cookie and validate it. Session DTOs below carry only
//! serde-safe types (never ph-cms types) across the boundary.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// tags Vec<String> -> JSON string for storage (server-side).
#[cfg(feature = "server")]
fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

/// stored tags JSON string -> Vec<String> for the DTO (server-side).
#[cfg(feature = "server")]
fn tags_from_json(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

/// Name of the HttpOnly session cookie (used only by the server-side helpers).
#[cfg(feature = "server")]
pub const SESSION_COOKIE: &str = "ph_session";

/// The authenticated staff member, as the `/desk` UI sees them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskSession {
    pub username: String,
    pub display_name: String,
    pub role: String,
}

/// One row of the editorial dashboard (any lifecycle state). `actions` are the
/// transitions THIS user may perform from the row's current state — computed
/// server-side so the UI can never offer an illegal move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskArticle {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub state: String,
    pub kind: String,
    pub byline: String,
    pub updated_at: i64,
    pub is_ai_assisted: bool,
    pub actions: Vec<DeskAction>,
    /// True when the source lead carried an `identification_risk` flag — victim
    /// may be identifiable (IPSO Clauses 7 & 11 / IMPRESS children+justice).
    pub id_risk: bool,
    /// True when the source lead is in a sexual-offence or child category —
    /// automatic anonymity duties apply (IMPRESS; IPSO Clauses 7 & 11).
    pub restrictions_review: bool,
}

/// One allowed lifecycle transition: the target state + a human button label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskAction {
    pub to: String,
    pub label: String,
}

/// A logged reader complaint (the staff inbox view).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskComplaint {
    pub id: i64,
    pub article_slug: String,
    pub complainant: String,
    pub complainant_email: String,
    pub category: String,
    pub body: String,
    pub status: String,
    pub acknowledged_at: Option<i64>,
    pub resolved_at: Option<i64>,
    /// IMPRESS targets: not acknowledged within 7 days / not resolved within 21.
    pub ack_overdue: bool,
    pub decision_overdue: bool,
    pub ts: i64,
}

/// One message in a complaint's handling thread (the desk detail view).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskComplaintMessage {
    pub author: String,
    pub channel: String, // "internal" | "reply"
    pub body: String,
    pub ts: i64,
}

/// A published correction (both versions kept, IMPRESS equal-prominence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskCorrection {
    pub id: i64,
    pub article_id: i64,
    pub original: String,
    pub corrected: String,
    pub reason: String,
    pub ts: i64,
}

/// A public news-list card from the live CMS feed (a story published via /desk).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedItem {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub kind: String,
    pub section: String,
    pub byline: String,
    pub iso_date: String,
}

/// A full public article from the live CMS, for the detail page when the slug is
/// not a compile-time seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicArticle {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub body: Vec<String>,
    pub kind: String,
    pub section: String,
    pub byline: String,
    pub iso_date: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: Vec<String>,
}

/// A staff member as the admin Staff tab sees them (no secrets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaffMember {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub email: String,
}

/// A staff member as the PUBLIC team page sees them — name + role only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamMember {
    pub display_name: String,
    pub role: String,
}

/// One entry in the hash-chained audit trail (for the admin viewer).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditRow {
    pub ts: i64,
    pub actor: String,
    pub action: String,
    pub subject: String,
    pub detail: String,
}

/// The audit trail + whether its hash chain still verifies (tamper-evidence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditLog {
    pub verified: bool,
    pub rows: Vec<AuditRow>,
}

/// An article in ANY state, for an authenticated staff draft preview (carries the
/// lifecycle state so the preview can banner "Draft — not yet published").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewArticle {
    pub title: String,
    pub summary: String,
    pub body: Vec<String>,
    pub kind: String,
    pub section: String,
    pub byline: String,
    pub state: String,
    pub iso_date: String,
    pub slug: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: Vec<String>,
    pub is_ai_assisted: bool,
}

// ---- server-only cookie helpers ---------------------------------------------

/// Attach the session cookie to the current server-fn response. `max_age` of 0
/// expires it (logout). `Secure` is on unless PH_DEV_INSECURE_COOKIE is set, so
/// login is testable over plain http with `dx serve` but hardened in production.
#[cfg(feature = "server")]
fn set_session_cookie(token: &str, max_age: i64) {
    use dioxus::fullstack::http::header::{HeaderValue, SET_COOKIE};
    use dioxus::fullstack::FullstackContext;
    let secure = if std::env::var("PH_DEV_INSECURE_COOKIE").is_ok() {
        ""
    } else {
        "; Secure"
    };
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}{secure}"
    );
    if let Some(ctx) = FullstackContext::current() {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            ctx.add_response_header(SET_COOKIE, value);
        }
    }
}

/// Read the raw session token from the request `Cookie` header. Absent or
/// malformed cookies return None (never an error) so logged-out is a clean state.
#[cfg(feature = "server")]
async fn session_token() -> Option<String> {
    use dioxus::fullstack::http::header::COOKIE;
    use dioxus::fullstack::http::HeaderMap;
    use dioxus::fullstack::FullstackContext;
    let headers: HeaderMap = FullstackContext::extract().await.ok()?;
    let raw = headers.get(COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|kv| {
        kv.trim()
            .strip_prefix(&format!("{SESSION_COOKIE}="))
            .map(str::to_string)
    })
}

/// Human label for a lifecycle action button.
#[cfg(feature = "server")]
fn action_label(from: ph_cms::State, to: ph_cms::State) -> &'static str {
    use ph_cms::State::*;
    match (from, to) {
        (Draft, Submitted) => "Submit",
        (Submitted, EditorialReview) => "Start review",
        (Submitted, Draft) => "Return to draft",
        (EditorialReview, LegalReview) => "Send to legal",
        (EditorialReview, Draft) => "Return to draft",
        (LegalReview, Scheduled) => "Approve + schedule",
        (LegalReview, Published) => "Approve + publish",
        (LegalReview, EditorialReview) => "Back to editorial",
        (Scheduled, Published) => "Publish now",
        (Published, Corrected) => "Mark corrected",
        (Published, Retracted) => "Retract",
        (Corrected, Retracted) => "Retract",
        _ => to.as_str(),
    }
}

/// Build the dashboard rows for a given role, attaching the per-article actions
/// that role may perform (the gate stays authoritative in ph-cms).
#[cfg(feature = "server")]
async fn build_desk(role_str: &str) -> Result<Vec<DeskArticle>, ServerFnError> {
    let role = ph_cms::Role::parse(role_str).map_err(ServerFnError::new)?;
    let arts = crate::cms::all_articles()
        .await
        .map_err(ServerFnError::new)?;
    // Build a lookup: promoted_article_id -> (id_risk, restrictions_review).
    // Fetching all leads in one query is cheaper than N+1 per article.
    let all_leads = crate::cms::leads(None).await.map_err(ServerFnError::new)?;
    let lead_flags: std::collections::HashMap<i64, (bool, bool)> = all_leads
        .into_iter()
        .filter_map(|l| {
            l.promoted_article_id.map(|art_id| {
                let v: serde_json::Value =
                    serde_json::from_str(&l.extracted_json).unwrap_or_default();
                let id_risk = v
                    .get("identification_risk")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                let restrictions_review = v
                    .get("restrictions_review")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                (art_id, (id_risk, restrictions_review))
            })
        })
        .collect();
    Ok(arts
        .into_iter()
        .map(|a| {
            let actions = ph_cms::State::parse(&a.state)
                .ok()
                .map(|st| {
                    ph_cms::allowed_transitions(st, role)
                        .into_iter()
                        .map(|to| DeskAction {
                            label: action_label(st, to).to_string(),
                            to: to.as_str().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (id_risk, restrictions_review) =
                lead_flags.get(&a.id).copied().unwrap_or((false, false));
            DeskArticle {
                id: a.id,
                slug: a.slug,
                title: a.title,
                state: a.state,
                kind: a.kind,
                byline: a.byline,
                updated_at: a.updated_at,
                is_ai_assisted: a.is_ai_assisted,
                actions,
                id_risk,
                restrictions_review,
            }
        })
        .collect())
}

/// Validate the session cookie or return an auth error. Server-only.
#[cfg(feature = "server")]
async fn require_session() -> Result<ph_cms::Session, ServerFnError> {
    let token = session_token()
        .await
        .ok_or_else(|| ServerFnError::new("not authenticated"))?;
    crate::cms::session_for(&token)
        .await
        .map_err(ServerFnError::new)?
        .ok_or_else(|| ServerFnError::new("not authenticated"))
}

/// Require a valid session whose role is admin (for staff management).
#[cfg(feature = "server")]
async fn require_admin() -> Result<ph_cms::Session, ServerFnError> {
    let s = require_session().await?;
    if s.role != "admin" {
        return Err(ServerFnError::new("only an admin may manage staff"));
    }
    Ok(s)
}

#[cfg(feature = "server")]
fn to_staff(v: Vec<ph_cms::StaffUser>) -> Vec<StaffMember> {
    v.into_iter()
        .map(|u| StaffMember {
            username: u.username,
            display_name: u.display_name,
            role: u.role,
            email: u.email.unwrap_or_default(),
        })
        .collect()
}

/// Unix seconds → "YYYY-MM-DD" (civil_from_days; no chrono).
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

// ---- endpoints --------------------------------------------------------------

/// Count of publicly visible articles in the live CMS database. Confirms the DB
/// is wired up + seeded.
#[server(endpoint = "cms_status")]
pub async fn cms_status() -> Result<i64, ServerFnError> {
    #[cfg(feature = "server")]
    {
        crate::cms::published_count()
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Log a staff member in: verify credentials, mint a session, set the cookie, and
/// return the session view for the UI.
#[server(endpoint = "staff_login")]
pub async fn staff_login(username: String, password: String) -> Result<DeskSession, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let token = crate::cms::login(&username, &password)
            .await
            .map_err(ServerFnError::new)?;
        let session = crate::cms::session_for(&token)
            .await
            .map_err(ServerFnError::new)?
            .ok_or_else(|| ServerFnError::new("session could not be established"))?;
        set_session_cookie(&token, ph_cms::SESSION_TTL_SECS);
        Ok(DeskSession {
            username: session.username,
            display_name: session.display_name,
            role: session.role,
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (username, password);
        Err(ServerFnError::new("server only"))
    }
}

/// Does this deployment still need first-run setup (no users yet)? Drives the
/// `/desk` install screen. No auth — this is the pre-account state.
#[server(endpoint = "staff_needs_install")]
pub async fn staff_needs_install() -> Result<bool, ServerFnError> {
    #[cfg(feature = "server")]
    {
        crate::cms::needs_install().await.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(false)
    }
}

/// First-run install: create the first administrator account and sign them in.
/// Valid ONLY while no users exist (enforced server-side); no prior auth.
#[server(endpoint = "staff_install")]
pub async fn staff_install(
    username: String,
    display_name: String,
    email: String,
    password: String,
) -> Result<DeskSession, ServerFnError> {
    #[cfg(feature = "server")]
    {
        if password.chars().count() < 8 {
            return Err(ServerFnError::new("password must be at least 8 characters"));
        }
        let token = crate::cms::install_admin(&username, &display_name, &email, &password)
            .await
            .map_err(ServerFnError::new)?;
        let session = crate::cms::session_for(&token)
            .await
            .map_err(ServerFnError::new)?
            .ok_or_else(|| ServerFnError::new("session could not be established"))?;
        set_session_cookie(&token, ph_cms::SESSION_TTL_SECS);
        Ok(DeskSession {
            username: session.username,
            display_name: session.display_name,
            role: session.role,
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (username, display_name, email, password);
        Err(ServerFnError::new("server only"))
    }
}

/// Who am I? Returns the current session, or None when logged out. Called on
/// `/desk` load to decide between the login form and the dashboard.
#[server(endpoint = "staff_me")]
pub async fn staff_me() -> Result<Option<DeskSession>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let Some(token) = session_token().await else {
            return Ok(None);
        };
        let session = crate::cms::session_for(&token)
            .await
            .map_err(ServerFnError::new)?;
        Ok(session.map(|s| DeskSession {
            username: s.username,
            display_name: s.display_name,
            role: s.role,
        }))
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(None)
    }
}

/// Return the signed-in account's contact email (for the Profile tab).
/// Returns an empty string when no email is set.
#[server(endpoint = "staff_profile")]
pub async fn staff_profile() -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        let email = crate::cms::get_user_email(&session.username)
            .await
            .map_err(ServerFnError::new)?;
        Ok(email.unwrap_or_default())
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Log out: destroy the server-side session and expire the cookie.
#[server(endpoint = "staff_logout")]
pub async fn staff_logout() -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        if let Some(token) = session_token().await {
            crate::cms::logout(&token)
                .await
                .map_err(ServerFnError::new)?;
        }
        set_session_cookie("", 0);
        Ok(())
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(())
    }
}

/// Change the logged-in user's password (verifies the current one server-side).
#[server(endpoint = "staff_change_password")]
pub async fn staff_change_password(current: String, new: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::change_password(&session.username, &current, &new)
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (current, new);
        Err(ServerFnError::new("server only"))
    }
}

/// Per-email cooldown for the forgot-password endpoint (one request / 60s),
/// pruned so the map only holds emails seen within the window. Bounds reset-link
/// churn and, once email is wired up, mailbox flooding.
#[cfg(feature = "server")]
fn forgot_rate_ok(email: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};
    const COOLDOWN_SECS: u64 = 60;
    static LAST: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut map = LAST.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap();
    map.retain(|_, &mut t| now.saturating_sub(t) < COOLDOWN_SECS);
    let key = email.trim().to_lowercase();
    if map.get(&key).is_some_and(|&t| now.saturating_sub(t) < COOLDOWN_SECS) {
        return false;
    }
    map.insert(key, now);
    true
}

/// Begin password recovery. Always succeeds for a well-formed email — whether or
/// not it matches an account — so registered emails can't be probed. The reset
/// link is logged server-side (and, once a provider is configured, emailed). No
/// session required: this is the locked-out path.
#[server(endpoint = "staff_forgot_password")]
pub async fn staff_forgot_password(email: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let email = email.trim().to_string();
        // Quietly no-op on bad input or when throttled — reveal nothing either way.
        if email.is_empty() || !email.contains('@') || !forgot_rate_ok(&email) {
            return Ok(());
        }
        if let Err(e) = crate::cms::request_password_reset(&email).await {
            // Surface DB failures only in the server log; the client still learns
            // nothing (same Ok response as a non-matching email).
            eprintln!("[ph-press] password-reset request error: {e}");
        }
        Ok(())
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = email;
        Ok(())
    }
}

/// Complete password recovery: set a new password from a valid reset token. No
/// session required (the user is locked out). Enforces a minimum length; an
/// invalid/expired/used token yields a generic error. On success the account's
/// other sessions are already destroyed, so the user signs in fresh.
#[server(endpoint = "staff_reset_password")]
pub async fn staff_reset_password(token: String, new: String) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        if new.chars().count() < 8 {
            return Err(ServerFnError::new(
                "password must be at least 8 characters",
            ));
        }
        crate::cms::complete_password_reset(&token, &new)
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, new);
        Err(ServerFnError::new("server only"))
    }
}

/// The editorial dashboard listing — every article in any state, with the
/// actions this user may perform. Requires a valid session.
#[server(endpoint = "desk_articles")]
pub async fn desk_articles() -> Result<Vec<DeskArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        build_desk(&session.role).await
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Apply a lifecycle transition to an article, then return the refreshed list.
/// The role gate (publish only via legal sign-off) is enforced server-side.
#[server(endpoint = "desk_transition")]
pub async fn desk_transition(
    id: i64,
    to: String,
    note: String,
) -> Result<Vec<DeskArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::transition(&session.username, id, &to, &note)
            .await
            .map_err(ServerFnError::new)?;
        build_desk(&session.role).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, to, note);
        Err(ServerFnError::new("server only"))
    }
}

/// Create a new Draft authored by the current user, then return the refreshed list.
#[server(endpoint = "desk_create")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_create(
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
    meta_description: String,
    og_image_url: String,
    tags: Vec<String>,
) -> Result<Vec<DeskArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::create_draft(
            &session.username,
            &session.display_name,
            &title,
            &summary,
            &kind,
            &section,
            &body,
            &meta_description,
            &og_image_url,
            &tags_to_json(&tags),
        )
        .await
        .map_err(ServerFnError::new)?;
        build_desk(&session.role).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (title, summary, kind, section, body, meta_description, og_image_url, tags);
        Err(ServerFnError::new("server only"))
    }
}

// ---- complaints + corrections (IMPRESS) ------------------------------------

#[cfg(feature = "server")]
fn to_complaints(v: Vec<ph_cms::Complaint>) -> Vec<DeskComplaint> {
    const DAY: i64 = 86_400;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    v.into_iter()
        .map(|c| DeskComplaint {
            id: c.id,
            article_slug: c.article_slug,
            complainant: c.complainant,
            complainant_email: c.complainant_email,
            category: c.category,
            body: c.body,
            ack_overdue: c.acknowledged_at.is_none() && now - c.ts > 7 * DAY,
            decision_overdue: c.resolved_at.is_none() && now - c.ts > 21 * DAY,
            status: c.status,
            acknowledged_at: c.acknowledged_at,
            resolved_at: c.resolved_at,
            ts: c.ts,
        })
        .collect()
}

#[cfg(feature = "server")]
fn to_corrections(v: Vec<ph_cms::Correction>) -> Vec<DeskCorrection> {
    v.into_iter()
        .map(|c| DeskCorrection {
            id: c.id,
            article_id: c.article_id,
            original: c.original,
            corrected: c.corrected,
            reason: c.reason,
            ts: c.ts,
        })
        .collect()
}

/// The complaints inbox.
#[server(endpoint = "desk_complaints")]
pub async fn desk_complaints() -> Result<Vec<DeskComplaint>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(to_complaints(
            crate::cms::complaints().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Record a complaint received another way (staff log it on the complainant's
/// behalf), then return the refreshed inbox.
#[server(endpoint = "desk_log_complaint")]
pub async fn desk_log_complaint(
    article_slug: String,
    complainant: String,
    email: String,
    category: String,
    body: String,
) -> Result<Vec<DeskComplaint>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        crate::cms::log_complaint(&article_slug, &complainant, &email, &category, &body)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_complaints(
            crate::cms::complaints().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (article_slug, complainant, email, category, body);
        Err(ServerFnError::new("server only"))
    }
}

/// Advance a complaint's status, then return the refreshed inbox.
#[server(endpoint = "desk_complaint_status")]
pub async fn desk_complaint_status(
    id: i64,
    status: String,
) -> Result<Vec<DeskComplaint>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::set_complaint_status(&session.username, id, &status)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_complaints(
            crate::cms::complaints().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, status);
        Err(ServerFnError::new("server only"))
    }
}

/// Build the (complaint, thread) DTO pair. Server-only helper — callers do auth.
#[cfg(feature = "server")]
async fn complaint_thread_dto(
    id: i64,
) -> Result<(DeskComplaint, Vec<DeskComplaintMessage>), ServerFnError> {
    let (c, thread) = crate::cms::complaint_detail(id)
        .await
        .map_err(ServerFnError::new)?;
    let complaint = to_complaints(vec![c])
        .into_iter()
        .next()
        .ok_or_else(|| ServerFnError::new("complaint not found"))?;
    let messages = thread
        .into_iter()
        .map(|m| DeskComplaintMessage {
            author: m.author,
            channel: m.channel,
            body: m.body,
            ts: m.ts,
        })
        .collect();
    Ok((complaint, messages))
}

/// A complaint + its full handling thread (the desk detail view). Session required.
#[server(endpoint = "desk_complaint_thread")]
pub async fn desk_complaint_thread(
    id: i64,
) -> Result<(DeskComplaint, Vec<DeskComplaintMessage>), ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        complaint_thread_dto(id).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = id;
        Err(ServerFnError::new("server only"))
    }
}

/// Add a staff-only internal note to a complaint; returns the refreshed thread.
#[server(endpoint = "desk_complaint_note")]
pub async fn desk_complaint_note(
    id: i64,
    body: String,
) -> Result<(DeskComplaint, Vec<DeskComplaintMessage>), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::add_complaint_note(&session.username, id, &body)
            .await
            .map_err(ServerFnError::new)?;
        complaint_thread_dto(id).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, body);
        Err(ServerFnError::new("server only"))
    }
}

/// Reply to the complainant (recorded + emailed via SES); returns the refreshed thread.
#[server(endpoint = "desk_complaint_reply")]
pub async fn desk_complaint_reply(
    id: i64,
    body: String,
) -> Result<(DeskComplaint, Vec<DeskComplaintMessage>), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::send_complaint_reply(&session.username, id, &body)
            .await
            .map_err(ServerFnError::new)?;
        complaint_thread_dto(id).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, body);
        Err(ServerFnError::new("server only"))
    }
}

/// The corrections archive.
#[server(endpoint = "desk_corrections")]
pub async fn desk_corrections() -> Result<Vec<DeskCorrection>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(to_corrections(
            crate::cms::corrections()
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Record a correction against an article, then return the refreshed archive.
#[server(endpoint = "desk_add_correction")]
pub async fn desk_add_correction(
    article_id: i64,
    original: String,
    corrected: String,
    reason: String,
) -> Result<Vec<DeskCorrection>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::add_correction(
            &session.username,
            article_id,
            &original,
            &corrected,
            &reason,
        )
        .await
        .map_err(ServerFnError::new)?;
        Ok(to_corrections(
            crate::cms::corrections()
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (article_id, original, corrected, reason);
        Err(ServerFnError::new("server only"))
    }
}

// ---- public CMS feed (the live site reads the DB) --------------------------

/// Publicly visible articles from the live CMS (published/corrected), newest
/// first. The public pages merge these with the compile-time seeds so a story
/// published in /desk shows on the live site. Public — no session required.
#[server(endpoint = "published_feed")]
pub async fn published_feed() -> Result<Vec<FeedItem>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let arts = crate::cms::public_feed()
            .await
            .map_err(ServerFnError::new)?;
        Ok(arts
            .into_iter()
            .map(|a| {
                let iso_date = ymd(a.published_at.unwrap_or(a.updated_at));
                FeedItem {
                    slug: a.slug,
                    title: a.title,
                    summary: a.summary,
                    kind: a.kind,
                    section: a.section,
                    byline: a.byline,
                    iso_date,
                }
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

// ---- staff management (admin) + public team -------------------------------

/// List staff (admin only).
#[server(endpoint = "desk_staff")]
pub async fn desk_staff() -> Result<Vec<StaffMember>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_admin().await?;
        Ok(to_staff(
            crate::cms::list_staff().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Create a staff member at a role (admin only), then return the refreshed list.
#[server(endpoint = "desk_add_staff")]
pub async fn desk_add_staff(
    username: String,
    display_name: String,
    role: String,
    password: String,
    email: String,
) -> Result<Vec<StaffMember>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_admin().await?;
        crate::cms::create_staff(&username, &display_name, &role, &password, &email)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_staff(
            crate::cms::list_staff().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (username, display_name, role, password, email);
        Err(ServerFnError::new("server only"))
    }
}

/// The hash-chained audit trail (admin only), newest first, with the chain
/// integrity flag — the tamper-evident record IMPRESS accountability relies on.
#[server(endpoint = "desk_audit")]
pub async fn desk_audit() -> Result<AuditLog, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_admin().await?;
        let (verified, rows) = crate::cms::audit_log().await.map_err(ServerFnError::new)?;
        let mut rows: Vec<AuditRow> = rows
            .into_iter()
            .map(|(ts, actor, action, subject, detail)| AuditRow {
                ts,
                actor,
                action,
                subject,
                detail,
            })
            .collect();
        rows.reverse(); // newest first
        Ok(AuditLog { verified, rows })
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Submit a complaint from the PUBLIC site (no session) — it lands straight in the
/// /desk Complaints inbox (status "received"), audited. The body is required.
/// Public per-article complaint submission. The complaint is ALWAYS recorded; the
/// acknowledgement email is rate-capped in the glue so the unauthenticated endpoint
/// can't be used to relay mail to arbitrary addresses through our domain. Returns
/// the reference for the confirmation screen. No auth.
#[server(endpoint = "submit_complaint")]
pub async fn submit_complaint(
    article_slug: String,
    complainant: String,
    email: String,
    category: String,
    body: String,
) -> Result<String, ServerFnError> {
    #[cfg(feature = "server")]
    {
        crate::cms::submit_complaint(&article_slug, &complainant, &email, &category, &body)
            .await
            .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (article_slug, complainant, email, category, body);
        Err(ServerFnError::new("server only"))
    }
}

/// The public editorial team — display name + role only, excluding the system
/// bootstrap admin account. Public (no session).
#[server(endpoint = "public_team")]
pub async fn public_team() -> Result<Vec<TeamMember>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let admin_user = std::env::var("PH_ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
        let staff = crate::cms::list_staff().await.map_err(ServerFnError::new)?;
        Ok(staff
            .into_iter()
            .filter(|u| u.username != admin_user)
            .map(|u| TeamMember {
                display_name: u.display_name,
                role: u.role,
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Update an editable article's content (pre-publish only; published changes go
/// through corrections). Requires a valid session.
#[server(endpoint = "desk_update")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_update(
    id: i64,
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
    meta_description: String,
    og_image_url: String,
    tags: Vec<String>,
    slug: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::update_article(
            &session.username,
            id,
            &title,
            &summary,
            &kind,
            &section,
            &body,
            &meta_description,
            &og_image_url,
            &tags_to_json(&tags),
            &slug,
        )
        .await
        .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, title, summary, kind, section, body, meta_description, og_image_url, tags, slug);
        Err(ServerFnError::new("server only"))
    }
}

/// Preview ANY article by id (any state) — authenticated staff only, so an editor
/// can read a draft before it is published.
#[server(endpoint = "desk_preview")]
pub async fn desk_preview(id: i64) -> Result<Option<PreviewArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        let Some(a) = crate::cms::preview_article(id)
            .await
            .map_err(ServerFnError::new)?
        else {
            return Ok(None);
        };
        let body: Vec<String> = serde_json::from_str(&a.body).unwrap_or_default();
        let iso_date = ymd(a.published_at.unwrap_or(a.updated_at));
        Ok(Some(PreviewArticle {
            title: a.title,
            summary: a.summary,
            body,
            kind: a.kind,
            section: a.section,
            byline: a.byline,
            state: a.state,
            iso_date,
            slug: a.slug,
            meta_description: a.meta_description,
            og_image_url: a.og_image_url,
            tags: tags_from_json(&a.tags),
            is_ai_assisted: a.is_ai_assisted,
        }))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = id;
        Ok(None)
    }
}

/// A full public article by slug from the live CMS (published/corrected only),
/// for the detail page when the slug is not a compile-time seed. Public.
#[server(endpoint = "public_article")]
pub async fn public_article(slug: String) -> Result<Option<PublicArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let Some(a) = crate::cms::public_article(&slug)
            .await
            .map_err(ServerFnError::new)?
        else {
            return Ok(None);
        };
        let body: Vec<String> = serde_json::from_str(&a.body).unwrap_or_default();
        let iso_date = ymd(a.published_at.unwrap_or(a.updated_at));
        Ok(Some(PublicArticle {
            slug: a.slug,
            title: a.title,
            summary: a.summary,
            body,
            kind: a.kind,
            section: a.section,
            byline: a.byline,
            iso_date,
            meta_description: a.meta_description,
            og_image_url: a.og_image_url,
            tags: tags_from_json(&a.tags),
        }))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = slug;
        Ok(None)
    }
}

// ---- crawler intake + conviction database + court-watch --------------------

/// A crawled LEAD as the Intake desk sees it. Everything here is UNVERIFIED
/// machine output; the editor turns a lead into our own report via the lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskLead {
    pub id: i64,
    pub source_key: String,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub offence_category: String,
    pub image_url: String,
    pub image_attribution: String,
    pub status: String,
    pub promoted_article_id: Option<i64>,
    pub created_at: i64,
    /// Machine hint that a victim may be identifiable (a stronger reporting-
    /// restriction prompt for the reviewer). Parsed from `extracted_json`.
    pub id_risk: bool,
}

/// A conviction-database entry as the desk sees it (any status).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskConviction {
    pub id: i64,
    pub name: String,
    pub area: String,
    pub offence: String,
    pub outcome: String,
    pub date: String,
    pub iso_date: String,
    pub lat: f64,
    pub lng: f64,
    pub article_id: Option<i64>,
    pub article_slug: String,
    pub source_url: String,
    pub source_name: String,
    pub status: String,
}

/// A private court-watch entry (never public).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskWatch {
    pub id: i64,
    pub court: String,
    pub case_ref: String,
    pub hearing_date: String,
    pub hearing_type: String,
    pub offence_category: String,
    pub source_url: String,
    pub notes: String,
    pub status: String,
}

/// A published conviction as the PUBLIC `/database` page sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicConviction {
    pub name: String,
    pub area: String,
    pub offence: String,
    pub outcome: String,
    pub date: String,
    pub iso_date: String,
    pub article_slug: String,
    pub source_url: String,
    pub source_name: String,
    pub lat: f64,
    pub lng: f64,
}

#[cfg(feature = "server")]
fn to_leads(v: Vec<ph_cms::ingest::IngestItem>) -> Vec<DeskLead> {
    v.into_iter()
        .map(|l| {
            let id_risk = serde_json::from_str::<serde_json::Value>(&l.extracted_json)
                .ok()
                .and_then(|v| v.get("identification_risk").and_then(|b| b.as_bool()))
                .unwrap_or(false);
            DeskLead {
                id: l.id,
                source_key: l.source_key,
                url: l.url,
                title: l.title,
                snippet: l.snippet,
                offence_category: l.offence_category,
                image_url: l.image_url,
                image_attribution: l.image_attribution,
                status: l.status,
                promoted_article_id: l.promoted_article_id,
                created_at: l.created_at,
                id_risk,
            }
        })
        .collect()
}

#[cfg(feature = "server")]
fn to_convictions(v: Vec<ph_cms::ingest::Conviction>) -> Vec<DeskConviction> {
    v.into_iter()
        .map(|c| DeskConviction {
            id: c.id,
            name: c.name,
            area: c.area,
            offence: c.offence,
            outcome: c.outcome,
            date: c.date,
            iso_date: c.iso_date,
            lat: c.lat,
            lng: c.lng,
            article_id: c.article_id,
            article_slug: c.article_slug,
            source_url: c.source_url,
            source_name: c.source_name,
            status: c.status,
        })
        .collect()
}

#[cfg(feature = "server")]
fn to_watch(v: Vec<ph_cms::courtwatch::CourtWatch>) -> Vec<DeskWatch> {
    v.into_iter()
        .map(|w| DeskWatch {
            id: w.id,
            court: w.court,
            case_ref: w.case_ref,
            hearing_date: w.hearing_date,
            hearing_type: w.hearing_type,
            offence_category: w.offence_category,
            source_url: w.source_url,
            notes: w.notes,
            status: w.status,
        })
        .collect()
}

/// Every crawled lead (any status), newest first — the Intake desk.
#[server(endpoint = "desk_leads")]
pub async fn desk_leads() -> Result<Vec<DeskLead>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(to_leads(
            crate::cms::leads(None).await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Promote a lead into a Draft article (legal-gated lifecycle), then refresh.
#[server(endpoint = "desk_promote_lead")]
pub async fn desk_promote_lead(
    id: i64,
    kind: String,
    section: String,
) -> Result<Vec<DeskLead>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::promote_lead(&session.username, id, &kind, &section)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_leads(
            crate::cms::leads(None).await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, kind, section);
        Err(ServerFnError::new("server only"))
    }
}

/// Dismiss a lead (not relevant / can't verify), then refresh.
#[server(endpoint = "desk_dismiss_lead")]
pub async fn desk_dismiss_lead(id: i64) -> Result<Vec<DeskLead>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::set_lead_status(&session.username, id, "dismissed")
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_leads(
            crate::cms::leads(None).await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = id;
        Err(ServerFnError::new("server only"))
    }
}

/// Every conviction-database entry (any status), newest first.
#[server(endpoint = "desk_convictions")]
pub async fn desk_convictions() -> Result<Vec<DeskConviction>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(to_convictions(
            crate::cms::convictions(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Create a draft conviction entry, then refresh the list.
#[server(endpoint = "desk_create_conviction")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_create_conviction(
    name: String,
    area: String,
    offence: String,
    outcome: String,
    date: String,
    iso_date: String,
    lat: f64,
    lng: f64,
    article_id: Option<i64>,
    article_slug: String,
    source_url: String,
    source_name: String,
) -> Result<Vec<DeskConviction>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::create_conviction(
            &session.username,
            &name,
            &area,
            &offence,
            &outcome,
            &date,
            &iso_date,
            lat,
            lng,
            article_id,
            &article_slug,
            &source_url,
            &source_name,
        )
        .await
        .map_err(ServerFnError::new)?;
        Ok(to_convictions(
            crate::cms::convictions(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (
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
        Err(ServerFnError::new("server only"))
    }
}

/// Edit a draft conviction entry, then refresh the list.
#[server(endpoint = "desk_update_conviction")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_update_conviction(
    id: i64,
    name: String,
    area: String,
    offence: String,
    outcome: String,
    date: String,
    iso_date: String,
    lat: f64,
    lng: f64,
    article_id: Option<i64>,
    article_slug: String,
    source_url: String,
    source_name: String,
) -> Result<Vec<DeskConviction>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::update_conviction(
            &session.username,
            id,
            &name,
            &area,
            &offence,
            &outcome,
            &date,
            &iso_date,
            lat,
            lng,
            article_id,
            &article_slug,
            &source_url,
            &source_name,
        )
        .await
        .map_err(ServerFnError::new)?;
        Ok(to_convictions(
            crate::cms::convictions(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (
            id,
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
        Err(ServerFnError::new("server only"))
    }
}

/// Publish or retract a conviction (publish requires a linked, published report).
#[server(endpoint = "desk_set_conviction_status")]
pub async fn desk_set_conviction_status(
    id: i64,
    status: String,
) -> Result<Vec<DeskConviction>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::set_conviction_status(&session.username, id, &status)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_convictions(
            crate::cms::convictions(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, status);
        Err(ServerFnError::new("server only"))
    }
}

/// The private court-watch list (soonest first). Requires a session.
#[server(endpoint = "desk_courtwatch")]
pub async fn desk_courtwatch() -> Result<Vec<DeskWatch>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(to_watch(
            crate::cms::court_watch(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Add a court-watch entry by hand, then refresh.
#[server(endpoint = "desk_add_watch")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_add_watch(
    court: String,
    case_ref: String,
    hearing_date: String,
    hearing_type: String,
    offence_category: String,
    source_url: String,
    notes: String,
) -> Result<Vec<DeskWatch>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::add_watch(
            &session.username,
            &court,
            &case_ref,
            &hearing_date,
            &hearing_type,
            &offence_category,
            &source_url,
            &notes,
        )
        .await
        .map_err(ServerFnError::new)?;
        Ok(to_watch(
            crate::cms::court_watch(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (
            court,
            case_ref,
            hearing_date,
            hearing_type,
            offence_category,
            source_url,
            notes,
        );
        Err(ServerFnError::new("server only"))
    }
}

/// Update a court-watch entry's status (+ optional note), then refresh.
#[server(endpoint = "desk_courtwatch_update")]
pub async fn desk_courtwatch_update(
    id: i64,
    status: String,
    note: String,
) -> Result<Vec<DeskWatch>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::set_watch_status(&session.username, id, &status, &note)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_watch(
            crate::cms::court_watch(None)
                .await
                .map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, status, note);
        Err(ServerFnError::new("server only"))
    }
}

/// Published conviction-database entries — the PUBLIC `/database` read. No
/// session required.
#[server(endpoint = "conviction_db")]
pub async fn conviction_db() -> Result<Vec<PublicConviction>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let rows = crate::cms::published_convictions()
            .await
            .map_err(ServerFnError::new)?;
        Ok(rows
            .into_iter()
            .map(|c| PublicConviction {
                name: c.name,
                area: c.area,
                offence: c.offence,
                outcome: c.outcome,
                date: c.date,
                iso_date: c.iso_date,
                article_slug: c.article_slug,
                source_url: c.source_url,
                source_name: c.source_name,
                lat: c.lat,
                lng: c.lng,
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Ok(Vec::new())
    }
}

/// A configured crawl source as the desk Sources view sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskSource {
    pub key: String,
    pub kind: String,
    pub label: String,
    pub url: String,
    pub enabled: bool,
    pub last_polled_at: Option<i64>,
}

/// Promote a lead into a draft article AND a linked draft conviction, then refresh.
#[server(endpoint = "desk_promote_lead_conviction")]
pub async fn desk_promote_lead_conviction(
    id: i64,
    kind: String,
    section: String,
) -> Result<Vec<DeskLead>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::promote_lead_to_conviction(&session.username, id, &kind, &section)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_leads(
            crate::cms::leads(None).await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, kind, section);
        Err(ServerFnError::new("server only"))
    }
}

/// The configured crawl sources + their last-poll times. Requires a session.
#[server(endpoint = "desk_sources")]
pub async fn desk_sources() -> Result<Vec<DeskSource>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        Ok(crate::cms::sources()
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(|s| DeskSource {
                key: s.key,
                kind: s.kind,
                label: s.label,
                url: s.url,
                enabled: s.enabled,
                last_polled_at: s.last_polled_at,
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}

/// Re-run AI generation on a draft promoted from a lead, overwriting its body + SEO.
/// Only works while the draft is still in `draft` state and AI is enabled.
#[server(endpoint = "desk_regenerate_draft")]
pub async fn desk_regenerate_draft(article_id: i64) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::regenerate_draft(&session.username, article_id).await.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    { let _ = article_id; Err(ServerFnError::new("server only")) }
}


/// Trigger one crawl pass now (background). Admin only.
#[server(endpoint = "desk_poll_now")]
pub async fn desk_poll_now() -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_admin().await?;
        crate::cms::crawl_now().await.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}
