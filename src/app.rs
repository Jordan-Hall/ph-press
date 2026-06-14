//! Router wiring + persistent shell for PH Press (the main public site).
//! Dioxus 0.8 `dioxus-router`: a `Routable` enum, one `#[layout(Shell)]` painting
//! the chrome (nav + footer) around every `Outlet`. Web build is wasm + SSG.

use dioxus::prelude::*;

use crate::assets::{FAVICON, PH_LOGO};
use crate::components::{ClosingCta, SiteFooter};
use crate::icons::svg;
use crate::pages::{
    About, Article, Cases, Contact, Database, Home, News, NotFound, Podcast, Privacy, Standards,
    Watch,
};

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(Shell)]
    #[route("/")]
    Home {},
    #[route("/news")]
    News {},
    #[route("/news/:slug")]
    Article { slug: String },
    #[route("/database")]
    Database {},
    #[route("/cases")]
    Cases {},
    #[route("/watch")]
    Watch {},
    #[route("/podcast")]
    Podcast {},
    #[route("/about")]
    About {},
    #[route("/standards")]
    Standards {},
    #[route("/contact")]
    Contact {},
    #[route("/privacy")]
    Privacy {},
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// SSG hook: `dx build --ssg` POSTs to `/api/static_routes` and pre-renders each
/// path it returns, so crawlers / link bots / no-JS clients get full HTML.
#[server(endpoint = "static_routes")]
async fn static_routes() -> Result<Vec<String>, ServerFnError> {
    // Non-dynamic routes from the enum, PLUS each published article so the
    // /news/:slug pages pre-render to real HTML (crawlable — essential for a
    // newsroom). The catch-all NotFound is dynamic and skipped automatically.
    let mut routes: Vec<String> =
        Route::static_routes().iter().map(ToString::to_string).collect();
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
            div { class: "stage-bg" }
            div { class: "stage-grid" }
            div { class: "stage-grain" }
            Router::<Route> {}
        }
    }
}

#[component]
fn Shell() -> Element {
    rsx! {
        NavBar {}
        main { id: "main", tabindex: "-1", Outlet::<Route> {} }
        ClosingCta {}
        SiteFooter {}
    }
}

fn nav_class(current: &Route, target: &Route) -> &'static str {
    if current == target { "nav-link on" } else { "nav-link" }
}

fn nav_aria(current: &Route, target: &Route) -> Option<&'static str> {
    if current == target { Some("page") } else { None }
}

#[component]
fn NavBar() -> Element {
    let route = use_route::<Route>();
    let mut menu = use_signal(|| false);
    let burger_icon = if menu() { "close" } else { "menu" };
    let burger_label = if menu() { "Close menu" } else { "Open menu" };
    rsx! {
        nav {
            class: "nav",
            onkeydown: move |e| {
                if e.key().to_string() == "Escape" { menu.set(false); }
            },
            div { class: "nav-inner",
                Link { class: "brand", to: Route::Home {}, onclick: move |_| menu.set(false),
                    img { class: "brand-logo", src: PH_LOGO, alt: "Predator Hunters", width: "500", height: "168" }
                    span { class: "brand-tag", "Predator Hunters" }
                }
                div { class: "nav-links",
                    Link { class: nav_class(&route, &Route::News {}), "aria-current": nav_aria(&route, &Route::News {}), to: Route::News {}, "News" }
                    Link { class: nav_class(&route, &Route::Database {}), "aria-current": nav_aria(&route, &Route::Database {}), to: Route::Database {}, "Database" }
                    Link { class: nav_class(&route, &Route::Cases {}), "aria-current": nav_aria(&route, &Route::Cases {}), to: Route::Cases {}, "Cases" }
                    Link { class: nav_class(&route, &Route::Watch {}), "aria-current": nav_aria(&route, &Route::Watch {}), to: Route::Watch {}, "Watch" }
                    Link { class: nav_class(&route, &Route::About {}), "aria-current": nav_aria(&route, &Route::About {}), to: Route::About {}, "About" }
                }
                div { class: "nav-right",
                    button {
                        class: "theme-toggle",
                        "aria-label": "Switch between light and dark theme",
                        onclick: move |_| {
                            let _ = dioxus::document::eval("var h=document.documentElement;var t=h.getAttribute('data-theme')==='light'?'dark':'light';h.setAttribute('data-theme',t);try{localStorage.setItem('ph-theme',t);}catch(e){}");
                        },
                        span { class: "ic-sun", dangerous_inner_html: svg("sun") }
                        span { class: "ic-moon", dangerous_inner_html: svg("moon") }
                    }
                    Link { class: "btn btn-primary btn-sm nav-cta", to: Route::Contact {},
                        "Get in touch"
                        span { dangerous_inner_html: svg("arrow-right") }
                    }
                    button {
                        class: "theme-toggle nav-burger",
                        "aria-label": burger_label,
                        "aria-expanded": "{menu()}",
                        "aria-controls": "nav-menu",
                        onclick: move |_| { let v = menu(); menu.set(!v); },
                        span { dangerous_inner_html: svg(burger_icon) }
                    }
                }
            }
            if menu() {
                div { class: "nav-menu", id: "nav-menu",
                    Link { class: nav_class(&route, &Route::News {}), to: Route::News {}, onclick: move |_| menu.set(false), "News" }
                    Link { class: nav_class(&route, &Route::Database {}), to: Route::Database {}, onclick: move |_| menu.set(false), "Database" }
                    Link { class: nav_class(&route, &Route::Cases {}), to: Route::Cases {}, onclick: move |_| menu.set(false), "Cases" }
                    Link { class: nav_class(&route, &Route::Watch {}), to: Route::Watch {}, onclick: move |_| menu.set(false), "Watch" }
                    Link { class: nav_class(&route, &Route::Podcast {}), to: Route::Podcast {}, onclick: move |_| menu.set(false), "Podcast" }
                    Link { class: nav_class(&route, &Route::About {}), to: Route::About {}, onclick: move |_| menu.set(false), "About" }
                    Link { class: nav_class(&route, &Route::Standards {}), to: Route::Standards {}, onclick: move |_| menu.set(false), "Standards" }
                    Link { class: "btn btn-primary nav-menu-cta", to: Route::Contact {}, onclick: move |_| menu.set(false),
                        "Get in touch"
                        span { dangerous_inner_html: svg("arrow-right") }
                    }
                }
            }
        }
    }
}
