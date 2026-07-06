//! /complaints-report — public IMPRESS transparency report.
//!
//! IMPRESS members are required to publish aggregate complaints-handling data.
//! This page shows ONLY counts, percentages, and status/category breakdowns.
//! It contains ZERO complainant-identifying data (no names, emails, or message
//! text). The underlying `public_complaints_report` server fn enforces this at
//! the API boundary.
//!
//! Methodology notes (displayed on the page):
//! - "Upheld" = outcome `upheld` or `partly_upheld`.
//! - "Not upheld" = outcome `not_upheld`.
//! - "Resolved" = any terminal status (upheld / partly upheld / not upheld / closed).
//! - Timeliness percentages are over *all complaints received* (conservative).
//! - A complaint received two days ago that is not yet acknowledged counts as
//!   pending, not excluded.

use dioxus::prelude::*;

use crate::api::public_complaints_report;

fn fmt_pct(n: f64) -> String {
    format!("{:.0}%", n)
}

#[component]
pub fn ComplaintsReport() -> Element {
    let stats = use_resource(move || async move { public_complaints_report().await });
    let g = stats.read();

    rsx! {
        crate::components::Seo {
            title: "Complaints transparency report | Predator Hunters",
            description: "How Predator Hunters handles reader complaints: aggregate statistics on outcomes, timeliness, and categories. Published in line with the IMPRESS Standards Code.",
            path: "/complaints-report",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Accountability" }
                h1 { class: "rise d2",
                    "Complaints "
                    span { class: "grad-text", "transparency report." }
                }
                p { class: "lede rise d3",
                    "As an IMPRESS-regulated publisher we are required to publish data on how we handle reader complaints. This report covers all complaints received since the publication launched. It contains aggregate statistics only \u{2014} no complainant names, contact details, or message content are ever shown here."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                match g.as_ref() {
                    None => rsx! {
                        p { class: "prose", style: "color:var(--muted);", "Loading\u{2026}" }
                    },
                    Some(Err(e)) => {
                        let msg = e.to_string();
                        rsx! {
                            p { class: "prose", style: "color:var(--muted);",
                                "The report could not be loaded. "
                                span { style: "opacity:.6;", "{msg}" }
                            }
                        }
                    },
                    Some(Ok(s)) => {
                        let total = s.total;
                        let upheld = s.upheld;
                        let not_upheld = s.not_upheld;
                        let resolved = s.resolved;
                        let acked_in_time = s.acked_in_time;
                        let resolved_in_time = s.resolved_in_time;
                        let upheld_pct = fmt_pct(s.upheld_pct);
                        let acked_pct = fmt_pct(s.acked_pct);
                        let resolved_pct = fmt_pct(s.resolved_pct);
                        let by_status = s.by_status.clone();
                        let by_category = s.by_category.clone();

                        if total == 0 {
                            rsx! {
                                div { class: "card reveal",
                                    p { class: "prose",
                                        "No complaints have been received since the publication launched."
                                    }
                                }
                            }
                        } else {
                            rsx! {
                                // ---- headline numbers ----
                                div { class: "grid-2 reveal",
                                    div { class: "card",
                                        p { class: "kicker", "Total complaints received" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{total}" }
                                    }
                                    div { class: "card",
                                        p { class: "kicker", "Resolved (all terminal outcomes)" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{resolved}" }
                                    }
                                    div { class: "card",
                                        p { class: "kicker", "Upheld or partly upheld" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{upheld}" }
                                        p { style: "color:var(--muted);font-size:.875rem;", "{upheld_pct}" " of resolved" }
                                    }
                                    div { class: "card",
                                        p { class: "kicker", "Not upheld" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{not_upheld}" }
                                    }
                                }

                                // ---- IMPRESS timeliness targets ----
                                h2 { class: "reveal", style: "margin-top:2.5rem;", "IMPRESS timeliness targets" }
                                p { class: "prose reveal",
                                    "The IMPRESS Standards Code requires publishers to acknowledge complaints promptly (within 7 days) and to give a final response within 21 days. The figures below are calculated over all complaints received."
                                }
                                div { class: "grid-2 reveal",
                                    div { class: "card",
                                        p { class: "kicker", "Acknowledged within 7 days" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{acked_in_time}" }
                                        p { style: "color:var(--muted);font-size:.875rem;", "{acked_pct}" " of all complaints received" }
                                    }
                                    div { class: "card",
                                        p { class: "kicker", "Final response within 21 days" }
                                        p { style: "font-size:2.4rem;font-weight:700;line-height:1;margin:.25em 0;", "{resolved_in_time}" }
                                        p { style: "color:var(--muted);font-size:.875rem;", "{resolved_pct}" " of all complaints received" }
                                    }
                                }

                                // ---- status breakdown ----
                                h2 { class: "reveal", style: "margin-top:2.5rem;", "Breakdown by status" }
                                div { class: "reveal",
                                    for (label, count) in by_status {
                                        div { key: "{label}", class: "card", style: "margin-bottom:.75rem;display:flex;justify-content:space-between;align-items:center;",
                                            span { style: "text-transform:capitalize;", "{label}" }
                                            span { style: "font-weight:700;font-size:1.15rem;", "{count}" }
                                        }
                                    }
                                }

                                // ---- category breakdown ----
                                h2 { class: "reveal", style: "margin-top:2.5rem;", "Breakdown by category" }
                                div { class: "reveal",
                                    for (label, count) in by_category {
                                        div { key: "{label}", class: "card", style: "margin-bottom:.75rem;display:flex;justify-content:space-between;align-items:center;",
                                            span { "{label}" }
                                            span { style: "font-weight:700;font-size:1.15rem;", "{count}" }
                                        }
                                    }
                                }

                                // ---- methodology note ----
                                div { class: "reveal", style: "margin-top:2.5rem;",
                                    h2 { "Methodology" }
                                    dl { class: "deflist",
                                        div { class: "def",
                                            dt { "Upheld" }
                                            dd { "A complaint where the investigation concluded that a breach of the IMPRESS Standards Code occurred, whether fully or in part (status \u{201c}upheld\u{201d} or \u{201c}partly upheld\u{201d})." }
                                        }
                                        div { class: "def",
                                            dt { "Not upheld" }
                                            dd { "A complaint where the investigation concluded there was no breach of the Standards Code." }
                                        }
                                        div { class: "def",
                                            dt { "Resolved" }
                                            dd { "Any complaint that has reached a terminal state: upheld, partly upheld, not upheld, or closed. Complaints still under investigation are not included in this count." }
                                        }
                                        div { class: "def",
                                            dt { "Timeliness denominators" }
                                            dd { "Timeliness percentages are calculated over all complaints received, including those still being handled. This is the most conservative and transparent measure." }
                                        }
                                        div { class: "def",
                                            dt { "Data coverage" }
                                            dd { "This report covers all complaints received since the publication launched. It is updated live from our complaints-management system." }
                                        }
                                    }
                                }

                                // ---- IMPRESS escalation note (only once registered) ----
                                if crate::components::regulator_registered() {
                                    div { class: "card reveal", style: "margin-top:2.5rem;",
                                        p { class: "prose",
                                            "If you are unhappy with our response to your complaint you can refer it to "
                                            a { href: "https://impress.press/complaints/", style: "color:var(--green-2);text-decoration:underline;text-underline-offset:3px;", rel: "noopener noreferrer", target: "_blank",
                                                "IMPRESS"
                                            }
                                            ", our independent regulator, free of charge."
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
