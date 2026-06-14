//! Standards — editorial standards, complaints, corrections, and transparency.
//! Built towards IMPRESS registration: this is the public-facing half of the
//! obligations (the workflow + records are enforced by the editorial system).

use dioxus::prelude::*;

use crate::icons::svg;

/// (term, definition) — the standards we hold ourselves to.
const STANDARDS: [(&str, &str); 6] = [
    ("Accuracy", "We take care to be accurate, we distinguish fact from comment, and we correct significant mistakes promptly and with equal prominence. Our reporting is checked against the court record before it is published."),
    ("Never before a charge", "We never name or identify anyone before they have been charged, and we hold footage back until there is a conviction. We do not prejudice a live case."),
    ("Children", "We do not identify a child involved in a case without consent, except where there is an exceptional public interest, and we never publish a child's private data."),
    ("Privacy", "We weigh a person's reasonable expectation of privacy against the public interest, and we report from the public record."),
    ("Justice", "We do not interfere with criminal investigations. We work with the police, and we keep complainants in sexual-offence cases anonymous."),
    ("Transparency", "We label opinion as opinion and any AI-assisted work as such, and we are open about who we are and how we are funded."),
];

#[component]
pub fn Standards() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Standards, complaints & corrections | Predator Hunters",
            description: "Our editorial standards, complaints process, corrections policy and transparency. Independent court-reporting journalism, working towards IMPRESS registration.",
            path: "/standards",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Standards" }
                h1 { class: "rise d2",
                    "How we report, and "
                    span { class: "grad-text", "how to hold us to it." }
                }
                p { class: "lede rise d3",
                    "We are an independent publisher working towards registration with IMPRESS, the UK's approved press regulator. These are the standards we hold ourselves to and the ways you can raise a concern."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Our standards" } h2 { "The lines we report by." } }
                dl { class: "deflist reveal",
                    for (term , def) in STANDARDS {
                        div { key: "{term}", class: "def", dt { "{term}" } dd { "{def}" } }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("mail") }
                        h3 { "Complaints" }
                        p { "If you think we have fallen short of these standards, tell us. Email a complaint to complaints@predatorhunters.co.uk with the article and what is wrong. We aim to respond within a few days, we keep a record of every complaint, and if you are not satisfied you can escalate to our regulator." }
                        a { class: "btn btn-ghost btn-sm", style: "margin-top:14px;", href: "mailto:complaints@predatorhunters.co.uk?subject=Complaint",
                            span { class: "ic", dangerous_inner_html: svg("mail") }
                            "complaints@predatorhunters.co.uk"
                        }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("doc") }
                        h3 { "Corrections" }
                        p { "When we get something significantly wrong we correct it promptly, with prominence equal to the original, and we keep both the correction and the original on the record. Our corrections are logged and published here as they happen." }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Who we are" } h2 { "Transparency." } }
                div { class: "prose reveal",
                    p { "Predator Hunters is an independent, self-funded local newsroom, reporting since 2022 and led by Jordan Hall. We cover local news and investigations, report from the courts, and offer rewards for information on serious crimes. We are not owned by, and do not act for, any police force or political party." }
                    p { "We are working towards registration with IMPRESS. Until that is complete we hold ourselves to the standards above and operate the same complaints and corrections process. We will publish our regulator details and trustmark here once registration is in place." }
                }
            }
        }
    }
}
