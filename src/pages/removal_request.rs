//! Removal request — public form for requesting removal of a conviction-database
//! entry under the Rehabilitation of Offenders Act, for factual inaccuracy, or
//! for disproportionate / no-longer-justified harm.

use dioxus::prelude::*;

use crate::api::submit_removal_request;
use crate::icons::svg;

#[component]
pub fn RemovalRequest() -> Element {
    let mut target_ref = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut reason = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut reference = use_signal(|| Option::<String>::None);
    let mut err = use_signal(|| Option::<String>::None);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match submit_removal_request(target_ref(), name(), email(), reason()).await {
                Ok(r) => reference.set(Some(r)),
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    if let Some(r) = reference() {
        return rsx! {
            document::Title { "Removal request submitted \u{00b7} Predator Hunters" }
            section { class: "wrap", style: "padding:56px 0; max-width:700px;",
                div { class: "card reveal",
                    div { class: "card-ic", dangerous_inner_html: svg("check") }
                    h3 { "Thank you \u{2014} your request has been received" }
                    p { {format!("Your reference is {r}. We will review this request and respond within 21 days.")} }
                    p { "We do not auto-delete anything. Every request is reviewed by a member of staff against the criteria below before any action is taken." }
                }
            }
        };
    }

    rsx! {
        crate::components::Seo {
            title: "Request removal from our conviction database | Predator Hunters",
            description: "You can request that an entry in our conviction database be reviewed for removal under the Rehabilitation of Offenders Act, for factual inaccuracy, or for disproportionate harm. We review every request.",
            path: "/removal-request",
            image: "/og.png",
        }

        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Conviction database" }
                h1 { class: "rise d2",
                    "Request removal from "
                    span { class: "grad-text", "the database." }
                }
                p { class: "lede rise d3",
                    "If you believe an entry should be removed, tell us why. We review every request against the criteria below before any action is taken. Nothing is auto-deleted."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Criteria" } h2 { "When we will consider removal." } }
                div { class: "grid-3 reveal",
                    div { class: "card",
                        div { class: "card-ic", dangerous_inner_html: svg("scale") }
                        h3 { "Spent conviction (ROA)" }
                        p { "If the conviction is spent under the Rehabilitation of Offenders Act 1974 and there is no overriding public interest in keeping it on the record, we will consider removing it." }
                    }
                    div { class: "card",
                        div { class: "card-ic", dangerous_inner_html: svg("doc") }
                        h3 { "Factual inaccuracy" }
                        p { "If the entry contains a material factual error \u{2014} the name, offence, date or outcome is wrong \u{2014} we will correct or remove it." }
                    }
                    div { class: "card",
                        div { class: "card-ic", dangerous_inner_html: svg("eye-off") }
                        h3 { "Disproportionate harm" }
                        p { "If keeping the entry causes harm that is clearly disproportionate to any public interest \u{2014} for example, if the offence was minor and time has passed \u{2014} we will weigh it carefully." }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Submit a request" } h2 { "Tell us what needs reviewing." } }
                div { class: "prose reveal", style: "margin-bottom:24px;",
                    p { "We will review your request against the criteria above and respond within 21 days. We do not auto-delete anything \u{2014} every request is checked by a member of staff." }
                }
                form { class: "complaint-form reveal", onsubmit: submit,
                    div { class: "cf-row",
                        input {
                            class: "cf-in",
                            r#type: "text",
                            placeholder: "Entry to remove (the person's name or article title)",
                            value: "{target_ref}",
                            oninput: move |e| target_ref.set(e.value()),
                            required: true,
                        }
                    }
                    div { class: "cf-row",
                        input {
                            class: "cf-in",
                            r#type: "text",
                            placeholder: "Your name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                            required: true,
                        }
                        input {
                            class: "cf-in",
                            r#type: "email",
                            placeholder: "Your email (so we can respond)",
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                            required: true,
                        }
                    }
                    textarea {
                        class: "cf-in cf-body",
                        rows: "6",
                        placeholder: "Explain why you believe the entry should be removed, and which of the criteria above applies.",
                        value: "{reason}",
                        oninput: move |e| reason.set(e.value()),
                        required: true,
                    }
                    if let Some(e) = err() {
                        p { class: "cf-err", "{e}" }
                    }
                    div { class: "cf-actions",
                        button {
                            class: "btn btn-primary",
                            r#type: "submit",
                            disabled: busy(),
                            if busy() { "Sending\u{2026}" } else { "Submit request" }
                        }
                        a {
                            class: "btn btn-ghost",
                            href: "mailto:database@predatorhunters.co.uk?subject=Removal%20request",
                            "Or email us instead"
                        }
                    }
                }
            }
        }

        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head", span { class: "sec-index", "Process" } h2 { "What happens next." } }
                dl { class: "deflist reveal",
                    div { class: "def", dt { "Review" } dd { "A member of staff checks your request against the criteria above and the public court record." } }
                    div { class: "def", dt { "Decision" } dd { "We aim to give you a decision within 21 days. If we uphold your request, the entry is hidden from the public database \u{2014} the underlying record is kept for our audit trail but not shown publicly." } }
                    div { class: "def", dt { "Nothing is auto-deleted" } dd { "Every request goes through a human review. We do not auto-delete anything, and all decisions are logged." } }
                    div { class: "def", dt { "If you disagree" } dd { "If you are not satisfied with our decision, you can contact our independent press regulator." } }
                }
            }
        }
    }
}
