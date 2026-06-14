//! Standards — editorial standards, complaints, corrections, and transparency.
//! Built towards IMPRESS registration: this is the public-facing half of the
//! obligations (the workflow + records are enforced by the editorial system).

use dioxus::prelude::*;

use crate::icons::svg;

/// (term, definition) — our standards. Written to satisfy the IMPRESS Standards
/// Code (which we publicly work towards) and, as a matter of practice, the wider
/// editors' code used by the alternative regulator.
const STANDARDS: [(&str, &str); 13] = [
    ("Accuracy", "We take care to be accurate, we distinguish fact from comment, and we correct significant mistakes promptly and with equal prominence. Court reporting is checked against the public court record before it is published."),
    ("Identification and charging", "As a rule we do not name or identify people before they are charged, and we hold footage back until there is a conviction, so we do not prejudice a live case or harm someone who is never charged. We will identify a person before charge only where there is a strong public interest and we can confirm the facts, for example a named suspect in a murder or other serious crime, or a confirmed arrest, as the IMPRESS Standards Code allows."),
    ("Active proceedings", "We do not publish anything that creates a substantial risk of serious prejudice to active legal proceedings, in line with the Contempt of Court Act 1981."),
    ("Children", "We take particular care with anyone under 18. We do not identify a child victim or witness, we do not report on a child's welfare without consent except where there is an exceptional public interest, and we never publish a child's private data."),
    ("Justice and victims", "We keep complainants in sexual-offence cases anonymous as the law requires, we do not interfere with criminal investigations, and we do not pay criminals or witnesses for their stories."),
    ("Privacy", "We respect a person's reasonable expectation of privacy and weigh it against the public interest. We report convictions from the public court record."),
    ("Sources", "We protect our confidential sources and whistleblowers. We keep sources anonymous and act only on what we can cross-reference and verify."),
    ("Harassment and discrimination", "We do not intimidate or persistently pursue people and we respect a reasonable request to desist, unless there is an overriding public interest. We do not incite hatred or refer to a person's protected characteristics unless genuinely relevant."),
    ("Reporting on suicide", "When we report a death by suicide we avoid excessive or technical detail of method, and we signpost sources of support."),
    ("Grief and shock", "We approach people affected by grief or shock with sympathy and discretion, and we publish such material with sensitivity."),
    ("Investigative methods", "We use confrontation, covert recording or other intrusive methods only where the story is in the public interest and the information could not reasonably be obtained any other way."),
    ("Identifying relatives", "We take care before identifying the relatives or friends of people accused or convicted of crime where they are not genuinely relevant to the story."),
    ("Transparency", "We label opinion as opinion and any AI-assisted work as such, we are open about who we are and how we are funded, and we publish our corrections and complaints process."),
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
                    p { "Predator Hunters is a small, independent local newsroom, reporting since 2022. It has two editors-in-chief, Jordan Upton and Scott Taylor, and is self-funded, mainly by Jordan Upton, with Scott Taylor contributing when needed. We cover local news and investigations, report from the courts, and offer rewards for information on serious crimes. We are not owned by, and do not act for, any police force or political party." }
                    p { "We are working towards registration with IMPRESS. Until that is complete we hold ourselves to the standards above and operate the same complaints and corrections process. We will publish our regulator details and trustmark here once registration is in place." }
                }
            }
        }
    }
}
