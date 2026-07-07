//! Article page (/news/:slug) — editorial reading layout + self-hosted video
//! player + the full social-share head (og:video / twitter:player) so a shared
//! link plays the video on Facebook and elsewhere. NewsArticle (+ VideoObject)
//! JSON-LD. Renders its own head (not the shared Seo) so og:type is "article"
//! and the video tags are conditional.

use dioxus::prelude::*;

use crate::api::{public_article, PublicArticle};
use crate::app::Route;
use crate::content::{by_slug, Article as Art};
use crate::icons::svg;

use crate::config::BASE_URL as BASE;

/// Estimated reading time in minutes from a word count (~200 wpm), min 1.
fn read_mins(words: usize) -> usize {
    words.div_ceil(200).max(1)
}

fn json_esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn news_jsonld(a: &Art) -> String {
    let img = format!("{BASE}{}", a.og_image());
    let video = a
        .video
        .as_ref()
        .map(|v| {
            format!(
                ",\"video\":{{\"@type\":\"VideoObject\",\"name\":\"{}\",\"thumbnailUrl\":\"{BASE}{}\",\"contentUrl\":\"{BASE}{}\",\"uploadDate\":\"{}\"}}",
                json_esc(a.title), v.poster, v.mp4, a.iso_date
            )
        })
        .unwrap_or_default();
    format!(
        "{{\"@context\":\"https://schema.org\",\"@type\":\"NewsArticle\",\"headline\":\"{}\",\"description\":\"{}\",\"articleSection\":\"{}\",\"datePublished\":\"{}\",\"image\":\"{}\",\"author\":{{\"@type\":\"Person\",\"name\":\"{}\"}},\"publisher\":{{\"@type\":\"NewsMediaOrganization\",\"name\":\"Predator Hunters\",\"url\":\"{BASE}/\"}}{}}}",
        json_esc(a.title), json_esc(a.summary), json_esc(a.section), a.iso_date, img, json_esc(a.byline), video
    )
}

/// BreadcrumbList JSON-LD (Home › News › this story) for breadcrumb rich results.
fn breadcrumb_ld(title: &str) -> String {
    format!(
        "{{\"@context\":\"https://schema.org\",\"@type\":\"BreadcrumbList\",\"itemListElement\":[{{\"@type\":\"ListItem\",\"position\":1,\"name\":\"Home\",\"item\":\"{BASE}/\"}},{{\"@type\":\"ListItem\",\"position\":2,\"name\":\"News\",\"item\":\"{BASE}/news\"}},{{\"@type\":\"ListItem\",\"position\":3,\"name\":\"{}\"}}]}}",
        json_esc(title)
    )
}

