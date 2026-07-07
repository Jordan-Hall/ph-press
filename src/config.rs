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

/// True if an env flag reads as on: "1", "true"/"t", "yes"/"y" (first byte, any
/// case). A `const fn` because `&str` can't be matched/compared in `const` context.
const fn env_is_true(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() > 0 && (b[0] == b'1' || b[0] == b't' || b[0] == b'T' || b[0] == b'y' || b[0] == b'Y')
}

/// Compile-time bool flag from an env var: on-values ⇒ true, any other value ⇒
/// false, unset ⇒ `$default`. Const-evaluated like `cfg_str!`.
macro_rules! cfg_bool {
    ($env:literal, $default:literal) => {
        match option_env!($env) {
            Some(v) => env_is_true(v),
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

// ---- Press regulation --------------------------------------------------------
// Whether we may make a *regulated-by* claim. Until registration is confirmed we
// hold ourselves to the Standards Code and say we intend to seek it — but we must
// NOT claim to be regulated by IMPRESS, call IMPRESS "our regulator", or tell
// complainants they can escalate to IMPRESS (IMPRESS only handles complaints about
// its own members). The const below is the *cautious build-time fallback*; the live
// value is the DB `setting` row toggled from the desk (Profile → Press regulation),
// so registration can be switched on without a redeploy.
//
// TWO SURFACES, TWO TIMINGS on the day we register:
//   1. Runtime (desk toggle) — flips every GATED surface for live JS visitors at
//      once: the footer statement, the Standards lede + Transparency prose, the
//      complaints/escalation copy, the removal-request "if you disagree" line, the
//      acknowledgement email, and the desk labels (all read the runtime status).
//   2. Baked SSG HTML (crawlers / no-JS) + the SEO <meta description> on /standards
//      (config-time literal, standards.rs:34) — these DON'T see the runtime toggle.
//      To make them correct too, change the `REGULATOR_REGISTERED` default below to
//      `true` and push: CI (triggered on `src/**`) rebuilds + redeploys. (The
//      `PH_REGULATOR_REGISTERED` build-time override exists for forks/local builds;
//      production does not pass it, so editing the default is the real lever.)
// Leaving the baked default at the cautious value only ever UNDER-claims to
// crawlers, which is safe. Confirm IMPRESS's official statement wording before
// enabling either path.

/// Cautious build-time fallback for whether we are a **registered** member of our
/// independent press regulator. The live value is the DB setting (desk toggle);
/// this is what the SSG pre-render and any missing-provider read fall back to.
pub const REGULATOR_REGISTERED: bool = cfg_bool!("PH_REGULATOR_REGISTERED", false);
/// Our independent press regulator (the one we are registered with / intend to seek).
pub const REGULATOR_NAME: &str = cfg_str!("PH_REGULATOR_NAME", "IMPRESS");
/// The regulator's homepage — used for the "regulated by {name}" identity link.
pub const REGULATOR_HOME_URL: &str = cfg_str!("PH_REGULATOR_HOME_URL", "https://impress.press/");
/// Where a reader escalates an unresolved complaint once we are registered.
pub const REGULATOR_URL: &str =
    cfg_str!("PH_REGULATOR_URL", "https://impress.press/complaints/");
/// Regulator contact details — the required "regulated by" statement must be shown
/// TOGETHER WITH how to contact IMPRESS. Confirm these against IMPRESS's current
/// details when you register (verified July 2026: phone/email below).
pub const REGULATOR_PHONE: &str = cfg_str!("PH_REGULATOR_PHONE", "020 3325 4288");
pub const REGULATOR_EMAIL: &str = cfg_str!("PH_REGULATOR_EMAIL", "complaints@impressreg.org.uk");
