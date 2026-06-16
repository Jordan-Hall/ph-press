//! Database — the PUBLIC conviction database: search the people we have reported
//! on once their case concluded, by name, area or offence, drawn from the public
//! court record + linked to the news report (the news<->database crossover). A
//! map shows where, for entries the court placed. Text search only: no photo
//! upload and no face-recognition on the public side.

use dioxus::prelude::*;

use crate::api::{conviction_db, PublicConviction};
use crate::app::Route;
use crate::content::{by_slug, CONVICTIONS};
use crate::icons::svg;

/// A unified conviction row for the public list — from the compile-time record or
/// the live database. Both link to our published report; database entries also
/// carry the court-record / news source they were drawn from.
struct Entry {
    name: String,
    area: String,
    offence: String,
    outcome: String,
    date: String,
    article_slug: String,
}

impl Entry {
    fn from_static(c: &'static crate::content::Conviction) -> Self {
        Entry {
            name: c.name.to_string(),
            area: c.area.to_string(),
            offence: c.offence.to_string(),
            outcome: c.outcome.to_string(),
            date: c.date.to_string(),
            article_slug: c.article.to_string(),
        }
    }
    fn from_public(c: &PublicConviction) -> Self {
        Entry {
            name: c.name.clone(),
            area: c.area.clone(),
            offence: c.offence.clone(),
            outcome: c.outcome.clone(),
            date: c.date.clone(),
            article_slug: c.article_slug.clone(),
        }
    }
}

/// Escape a string for embedding inside a single-quoted JS string literal.
fn js1(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', " ")
}

/// Truncate a summary to a tidy one-line snippet for the map popup.
fn snippet(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!(
            "{}\u{2026}",
            s.chars().take(max).collect::<String>().trim_end()
        )
    } else {
        s.to_string()
    }
}

// Leaflet init: poll until the library + map div exist, then drop a pin for each
// LOCATED conviction with a rich popup — the linked article's hero image, a short
// snapshot, and a link through to the full report. Guarded against SPA re-init.
fn map_js() -> String {
    let mut markers = String::new();
    for c in CONVICTIONS.iter().filter(|c| c.located()) {
        let art = by_slug(c.article);
        let img = art.map(|a| a.og_image()).unwrap_or_default();
        let summary = art.map(|a| a.summary).unwrap_or("");
        let area = if c.area.is_empty() {
            String::new()
        } else {
            format!("<span class=\"mappop-a\">{}</span>", c.area)
        };
        let popup = format!(
            "<div class=\"mappop\"><img src=\"{img}\" alt=\"\" loading=\"lazy\"/><div class=\"mappop-b\"><span class=\"mappop-k\">{off}</span><strong>{name}</strong>{area}<p>{snip}</p><a class=\"mappop-link\" href=\"/news/{slug}\">Read the full report \u{2192}</a></div></div>",
            img = img,
            off = c.offence,
            name = c.name,
            area = area,
            snip = snippet(summary, 96),
            slug = c.article,
        );
        markers.push_str(&format!(
            "L.marker([{lat},{lng}],{{icon:ic}}).addTo(m).bindPopup('{popup}',{{maxWidth:260,className:'mappop-wrap'}});",
            lat = c.lat,
            lng = c.lng,
            popup = js1(&popup),
        ));
    }
    let head = r#"(function init(){
  if(!window.L||!document.getElementById('phmap')){return setTimeout(init,150);}
  if(window.__phmap){return;} window.__phmap=true;
  var m=L.map('phmap',{scrollWheelZoom:false}).setView([52.55,-1.30],8);
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png',{maxZoom:18,attribution:'(c) OpenStreetMap contributors'}).addTo(m);
  var ic=L.icon({iconUrl:'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png',iconRetinaUrl:'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png',shadowUrl:'https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png',iconSize:[25,41],iconAnchor:[12,41],popupAnchor:[1,-34],shadowSize:[41,41]});
  "#;
    let tail = "\n})();";
    format!("{head}{markers}{tail}")
}

