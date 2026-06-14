//! Home — the front door of the main site. Mission + what we do + the database,
//! news, and media. Plain, human copy (no em-dashes), framed like the research
//! About page: frontline decoy work + court reporting, post-conviction.

use dioxus::prelude::*;

use crate::app::Route;
use crate::content::ARTICLES;
use crate::icons::svg;

/// (icon, title, description, route-label)
const PILLARS: [(&str, &str, &str); 3] = [
    (
        "scan",
        "We catch them",
        "We run online decoy operations, posing as children to find the adults who go looking for them. When it is safe, we confront them and hold them for the police with everything we have gathered.",
    ),
    (
        "doc",
        "We report it",
        "Once a case has been to court, we report it from the public record. We never name anyone before they are charged, and we hold footage back until there is a conviction.",
    ),
    (
        "scale",
        "We keep the record",
        "Our public database lets you look up convicted offenders by name, area and offence, drawn from the court record, so a community can see what the courts have decided.",
    ),
];

#[component]
pub fn Home() -> Element {
    rsx! {
        crate::components::Seo {
            title: "Predator Hunters — exposing predators, protecting children",
            description: "Independent child-protection and court-reporting journalism since 2017. Online decoy operations, court reporting, and a public database of convicted offenders drawn from the public record.",
            path: "/",
            image: "/og.png",
        }

        // ---------- HERO ----------
        header { class: "hero",
            div { class: "wrap",
                div { class: "hero-grid",
                    div {
                        div { class: "hero-eyebrow rise d1",
                            span { class: "dot" }
                            span { "On the front line since 2017" }
                        }
                        h1 { class: "rise d2",
                            "We catch predators. "
                            span { class: "grad-text", "We protect children." }
                        }
                        p { class: "hero-lede rise d3",
                            "Predator Hunters is an independent team that finds the adults who prey on children, hands the evidence to the police, and reports the cases once they have been to court. Nothing is named before a charge."
                        }
                        div { class: "hero-actions rise d4",
                            Link { class: "btn btn-primary", to: Route::News {},
                                "Latest news"
                                span { dangerous_inner_html: svg("arrow-right") }
                            }
                            Link { class: "btn btn-ghost", to: Route::Database {},
                                span { class: "ic", dangerous_inner_html: svg("scale") }
                                "Search the database"
                            }
                        }
                    }
                    div { class: "rise d4",
                        div { class: "readout",
                            div { class: "ro-scan" }
                            div { class: "readout-bar",
                                span { class: "tl", i {} i {} i {} }
                                b { "public record · court-sourced" }
                            }
                            div { class: "readout-body",
                                div { class: "ro-row", span { class: "ro-k", "decoy operations" } span { class: "ro-v good", span { class: "live" } "active" } }
                                div { class: "ro-row", span { class: "ro-k", "named before charge" } span { class: "ro-v", "never" } }
                                div { class: "ro-row", span { class: "ro-k", "footage before conviction" } span { class: "ro-v", "never" } }
                                div { class: "ro-row", span { class: "ro-k", "works with" } span { class: "ro-v", "the police" } }
                                div { class: "ro-row", span { class: "ro-k", "reporting" } span { class: "ro-v", "post-conviction" } }
                            }
                        }
                    }
                }
                dl { class: "hero-meta rise d5",
                    div { dt { "On the front line since" } dd { "2017" } }
                    div { dt { "We name anyone" } dd { "After charge" } }
                    div { dt { "We report" } dd { "Post-conviction" } }
                    div { dt { "We work with" } dd { "The police" } }
                }
            }
        }

        // ---------- LATEST NEWS ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Latest" }
                    h2 { "From the newsroom." }
                    p { class: "lede", "Court reporting, investigations and explainers, checked against the public record." }
                }
                div { class: "research-list",
                    for a in ARTICLES.iter() {
                        Link { key: "{a.slug}", class: "r-row reveal", to: Route::Article { slug: a.slug.to_string() },
                            span { class: "r-num", "{a.kind}" }
                            div { class: "r-title",
                                span { class: "r-ic", dangerous_inner_html: svg("doc") }
                                h3 { "{a.title}" }
                            }
                            p { class: "r-desc", "{a.summary}" }
                            div { style: "display:flex; align-items:center; gap:14px; justify-content:flex-end;",
                                span { style: "font-family:var(--mono); font-size:.68rem; letter-spacing:.12em; text-transform:uppercase; color:var(--muted);", "{a.date}" }
                                span { class: "r-arrow", dangerous_inner_html: svg("arrow-up-right") }
                            }
                        }
                    }
                }
                div { style: "margin-top:24px;",
                    Link { class: "btn btn-ghost", to: Route::News {},
                        "All news"
                        span { class: "ic", dangerous_inner_html: svg("arrow-right") }
                    }
                }
            }
        }

        // ---------- WHAT WE DO ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "What we do" }
                    h2 { "Catch, report, and keep the record." }
                }
                div { class: "grid-3",
                    for (icon , title , desc) in PILLARS {
                        div { key: "{title}", class: "card reveal",
                            div { class: "card-ic", dangerous_inner_html: svg(icon) }
                            h3 { "{title}" }
                            p { "{desc}" }
                        }
                    }
                }
            }
        }

        // ---------- DATABASE TEASER ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "statement-grid",
                    h2 { class: "statement reveal",
                        "A public record, "
                        span { class: "grad-text", "drawn from the courts." }
                    }
                    div { class: "statement-aside reveal",
                        p {
                            "Our conviction database lets anyone look up offenders who have been through the courts, by name, area or offence. Every entry comes from the public court record, only after a conviction, and can be corrected."
                        }
                        div { style: "margin-top:18px;",
                            Link { class: "btn btn-ghost", to: Route::Database {},
                                span { class: "ic", dangerous_inner_html: svg("scale") }
                                "Open the database"
                            }
                        }
                    }
                }
            }
        }

        // ---------- MEDIA ----------
        section { class: "section",
            div { class: "wrap",
                div { class: "sec-head",
                    span { class: "sec-index", "Watch + listen" }
                    h2 { "See the work for yourself." }
                    p { class: "lede", "Investigations, court reports and conversations, on video and on the podcast." }
                }
                div { class: "grid-2",
                    Link { class: "card reveal", to: Route::Watch {},
                        div { class: "card-ic", dangerous_inner_html: svg("camera") }
                        h3 { "Watch" }
                        p { "Investigations and court reports on video." }
                    }
                    Link { class: "card reveal", to: Route::Podcast {},
                        div { class: "card-ic", dangerous_inner_html: svg("waveform") }
                        h3 { "The podcast" }
                        p { "The stories behind the cases, in conversation." }
                    }
                }
            }
        }
    }
}
