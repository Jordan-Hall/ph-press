//! Podcast — Predator Hunters Talks. Episodes embedded (privacy-respecting
//! youtube-nocookie) + subscribe links.

use dioxus::prelude::*;

use crate::icons::svg;

const YT: &str = "https://www.youtube.com/@JordanHall_dev";

/// (youtube id, title) — newest first.
const EPISODES: [(&str, &str); 2] = [
    ("q0q2BUEOyQc", "Predator Hunters Talks"),
    ("RkeNqovwoaA", "Predator Hunters Talks"),
];

#[component]
pub fn Podcast() -> Element {
    let total = EPISODES.len();
    rsx! {
        crate::components::Seo {
            title: "Podcast | Predator Hunters",
            description: "Predator Hunters Talks: the stories behind the cases, the courts and the frontline work, in conversation.",
            path: "/podcast",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Podcast" }
                h1 { class: "rise d2",
                    "Predator Hunters "
                    span { class: "grad-text", "Talks." }
                }
                p { class: "lede rise d3",
                    "The stories behind the cases, the courts and the frontline work, in conversation. New episodes land here."
                }
                div { class: "hero-actions rise d4", style: "margin-top:24px; display:flex; gap:12px; flex-wrap:wrap;",
                    a { class: "btn btn-primary", href: "{YT}", target: "_blank", rel: "noopener",
                        span { dangerous_inner_html: svg("waveform") }
                        "Subscribe on YouTube"
                    }
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(16px,3vh,40px);",
            div { class: "wrap",
                div { class: "grid-2",
                    for (i , (id , title)) in EPISODES.iter().enumerate() {
                        div { key: "{id}", class: "card reveal", style: "padding:0; overflow:hidden;",
                            iframe {
                                src: "https://www.youtube-nocookie.com/embed/{id}",
                                title: "{title}",
                                style: "width:100%; aspect-ratio:16/9; height:auto; border:0; display:block; background:#000;",
                                "loading": "lazy",
                                "referrerpolicy": "strict-origin-when-cross-origin",
                                "allow": "accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share",
                                "allowfullscreen": "true",
                            }
                            div { style: "padding:20px 22px 24px;",
                                p { class: "kicker", "Episode {total - i}" }
                                h3 { style: "margin-top:8px;", "{title}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
