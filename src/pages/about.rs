//! About — who we are and where we came from. An independent LOCAL newsroom:
//! local news + investigations + court reporting + reward appeals + source
//! protection + a public conviction database. Reporting since 2022 (Derbyshire);
//! rewards since 2020; a core newsroom from 2026. Not child-protection-only.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

/// (year, event)
const TIMELINE: [(&str, &str); 4] = [
    ("2020", "We begin offering rewards for information that helps catch the people behind serious crimes: killers, rapists and abusers. People come to us because they trust us to keep them anonymous."),
    ("2022", "Our reporting starts with a single local story: a social club in Derbyshire selling alcohol to children. It is the first of many."),
    ("2025", "We report what we can, when we can, building relationships and trust across the communities we cover."),
    ("2026", "The newsroom becomes a core part of who we are: local news, court reporting from the public record, reward appeals, and a public conviction database."),
];

#[component]
pub fn About() -> Element {
    rsx! {
        crate::components::Seo {
            title: "About | Predator Hunters",
            description: "Predator Hunters is an independent local newsroom: local news and investigations, court reporting from the public record, reward appeals for information on serious crimes, and a public conviction database. We protect our sources.",
            path: "/about",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "About" }
                h1 { class: "rise d2",
                    "Independent local journalism, "
                    span { class: "grad-text", "built on trust." }
                }
                p { class: "lede rise d3",
                    "We are small, independent and self-funded. We break local news, report from the courts, offer rewards for information on serious crimes, and protect the people who come to us."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(16px,3vh,40px);",
            div { class: "wrap",
                div { class: "prose reveal",
                    p {
                        "It started with a single local story. In 2022 we reported that a social club in Derbyshire was selling alcohol to children. People talked to us because they trusted us to protect them, and that trust is still the foundation of everything we do."
                    }
                    p {
                        "Since 2020 we have offered rewards for information that helps catch the people behind serious crimes. We break local news of all kinds, not only the headline cases. We report concluded cases from the public court record, and we keep a public database of convictions so a community can see what the courts have decided. Where there is a child-protection angle we draw on years of frontline experience, but we are a "
                        strong { "local newsroom first." }
                    }
                    p {
                        "Some lines do not move. We "
                        strong { "protect our sources" }
                        ". We keep them anonymous, and we act only on what we can cross-reference and verify. We never name anyone before they are charged. On the cases we cover, we report from the record."
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Timeline" }
                    h2 { "How we got here." }
                }
                dl { class: "deflist reveal",
                    for (year , event) in TIMELINE {
                        div { key: "{year}", class: "def",
                            dt { "{year}" }
                            dd { "{event}" }
                        }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("check") }
                        h3 { "What we are" }
                        p { "An independent local newsroom. We break local news and investigations, report concluded cases from the public court record, offer rewards for information on serious crimes, and keep a public conviction database. We protect our sources." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("eye-off") }
                        h3 { "What we are not" }
                        p { "We are not the police, not a surveillance company, and not in it for a show. We never name anyone before they are charged. We act only on what we can verify, and we work alongside the police, not in their place." }
                    }
                }
                div { style: "margin-top:28px; display:flex; gap:12px; flex-wrap:wrap;",
                    Link { class: "btn btn-ghost", to: Route::Standards {},
                        "Our standards & complaints"
                        span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                    }
                    a { class: "btn btn-ghost", href: "https://research.predatorhunters.co.uk", target: "_blank", rel: "noopener",
                        span { class: "ic", dangerous_inner_html: svg("cpu") }
                        "Our research & AI"
                    }
                }
            }
        }
    }
}
