//! Complaints policy — public IMPRESS-aligned page at `/complaints`.
//! This is the *index* page explaining how to complain; the per-article
//! complaint form lives at `/complaints/:slug` (unchanged).

use dioxus::prelude::*;

use crate::icons::svg;

/// Step-by-step complaints procedure — displayed as a definition list.
const PROCEDURE: [(&str, &str); 5] = [
    (
        "How to make a complaint",
        "Use the \"Make a complaint\" link on any article, or email \
         complaints@predatorhunters.co.uk with the article link or title, the \
         date of publication, and what you believe is inaccurate or unfair. You \
         do not need a lawyer, and there is no charge.",
    ),
    (
        "Who handles it",
        "Your complaint is reviewed by one of our editors-in-chief — Jordan \
         Upton or Scott Taylor — and, where possible, not the person responsible \
         for the item.",
    ),
    (
        "Acknowledgement",
        "We acknowledge every complaint promptly. In line with the IMPRESS \
         Standards Code we aim to acknowledge within 7 days of receipt.",
    ),
    (
        "Final response",
        "We aim to give a full, reasoned final response within 21 days of \
         receipt. If the matter requires longer we will tell you why and keep \
         you updated.",
    ),
    (
        "If a correction is needed",
        "Where we find we were wrong we correct or clarify the article quickly, \
         with prominence equal to the original, and we publish the correction in \
         our public corrections log at /corrections, keeping both versions on the \
         record.",
    ),
];

#[component]
pub fn ComplaintsPolicy() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Complaints policy | Predator Hunters",
            description: "How to make a complaint about Predator Hunters: our IMPRESS-aligned process, timelines, and how to escalate to our independent regulator.",
            path: "/complaints",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Complaints" }
                h1 { class: "rise d2",
                    "If we get it wrong, "
                    span { class: "grad-text", "hold us to it." }
                }
                p { class: "lede rise d3",
                    "We take complaints seriously. Here is exactly how to raise one and what happens next."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Our process" }
                    h2 { "What to expect." }
                }
                div { class: "prose reveal",
                    p {
                        "We are an independent publisher working towards registration with \
                         IMPRESS, the UK's approved press regulator. We follow the IMPRESS \
                         Standards Code on complaints and corrections, and we operate the \
                         process described below for every complaint we receive."
                    }
                }
                dl { class: "deflist reveal", style: "margin-top:18px;",
                    for (term, def) in PROCEDURE {
                        div { key: "{term}", class: "def",
                            dt { "{term}" }
                            dd { "{def}" }
                        }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Escalation" }
                    h2 { "Not satisfied? Take it further." }
                }
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("scale") }
                    h3 { "IMPRESS — our independent regulator" }
                    p {
                        "If you are unhappy with our final response, or if we have not \
                         responded within the timescales above, you can refer your complaint \
                         to IMPRESS, our independent regulator, free of charge."
                    }
                    a {
                        class: "btn btn-primary",
                        href: "https://impress.press/complaints/",
                        rel: "noopener noreferrer",
                        target: "_blank",
                        span { class: "ic", dangerous_inner_html: svg("arrow-up-right") }
                        "Refer to IMPRESS"
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Make a complaint" }
                    h2 { "Ready to raise a concern?" }
                }
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("doc") }
                        h3 { "On a specific article" }
                        p {
                            "Every article on our site has a \"Make a complaint\" link at \
                             the bottom. Use it to open a form pre-filled with that article \
                             — the quickest way to get your complaint to the right place."
                        }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("mail") }
                        h3 { "By email" }
                        p {
                            "Email us at complaints@predatorhunters.co.uk with the article \
                             link or title, the date, and what you believe is wrong or unfair. \
                             We will acknowledge it and keep you updated."
                        }
                        a {
                            class: "btn btn-ghost",
                            href: "mailto:complaints@predatorhunters.co.uk?subject=Complaint",
                            span { class: "ic", dangerous_inner_html: svg("mail") }
                            "Email a complaint"
                        }
                    }
                }
            }
        }
    }
}
