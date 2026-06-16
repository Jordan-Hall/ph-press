//! Database — the PUBLIC conviction database: search the people we have reported
//! on once their case concluded, by name, area or offence, drawn from the public
//! court record + linked to the news report (the news<->database crossover). A
//! map shows where, for entries the court placed. Text search only: no photo
//! upload and no face-recognition on the public side.

use dioxus::prelude::*;

use crate::app::Route;
use crate::content::{by_slug, CONVICTIONS};
use crate::icons::svg;

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
  var ic=L.icon({iconUrl:'/vendor/leaflet/images/marker-icon.png',iconRetinaUrl:'/vendor/leaflet/images/marker-icon-2x.png',shadowUrl:'/vendor/leaflet/images/marker-shadow.png',iconSize:[25,41],iconAnchor:[12,41],popupAnchor:[1,-34],shadowSize:[41,41]});
  "#;
    let tail = "\n})();";
    format!("{head}{markers}{tail}")
}

#[component]
pub fn Database() -> Element {
    let mut query = use_signal(String::new);
    let q = query().to_lowercase();
    let q = q.trim();
    let matches: Vec<&'static crate::content::Conviction> = CONVICTIONS
        .iter()
        .filter(|c| {
            q.is_empty()
                || c.name.to_lowercase().contains(q)
                || c.area.to_lowercase().contains(q)
                || c.offence.to_lowercase().contains(q)
        })
        .collect();
    let count = matches.len();
    let total = CONVICTIONS.len();
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
        // Leaflet — SELF-HOSTED (no third-party CDN, so the CSP stays 'self'; no
        // visitor IP leaks to unpkg). Only the map TILES come from the OpenStreetMap
        // tile service (a map-data service, not a script — can't be self-hosted).
        dioxus::document::Link { rel: "stylesheet", href: "/vendor/leaflet/leaflet.css" }
        dioxus::document::Script { src: "/vendor/leaflet/leaflet.js", defer: true }

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
                    for c in matches.iter() {
                        Link { key: "{c.name}", class: "r-row reveal", to: Route::Article { slug: c.article.to_string() },
                            div {
                                span { class: "r-num", "{c.offence}" }
                                h3 { class: "hl", "{c.name}" }
                                p { class: "r-desc",
                                    if c.area.is_empty() { "Area not stated by the court. " } else { "{c.area}. " }
                                    "{c.outcome}."
                                }
                            }
                            div { class: "r-meta",
                                span { class: "byline", "{c.date}" }
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
