//! Standards — editorial standards, complaints, corrections, and transparency.
//! Built towards IMPRESS registration: this is the public-facing half of the
//! obligations (the workflow + records are enforced by the editorial system).

use dioxus::prelude::*;

use crate::api::submit_complaint;
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

        // ---------- COMPLAINTS PROCEDURE ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Complaints" } h2 { "If we get it wrong, hold us to it." } }
                div { class: "prose reveal",
                    p { "If you think we have fallen short of the standards above, tell us and we will look into it. You do not need a lawyer, and it does not cost anything." }
                }
                dl { class: "deflist reveal", style: "margin-top:18px;",
                    div { class: "def", dt { "How to complain" } dd { "Email complaints@predatorhunters.co.uk with the article or video, the date, and what you believe is inaccurate or unfair." } }
                    div { class: "def", dt { "Who handles it" } dd { "One of our editors-in-chief, Jordan Upton or Scott Taylor, and where possible not the person responsible for the item." } }
                    div { class: "def", dt { "How long it takes" } dd { "We acknowledge your complaint within 7 days and aim to give you a decision within 21 days. If it needs longer, we will tell you why." } }
                    div { class: "def", dt { "If we got it wrong" } dd { "We correct or clarify it quickly, with prominence equal to the original, and we keep both versions on the record." } }
                    div { class: "def", dt { "If you are not satisfied" } dd { "You can take your complaint to our independent press regulator. We keep a record of every complaint we receive." } }
                }
                ComplaintForm {}
            }
        }

        // ---------- CORRECTIONS ARCHIVE ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Corrections" } h2 { "Our corrections, in the open." } }
                div { class: "prose reveal",
                    p { "When we get something significantly wrong we correct it quickly, with prominence equal to the original, and we keep both the correction and what we first published on the record. Every correction we make is listed here." }
                }
                div { class: "card reveal", style: "margin-top:18px; max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("check") }
                    h3 { "No corrections yet" }
                    p { "We have not had to publish a correction so far. When we do, it will appear here with the date and what changed." }
                }
            }
        }

        // ---------- WHISTLEBLOWING + CONSCIENCE ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Speaking up" } h2 { "Whistleblowing and conscience." } }
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("shield") }
                        h3 { "Whistleblowing" }
                        p { "Anyone who works with us can raise a concern about wrongdoing, including anything that falls short of these standards, safely and in confidence. Email confidential@predatorhunters.co.uk and we will protect your identity." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("check") }
                        h3 { "Conscience clause" }
                        p { "No one who works with us will be made to act against this code or against their own conscience, and no one will be penalised for refusing to." }
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

/// Public complaint form — submits straight into the /desk Complaints inbox.
#[component]
fn ComplaintForm() -> Element {
    let mut slug = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut body = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut done = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match submit_complaint(slug(), name(), body()).await {
                Ok(()) => done.set(true),
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    if done() {
        return rsx! {
            div { class: "card reveal", style: "margin-top:24px; max-width:680px;",
                div { class: "card-ic", dangerous_inner_html: svg("check") }
                h3 { "Thank you — your complaint is logged" }
                p { "We have received it and will acknowledge it within 7 days. If you left a way to reach you, we will be in touch." }
            }
        };
    }

    rsx! {
        form { class: "complaint-form reveal", onsubmit: submit,
            div { class: "cf-row",
                input {
                    class: "cf-in",
                    r#type: "text",
                    placeholder: "Which article or video? (link or title — optional)",
                    value: "{slug}",
                    oninput: move |e| slug.set(e.value()),
                }
                input {
                    class: "cf-in",
                    r#type: "text",
                    placeholder: "Your name or email (optional)",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
            }
            textarea {
                class: "cf-in cf-body",
                rows: "5",
                placeholder: "What do you believe is inaccurate or unfair?",
                value: "{body}",
                oninput: move |e| body.set(e.value()),
            }
            if let Some(e) = err() {
                p { class: "cf-err", "{e}" }
            }
            div { class: "cf-actions",
                button { class: "btn btn-primary", r#type: "submit", disabled: busy(),
                    if busy() { "Sending…" } else { "Submit complaint" }
                }
                a { class: "btn btn-ghost", href: "mailto:complaints@predatorhunters.co.uk?subject=Complaint", "Or email us instead" }
            }
        }
    }
}
