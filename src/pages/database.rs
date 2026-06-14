//! Database — the PUBLIC conviction database: search post-conviction offenders
//! by name, area and offence, drawn from the public court record. Search UI +
//! framing here; the search backend + entries land in WS3 (crates/ph-offender).
//! No face upload, no face-recognition on the public side.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

#[component]
pub fn Database() -> Element {
    let mut query = use_signal(String::new);
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
                div { class: "rise d4", style: "margin-top:28px; max-width:560px;",
                    div { style: "display:flex; gap:10px; flex-wrap:wrap;",
                        input {
                            r#type: "search",
                            style: "flex:1; min-width:220px; min-height:48px; padding:12px 16px; font:inherit; font-size:1rem; color:var(--head); background:var(--bg); border:1px solid var(--hair-strong); border-radius:999px;",
                            placeholder: "Name, town or offence...",
                            "aria-label": "Search the conviction database",
                            value: "{query}",
                            oninput: move |e| query.set(e.value()),
                        }
                        button { class: "btn btn-primary",
                            span { class: "ic", dangerous_inner_html: svg("scale") }
                            "Search"
                        }
                    }
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:760px;",
                    div { class: "card-ic", dangerous_inner_html: svg("doc") }
                    h3 { "We are building it from the court record" }
                    p { "Our editorial team is compiling the database from concluded court cases, entry by entry, checked against the public record. Search goes live as entries are verified. It will only ever list people whose cases have concluded in court, never anyone whose case is still live." }
                    p { style: "margin-top:12px;", "Think an entry is wrong, or that something needs correcting or removing? Tell us and we will check it against the record." }
                    a { class: "btn btn-ghost btn-sm", style: "margin-top:14px;", href: "mailto:database@predatorhunters.co.uk?subject=Database%20correction",
                        span { class: "ic", dangerous_inner_html: svg("mail") }
                        "database@predatorhunters.co.uk"
                    }
                }
                div { class: "grid-3", style: "margin-top:24px;",
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("scale") } h3 { "Court-sourced" } p { "Every entry is drawn from the public court record." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("check") } h3 { "Post-conviction only" } p { "Never anyone before a charge or during a live case." } }
                    div { class: "card reveal", div { class: "card-ic", dangerous_inner_html: svg("eye-off") } h3 { "Correctable" } p { "A right to have an entry checked, corrected, or reviewed." } }
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
