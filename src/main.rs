//! Predator Hunters — the main public site (journalism + court reporting + the
//! public conviction database). All-Rust Dioxus 0.8 with fullstack SSG: every
//! static route + article is pre-rendered to real HTML (`dx build --ssg`), then
//! the wasm hydrates. Crawlers / link-preview bots / no-JS get the full body.
//!
//! EDITORIAL VOICE (load-bearing): independent court-reporting journalism. We
//! run online decoy operations and report INDEPENDENTLY of any police force —
//! never claim we "work alongside" or "hand evidence to" the police (we don't).
//! We never name anyone before they are charged, and we report only on cases
//! concluded in court, from the public record. No public face-recognition.

mod api;
mod app;
mod assets;
#[cfg(feature = "server")]
mod cms;
mod components;
mod config;
mod content;
mod feeds;
mod icons;
mod md;
mod pages;

fn main() {
    let builder = dioxus::LaunchBuilder::new();
    // SERVER build only: enable incremental static generation so the `--ssg`
    // pre-render writes the finished HTML to disk (the client/web build has no
    // `ServeConfig`). Default static dir is `./static`.
    #[cfg(feature = "server")]
    let builder = builder.with_cfg(
        dioxus::server::ServeConfig::new()
            .incremental(dioxus::server::IncrementalRendererConfig::new()),
    );
    builder.launch(app::App);
}
