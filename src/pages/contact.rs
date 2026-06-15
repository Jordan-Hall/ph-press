//! Contact — the ways to reach us: tips, press, safeguarding, complaints.

use dioxus::prelude::*;

use crate::icons::svg;

use crate::config;

/// (icon, title, description, email, mail-subject). The email + href are derived
/// from the white-label config so a fork rebrands contact in one place.
const LANES: [(&str, &str, &str, &str, &str); 4] = [
    (
        "shield",
        "Report a predator / a tip",
        "If you have information about someone targeting children, contact us in confidence. If a child is in immediate danger, call 999.",
        config::TIPS_EMAIL,
        "Tip",
    ),
    (
        "doc",
        "Press & media",
        "Journalists, broadcasters and researchers: for interviews, footage requests and court-reporting queries.",
        config::PRESS_EMAIL,
        "Press",
    ),
    (
        "shield-check",
        "Safeguarding partners",
        "Schools, charities and platforms working to keep children safe and wanting to work with us.",
        config::PRESS_EMAIL,
        "Partnership",
    ),
    (
        "scale",
        "Complaints & corrections",
        "Think we got something wrong, or that a database entry needs checking? Tell us and we will review it against the record.",
        config::COMPLAINTS_EMAIL,
        "Complaint",
    ),
];

#[component]
pub fn Contact() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Contact | Predator Hunters",
            description: "Reach Predator Hunters: tips, press and media, safeguarding partnerships, and complaints or corrections.",
            path: "/contact",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Contact" }
                h1 { class: "rise d2",
                    "Get in "
                    span { class: "grad-text", "touch." }
                }
                p { class: "lede rise d3",
                    "Whatever you bring, a tip, a story, support or a concern, start here. If a child is in immediate danger, call 999."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "grid-2",
                    for (icon , title , desc , email , subject) in LANES {
                        div { key: "{title}", class: "card reveal",
                            div { class: "card-ic", dangerous_inner_html: svg(icon) }
                            h3 { "{title}" }
                            p { "{desc}" }
                            a { class: "btn btn-ghost btn-sm", style: "margin-top:16px;", href: "mailto:{email}?subject={subject}",
                                span { class: "ic", dangerous_inner_html: svg("mail") }
                                "{email}"
                            }
                        }
                    }
                }
            }
        }
    }
}
