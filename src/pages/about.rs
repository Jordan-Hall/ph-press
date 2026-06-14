//! About — who we are and where we came from, framed like the research site's
//! About: decoy origin, court reporting, post-conviction, the lines we hold.

use dioxus::prelude::*;

use crate::app::Route;
use crate::icons::svg;

/// (year, event)
const TIMELINE: [(&str, &str); 4] = [
    ("2017", "Predator Hunters begins as an online decoy operation. We find the adults who go looking for children, hand the evidence to the police, and teach parents what to watch for."),
    ("2020", "Our court reporting grows. We start keeping a careful, public record of cases once they have concluded, drawn from the court record."),
    ("2022", "The research lab opens, turning years of frontline experience into privacy-first tools that protect children on their own devices."),
    ("Today", "A smaller frontline team still runs the decoy work. Most of our effort now goes into reporting, the public record, and building the tools, and we are working towards independent press regulation."),
];

#[component]
pub fn About() -> Element {
    rsx! {
        crate::components::Seo {
            title: "About: an independent child-protection team since 2017 | Predator Hunters",
            description: "Predator Hunters is an independent child-protection and court-reporting team. We run online decoy operations, work with the police, and report on cases once they have been to court.",
            path: "/about",
            image: "/og.png",
        }
        header { class: "page-head",
            div { class: "wrap",
                p { class: "eyebrow rise d1", "About" }
                h1 { class: "rise d2",
                    "An independent team on the front line since "
                    span { class: "grad-text", "2017." }
                }
                p { class: "lede rise d3",
                    "We are small, self-funded, and have spent the better part of a decade protecting children online and reporting the people who harm them, from the public court record."
                }
            }
        }

        section { class: "section", style: "padding-top:clamp(20px,4vh,48px);",
            div { class: "wrap",
                div { class: "prose reveal",
                    p {
                        "We have been at this for nearly ten years. It started on the front line, with decoy operations. Posing as children online to find the adults who go looking for them, gathering the evidence, confronting them when it is safe to do so, and holding them for the police. A smaller team still does that work today. It is careful, draining work, and it taught us how grooming actually unfolds."
                    }
                    p {
                        "Out of that came the rest of what we do. We report on cases once they have been to court, we keep a public record of convictions drawn from the court record, and we build "
                        strong { "privacy-first" }
                        " tools to protect children. Two lines have never moved. We never name anyone before they are charged, and we hold any footage back until there is a conviction, censored where it is needed and shown only when it genuinely helps people keep children safe."
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
                        p { "A frontline child-protection team with years of real experience. We run online decoy operations to find adults who go looking for children, and when it is safe, we confront them and hold them for the police. We report on cases once they have been to court, and we keep a public record of convictions from the court record." }
                    }
                    div { class: "card reveal",
                        div { class: "card-ic", style: "color:var(--orange);background:rgba(245,130,32,.10);border-color:rgba(245,130,32,.22);", dangerous_inner_html: svg("eye-off") }
                        h3 { "What we are not" }
                        p { "We are not the police, not a surveillance company, and not in it for a show. We never name anyone before they are charged. We hold footage back until there is a conviction, censor it where needed, and only run it when it genuinely teaches people how to keep children safe. We work with the police, not in their place." }
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
