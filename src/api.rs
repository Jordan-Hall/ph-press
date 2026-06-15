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
    pub body: String,
    pub status: String,
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
}

/// A staff member as the admin Staff tab sees them (no secrets).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaffMember {
    pub username: String,
    pub display_name: String,
    pub role: String,
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
pub async fn desk_create(
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
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
        )
        .await
        .map_err(ServerFnError::new)?;
        build_desk(&session.role).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (title, summary, kind, section, body);
        Err(ServerFnError::new("server only"))
    }
}

// ---- complaints + corrections (IMPRESS) ------------------------------------

#[cfg(feature = "server")]
fn to_complaints(v: Vec<ph_cms::Complaint>) -> Vec<DeskComplaint> {
    v.into_iter()
        .map(|c| DeskComplaint {
            id: c.id,
            article_slug: c.article_slug,
            complainant: c.complainant,
            body: c.body,
            status: c.status,
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

/// Record a complaint received by any means, then return the refreshed inbox.
#[server(endpoint = "desk_log_complaint")]
pub async fn desk_log_complaint(
    article_slug: String,
    complainant: String,
    body: String,
) -> Result<Vec<DeskComplaint>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_session().await?;
        crate::cms::log_complaint(&article_slug, &complainant, &body)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_complaints(
            crate::cms::complaints().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (article_slug, complainant, body);
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
) -> Result<Vec<StaffMember>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        require_admin().await?;
        crate::cms::create_staff(&username, &display_name, &role, &password)
            .await
            .map_err(ServerFnError::new)?;
        Ok(to_staff(
            crate::cms::list_staff().await.map_err(ServerFnError::new)?,
        ))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (username, display_name, role, password);
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
pub async fn desk_update(
    id: i64,
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
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
        )
        .await
        .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, title, summary, kind, section, body);
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
        }))
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = slug;
        Ok(None)
    }
}
