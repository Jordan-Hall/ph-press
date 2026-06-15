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
    ("2022", "Our first major story is a murder. We publish details the police have not, and afterwards officers come to us asking for our sources. We protect them. Around the same time we report a social club in Derbyshire selling alcohol to children."),
    ("2025", "We widen into local investigations: bailiffs and arrests at a local hospital, and council housing decisions that leave residents without homes, while keeping every source anonymous."),
    ("2026", "Court reporting and a public conviction database become a core part of who we are, and we open our newsroom."),
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
                        "It started with the kind of story that decides what a newsroom is. We covered a murder and published details the police had not. When officers came to us asking for our sources, we protected them. People trust us because of that, and that trust is still the foundation of everything we do."
                    }
                    p {
                        "Since then we have broken local news of all kinds: a social club in Derbyshire selling alcohol to children, bailiffs and arrests at a local hospital, council housing decisions that left residents without homes. Since 2020 we have offered rewards for information that helps catch the people behind serious crimes. We report concluded cases from the public court record and keep a public database of convictions. Where there is a child-protection angle we draw on years of frontline experience, but we are a "
                        strong { "local newsroom first." }
                    }
                    p {
                        "Some lines do not move. We "
                        strong { "protect our sources" }
                        ". We keep them anonymous, and we act only on what we can cross-reference and verify. As a rule we do not name anyone before they are charged. On the cases we cover, we report from the record."
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
                        p { "We are not the police, not a surveillance company, and not in it for a show. As a rule we do not name anyone before they are charged. We act only on what we can verify, and we work alongside the police, not in their place." }
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

        // ---------- THE TEAM ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "The team" } h2 { "Who runs the newsroom." } }
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { style: "width:76px; height:76px; border-radius:999px; background:var(--red); display:grid; place-items:center; font-family:var(--serif); font-size:1.6rem; color:var(--on-red); margin-bottom:16px; box-shadow:0 8px 20px -10px rgba(0,0,0,.5);", "JU" }
                        h3 { "Jordan Upton" }
                        p { style: "font-family:var(--mono); font-size:.72rem; letter-spacing:.14em; text-transform:uppercase; color:var(--red); margin:4px 0 10px;", "Editor-in-chief" }
                        p { "Jordan leads our frontline work and our reporting, shares editorial control, and self-funds most of the newsroom." }
                    }
                    div { class: "card reveal",
                        div { style: "width:76px; height:76px; border-radius:999px; background:var(--red); display:grid; place-items:center; font-family:var(--serif); font-size:1.6rem; color:var(--on-red); margin-bottom:16px; box-shadow:0 8px 20px -10px rgba(0,0,0,.5);", "ST" }
                        h3 { "Scott Taylor" }
                        p { style: "font-family:var(--mono); font-size:.72rem; letter-spacing:.14em; text-transform:uppercase; color:var(--red); margin:4px 0 10px;", "Editor-in-chief" }
                        p { "Scott shares editorial control, works on the frontline alongside Jordan, and leads on the press side, from reporting to standards." }
                    }
                }
            }
        }
    }
}
