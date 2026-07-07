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
                // Build-time const (not the runtime helper): the baked <meta>/OG
                // description can only change on a rebuild, so it flips together with
                // the SSG-baked body when REGULATOR_REGISTERED is set true and redeployed
                // (see config.rs). Cautious by default.
                description: if crate::config::REGULATOR_REGISTERED {
                    "Our editorial standards, complaints process, corrections policy and transparency. Independent court-reporting journalism regulated by IMPRESS, the UK's approved press regulator.".to_string()
                } else {
                    "Our editorial standards, complaints process, corrections policy and transparency. Independent court-reporting journalism that holds itself to the IMPRESS Standards Code and intends to seek registration.".to_string()
                },
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
                        if crate::components::regulator_registered() {
                            "We are an independent publisher regulated by IMPRESS, the UK's approved press regulator. Below are the standards we hold ourselves to and the ways you can raise a concern."
                        } else {
                            "We are an independent publisher that holds itself to the IMPRESS Standards Code and intends to seek registration with IMPRESS, the UK's approved press regulator. Below are those standards and the ways you can raise a concern."
                        }
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
                        div { class: "def", dt { "If you are not satisfied" } dd {
                            if crate::components::regulator_registered() {
                                "If you are still not satisfied with our final response, you can escalate your complaint to IMPRESS, our independent press regulator. We keep a record of every complaint we receive."
                            } else {
                                "We keep a record of every complaint we receive. We intend to seek registration with IMPRESS, the UK's approved press regulator; if we are registered you will be able to escalate an unresolved complaint to them."
                            }
                        } }
                    }
                    ComplaintForm { slug: String::new() }
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
                        if crate::components::regulator_registered() {
                            p { "We hold ourselves to the IMPRESS Standards Code and are regulated by IMPRESS, the UK's approved press regulator. We operate the complaints and corrections process set out above, and if you are not satisfied with our final response you can refer an unresolved complaint to IMPRESS. Our regulator details and trustmark are published here." }
                        } else {
                            p { "We hold ourselves to the IMPRESS Standards Code and intend to seek registration with IMPRESS, the UK's approved press regulator. Until we are registered we operate the same complaints and corrections process set out above, but we are not yet regulated by IMPRESS and cannot refer complaints to them. We will publish our regulator details and trustmark here once registration is in place." }
                        }
                        p { "We monitor public sources \u{2014} court judgments and news reports \u{2014} to find concluded cases that fall within what we cover. Anything found this way is treated only as an unverified lead: an editor checks it against the public court record, clears any reporting restrictions, and writes our own report, which still goes through legal sign-off before publication. We do not republish another outlet's text or photographs, and every database entry links to our own report and cites the record it was drawn from." }
    }
                }
            }
        }
}

/// Public complaint form — submits into the /desk Complaints inbox (IMPRESS).
/// When `slug` is supplied (the per-article entry point) it is the fixed subject;
/// otherwise the reader names the article themselves.
#[component]
pub fn ComplaintForm(slug: String) -> Element {
    let fixed_slug = slug;
    let init_about = fixed_slug.clone();
    let mut about = use_signal(move || init_about);
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut category = use_signal(String::new);
    let mut body = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut reference = use_signal(|| Option::<String>::None);
    let mut err = use_signal(|| Option::<String>::None);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match submit_complaint(about(), name(), email(), category(), body()).await {
                Ok(r) => reference.set(Some(r)),
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    if let Some(r) = reference() {
        return rsx! {
            div { class: "card reveal", style: "margin-top:24px; max-width:680px;",
                div { class: "card-ic", dangerous_inner_html: svg("check") }
                h3 { "Thank you — your complaint is logged" }
                p { {format!("Your reference is {r}. We've emailed an acknowledgement to the address you gave. In line with the IMPRESS Standards Code we aim to give a final response within 21 days.")} }
                if crate::components::regulator_registered() {
                    p { "If you're unhappy with our final response you can refer the matter to "
                        a { href: crate::config::REGULATOR_URL, "{crate::config::REGULATOR_NAME}" }
                        ", our independent regulator."
                    }
                } else {
                    p { "If you're unhappy with our final response, tell us why and we'll look at it again. We intend to seek registration with IMPRESS; if we are registered you'll be able to escalate an unresolved complaint to them." }
                }
            }
        };
    }

    let has_fixed = !fixed_slug.is_empty();
    rsx! {
        form { class: "complaint-form reveal", onsubmit: submit,
            if has_fixed {
                p { class: "cf-about", {format!("About: {fixed_slug}")} }
            }
            div { class: "cf-row",
                if !has_fixed {
                    input { class: "cf-in", r#type: "text", placeholder: "Which article or video? (link or title)", value: "{about}", oninput: move |e| about.set(e.value()) }
                }
                input { class: "cf-in", r#type: "text", placeholder: "Your name", value: "{name}", oninput: move |e| name.set(e.value()), required: true }
            }
            div { class: "cf-row",
                input { class: "cf-in", r#type: "email", placeholder: "Your email (so we can respond)", value: "{email}", oninput: move |e| email.set(e.value()), required: true }
                select { class: "cf-in", value: "{category}", oninput: move |e| category.set(e.value()),
                    option { value: "", "What does it concern? (optional)" }
                    option { value: "Accuracy", "Accuracy" }
                    option { value: "Privacy", "Privacy" }
                    option { value: "Harassment", "Harassment" }
                    option { value: "Children", "Children" }
                    option { value: "Discrimination", "Discrimination" }
                    option { value: "Right of reply", "Right of reply" }
                    option { value: "Other", "Other" }
                }
            }
            textarea { class: "cf-in cf-body", rows: "5", placeholder: "What do you believe is inaccurate or unfair?", value: "{body}", oninput: move |e| body.set(e.value()), required: true }
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

/// `/complaints/:slug` — the per-article complaint entry point (linked from each
/// article). Pre-fills the subject and explains the IMPRESS process.
#[component]
pub fn ComplaintPage(slug: String) -> Element {
    rsx! {
        document::Title { "Make a complaint · Predator Hunters" }
        section { class: "wrap", style: "padding:56px 0;max-width:700px;",
            p { class: "eyebrow", "Complaints" }
            h1 { "Make a complaint about this article" }
            p { class: "lead",
                "Tell us what's wrong — an inaccuracy, a privacy concern, or another breach of the "
                Link { to: crate::app::Route::Standards {}, "IMPRESS Standards Code" }
                ". We acknowledge complaints promptly and aim to give a final response within 21 days. "
                if crate::components::regulator_registered() {
                    "If you're unhappy with that response you can refer the matter to IMPRESS, our independent regulator."
                } else {
                    "If you're unhappy with that response, tell us why and we'll look at it again. We intend to seek IMPRESS registration; if we are registered you'll be able to escalate an unresolved complaint to them."
                }
            }
            ComplaintForm { slug }
        }
    }
}
