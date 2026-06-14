//! Home — the newsroom front page. A lead story, a secondary story grid, a
//! "Latest" rail, then what the newsroom does. Headline-led, not a marketing
//! hero. Positioning: independent LOCAL news + investigations + court reporting
//! + reward appeals + source protection + a public conviction database.

use dioxus::prelude::*;

use crate::app::Route;
use crate::content::ARTICLES;
use crate::icons::svg;

/// (icon, title, description) — what the newsroom does.
const STRANDS: [(&str, &str, &str); 4] = [
    (
        "doc",
        "Local news",
        "We break local stories that matter to the communities we cover, not only the headline crimes.",
    ),
    (
        "scale",
        "Court reporting",
        "We report concluded cases from the public court record, and name people only after a conviction.",
    ),
    (
        "shield",
        "Reward appeals",
        "We offer rewards for information that helps catch killers, rapists and abusers, and bring them to court.",
    ),
    (
        "lock",
        "Protected sources",
        "We keep our sources anonymous and act only on what we can cross-reference and verify.",
    ),
];

#[component]
pub fn Home() -> Element {
    let lead = &ARTICLES[0];
    let rest = &ARTICLES[1..];
    rsx! {
        crate::components::Seo {
            title: "Predator Hunters — independent local news, investigations & court reporting",
            description: "An independent local newsroom: local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, protected sources, and a public conviction database.",
            path: "/",
            image: "/og.png",
        }

        // ---------- FRONT PAGE ----------
        section { style: "padding:clamp(20px,3vh,40px) 0;",
            div { class: "wrap front",
                // main column
                div {
                    Link { class: "lead", to: Route::Article { slug: lead.slug.to_string() },
                        if lead.image.is_some() {
                            img { class: "media lead-media", src: lead.image.unwrap_or(""), alt: "{lead.title}", loading: "lazy" }
                        }
                        span { class: "kicker", "{lead.kind}" }
                        h1 { class: "hl", "{lead.title}" }
                        p { class: "standfirst", "{lead.summary}" }
                        div { class: "byline",
                            span { "By {lead.byline}" }
                            span { class: "sep", "·" }
                            span { "{lead.date}" }
                        }
                    }
                    div { class: "front-grid",
                        for a in rest {
                            Link { key: "{a.slug}", class: "story", to: Route::Article { slug: a.slug.to_string() },
                                if a.image.is_some() {
                                    img { class: "media story-media", src: a.image.unwrap_or(""), alt: "{a.title}", loading: "lazy" }
                                }
                                span { class: "kicker", "{a.kind}" }
                                h3 { class: "hl", "{a.title}" }
                                p { "{a.summary}" }
                                div { class: "byline", "By {a.byline} · {a.date}" }
                            }
                        }
                    }
                }
                // rail
                aside {
                    div { class: "rail",
                        ul { class: "rail-list",
                            for (i , a) in ARTICLES.iter().enumerate() {
                                li { key: "{a.slug}", class: "rail-item",
                                    span { class: "n", "{i + 1}" }
                                    Link { to: Route::Article { slug: a.slug.to_string() },
                                        h3 { class: "hl", "{a.title}" }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "rail",
                        div { class: "rail-note",
                            h4 { "The newsroom" }
                            p { "Independent local journalism since 2022. We protect our sources, report from the public record, and never name anyone before a charge." }
                            div { style: "margin-top:14px;",
                                Link { class: "btn btn-ghost btn-sm", to: Route::About {}, "About us" }
                            }
                        }
                    }
                }
            }
        }

        // ---------- WHAT WE DO ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "section-label", span { class: "sec-index", "What we do" } }
                div { class: "grid-4",
                    for (icon , title , desc) in STRANDS {
                        div { key: "{title}", class: "card reveal",
                            div { class: "card-ic", dangerous_inner_html: svg(icon) }
                            h3 { "{title}" }
                            p { "{desc}" }
                        }
                    }
                }
            }
        }

        // ---------- WATCH + LISTEN ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "section-label", span { class: "sec-index", "Watch + listen" } }
                div { class: "grid-2",
                    Link { class: "card reveal", to: Route::Watch {},
                        div { class: "card-ic", dangerous_inner_html: svg("camera") }
                        h3 { "Watch" }
                        p { "Investigations and court reports on video." }
                    }
                    Link { class: "card reveal", to: Route::Podcast {},
                        div { class: "card-ic", dangerous_inner_html: svg("waveform") }
                        h3 { "The podcast" }
                        p { "The stories behind the cases, in conversation." }
                    }
                }
            }
        }
    }
}
