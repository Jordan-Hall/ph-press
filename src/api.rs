//! Server functions: the typed RPC boundary. Each `#[server]` fn is defined for
//! both targets — on the client it becomes an HTTP call, on the server it runs
//! the body. All server-only work (the CMS/SQLite) sits behind
//! `#[cfg(feature = "server")]` so the wasm build never pulls it in.

use dioxus::prelude::*;

/// Count of publicly visible articles in the live CMS database. Confirms the DB
/// is wired up + seeded; also the first thing the editorial console can call.
#[server(endpoint = "cms_status")]
pub async fn cms_status() -> Result<i64, ServerFnError> {
    #[cfg(feature = "server")]
    {
        crate::cms::published_count().await.map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("server only"))
    }
}
