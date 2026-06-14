//! Shared chrome: per-page SEO head, a closing call-to-action, and the footer.
//! Framing-disciplined (independent court-reporting journalism; post-conviction;
//! never name before a charge), matching the research site's About voice.

use dioxus::prelude::*;

use crate::app::Route;
use crate::assets::PH_LOGO;
use crate::icons::svg;

const BASE: &str = "https://predatorhunters.co.uk";

/// Per-route SEO head: title, description, canonical, Open Graph + Twitter.
#[component]
pub fn Seo(title: String, description: String, path: String, image: String) -> Element {
    let url = format!("{BASE}{path}");
    let img = format!("{BASE}{image}");
    rsx! {
        dioxus::document::Title { "{title}" }
        dioxus::document::Meta { name: "description", content: "{description}" }
        dioxus::document::Link { rel: "canonical", href: "{url}" }
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
    rsx! {
        footer { class: "footer",
            div { class: "wrap",
                div { class: "footer-top",
                    div {
                        Link { class: "brand", to: Route::Home {},
                            img { class: "brand-logo", src: PH_LOGO, alt: "Predator Hunters", width: "500", height: "168" }
                            span { class: "brand-tag", "Predator Hunters" }
                        }
                        p { class: "footer-blurb",
                            "Independent local journalism. Local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, and a public conviction database. Reporting since 2022."
                        }
                    }
                    div {
                        h4 { "Read" }
                        ul {
                            li { Link { to: Route::News {}, "News" } }
                            li { Link { to: Route::Watch {}, "Watch" } }
                            li { Link { to: Route::Podcast {}, "Podcast" } }
                        }
                    }
                    div {
                        h4 { "Organisation" }
                        ul {
                            li { Link { to: Route::About {}, "About us" } }
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
                            li { a { href: "mailto:press@predatorhunters.co.uk", "press@predatorhunters.co.uk" } }
                        }
                        div { style: "margin-top:18px;",
                            img { class: "brand-logo", src: PH_LOGO, alt: "Predator Hunters", width: "500", height: "168", style: "height:46px;" }
                        }
                    }
                }
                div { class: "footer-bottom",
                    p { "© 2026 Predator Hunters. All rights reserved." }
                    p { class: "legal",
                        "Independent journalism. We keep our sources anonymous and act only on what we can cross-reference and verify. We offer rewards for information on serious crimes. We report from the public record, and on concluded court cases we name only after conviction. We work independently of any police force. Complaints: see our Standards page."
                    }
                }
            }
        }
    }
}
