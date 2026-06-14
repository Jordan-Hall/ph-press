//! Podcast — the stories behind the cases. Player/feed embed lands in WS5 once
//! the podcast host/feed is wired; framed shell + subscribe links here.

use dioxus::prelude::*;

use crate::icons::svg;

#[component]
pub fn Podcast() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Podcast | Predator Hunters",
            description: "The Predator Hunters podcast: the stories behind the cases, in conversation.",
            path: "/podcast",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Podcast" }
                h1 { class: "rise d2",
                    "The stories behind "
                    span { class: "grad-text", "the cases." }
                }
                p { class: "lede rise d3",
                    "Conversations about the work, the cases, and keeping children safe online. New episodes will stream right here."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("waveform") }
                    h3 { "Streaming here soon" }
                    p { "We are setting up the podcast player so you can listen to every episode on this page, and subscribe wherever you get your podcasts. Watch this space." }
                }
            }
        }
    }
}
