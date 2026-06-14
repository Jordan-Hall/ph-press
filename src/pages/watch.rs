//! Watch — video investigations + court reports. Real embeds (privacy-respecting
//! youtube-nocookie) land in WS5 once specific video IDs are wired + the CSP is
//! set; for now a prominent channel link + what-you-will-find.

use dioxus::prelude::*;

use crate::icons::svg;

const YT: &str = "https://www.youtube.com/@JordanHall_dev";
const FB: &str = "https://www.facebook.com/Online.Stings";

#[component]
pub fn Watch() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Watch | Predator Hunters",
            description: "Video investigations and court reports from Predator Hunters, on YouTube and Facebook.",
            path: "/watch",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Watch" }
                h1 { class: "rise d2",
                    "See the work "
                    span { class: "grad-text", "for yourself." }
                }
                p { class: "lede rise d3",
                    "Investigations and court reports on video. Footage is only ever published after a conviction, and censored where it is needed."
                }
                div { class: "hero-actions rise d4", style: "margin-top:28px;",
                    a { class: "btn btn-primary", href: "{YT}", target: "_blank", rel: "noopener",
                        span { dangerous_inner_html: svg("camera") }
                        "YouTube channel"
                    }
                    a { class: "btn btn-ghost", href: "{FB}", target: "_blank", rel: "noopener",
                        span { class: "ic", dangerous_inner_html: svg("facebook") }
                        "Facebook"
                    }
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("camera") }
                    h3 { "Latest videos, embedded soon" }
                    p { "We are wiring privacy-respecting embeds of our latest videos straight into this page. In the meantime, the full archive is on our YouTube channel and Facebook." }
                }
            }
        }
    }
}
