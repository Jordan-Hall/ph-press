//! Home — the newsroom front page. DATA-DRIVEN: the lead is the newest article and
//! the body is built from the section taxonomy (content::SECTIONS), so as articles
//! are added — here or, later, via the CMS — the front page reorganises itself with
//! no layout code to touch. Headline-led, press-standard, section-organised.

use dioxus::prelude::*;

use crate::app::Route;
use crate::config;
use crate::content::{in_section, Article, ARTICLES, SECTIONS};
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

/// kebab-case anchor id for a section name (e.g. "Crime" -> "s-crime").
fn anchor(section: &str) -> String {
    format!("s-{}", section.to_lowercase().replace(' ', "-"))
}

#[component]
pub fn Home() -> Element {
    let lead = &ARTICLES[0];

    // Build the populated sections (excluding the lead, which has its own slot),
    // in SECTIONS display order. Empty sections simply don't render.
    let sections: Vec<(&'static str, String, Vec<&'static Article>)> = SECTIONS
        .iter()
        .map(|s| {
            let arts: Vec<&'static Article> = in_section(s)
                .into_iter()
                .filter(|a| a.slug != lead.slug)
                .collect();
            (*s, anchor(s), arts)
        })
        .filter(|(_, _, arts)| !arts.is_empty())
        .collect();

    // NewsMediaOrganization JSON-LD for rich results / knowledge panel.
    let org_ld = format!(
        r#"{{"@context":"https://schema.org","@type":"NewsMediaOrganization","name":"{name}","url":"{base}","slogan":"{tagline}","sameAs":["https://www.facebook.com/Online.Stings","https://www.youtube.com/@JordanHall_dev","https://x.com/PredHunTers"]}}"#,
        name = config::SITE_NAME,
        base = config::BASE_URL,
        tagline = config::TAGLINE,
    );

    rsx! {
        crate::components::Seo {
            title: "Predator Hunters: independent local news, investigations and court reporting",
            description: "An independent local newsroom: local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, protected sources, and a public conviction database.",
            path: "/",
            image: "/og.png",
        }
        document::Script { r#type: "application/ld+json", dangerous_inner_html: org_ld }

        // ---------- SECTION NAV ----------
        nav { class: "sec-nav", "aria-label": "Sections",
            for (sec , anch , _) in sections.iter() {
                a { key: "{sec}", class: "sec-chip", href: "#{anch}", "{sec}" }
            }
            Link { class: "sec-chip db", to: Route::Database {}, "Convictions" }
        }

        // ---------- LEAD ----------
        section { class: "section", style: "padding-top:clamp(10px,2vh,22px);",
            div { class: "wrap",
                Link { class: "hero-lead", to: Route::Article { slug: lead.slug.to_string() },
                    if lead.image.is_some() {
                        img { class: "media", src: lead.image.unwrap_or(""), alt: "{lead.title}", loading: "lazy" }
                    }
                    div {
                        span { class: "kicker", "{lead.section}" }
                        h1 { class: "hl", "{lead.title}" }
                        p { class: "standfirst", "{lead.summary}" }
                        div { class: "byline",
                            span { "By {lead.byline}" }
                            span { class: "sep", "·" }
                            span { "{lead.date}" }
                        }
                    }
                }
            }
        }

        // ---------- SECTION CLUSTERS (data-driven) ----------
        for (sec , anch , arts) in sections.iter() {
            section { key: "{sec}", id: "{anch}", class: "section sec-block",
                div { class: "wrap",
                    div { class: "section-label",
                        span { class: "sec-index", "{sec}" }
                        Link { class: "sec-more", to: Route::News {}, "More" }
                    }
                    div { class: "cards",
                        for a in arts.iter() {
                            Link { key: "{a.slug}", class: "ncard", to: Route::Article { slug: a.slug.to_string() },
                                if a.image.is_some() {
                                    img { class: "media", src: a.image.unwrap_or(""), alt: "{a.title}", loading: "lazy" }
                                }
                                div { class: "ncard-body",
                                    span { class: "kicker", "{a.section}" }
                                    h3 { class: "hl", "{a.title}" }
                                    p { "{a.summary}" }
                                    div { class: "byline", "By {a.byline} · {a.date}" }
                                }
                            }
                        }
                    }
                }
            }
        }

        section { class: "section", style: "padding-top:0;",
            div { class: "wrap",
                Link { class: "btn btn-ghost", to: Route::News {},
                    "All news"
                    span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                }
            }
        }

        // ---------- DATABASE TEASER ----------
        section { class: "section",
            div { class: "wrap",
                Link { class: "db-teaser reveal", to: Route::Database {},
                    div {
                        span { class: "kicker", "Public record" }
                        h2 { class: "hl", "Search the conviction database" }
                        p { "Look up the people we have reported on once their case concluded, by name, area or offence. Court-sourced, post-conviction, and correctable." }
                    }
                    span { class: "db-teaser-go", dangerous_inner_html: svg("arrow-up-right") }
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
