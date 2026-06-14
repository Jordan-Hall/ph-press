//! News — the newsroom index. Article list lands with the editorial CMS (WS2);
//! this is the framed shell + the standards link.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

#[component]
pub fn News() -> Element {
    rsx! {
        crate::components::Seo {
            title: "News | Predator Hunters",
            description: "Court reporting and investigations from Predator Hunters. We report on cases once they have been to court, from the public record.",
            path: "/news",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Newsroom" }
                h1 { class: "rise d2",
                    "Court reporting, "
                    span { class: "grad-text", "from the public record." }
                }
                p { class: "lede rise d3",
                    "We report on cases once they have concluded in court. Every story is checked against the court record, and nothing names anyone before they are charged."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("doc") }
                    h3 { "The newsroom is launching" }
                    p { "Our first reports are in editorial review and will publish here shortly, each one fact-checked against the court record and signed off by an editor before it goes live." }
                    div { style: "margin-top:14px; display:flex; gap:12px; flex-wrap:wrap;",
                        Link { class: "btn btn-ghost btn-sm", to: Route::Cases {}, "Browse cases" }
                        Link { class: "btn btn-ghost btn-sm", to: Route::Standards {}, "Our standards" }
                    }
                }
            }
        }
    }
}