#[component]
pub fn Article(slug: String) -> Element {
    // Compile-time seed? Render the full editorial layout below. Otherwise it may
    // be a story published live via /desk — fetch it from the CMS by slug.
    let Some(a) = by_slug(&slug) else {
        return rsx! { LiveArticle { key: "{slug}", slug } };
    };

    let url = format!("{BASE}/news/{}", a.slug);
    let img = format!("{BASE}{}", a.og_image());
    // Precompute video fields (rsx can't hold `let` bindings inside conditionals).
    let has_video = a.video.is_some();
    let mp4_abs = a
        .video
        .as_ref()
        .map(|v| format!("{BASE}{}", v.mp4))
        .unwrap_or_default();
    let mp4_path = a.video.as_ref().map(|v| v.mp4).unwrap_or("");
    let poster = a.video.as_ref().map(|v| v.poster).unwrap_or("");
    let vw = a.video.as_ref().map(|v| v.width).unwrap_or(0);
    let vh = a.video.as_ref().map(|v| v.height).unwrap_or(0);
    // YouTube embed + hero image.
    let has_youtube = a.youtube.is_some();
    let yt_id = a.youtube.unwrap_or("");
    let hero = a.image.unwrap_or("");
    let has_hero = a.image.is_some() && !has_youtube && !has_video;
    let mins = read_mins(a.body.iter().map(|p| p.split_whitespace().count()).sum());
    // Kicker label: precomputed so rsx never does mixed interpolation.
    let kicker_text = format!("{} · {}", a.section, a.kind);
    // Support signpost: opt-in via `@support` anywhere in the body.
    let has_support = a.body.iter().any(|b| crate::md::is_support_block(b));

    rsx! {
        // ---- head: per-article SEO + social-share (incl. video) ----
        dioxus::document::Title { "{a.title} | Predator Hunters" }
        dioxus::document::Meta { name: "description", content: "{a.summary}" }
        dioxus::document::Link { rel: "canonical", href: "{url}" }
        dioxus::document::Meta { property: "og:type", content: "article" }
        dioxus::document::Meta { property: "og:title", content: "{a.title}" }
        dioxus::document::Meta { property: "og:description", content: "{a.summary}" }
        dioxus::document::Meta { property: "og:url", content: "{url}" }
        dioxus::document::Meta { property: "og:image", content: "{img}" }
        dioxus::document::Meta { property: "article:published_time", content: "{a.iso_date}" }
        dioxus::document::Meta { property: "article:section", content: "{a.section}" }
        dioxus::document::Meta { property: "article:author", content: "{a.byline}" }
        if has_video {
            // Video share card — a shared link plays the video inline on Facebook etc.
            dioxus::document::Meta { property: "og:video", content: "{mp4_abs}" }
            dioxus::document::Meta { property: "og:video:secure_url", content: "{mp4_abs}" }
            dioxus::document::Meta { property: "og:video:type", content: "video/mp4" }
            dioxus::document::Meta { property: "og:video:width", content: "{vw}" }
            dioxus::document::Meta { property: "og:video:height", content: "{vh}" }
            dioxus::document::Meta { name: "twitter:card", content: "player" }
            dioxus::document::Meta { name: "twitter:player:stream", content: "{mp4_abs}" }
            dioxus::document::Meta { name: "twitter:player:stream:content_type", content: "video/mp4" }
            dioxus::document::Meta { name: "twitter:player:width", content: "{vw}" }
            dioxus::document::Meta { name: "twitter:player:height", content: "{vh}" }
        } else {
            dioxus::document::Meta { name: "twitter:card", content: "summary_large_image" }
        }
        dioxus::document::Meta { name: "twitter:title", content: "{a.title}" }
        dioxus::document::Meta { name: "twitter:description", content: "{a.summary}" }
        dioxus::document::Meta { name: "twitter:image", content: "{img}" }
        script { r#type: "application/ld+json", dangerous_inner_html: news_jsonld(a) }
        script { r#type: "application/ld+json", dangerous_inner_html: breadcrumb_ld(a.title) }

        // ---- editorial layout ----
        article { class: "art-page",
            // ── Article header: kicker → headline → standfirst → byline + red rule ──
            header { class: "art-header",
                div { class: "wrap art-header-inner",
                    p { class: "art-kicker rise d1", "{kicker_text}" }
                    h1 { class: "art-headline rise d2", "{a.title}" }
                    p { class: "art-standfirst rise d3", "{a.summary}" }
                    div { class: "art-byline-row rise d4",
                        span { class: "art-by", "By " b { "{a.byline}" } }
                        span { class: "art-sep", "·" }
                        time { class: "art-date", datetime: "{a.iso_date}", "{a.date}" }
                        span { class: "art-sep", "·" }
                        span { class: "art-reading", "{mins} min read" }
                    }
                }
            }

            // ── Lead media: video / YouTube / hero image in a framed figure ──
            section { class: "section art-body-section",
                div { class: "wrap art-body-wrap",
                    if has_youtube {
                        figure { class: "art-lead-figure reveal",
                            div { class: "art-lead-embed",
                                iframe {
                                    src: "https://www.youtube-nocookie.com/embed/{yt_id}",
                                    title: "{a.title}",
                                    style: "width:100%;height:100%;border:0;",
                                    "loading": "lazy",
                                    "referrerpolicy": "strict-origin-when-cross-origin",
                                    "allow": "accelerometer; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share",
                                    "allowfullscreen": "true",
                                }
                            }
                        }
                    } else if has_video {
                        figure { class: "art-lead-figure reveal",
                            video {
                                controls: true,
                                preload: "none",
                                poster: "{poster}",
                                width: "{vw}",
                                height: "{vh}",
                                style: "width:100%;height:auto;display:block;",
                                source { src: "{mp4_path}", r#type: "video/mp4" }
                                "Your browser does not support the video tag."
                            }
                        }
                    } else if has_hero {
                        figure { class: "art-lead-figure reveal",
                            img { class: "art-lead-img", src: "{hero}", alt: "{a.title}", loading: "lazy" }
                        }
                    }

                    // ── Prose body ──
                    div { class: "prose art-prose reveal",
                        for para in a.body.iter() {
                            div { dangerous_inner_html: crate::md::block_html(para) }
                        }
                        // End-mark — ◆ closes every piece with a publisher mark.
                        p { class: "art-endmark", aria_hidden: "true", "◆" }
                    }

                    // ── Support signpost (IMPRESS Cl.9 / IPSO Cl.5 — suicide/self-harm) ──
                    // Shown only when the author has added `@support` in the body.
                    if has_support {
                        SupportBox {}
                    }

                    // ── Article footer: published date + complaint link + nav ──
                    footer { class: "art-footer",
                        div { class: "art-footer-meta",
                            span { class: "art-footer-date",
                                "Published "
                                time { datetime: "{a.iso_date}", "{a.date}" }
                            }
                        }
                        div { class: "art-footer-links",
                            Link { class: "btn btn-ghost btn-sm", to: Route::News {},
                                span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                                "More from the newsroom"
                            }
                            Link { class: "btn btn-ghost btn-sm", to: Route::Standards {},
                                span { class: "ic", dangerous_inner_html: svg("scale") }
                                "Our standards"
                            }
                            Link { class: "art-complaint-link", to: Route::ComplaintPage { slug: a.slug.to_string() },
                                "Make a complaint about this article"
                            }
                        }
                    }
                }
            }
        }
        RelatedStories { section: a.section.to_string(), exclude: a.slug.to_string() }
    }
}

/// A story published live via /desk (not a compile-time seed). Fetched from the
/// CMS by slug and rendered in the reading layout. NewsArticle JSON-LD + article
/// OG so live stories share + index correctly.
#[component]
fn LiveArticle(slug: String) -> Element {
    // Keyed by slug at the call site, so each article is a fresh instance and the
    // resource refetches per slug.
    let res = use_resource(move || {
        let slug = slug.clone();
        async move { public_article(slug).await }
    });
    let guard = res.read();
    match guard.as_ref() {
        None => rsx! {
            header { class: "page-head",
                div { class: "wrap", p { class: "lede", "Loading\u{2026}" } }
            }
        },
        Some(Ok(Some(a))) => rsx! { LiveArticleBody { a: a.clone() } },
        _ => rsx! {
            dioxus::document::Meta { name: "robots", content: "noindex, follow" }
            header { class: "page-head",
                div { class: "wrap",
                    p { class: "eyebrow rise d1", "Not found" }
                    h1 { class: "rise d2", "That story " span { class: "grad-text", "isn't here." } }
                    p { class: "lede rise d3", "The link may be old or mistyped." }
                    div { class: "rise d4", style: "margin-top:28px; display:flex; gap:12px; flex-wrap:wrap;",
                        Link { class: "btn btn-primary", to: Route::News {}, "Back to the newsroom" }
                    }
                }
            }
        },
    }
}

#[component]
fn LiveArticleBody(a: PublicArticle) -> Element {
    let url = format!("{BASE}/news/{}", a.slug);
    // Meta description: explicit field, else fall back to the standfirst.
    let desc = if a.meta_description.trim().is_empty() {
        a.summary.clone()
    } else {
        a.meta_description.clone()
    };
    // OG image must be ABSOLUTE for social scrapers. Empty -> omitted.
    let og_image = if a.og_image_url.trim().is_empty() {
        String::new()
    } else if a.og_image_url.starts_with("http://") || a.og_image_url.starts_with("https://") {
        a.og_image_url.clone()
    } else {
        format!("{BASE}{}", a.og_image_url)
    };
    let has_og_image = !og_image.is_empty();
    let keywords = a.tags.join(", ");
    let has_keywords = !keywords.is_empty();
    let kw_ld = if has_keywords { format!(",\"keywords\":\"{}\"", json_esc(&keywords)) } else { String::new() };
    let jsonld = format!(
        "{{\"@context\":\"https://schema.org\",\"@type\":\"NewsArticle\",\"headline\":\"{}\",\"description\":\"{}\",\"articleSection\":\"{}\",\"datePublished\":\"{}\",\"author\":{{\"@type\":\"Person\",\"name\":\"{}\"}},\"publisher\":{{\"@type\":\"NewsMediaOrganization\",\"name\":\"Predator Hunters\",\"url\":\"{BASE}/\"}}{}}}",
        json_esc(&a.title), json_esc(&desc), json_esc(&a.section), a.iso_date, json_esc(&a.byline), kw_ld
    );
    let mins = read_mins(a.body.iter().map(|p| p.split_whitespace().count()).sum());
    // Kicker label: precomputed so rsx never does mixed interpolation.
    let kicker_text = format!("{} · {}", a.section, a.kind);
    let slug_for_complaint = a.slug.clone();
    // Support signpost: opt-in via `@support` anywhere in the body.
    let has_support = a.body.iter().any(|b| crate::md::is_support_block(b));
    rsx! {
        dioxus::document::Title { "{a.title} | Predator Hunters" }
        dioxus::document::Meta { name: "description", content: "{desc}" }
        if has_keywords {
            dioxus::document::Meta { name: "keywords", content: "{keywords}" }
        }
        dioxus::document::Link { rel: "canonical", href: "{url}" }
        dioxus::document::Meta { property: "og:type", content: "article" }
        dioxus::document::Meta { property: "og:title", content: "{a.title}" }
        dioxus::document::Meta { property: "og:description", content: "{desc}" }
        dioxus::document::Meta { property: "og:url", content: "{url}" }
        if has_og_image {
            dioxus::document::Meta { property: "og:image", content: "{og_image}" }
        }
        dioxus::document::Meta { property: "article:section", content: "{a.section}" }
        dioxus::document::Meta { property: "article:published_time", content: "{a.iso_date}" }
        dioxus::document::Meta { name: "twitter:card", content: "summary_large_image" }
        dioxus::document::Meta { name: "twitter:title", content: "{a.title}" }
        dioxus::document::Meta { name: "twitter:description", content: "{desc}" }
        if has_og_image {
            dioxus::document::Meta { name: "twitter:image", content: "{og_image}" }
        }
        script { r#type: "application/ld+json", dangerous_inner_html: jsonld }
        script { r#type: "application/ld+json", dangerous_inner_html: breadcrumb_ld(&a.title) }

        article { class: "art-page",
            // ── Article header: kicker → headline → standfirst → byline row ──
            header { class: "art-header",
                div { class: "wrap art-header-inner",
                    p { class: "art-kicker rise d1", "{kicker_text}" }
                    h1 { class: "art-headline rise d2", "{a.title}" }
                    p { class: "art-standfirst rise d3", "{a.summary}" }
                    div { class: "art-byline-row rise d4",
                        span { class: "art-by", "By " b { "{a.byline}" } }
                        span { class: "art-sep", "·" }
                        time { class: "art-date", datetime: "{a.iso_date}", "{a.iso_date}" }
                        span { class: "art-sep", "·" }
                        span { class: "art-reading", "{mins} min read" }
                    }
                }
            }

            // ── Prose body (live articles embed images inline via md-figure) ──
            section { class: "section art-body-section",
                div { class: "wrap art-body-wrap",
                    div { class: "prose art-prose reveal",
                        for para in a.body.iter() {
                            div { dangerous_inner_html: crate::md::block_html(para) }
                        }
                        // End-mark
                        p { class: "art-endmark", aria_hidden: "true", "◆" }
                    }

                    // ── Support signpost (IMPRESS Cl.9 / IPSO Cl.5 — suicide/self-harm) ──
                    // Shown only when the author has added `@support` in the body.
                    if has_support {
                        SupportBox {}
                    }

                    // ── Article footer: published date + complaint link + nav ──
                    footer { class: "art-footer",
                        div { class: "art-footer-meta",
                            span { class: "art-footer-date",
                                "Published "
                                time { datetime: "{a.iso_date}", "{a.iso_date}" }
                            }
                        }
                        div { class: "art-footer-links",
                            Link { class: "btn btn-ghost btn-sm", to: Route::News {},
                                span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                                "More from the newsroom"
                            }
                            Link { class: "btn btn-ghost btn-sm", to: Route::Standards {},
                                span { class: "ic", dangerous_inner_html: svg("scale") }
                                "Our standards"
                            }
                            Link { class: "art-complaint-link", to: Route::ComplaintPage { slug: slug_for_complaint },
                                "Make a complaint about this article"
                            }
                        }
                    }
                }
            }
        }
        RelatedStories { section: a.section.clone(), exclude: a.slug.clone() }
    }
}

/// Samaritans support signpost — shown on articles that report suicide or
/// self-harm (opted in via an `@support` line in the body). Calm, neutral card:
/// never alarming red, uses muted paper tones so it reads as care not warning.
#[component]
fn SupportBox() -> Element {
    rsx! {
        aside { class: "art-support-box",
            div { class: "art-support-inner",
                span { class: "art-support-icon", aria_hidden: "true", "♡" }
                div { class: "art-support-text",
                    p { class: "art-support-heading", "If you need support" }
                    p { class: "art-support-body",
                        "If you or someone you know is struggling, you can talk to "
                        strong { "Samaritans" }
                        " any time, free, 24 hours a day."
                    }
                    ul { class: "art-support-links",
                        li {
                            "Call free: "
                            a {
                                href: "tel:116123",
                                class: "art-support-link",
                                "116 123"
                            }
                        }
                        li {
                            "Email: "
                            a {
                                href: "mailto:jo@samaritans.org",
                                class: "art-support-link",
                                "jo@samaritans.org"
                            }
                        }
                        li {
                            "Web: "
                            a {
                                href: "https://www.samaritans.org",
                                class: "art-support-link",
                                rel: "noopener",
                                target: "_blank",
                                "samaritans.org"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// "More from this section" — up to three other seed stories in the same section,
/// newest first. A standard newsroom cross-link block: keeps readers in the
/// story and strengthens internal linking for search. Renders nothing if the
/// section has no other stories. Live (CMS) stories relate against the seeds.
#[component]
fn RelatedStories(section: String, exclude: String) -> Element {
    let mut related: Vec<&'static Art> = crate::content::in_section(&section)
        .into_iter()
        .filter(|a| a.slug != exclude)
        .collect();
    related.sort_by(|a, b| b.iso_date.cmp(a.iso_date));
    related.truncate(3);
    if related.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "section", style: "padding-top:8px;",
            div { class: "wrap", style: "max-width:760px;",
                div { class: "section-label", style: "margin-bottom:14px;",
                    span { class: "sec-index", "More from {section}" }
                    Link { class: "sec-more", to: Route::News {}, "All news" }
                }
                div { class: "research-list",
                    for a in related.iter() {
                        Link {
                            key: "{a.slug}",
                            class: "r-row reveal",
                            to: Route::Article { slug: a.slug.to_string() },
                            div {
                                span { class: "r-num", "{a.section} · {a.kind}" }
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
            }
        }
    }
}
