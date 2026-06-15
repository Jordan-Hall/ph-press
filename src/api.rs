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

/// Name of the HttpOnly session cookie.
pub const SESSION_COOKIE: &str = "ph_session";

/// The authenticated staff member, as the `/desk` UI sees them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeskSession {
    pub username: String,
    pub display_name: String,
    pub role: String,
}

/// One row of the editorial dashboard (any lifecycle state).
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

/// The editorial dashboard listing — every article in any state. Requires a valid
/// session; returns an auth error otherwise.
#[server(endpoint = "desk_articles")]
pub async fn desk_articles() -> Result<Vec<DeskArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let token = session_token()
            .await
            .ok_or_else(|| ServerFnError::new("not authenticated"))?;
        crate::cms::session_for(&token)
            .await
            .map_err(ServerFnError::new)?
            .ok_or_else(|| ServerFnError::new("not authenticated"))?;
        let arts = crate::cms::all_articles()
            .await
            .map_err(ServerFnError::new)?;
        Ok(arts
            .into_iter()
            .map(|a| DeskArticle {
                id: a.id,
                slug: a.slug,
                title: a.title,
                state: a.state,
                kind: a.kind,
                byline: a.byline,
                updated_at: a.updated_at,
                is_ai_assisted: a.is_ai_assisted,
            })
            .collect())
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}
