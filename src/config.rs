//! White-label configuration. A fork sets these at BUILD time via environment
//! variables (e.g. `PH_SITE_NAME="Acme Watch" PH_BASE_URL="https://acmewatch.org"
//! dx build --fullstack --ssg --release`); `option_env!` bakes the values into
//! BOTH the wasm bundle and the SSG HTML, so there is no runtime wiring. Defaults
//! are the Predator Hunters identity.
//!
//! This centralises the newsroom's **identity** — name, tagline, base URL and
//! contact addresses — that appear in the chrome, SEO and contact lanes. Editorial
//! prose (the org's story, article copy) lives in `content.rs` and the pages; a
//! fork rewrites that directly. The brand palette lives in `index.html` (`:root`
//! CSS variables) — a fork edits those to recolour.

/// Compile-time env override with a literal default (const-evaluated).
macro_rules! cfg_str {
    ($env:literal, $default:literal) => {
        match option_env!($env) {
            Some(v) => v,
            None => $default,
        }
    };
}

/// Newsroom name — masthead, page titles, Open Graph `site_name`.
pub const SITE_NAME: &str = cfg_str!("PH_SITE_NAME", "Predator Hunters");
/// One-line tagline shown under the masthead brand.
pub const TAGLINE: &str = cfg_str!("PH_TAGLINE", "Independent local journalism");
/// Public base URL, no trailing slash — used to build canonical + OG URLs.
pub const BASE_URL: &str = cfg_str!("PH_BASE_URL", "https://predatorhunters.co.uk");

/// Contact addresses (tips, press, complaints).
pub const TIPS_EMAIL: &str = cfg_str!("PH_TIPS_EMAIL", "tips@predatorhunters.co.uk");
pub const PRESS_EMAIL: &str = cfg_str!("PH_PRESS_EMAIL", "press@predatorhunters.co.uk");
pub const COMPLAINTS_EMAIL: &str =
    cfg_str!("PH_COMPLAINTS_EMAIL", "complaints@predatorhunters.co.uk");
