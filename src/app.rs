//! Router wiring + persistent shell for PH Press (the main public site).
//! Dioxus 0.8 `dioxus-router`: a `Routable` enum, one `#[layout(Shell)]` painting
//! the newsroom masthead + footer around every `Outlet`. Web build is wasm + SSG.

use dioxus::prelude::*;

use crate::assets::{FAVICON, PH_LOGO};
use crate::components::{ClosingCta, SiteFooter};
use crate::icons::svg;
use crate::pages::{
    About, Article, ComplaintPage, Contact, Database, Desk, DeskForgot, DeskPreview, DeskReset,
    Governance, Home, News, NotFound, Podcast, Privacy, Standards, Team, Watch, WriteArticle,
};

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    // Staff editorial console — declared BEFORE the public Shell layout so it
    // renders with its own chrome (no public masthead/footer). noindex + unlinked.
    #[route("/desk")]
    Desk {},
    #[route("/desk/preview/:id")]
    DeskPreview { id: i64 },
    #[route("/desk/edit/:id")]
    WriteArticle { id: i64 },
    #[route("/desk/forgot")]
    DeskForgot {},
    #[route("/desk/reset/:token")]
    DeskReset { token: String },
    #[layout(Shell)]
    #[route("/")]
    Home {},
    #[route("/news")]
    News {},
    #[route("/news/:slug")]
    Article { slug: String },
    #[route("/database")]
    Database {},
    #[route("/watch")]
    Watch {},
    #[route("/podcast")]
    Podcast {},
    #[route("/about")]
    About {},
    #[route("/team")]
    Team {},
    #[route("/standards")]
    Standards {},
    #[route("/complaints/:slug")]
    ComplaintPage { slug: String },
    #[route("/governance")]
    Governance {},
    #[route("/contact")]
    Contact {},
    #[route("/privacy")]
    Privacy {},
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// SSG hook: `dx build --ssg` pre-renders each path returned here, so crawlers /
/// link bots / no-JS clients get full HTML — essential for a newsroom.
#[server(endpoint = "static_routes")]
async fn static_routes() -> Result<Vec<String>, ServerFnError> {
    let mut routes: Vec<String> = Route::static_routes()
        .iter()
        .map(ToString::to_string)
        .collect();
    for a in crate::content::ARTICLES {
        routes.push(format!("/news/{}", a.slug));
    }
    Ok(routes)
}

#[component]
pub fn App() -> Element {
    rsx! {
        dioxus::document::Link { rel: "icon", href: FAVICON }
        div { class: "theme-root",
            a { class: "skip-link", href: "#main", "Skip to content" }
            Router::<Route> {}
        }
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        Masthead {}
        main { id: "main", tabindex: "-1", Outlet::<Route> {} }
        ClosingCta {}
        SiteFooter {}
    }
}

fn mh_class(current: &Route, target: &Route) -> &'static str {
    if current == target {
        "mh-link on"
    } else {
        "mh-link"
    }
}

fn mh_aria(current: &Route, target: &Route) -> Option<&'static str> {
    if current == target {
        Some("page")
    } else {
        None
    }
}

#[component]
fn Masthead() -> Element {
    let route = use_route::<Route>();
    rsx! {
        header { class: "masthead",
            div { class: "wrap",
                div { class: "mh-bar",
                    Link { class: "brand", to: Route::Home {},
                        img { class: "brand-logo", src: PH_LOGO, alt: crate::config::SITE_NAME, width: "500", height: "168" }
                        span { class: "brand-tag", {crate::config::TAGLINE} }
                    }
                    div { class: "mh-right",
                        button {
                            class: "theme-toggle",
                            "aria-label": "Switch between light and dark theme",
                            onclick: move |_| {
                                let _ = dioxus::document::eval("var h=document.documentElement;var t=h.getAttribute('data-theme')==='light'?'dark':'light';h.setAttribute('data-theme',t);try{localStorage.setItem('ph-theme',t);}catch(e){}");
                            },
                            span { class: "ic-sun", dangerous_inner_html: svg("sun") }
                            span { class: "ic-moon", dangerous_inner_html: svg("moon") }
                        }
                        Link { class: "btn btn-primary btn-sm mh-cta", to: Route::Contact {},
                            "Get in touch"
                        }
                    }
                }
                nav { class: "mh-nav", "aria-label": "Sections",
                    Link { class: mh_class(&route, &Route::News {}), "aria-current": mh_aria(&route, &Route::News {}), to: Route::News {}, "News" }
                    Link { class: mh_class(&route, &Route::Database {}), "aria-current": mh_aria(&route, &Route::Database {}), to: Route::Database {}, "Database" }
                    Link { class: mh_class(&route, &Route::Watch {}), "aria-current": mh_aria(&route, &Route::Watch {}), to: Route::Watch {}, "Watch" }
                    Link { class: mh_class(&route, &Route::Podcast {}), "aria-current": mh_aria(&route, &Route::Podcast {}), to: Route::Podcast {}, "Podcast" }
                    Link { class: mh_class(&route, &Route::About {}), "aria-current": mh_aria(&route, &Route::About {}), to: Route::About {}, "About" }
                    Link { class: mh_class(&route, &Route::Standards {}), "aria-current": mh_aria(&route, &Route::Standards {}), to: Route::Standards {}, "Standards" }
                }
            }
        }
    }
}
