//! Governance & transparency — IMPRESS-readiness policies covering ownership,
//! conflicts of interest, and whistleblower / source protection.

use dioxus::prelude::*;

use crate::icons::svg;

/// (term, definition) — ownership & funding facts.
const OWNERSHIP: [(&str, &str); 6] = [
    ("Who we are", "Predator Hunters is an independent publisher reporting since 2022. It is owned and operated by Jordan Upton as a sole trader, trading as Predator Hunters."),
    ("Editors-in-chief", "The publication has two editors-in-chief: Jordan Upton and Scott Taylor. They are jointly responsible for all editorial decisions."),
    ("How we are funded", "The publication is self-funded by Jordan Upton. We receive no public or charitable grants and no external funding, and we do not carry advertising that targets our editorial decisions."),
    ("External investment", "We have no external investors, loans, or institutional funders."),
    ("No payment from subjects", "We do not accept payment, gifts, hospitality, or any other benefit from the subjects of our coverage, from people seeking to influence coverage, or from parties with an interest in the outcome of a story. Acceptance of any such benefit is grounds for immediate disciplinary action."),
    ("How to contact us", "Predator Hunters operates as a sole trader and has no registered company address. You can reach us through our contact page, by email at press@predatorhunters.co.uk, or in confidence at confidential@predatorhunters.co.uk."),
];

/// (term, definition) — conflicts of interest policy.
const CONFLICTS: [(&str, &str); 5] = [
    ("Declaration", "Staff and contributors must declare to an editor any personal, financial, or professional interest that could affect — or be seen to affect — their reporting on a story. Declarations are recorded."),
    ("Recusal", "Anyone with a declared conflict does not report on, edit, or make decisions about the affected story or subject. The work is reassigned."),
    ("Financial interests", "Staff and contributors do not hold financial positions — shares, employment, or paid directorships — in organisations they cover without prior disclosure. Where a disclosed interest is unavoidable, it is stated in the relevant content."),
    ("Personal relationships", "Staff and contributors do not report on individuals with whom they have a close personal or family relationship without prior disclosure and editorial approval. Where approved, the relationship is stated."),
    ("Gifts and hospitality", "We do not accept gifts, free travel, or hospitality from sources or subjects of coverage beyond items of token value (under \u{00a3}20). Any borderline acceptance is disclosed to an editor and logged."),
];

#[component]
pub fn Governance() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Governance & transparency | Predator Hunters",
            description: "Predator Hunters governance: ownership and funding, conflicts of interest policy, and whistleblowers charter with source protection.",
            path: "/governance",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Governance" }
                h1 { class: "rise d2",
                    "Who we are, how we are funded, and "
                    span { class: "grad-text", "how we stay honest." }
                }
                p { class: "lede rise d3",
                    "These policies sit alongside our "
                    Link { to: crate::app::Route::Standards {}, "editorial standards and complaints process" }
                    ". Together they set out what we stand for, who owns us, how conflicts are managed, and how we protect those who trust us with sensitive information."
                }
            }
        }

        // ---------- OWNERSHIP & FUNDING ----------
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Ownership & funding" }
                    h2 { "Who owns and funds this publication." }
                }
                div { class: "prose reveal",
                    p { "We believe readers have a right to know who is behind what they read, how a publication sustains itself, and whether any financial relationship could influence what is covered. The information below answers those questions." }
                }
                dl { class: "deflist reveal", style: "margin-top:18px;",
                    for (term , def) in OWNERSHIP {
                        div { key: "{term}", class: "def",
                            dt { "{term}" }
                            dd { "{def}" }
                        }
                    }
                }
            }
        }

        // ---------- CONFLICTS OF INTEREST ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Conflicts of interest" }
                    h2 { "How we declare and manage conflicts." }
                }
                div { class: "prose reveal",
                    p { "A conflict of interest arises whenever a personal, financial, or professional connection could influence — or appear to influence — how a story is reported. We take conflicts seriously because reader trust depends on our independence being real and visible." }
                }
                dl { class: "deflist reveal", style: "margin-top:18px;",
                    for (term , def) in CONFLICTS {
                        div { key: "{term}", class: "def",
                            dt { "{term}" }
                            dd { "{def}" }
                        }
                    }
                }
                div { class: "card reveal", style: "margin-top:24px; max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("scale") }
                    h3 { "Editorial independence in practice" }
                    p { "No advertiser, funder, public body, or external party has ever been given the right to approve, delay, or suppress a story. That is not something we will negotiate." }
                }
            }
        }

        // ---------- WHISTLEBLOWERS' CHARTER & SOURCE PROTECTION ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Source protection" }
                    h2 { "Whistleblowers and confidential sources." }
                }
                div { class: "prose reveal",
                    p { "The ability of a free press to hold power to account depends on people feeling safe to come forward. We take source protection seriously and apply it consistently." }
                }
                div { class: "grid-2",
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("shield") }
                        h3 { "We protect your identity" }
                        p { "We do not reveal the identity of a confidential source to anyone without the source's explicit consent. We will not confirm or deny that a specific person has spoken to us." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("shield-check") }
                        h3 { "We resist disclosure" }
                        p { "If a court or authority seeks to compel us to disclose a confidential source, we will resist disclosure to the fullest extent the law permits and take independent legal advice before complying with any such demand." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("doc") }
                        h3 { "We handle material securely" }
                        p { "Documents, files, and communications provided in confidence are stored securely, with access limited to those who need it. We delete or destroy material when it is no longer needed for the story and retaining it creates unnecessary risk." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", dangerous_inner_html: svg("check") }
                        h3 { "Conscience and retaliation" }
                        p { "No one who works with us will be required to act against this code or against their conscience, and no one will be penalised or pressured for refusing to. Whistleblowers within our organisation can raise concerns safely and in confidence." }
                    }
                }
                div { class: "prose reveal", style: "margin-top:32px;",
                    p {
                        "If you are a whistleblower or have information about serious wrongdoing, contact us in confidence at "
                        a {
                            href: "mailto:confidential@predatorhunters.co.uk",
                            style: "color:var(--green-2); text-decoration:underline; text-underline-offset:3px;",
                            "confidential@predatorhunters.co.uk"
                        }
                        ". We treat every message to this address with the strictest confidence."
                    }
                    p { "We will not report on a source or whistleblower as a subject of a story without their informed consent." }
                }
            }
        }

        // ---------- POLICY REVIEW ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "card reveal", style: "max-width:680px;",
                    div { class: "card-ic", dangerous_inner_html: svg("doc") }
                    h3 { "Policy review" }
                    p { "These policies are reviewed at least annually and whenever there is a material change in our funding, ownership, or editorial structure. Last reviewed June 2026." }
                    p { style: "margin-top:12px;",
                        "Questions about any of this can be sent to "
                        a {
                            href: "mailto:press@predatorhunters.co.uk",
                            style: "color:var(--green-2); text-decoration:underline; text-underline-offset:3px;",
                            "press@predatorhunters.co.uk"
                        }
                        "."
                    }
                }
            }
        }
    }
}
