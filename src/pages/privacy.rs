//! Privacy — plain-language privacy policy for the public site.

use dioxus::prelude::*;

use crate::icons::svg;

/// (term, definition)
const POLICY: [(&str, &str); 6] = [
    ("This website", "We keep one thing in your browser: whether you chose light or dark mode. No tracking cookies, no advertising, and no analytics that identify you."),
    ("When you contact us", "If you email us a tip, a press query or a complaint, we keep that message so we can act on it and keep a record, and nothing more. We do not sell it or use it to build a profile."),
    ("Court reporting", "Our reporting uses what is on the public court record, after a case has concluded. As a rule we do not name anyone before they are charged, and we never publish a child's private data."),
    ("The conviction database", "Entries are drawn from the public court record, post-conviction only. We handle criminal-conviction data under UK GDPR with a documented lawful basis, and you can ask us to check, correct or review an entry."),
    ("Your data and your rights", "We follow UK GDPR and the ICO's guidance. You can ask what we hold, ask us to correct it, or raise a concern, and a person will answer."),
    ("Embeds", "Some pages embed video or audio from third parties (for example YouTube). When you play them, those providers may set their own cookies. We use privacy-respecting embeds where we can."),
];

#[component]
pub fn Privacy() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Privacy | Predator Hunters",
            description: "How Predator Hunters handles data: no tracking, court-record-only reporting, the conviction database under UK GDPR, and your rights.",
            path: "/privacy",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Privacy" }
                h1 { class: "rise d2",
                    "Plain words about "
                    span { class: "grad-text", "your data." }
                }
                p { class: "lede rise d3",
                    "We keep as little as we can, and we are clear about what we hold and why."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                dl { class: "deflist reveal",
                    for (term , def) in POLICY {
                        div { key: "{term}", class: "def", dt { "{term}" } dd { "{def}" } }
                    }
                }
                p { class: "prose", style: "margin-top:28px;",
                    "Questions about any of this go to "
                    a { href: "mailto:privacy@predatorhunters.co.uk", style: "color:var(--green-2); text-decoration:underline; text-underline-offset:3px;", "privacy@predatorhunters.co.uk" }
                    ". Last updated June 2026."
                }
                div { style: "margin-top:18px;",
                    a { class: "btn btn-ghost", href: "mailto:privacy@predatorhunters.co.uk",
                        span { class: "ic", dangerous_inner_html: svg("mail") }
                        "Ask us anything"
                    }
                }
            }
        }
    }
}
