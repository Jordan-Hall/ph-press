//! Corrections & clarifications log — public IMPRESS-aligned page at `/corrections`.
//! Lists every published correction, newest first, with equal prominence to the
//! original and the corrected text (IMPRESS Standards Code §2).

use dioxus::prelude::*;

use crate::api::{public_corrections, PublicCorrection};
use crate::app::Route;
use crate::icons::svg;

#[component]
pub fn CorrectionsLog() -> Element {
    let mut corrections = use_signal(Vec::<PublicCorrection>::new);
    use_resource(move || async move {
        if let Ok(v) = public_corrections().await {
            corrections.set(v);
        }
    });
    let list = corrections.read();

    rsx! {
        crate::components::Seo {
            title: "Corrections & clarifications | Predator Hunters",
            description: "Every correction and clarification we have published, with the original and the corrected text kept on the record. IMPRESS Standards Code compliance.",
            path: "/corrections",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Corrections" }
                h1 { class: "rise d2",
                    "Getting it right, "
                    span { class: "grad-text", "on the record." }
                }
                p { class: "lede rise d3",
                    "When we make a significant error we correct it promptly, with prominence \
                     equal to the original. Every correction is published here with both \
                     versions kept for the record."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Our policy" }
                    h2 { "How corrections work." }
                }
                div { class: "prose reveal",
                    p {
                        "In line with the IMPRESS Standards Code we correct significant \
                         mistakes promptly and with equal prominence to the original \
                         publication. We keep both the original text and the correction \
                         on the record so readers can see exactly what changed and why. \
                         Minor editorial updates that do not change the substance of a \
                         story (such as fixing a typo) are made silently. Substantive \
                         corrections appear in this log."
                    }
                    p {
                        "If you believe something we have published is inaccurate or unfair, \
                         please use the \"Make a complaint\" link on the relevant article or \
                         visit our "
                        Link { to: Route::ComplaintsPolicy {}, "complaints policy page" }
                        "."
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Log" }
                    h2 { "Published corrections." }
                }
                if list.is_empty() {
                    div { class: "card reveal", style: "margin-top:18px; max-width:680px;",
                        div { class: "card-ic", dangerous_inner_html: svg("check") }
                        h3 { "No corrections yet" }
                        p {
                            "We have not had to publish a correction so far. When we do, \
                             every one will appear here with the date, the original text, \
                             the corrected text, and the reason for the change."
                        }
                    }
                } else {
                    div { class: "research-list", style: "margin-top:18px;",
                        for c in list.iter() {
                            {
                                let article_label = if c.article_title.is_empty() {
                                    format!("Correction #{}", c.id)
                                } else {
                                    c.article_title.clone()
                                };
                                let slug = c.article_slug.clone();
                                let has_slug = !slug.is_empty();
                                let id = c.id;
                                let iso_date = c.iso_date.clone();
                                let original = c.original.clone();
                                let corrected = c.corrected.clone();
                                let reason = c.reason.clone();
                                rsx! {
                                    div { key: "{id}", class: "r-row reveal",
                                        div {
                                            span { class: "r-num", "{iso_date}" }
                                            if has_slug {
                                                h3 { class: "hl",
                                                    Link {
                                                        to: Route::Article { slug },
                                                        "{article_label}"
                                                    }
                                                }
                                            } else {
                                                h3 { class: "hl", "{article_label}" }
                                            }
                                            div { class: "correction-entry", style: "margin-top:10px;",
                                                p { class: "r-desc",
                                                    strong { "Originally: " }
                                                    "{original}"
                                                }
                                                p { class: "r-desc",
                                                    strong { "Corrected: " }
                                                    "{corrected}"
                                                }
                                                if !reason.is_empty() {
                                                    p { class: "r-desc",
                                                        strong { "Reason: " }
                                                        "{reason}"
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
        }
    }
}
