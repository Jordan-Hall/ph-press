//! Home — the newsroom front page. DATA-DRIVEN: the compile-time seeds are the
//! baseline and the live CMS feed (published via /desk) is merged in, then the
//! lead = newest and the body is built from the section taxonomy. So as stories
//! are published the front page reorganises itself with no layout code to touch.

use dioxus::prelude::*;

use crate::api::published_feed;
use crate::app::Route;
use crate::assets::PH_LOGO;
use crate::config;
use crate::content::{ARTICLES, SECTIONS};
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

/// A unified front-page card, from a compile-time seed or the live CMS feed.
#[derive(Clone, PartialEq)]
struct Card {
    slug: String,
    title: String,
    summary: String,
    section: String,
    byline: String,
    date: String,
    iso: String,
    image: Option<String>,
}

/// kebab-case anchor id for a section name (e.g. "Crime" -> "s-crime").
fn anchor(section: &str) -> String {
    format!("s-{}", section.to_lowercase().replace(' ', "-"))
}

#[component]
pub fn Home() -> Element {
    let feed = use_resource(move || published_feed());

    // Seeds first (full fidelity), then any published story that isn't a seed.
    let mut cards: Vec<Card> = ARTICLES
        .iter()
        .map(|a| Card {
            slug: a.slug.to_string(),
            title: a.title.to_string(),
            summary: a.summary.to_string(),
            section: a.section.to_string(),
            byline: a.byline.to_string(),
            date: a.date.to_string(),
            iso: a.iso_date.to_string(),
            image: a.image.map(|s| s.to_string()),
        })
        .collect();
    {
        let g = feed.read();
        if let Some(Ok(items)) = g.as_ref() {
            for f in items {
                if !ARTICLES.iter().any(|a| a.slug == f.slug) {
                    cards.push(Card {
                        slug: f.slug.clone(),
                        title: f.title.clone(),
                        summary: f.summary.clone(),
                        section: f.section.clone(),
                        byline: f.byline.clone(),
                        date: f.iso_date.clone(),
                        iso: f.iso_date.clone(),
                        image: None,
                    });
                }
            }
        }
    }
    cards.sort_by(|a, b| b.iso.cmp(&a.iso));

    // Lead with the newest hard-news story (a front page leads with news, not an
    // announcement/explainer); those still appear in the Community cluster.
    let lead_idx = cards
        .iter()
        .position(|c| c.section != "Community")
        .unwrap_or(0);
    let lead = cards[lead_idx].clone();
    let rest: Vec<Card> = cards
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != lead_idx)
        .map(|(_, c)| c.clone())
        .collect();
    // Populated sections (excluding the lead), in SECTIONS display order.
    let sections: Vec<(&'static str, String, Vec<Card>)> = SECTIONS
        .iter()
        .map(|&s| {
            let arts: Vec<Card> = rest.iter().filter(|c| c.section == s).cloned().collect();
            (s, anchor(s), arts)
        })
        .filter(|(_, _, arts)| !arts.is_empty())
        .collect();

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
                Link { class: "hero-lead", to: Route::Article { slug: lead.slug.clone() },
                    if let Some(src) = lead.image.as_ref() {
                        img { class: "media", src: "{src}", alt: "{lead.title}", loading: "lazy" }
                    } else {
                        img { class: "media logo", src: PH_LOGO, alt: "{lead.title}", loading: "lazy" }
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
                        for c in arts.iter() {
                            Link { key: "{c.slug}", class: "ncard", to: Route::Article { slug: c.slug.clone() },
                                if let Some(src) = c.image.as_ref() {
                                    img { class: "media", src: "{src}", alt: "{c.title}", loading: "lazy" }
                                } else {
                                    img { class: "media logo", src: PH_LOGO, alt: "{c.title}", loading: "lazy" }
                                }
                                div { class: "ncard-body",
                                    span { class: "kicker", "{c.section}" }
                                    h3 { class: "hl", "{c.title}" }
                                    p { "{c.summary}" }
                                    div { class: "byline", "By {c.byline} · {c.date}" }
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
