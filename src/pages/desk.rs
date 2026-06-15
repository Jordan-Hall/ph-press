//! `/desk` — the staff editorial console. Deliberately NOT at `/admin`; it is
//! noindex, unlinked from the public site, and absent from the sitemap. Renders a
//! login form until authenticated, then a read-only editorial dashboard listing
//! every article and its lifecycle state. Lifecycle actions (submit/review/
//! publish) land in a later increment; this is the auth backbone + dashboard.

use dioxus::prelude::*;

use crate::api::{desk_articles, staff_login, staff_logout, staff_me, DeskArticle, DeskSession};

/// Auth state for the console shell.
#[derive(Clone, PartialEq)]
enum Auth {
    Loading,
    Out,
    In(DeskSession),
}

#[component]
pub fn Desk() -> Element {
    let mut auth = use_signal(|| Auth::Loading);

    // Resolve the current session once on load (server reads the HttpOnly cookie).
    use_resource(move || async move {
        let me = staff_me().await.ok().flatten();
        auth.set(me.map_or(Auth::Out, Auth::In));
    });

    let state = auth.read().clone();
    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Editorial desk · Predator Hunters" }
        div { class: "desk-root",
            match state {
                Auth::Loading => rsx! { div { class: "desk-loading", "Loading the desk…" } },
                Auth::Out => rsx! { DeskLogin { auth } },
                Auth::In(user) => rsx! { DeskDashboard { user, auth } },
            }
        }
    }
}

#[component]
fn DeskLogin(auth: Signal<Auth>) -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            error.set(None);
            match staff_login(username(), password()).await {
                Ok(user) => auth.set(Auth::In(user)),
                Err(_) => error.set(Some("Invalid username or password.".to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "desk-login",
            form { class: "desk-card", onsubmit: submit,
                p { class: "desk-eyebrow", "Predator Hunters" }
                h1 { class: "desk-title", "Editorial desk" }
                p { class: "desk-sub", "Sign in to manage the newsroom." }
                label { class: "desk-field",
                    span { "Username" }
                    input {
                        r#type: "text",
                        autocomplete: "username",
                        value: "{username}",
                        oninput: move |e| username.set(e.value()),
                        autofocus: true,
                    }
                }
                label { class: "desk-field",
                    span { "Password" }
                    input {
                        r#type: "password",
                        autocomplete: "current-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }
                if let Some(msg) = error() {
                    p { class: "desk-error", "{msg}" }
                }
                button { class: "desk-btn", r#type: "submit", disabled: busy(),
                    if busy() { "Signing in…" } else { "Sign in" }
                }
            }
        }
    }
}

#[component]
fn DeskDashboard(user: DeskSession, auth: Signal<Auth>) -> Element {
    let articles = use_resource(move || async move { desk_articles().await.unwrap_or_default() });

    let logout = move |_| {
        spawn(async move {
            let _ = staff_logout().await;
            auth.set(Auth::Out);
        });
    };

    let rows = articles.read().clone();
    rsx! {
        header { class: "desk-top",
            div { class: "desk-top-in",
                div {
                    p { class: "desk-eyebrow", "Editorial desk" }
                    h1 { class: "desk-h1", "Welcome, {user.display_name}" }
                }
                div { class: "desk-top-right",
                    span { class: "desk-role", "{role_label(&user.role)}" }
                    button { class: "desk-btn ghost", onclick: logout, "Sign out" }
                }
            }
        }
        main { class: "desk-main",
            section { class: "desk-panel",
                div { class: "desk-panel-head",
                    h2 { "Articles" }
                    match &rows {
                        Some(v) => rsx! { span { class: "desk-count", "{v.len()} total" } },
                        None => rsx! {},
                    }
                }
                match rows {
                    None => rsx! { p { class: "desk-muted", "Loading articles…" } },
                    Some(v) if v.is_empty() => rsx! { p { class: "desk-muted", "No articles yet." } },
                    Some(v) => rsx! {
                        table { class: "desk-table",
                            thead {
                                tr {
                                    th { "Title" }
                                    th { "State" }
                                    th { "Kind" }
                                    th { "Byline" }
                                    th { "Updated" }
                                }
                            }
                            tbody {
                                for a in v {
                                    DeskRow { key: "{a.id}", a }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn DeskRow(a: DeskArticle) -> Element {
    rsx! {
        tr {
            td {
                span { class: "desk-row-title", "{a.title}" }
                if a.is_ai_assisted {
                    span { class: "desk-tag ai", "AI-assisted" }
                }
            }
            td {
                span { class: "desk-state s-{a.state}", "{state_label(&a.state)}" }
            }
            td { class: "desk-muted", "{a.kind}" }
            td { class: "desk-muted", "{a.byline}" }
            td { class: "desk-muted", "{ymd(a.updated_at)}" }
        }
    }
}

fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => "Admin",
        "editor" => "Editor",
        "legal" => "Legal reviewer",
        "sub_editor" => "Sub-editor",
        "writer" => "Writer",
        _ => "Staff",
    }
}

fn state_label(state: &str) -> &str {
    match state {
        "draft" => "Draft",
        "submitted" => "Submitted",
        "editorial_review" => "Editorial review",
        "legal_review" => "Legal review",
        "scheduled" => "Scheduled",
        "published" => "Published",
        "corrected" => "Corrected",
        "retracted" => "Retracted",
        _ => state,
    }
}

/// Unix seconds → "YYYY-MM-DD" (civil_from_days; no chrono dependency).
fn ymd(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
