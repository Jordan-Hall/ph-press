//! Watch — video investigations + court reports. A gallery of the reports that
//! carry video (privacy-respecting youtube-nocookie), each linking to the full
//! story, plus the channel + Facebook.

use dioxus::prelude::*;

use crate::app::Route;
use crate::content::ARTICLES;
use crate::icons::svg;

const YT: &str = "https://www.youtube.com/@JordanHall_dev";
const FB: &str = "https://www.facebook.com/Online.Stings";

#[component]
pub fn Watch() -> Element {
    let videos: Vec<&'static crate::content::Article> =
        ARTICLES.iter().filter(|a| a.youtube.is_some()).collect();
    rsx! {
        crate::components::Seo {
            title: "Watch | Predator Hunters",
            description: "Video investigations and court reports from Predator Hunters. Footage is published only after a conviction, and censored where needed.",
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
                    "Investigations and court reports on video. We publish footage only after a conviction, and censor it where it is needed."
                }
                div { class: "hero-actions rise d4", style: "margin-top:24px; display:flex; gap:12px; flex-wrap:wrap;",
                    a { class: "btn btn-primary", href: "{YT}", target: "_blank", rel: "noopener",
                        span { dangerous_inner_html: svg("camera") }
                        "YouTube channel"
                    }
                    a { class: "btn btn-ghost", href: "{FB}", target: "_blank", rel: "noopener",
                        span { class: "ic", dangerous_inner_html: svg("globe") }
                        "Facebook"
                    }
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(16px,3vh,40px);",
            div { class: "wrap",
                div { class: "cards",
                    for a in videos.iter() {
                        div { key: "{a.slug}", class: "ncard", style: "padding:0;",
                            iframe {
                                src: "https://www.youtube-nocookie.com/embed/{a.youtube.unwrap_or(\"\")}",
                                title: "{a.title}",
                                style: "width:100%; aspect-ratio:16/9; height:auto; border:0; display:block; background:#000;",
                                "loading": "lazy",
                                "referrerpolicy": "strict-origin-when-cross-origin",
                                "allow": "accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share",
                                "allowfullscreen": "true",
                            }
                            div { class: "ncard-body",
                                span { class: "kicker", "{a.kind}" }
                                h3 { class: "hl", "{a.title}" }
                                Link { class: "byline", to: Route::Article { slug: a.slug.to_string() }, style: "color:var(--red);", "Read the full report" }
                            }
                        }
                    }
                }
            }
        }
    }
}
