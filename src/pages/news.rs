//! News — the newsroom index. DATA-DRIVEN: the compile-time seeds are the baseline
//! and the live CMS feed (published via /desk) is merged in, so newly published
//! stories appear here without a code change. SSG renders the seeds (crawlable);
//! the client fetches the live feed and folds in anything new. Newest first.

use dioxus::prelude::*;

use crate::api::published_feed;
use crate::app::Route;
use crate::assets::PH_LOGO;
use crate::content::{ARTICLES, SECTIONS};
use crate::icons::svg;

/// A unified news-list card, from either a compile-time seed or the live CMS feed.
#[derive(Clone, PartialEq)]
struct Card {
    slug: String,
    title: String,
    summary: String,
    kind: String,
    section: String,
    date: String,
    iso: String,
    image: Option<String>,
}

#[component]
pub fn News() -> Element {
    // Live CMS feed (client-fetched on the web target; merged with the seeds).
    let feed = use_resource(move || published_feed());
    // Active section filter (None = all). Client-side, so the SSG page still
    // renders every story for crawlers; the filter just narrows what's shown.
    let mut filter = use_signal(|| Option::<&'static str>::None);

    let mut cards: Vec<Card> = ARTICLES
        .iter()
        .map(|a| Card {
            slug: a.slug.to_string(),
            title: a.title.to_string(),
            summary: a.summary.to_string(),
            kind: a.kind.to_string(),
            section: a.section.to_string(),
            date: a.date.to_string(),
            iso: a.iso_date.to_string(),
            image: a.image.map(|s| s.to_string()),
        })
        .collect();
    {
        // Fold in published stories that aren't compile-time seeds (new in /desk).
        let g = feed.read();
        if let Some(Ok(items)) = g.as_ref() {
            for f in items {
                if !ARTICLES.iter().any(|a| a.slug == f.slug) {
                    cards.push(Card {
                        slug: f.slug.clone(),
                        title: f.title.clone(),
                        summary: f.summary.clone(),
                        kind: f.kind.clone(),
                        section: f.section.clone(),
                        date: f.iso_date.clone(),
                        iso: f.iso_date.clone(),
                        image: None,
                    });
                }
            }
        }
    }
    cards.sort_by(|a, b| b.iso.cmp(&a.iso));

    // Only offer a filter chip for sections that actually have stories, in the
    // taxonomy's display order (matches the front page's section nav).
    let present: Vec<&'static str> = SECTIONS
        .iter()
        .copied()
        .filter(|s| cards.iter().any(|c| c.section == *s))
        .collect();

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
        // ---------- SECTION FILTER ----------
        nav { class: "sec-nav", "aria-label": "Filter by section",
            button {
                class: if filter().is_none() { "sec-chip active" } else { "sec-chip" },
                "aria-pressed": filter().is_none(),
                onclick: move |_| filter.set(None),
                "All"
            }
            for s in present.iter().copied() {
                button {
                    key: "{s}",
                    class: if filter() == Some(s) { "sec-chip active" } else { "sec-chip" },
                    "aria-pressed": filter() == Some(s),
                    onclick: move |_| filter.set(Some(s)),
                    "{s}"
                }
            }
            Link { class: "sec-chip db", to: Route::Database {}, "Convictions" }
        }

        section { class: "section", style: "padding-top:clamp(16px,3vh,32px);",
            div { class: "wrap",
                div { class: "research-list",
                    for c in cards.iter().filter(|c| filter().is_none_or(|s| c.section == s)) {
                        Link {
                            key: "{c.slug}",
                            class: "r-row has-img reveal",
                            to: Route::Article { slug: c.slug.clone() },
                            if let Some(src) = c.image.as_ref() {
                                img { class: "r-thumb", src: "{src}", alt: "{c.title}", loading: "lazy" }
                            } else {
                                img { class: "r-thumb logo", src: PH_LOGO, alt: "{c.title}", loading: "lazy" }
                            }
                            div {
                                span { class: "r-num", "{c.section} · {c.kind}" }
                                h3 { class: "hl", "{c.title}" }
                                p { class: "r-desc", "{c.summary}" }
                            }
                            div { class: "r-meta",
                                span { class: "byline", "{c.date}" }
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
