//! News — the newsroom index. Lists published articles (from content::ARTICLES)
//! as cards linking to /news/:slug. Real CMS-backed data lands in WS2.

use dioxus::prelude::*;

use crate::app::Route;
use crate::content::ARTICLES;
use crate::icons::svg;

#[component]
pub fn News() -> Element {
    rsx! {
        crate::components::Seo {
            title: "News | Predator Hunters",
            description: "Court reporting, investigations and explainers from Predator Hunters. We report on cases once they have been to court, from the public record.",
            path: "/news",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Newsroom" }
                h1 { class: "rise d2",
                    "Reporting, "
                    span { class: "grad-text", "from the public record." }
                }
                p { class: "lede rise d3",
                    "Court reports, investigations and explainers. Every story is checked against the court record, and as a rule we do not name anyone before they are charged."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "research-list",
                    for a in ARTICLES.iter() {
                        Link {
                            key: "{a.slug}",
                            class: if a.image.is_some() { "r-row has-img reveal" } else { "r-row reveal" },
                            to: Route::Article { slug: a.slug.to_string() },
                            if a.image.is_some() {
                                img { class: "r-thumb", src: a.image.unwrap_or(""), alt: "{a.title}", loading: "lazy" }
                            }
                            div {
                                span { class: "r-num", "{a.kind}" }
                                h3 { class: "hl", "{a.title}" }
                                p { class: "r-desc", "{a.summary}" }
                            }
                            div { class: "r-meta",
                                span { class: "byline", "{a.date}" }
                                span { class: "r-arrow", dangerous_inner_html: svg("arrow-up-right") }
                            }
                        }
                    }
                }
                p { class: "prose", style: "margin-top:28px; color:var(--muted); font-size:.9rem;",
                    "Court reports of concluded cases publish here as they clear editorial and legal review."
                }
            }
        }
    }
}
