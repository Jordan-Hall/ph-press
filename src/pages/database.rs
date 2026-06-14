//! Database — the PUBLIC conviction database: search post-conviction offenders
//! by name, area and offence, drawn from the public court record. This is the
//! search EXPERIENCE + safeguards; the SQLite-backed search + verified entries
//! land in WS3 (crates/ph-offender). No photo upload, no face-recognition on the
//! public side — text search only. Nothing here fabricates a conviction: with no
//! verified entries yet, search shows an honest "being compiled" state.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

/// Offence-category filters (a search taxonomy, not records).
const CATEGORIES: [&str; 5] = [
    "All offences",
    "Grooming & communication",
    "Sexual assault",
    "Indecent images",
    "Other",
];

const CHIP_BASE: &str = "font-family:var(--mono); font-size:.72rem; letter-spacing:.1em; text-transform:uppercase; padding:9px 15px; border-radius:999px; cursor:pointer; transition:all .18s; border:1px solid var(--hair-strong); background:var(--hair); color:var(--ink-2);";
const CHIP_ON: &str = "font-family:var(--mono); font-size:.72rem; letter-spacing:.1em; text-transform:uppercase; padding:9px 15px; border-radius:999px; cursor:pointer; transition:all .18s; border:1px solid transparent; background:var(--grad); color:var(--on-grad); box-shadow:0 10px 24px -12px rgba(245,130,32,.6);";

#[component]
pub fn Database() -> Element {
    let mut query = use_signal(String::new);
    let mut category = use_signal(|| "All offences".to_string());
    let mut searched = use_signal(|| false);

    let q = query();
    let did_search = searched();
    // Honest result state — there are no verified public entries yet, so every
    // search resolves to the "being compiled" state (never a fabricated record).
    let result_line = if did_search && !q.trim().is_empty() {
        format!("No published entries match \u{201c}{}\u{201d} yet.", q.trim())
    } else {
        "No published entries yet.".to_string()
    };

    rsx! {
        crate::components::Seo {
            title: "Conviction database | Predator Hunters",
            description: "Search convicted offenders by name, area and offence. Every entry is drawn from the public court record, post-conviction only, and can be corrected.",
            path: "/database",
            image: "/og.png",
        }

        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Conviction database" }
                h1 { class: "rise d2",
                    "The public record, "
                    span { class: "grad-text", "in one place." }
                }
                p { class: "lede rise d3",
                    "Look up offenders who have been through the courts, by name, area or offence. Every entry comes from the public court record, only after a conviction, and can be corrected."
                }
            }
        }

        // ---------- SEARCH CONSOLE ----------
        section { class: "section", style: "padding-top:clamp(16px,3vh,36px);",
            div { class: "wrap", style: "max-width:880px;",
                div {
                    class: "reveal",
                    style: "border:1px solid var(--hair-strong); border-radius:var(--r-lg); background:var(--readout-bg); box-shadow:0 40px 90px -50px rgba(0,0,0,.9), inset 0 1px 0 rgba(255,255,255,.05); overflow:hidden;",
                    // console bar
                    div { style: "display:flex; align-items:center; gap:9px; padding:12px 16px; border-bottom:1px solid var(--hair);",
                        span { style: "font-family:var(--mono); font-size:.68rem; letter-spacing:.18em; text-transform:uppercase; color:var(--muted);", "Search the public record" }
                        span { style: "margin-left:auto; font-family:var(--mono); font-size:.66rem; letter-spacing:.12em; text-transform:uppercase; color:var(--green-2); display:inline-flex; align-items:center; gap:7px;",
                            span { style: "width:6px;height:6px;border-radius:50%;background:var(--green);box-shadow:0 0 10px var(--green-glow);" }
                            "court-sourced"
                        }
                    }
                    div { style: "padding:22px;",
                        form {
                            onsubmit: move |e| { e.prevent_default(); searched.set(true); },
                            style: "display:flex; gap:10px; flex-wrap:wrap;",
                            div { style: "flex:1; min-width:240px; display:flex; align-items:center; gap:10px; padding:0 16px; background:var(--bg); border:1px solid var(--hair-strong); border-radius:999px;",
                                span { style: "color:var(--muted); display:inline-flex;", dangerous_inner_html: svg("scan") }
                                input {
                                    r#type: "search",
                                    style: "flex:1; min-height:50px; border:0; outline:0; background:transparent; font:inherit; font-size:1rem; color:var(--head);",
                                    placeholder: "Name, town or offence...",
                                    "aria-label": "Search the conviction database",
                                    value: "{query}",
                                    oninput: move |e| { query.set(e.value()); if e.value().is_empty() { searched.set(false); } },
                                }
                            }
                            button { r#type: "submit", class: "btn btn-primary",
                                span { class: "ic", dangerous_inner_html: svg("scale") }
                                "Search"
                            }
                        }
                        // offence filter chips
                        p { style: "font-family:var(--mono); font-size:.66rem; letter-spacing:.2em; text-transform:uppercase; color:var(--muted); margin:22px 0 12px;", "Filter by offence" }
                        div { style: "display:flex; flex-wrap:wrap; gap:9px;",
                            for cat in CATEGORIES {
                                button {
                                    key: "{cat}",
                                    style: if category() == cat { CHIP_ON } else { CHIP_BASE },
                                    "aria-pressed": if category() == cat { "true" } else { "false" },
                                    onclick: move |_| category.set(cat.to_string()),
                                    "{cat}"
                                }
                            }
                        }
                    }
                }

                // ---------- RESULTS (honest empty state) ----------
                div { "aria-live": "polite", style: "margin-top:22px;",
                    div { class: "card", style: "text-align:center; padding:44px 28px;",
                        div { class: "card-ic", style: "margin:0 auto 18px;", dangerous_inner_html: svg("doc") }
                        h3 { "{result_line}" }
                        p { style: "max-width:52ch; margin:12px auto 0;",
                            "Our editorial team is compiling the database from concluded court cases, one verified entry at a time. Search goes live as entries clear review. It will only ever list people whose cases have concluded in court, never anyone whose case is still live."
                        }
                        a { class: "btn btn-ghost btn-sm", style: "margin-top:20px;", href: "mailto:database@predatorhunters.co.uk?subject=Database%20correction%20or%20removal",
                            span { class: "ic", dangerous_inner_html: svg("mail") }
                            "Request a correction or removal"
                        }
                    }
                }
            }
        }

        // ---------- SAFEGUARDS ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "How the database works" }
                    h2 { "Built from the record, " span { class: "grad-text", "and accountable." } }
                }
                div { class: "grid-4",
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("scale") } h3 { "Court-sourced" } p { "Every entry is drawn from the public court record, not from allegations." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("check") } h3 { "Post-conviction only" } p { "Never anyone before a charge. We name after conviction, from the record." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("shield") } h3 { "Never a live case" } p { "An active-proceedings gate keeps out anyone whose case has not concluded." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("eye-off") } h3 { "Correctable" } p { "A right to have an entry checked, corrected, or reviewed for removal." } }
                }
                div { style: "margin-top:26px;",
                    Link { class: "btn btn-ghost", to: Route::Standards {},
                        "How we handle data + complaints"
                        span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                    }
                }
            }
        }
    }
}
