//! /team — the public editorial team, built from the live staff list (display
//! name + role only; the system admin account is excluded server-side). Populates
//! automatically as an admin adds staff in /desk → Staff. Client-fetched.

use dioxus::prelude::*;

use crate::api::public_team;

fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => "Editor-in-chief",
        "editor" => "Editor",
        "legal" => "Legal reviewer",
        "sub_editor" => "Sub-editor",
        "writer" => "Reporter",
        _ => "Editorial",
    }
}

#[component]
pub fn Team() -> Element {
    let team = use_resource(move || async move { public_team().await });
    let g = team.read();
    let members = match g.as_ref() {
        Some(Ok(v)) => v.clone(),
        _ => Vec::new(),
    };

    rsx! {
        crate::components::Seo {
            title: "Our team | Predator Hunters",
            description: "The journalists and editors behind Predator Hunters — independent local reporting, court coverage and investigations.",
            path: "/team",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "Who we are" }
                h1 { class: "rise d2", "Our " span { class: "grad-text", "team." } }
                p { class: "lede rise d3",
                    "The journalists and editors behind our reporting. Our sources stay anonymous; the people who stand behind the work do not."
                }
            }
        }
        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                if members.is_empty() {
                    p { class: "prose", style: "color:var(--muted);", "Our editorial team will be listed here." }
                }
                div { class: "grid-3",
                    for m in members {
                        div { key: "{m.display_name}", class: "card reveal",
                            h3 { "{m.display_name}" }
                            p { class: "kicker", "{role_label(&m.role)}" }
                        }
                    }
                }
            }
        }
    }
}
