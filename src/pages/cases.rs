//! Cases — the post-conviction case archive. Entries land with the CMS/database
//! (WS2/WS3); framed shell here.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

#[component]
pub fn Cases() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Cases | Predator Hunters",
            description: "An archive of concluded court cases we have reported, drawn from the public record, post-conviction only.",
            path: "/cases",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Case archive" }
                h1 { class: "rise d2",
                    "Cases that have been "
                    span { class: "grad-text", "through the courts." }
                }
                p { class: "lede rise d3",
                    "A growing archive of concluded cases, each drawn from the public court record. The searchable conviction database sits alongside it."
                }
                div { class: "hero-actions rise d4", style: "margin-top:28px;",
                    Link { class: "btn btn-primary", to: Route::Database {},
                        "Search the database"
                        span { dangerous_inner_html: svg("arrow-right") }
                    }
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("scale") }
                    h3 { "The archive is being compiled" }
                    p { "We are adding concluded cases one at a time, each checked against the court record and run through editorial review. They appear here and in the conviction database as they are verified." }
                }
            }
        }
    }
}