#[component]
pub fn Database() -> Element {
    let mut query = use_signal(String::new);
    // Published entries from the live database are merged with the compile-time
    // record below. Compile-time entries render server-side (crawlable); database
    // entries load on the client after hydration.
    let mut db = use_signal(Vec::<PublicConviction>::new);
    use_resource(move || async move {
        if let Ok(v) = conviction_db().await {
            db.set(v);
        }
    });
    let q = query().to_lowercase();
    let q = q.trim();
    let mut entries: Vec<Entry> = CONVICTIONS.iter().map(Entry::from_static).collect();
    entries.extend(db.read().iter().map(Entry::from_public));
    let total = entries.len();
    let matches: Vec<Entry> = entries
        .into_iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(q)
                || e.area.to_lowercase().contains(q)
                || e.offence.to_lowercase().contains(q)
        })
        .collect();
    let count = matches.len();
    let result_note = if q.is_empty() {
        format!("{total} convictions on the record")
    } else if count == 0 {
        format!("No entries match \u{201c}{}\u{201d}", query().trim())
    } else {
        format!("{count} of {total} entries match")
    };

    rsx! {
        crate::components::Seo {
            title: "Conviction database | Predator Hunters",
            description: "Search the people we have reported on once their case concluded, by name, area or offence. Every entry is drawn from the public court record and linked to our report.",
            path: "/database",
            image: "/og.png",
        }
        // Leaflet (privacy-respecting OpenStreetMap tiles).
        dioxus::document::Link { rel: "stylesheet", href: "https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" }
        dioxus::document::Script { src: "https://unpkg.com/leaflet@1.9.4/dist/leaflet.js", defer: true }

        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Conviction database" }
                h1 { class: "rise d2",
                    "The public record, "
                    span { class: "grad-text", "in one place." }
                }
                p { class: "lede rise d3",
                    "Look up the people we have reported on once their case concluded, by name, area or offence. Every entry comes from the public court record, only after a conviction, and links to our report. It can be corrected."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(14px,2.5vh,32px);",
            div { class: "wrap", style: "max-width:920px;",
                // search
                form { onsubmit: move |e| e.prevent_default(), style: "display:flex; gap:10px; flex-wrap:wrap; margin-bottom:14px;",
                    div { style: "flex:1; min-width:240px; display:flex; align-items:center; gap:10px; padding:0 16px; background:var(--paper-2); border:1px solid var(--rule-2); border-radius:8px;",
                        span { style: "color:var(--muted); display:inline-flex;", dangerous_inner_html: svg("scan") }
                        input {
                            r#type: "search",
                            style: "flex:1; min-height:50px; border:0; outline:0; background:transparent; font:inherit; font-size:1rem; color:var(--ink);",
                            placeholder: "Search by name, area or offence...",
                            "aria-label": "Search the conviction database",
                            value: "{query}",
                            oninput: move |e| query.set(e.value()),
                        }
                    }
                }
                p { style: "font-family:var(--mono); font-size:.72rem; letter-spacing:.12em; text-transform:uppercase; color:var(--muted); margin:0 0 18px;", "{result_note}" }

                // map — isolation:isolate confines Leaflet's internal z-indexes
                // (panes/controls/popups go up to ~1000) so they never paint over
                // the sticky masthead (which sits in its own z-index:50 layer).
                div { class: "card", style: "padding:0; overflow:hidden; margin-bottom:24px; position:relative; z-index:0; isolation:isolate;",
                    div { id: "phmap", style: "height:360px; width:100%; background:var(--sunk);" }
                }
                script { dangerous_inner_html: map_js() }

                // entries
                div { class: "research-list",
                    for e in matches.iter() {
                        Link { key: "{e.name}-{e.article_slug}", class: "r-row reveal", to: Route::Article { slug: e.article_slug.clone() },
                            div {
                                span { class: "r-num", "{e.offence}" }
                                h3 { class: "hl", "{e.name}" }
                                p { class: "r-desc",
                                    if e.area.is_empty() { "Area not stated by the court. " } else { "{e.area}. " }
                                    "{e.outcome}."
                                }
                            }
                            div { class: "r-meta",
                                span { class: "byline", "{e.date}" }
                                span { class: "r-arrow", dangerous_inner_html: svg("arrow-up-right") }
                            }
                        }
                    }
                }

                a { class: "btn btn-ghost btn-sm", style: "margin-top:22px;", href: "mailto:database@predatorhunters.co.uk?subject=Database%20correction%20or%20removal",
                    span { class: "ic", dangerous_inner_html: svg("mail") }
                    "Request a correction or removal"
                }
            }
        }

        // safeguards
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "How the database works" } h2 { "Built from the record, " span { class: "grad-text", "and accountable." } } }
                div { class: "grid-4",
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("scale") } h3 { "Court-sourced" } p { "Every entry is drawn from the public court record, not from allegations." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("check") } h3 { "Post-conviction only" } p { "Never anyone before a charge or during a live case." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("doc") } h3 { "Linked to our report" } p { "Each entry links to the story we published on the case." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("eye-off") } h3 { "Correctable" } p { "A right to have an entry checked, corrected, or reviewed for removal." } }
                }
            }
        }
    }
}
