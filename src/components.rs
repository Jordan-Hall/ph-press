//! Shared chrome: per-page SEO head, a closing call-to-action, and the footer.
//! Framing-disciplined (independent court-reporting journalism; post-conviction;
//! never name before a charge), matching the research site's About voice.

use dioxus::prelude::*;

use crate::app::Route;
use crate::assets::PH_LOGO;
use crate::icons::svg;

use crate::config;

/// Live "are we a registered member of our press regulator?" status, provided by
/// the public `Shell` and fetched once on the client. It seeds from the cautious
/// build-time fallback (`config::REGULATOR_REGISTERED`, false) so the pre-rendered
/// SSG HTML never over-claims; the client updates it from the runtime setting.
#[derive(Clone, Copy)]
pub struct RegulatorStatus(pub Signal<bool>);

/// Read the live regulator-registered status (reactive — re-renders when the
/// client fetch resolves). Falls back to the build-time const when no provider is
/// in scope (e.g. a component rendered outside the public `Shell`).
pub fn regulator_registered() -> bool {
    match try_consume_context::<RegulatorStatus>() {
        Some(RegulatorStatus(sig)) => *sig.read(),
        None => config::REGULATOR_REGISTERED,
    }
}

/// Per-route SEO head: title, description, canonical, Open Graph + Twitter.
#[component]
pub fn Seo(title: String, description: String, path: String, image: String) -> Element {
    let url = format!("{}{path}", config::BASE_URL);
    let img = format!("{}{image}", config::BASE_URL);
    rsx! {
        dioxus::document::Title { "{title}" }
        dioxus::document::Meta { name: "description", content: "{description}" }
        dioxus::document::Link { rel: "canonical", href: "{url}" }
        dioxus::document::Meta { property: "og:site_name", content: config::SITE_NAME }
        dioxus::document::Meta { property: "og:title", content: "{title}" }
        dioxus::document::Meta { property: "og:description", content: "{description}" }
        dioxus::document::Meta { property: "og:url", content: "{url}" }
        dioxus::document::Meta { property: "og:image", content: "{img}" }
        dioxus::document::Meta { property: "og:type", content: "website" }
        dioxus::document::Meta { name: "twitter:card", content: "summary_large_image" }
        dioxus::document::Meta { name: "twitter:title", content: "{title}" }
        dioxus::document::Meta { name: "twitter:description", content: "{description}" }
        dioxus::document::Meta { name: "twitter:image", content: "{img}" }
    }
}

/// Shared closing CTA. Suppressed on Contact (it would repeat that page).
#[component]
pub fn ClosingCta() -> Element {
    let route = use_route::<Route>();
    if route == (Route::Contact {}) {
        return rsx! {};
    }
    rsx! {
        section { class: "section",
            div { class: "wrap",
                div { class: "cta reveal",
                    div { class: "cta-inner",
                        p { class: "eyebrow", style: "margin-bottom:18px;", "Seen something? Want to help?" }
                        h2 {
                            "Got a story? "
                            span { class: "grad-text", "Sources protected." }
                        }
                        p { class: "lede",
                            "We are an independent local newsroom. We keep our sources anonymous and act only on what we can cross-reference, we offer rewards for information on serious crimes, and we report from the public record. If you have a story or information, get in touch."
                        }
                        div { class: "cta-actions",
                            Link { class: "btn btn-primary", to: Route::Contact {},
                                "Get in touch"
                                span { dangerous_inner_html: svg("arrow-right") }
                            }
                            Link { class: "btn btn-ghost", to: Route::News {},
                                span { class: "ic", dangerous_inner_html: svg("doc") }
                                "Read the latest"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SiteFooter() -> Element {
    let site_name = config::SITE_NAME;
    let press_email = config::PRESS_EMAIL;
    rsx! {
        footer { class: "footer",
            div { class: "wrap",
                div { class: "footer-top",
                    div {
                        Link { class: "brand", to: Route::Home {},
                            img { class: "brand-logo", src: PH_LOGO, alt: config::SITE_NAME, width: "500", height: "168" }
                            span { class: "brand-tag", "{site_name}" }
                        }
                        p { class: "footer-blurb",
                            "Independent local journalism. Local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, and a public conviction database. Reporting since 2022."
                        }
                    }
                    div {
                        h4 { "Read" }
                        ul {
                            li { Link { to: Route::News {}, "News" } }
                            li { Link { to: Route::Database {}, "Conviction database" } }
                            li { Link { to: Route::Watch {}, "Watch" } }
                            li { Link { to: Route::Podcast {}, "Podcast" } }
                        }
                    }
                    div {
                        h4 { "Organisation" }
                        ul {
                            li { Link { to: Route::About {}, "About us" } }
                            li { Link { to: Route::Team {}, "Our team" } }
                            li { Link { to: Route::Standards {}, "Standards & complaints" } }
                            li { Link { to: Route::Contact {}, "Contact" } }
                            li { Link { to: Route::Privacy {}, "Privacy" } }
                            li { a { href: "https://research.predatorhunters.co.uk", target: "_blank", rel: "noopener", "Research & AI ↗" } }
                        }
                    }
                    div {
                        h4 { "Follow" }
                        ul {
                            li { a { href: "https://www.facebook.com/Online.Stings", target: "_blank", rel: "noopener", "Facebook ↗" } }
                            li { a { href: "https://www.youtube.com/@JordanHall_dev", target: "_blank", rel: "noopener", "YouTube ↗" } }
                            li { a { href: "https://x.com/PredHunTers", target: "_blank", rel: "noopener", "X · @PredHunTers ↗" } }
                            li { a { href: "mailto:{press_email}", "{press_email}" } }
                        }
                        div { style: "margin-top:18px;",
                            img { class: "brand-logo", src: PH_LOGO, alt: config::SITE_NAME, width: "500", height: "168", style: "height:46px;" }
                        }
                    }
                }
                // Regulatory statement — only shown once we are a registered member of
                // our press regulator (live runtime status; see RegulatorStatus). Until
                // then we make no "regulated by" claim; the honest "intend to seek
                // registration" language lives on the Standards page.
                if regulator_registered() {
                    div { class: "footer-regulated",
                        p {
                            "{site_name} is regulated by "
                            a { href: config::REGULATOR_URL, target: "_blank", rel: "noopener",
                                "{config::REGULATOR_NAME}"
                            }
                            ", the independent monitor for the press. "
                            a { href: "/complaints", "Make a complaint" }
                            " · "
                            a { href: "/corrections", "Corrections & clarifications" }
                        }
                    }
                }
                div { class: "footer-bottom",
                    p { "© 2026 {site_name}. All rights reserved." }
                    p { class: "legal",
                        "Independent journalism. We keep our sources anonymous and act only on what we can cross-reference and verify. We offer rewards for information on serious crimes. We report from the public record, and on concluded court cases we name only after conviction. We work independently of any police force. Complaints: see our Standards page."
                    }
                }
            }
        }
    }
}
