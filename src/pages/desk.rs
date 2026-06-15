//! `/desk` — the staff editorial console. Deliberately NOT at `/admin`; it is
//! noindex, unlinked from the public site, and absent from the sitemap. Renders a
//! login form until authenticated, then the editorial dashboard: create drafts
//! and move articles through the role-gated lifecycle (the gate — publish only via
//! legal sign-off — is enforced server-side; the UI only shows allowed actions).

use dioxus::prelude::*;

use crate::api::{
    desk_add_correction, desk_articles, desk_complaint_status, desk_complaints, desk_corrections,
    desk_create, desk_log_complaint, desk_preview, desk_transition, staff_change_password,
    staff_login, staff_logout, staff_me, DeskArticle, DeskComplaint, DeskCorrection, DeskSession,
    PreviewArticle,
};
use crate::app::Route;

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

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Articles,
    Complaints,
    Corrections,
    Settings,
}

#[component]
fn DeskDashboard(user: DeskSession, auth: Signal<Auth>) -> Element {
    let mut tab = use_signal(|| Tab::Articles);

    let logout = move |_| {
        spawn(async move {
            let _ = staff_logout().await;
            auth.set(Auth::Out);
        });
    };

    let active = *tab.read();
    let tab_class = |t: Tab| {
        if active == t {
            "desk-tab on"
        } else {
            "desk-tab"
        }
    };
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
            nav { class: "desk-tabs", "aria-label": "Sections",
                button { class: tab_class(Tab::Articles), onclick: move |_| tab.set(Tab::Articles), "Articles" }
                button { class: tab_class(Tab::Complaints), onclick: move |_| tab.set(Tab::Complaints), "Complaints" }
                button { class: tab_class(Tab::Corrections), onclick: move |_| tab.set(Tab::Corrections), "Corrections" }
                button { class: tab_class(Tab::Settings), onclick: move |_| tab.set(Tab::Settings), "Settings" }
            }
        }
        main { class: "desk-main",
            match active {
                Tab::Articles => rsx! { ArticlesPanel {} },
                Tab::Complaints => rsx! { ComplaintsPanel {} },
                Tab::Corrections => rsx! { CorrectionsPanel {} },
                Tab::Settings => rsx! { SettingsPanel {} },
            }
        }
    }
}

#[component]
fn ArticlesPanel() -> Element {
    let mut articles = use_signal(|| Option::<Vec<DeskArticle>>::None);
    let busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);

    use_resource(move || async move {
        match desk_articles().await {
            Ok(list) => articles.set(Some(list)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let rows = articles.read().clone();
    let count = rows.as_ref().map(|v| v.len());
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Articles" }
                div { class: "desk-head-right",
                    if let Some(n) = count {
                        span { class: "desk-count", "{n} total" }
                    }
                    button {
                        class: "desk-btn sm",
                        onclick: move |_| {
                            let open = show_new();
                            show_new.set(!open);
                        },
                        if show_new() { "Close" } else { "New draft" }
                    }
                }
            }
            if show_new() {
                NewDraftForm { articles, busy }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading articles…" } },
                Some(v) if v.is_empty() => rsx! {
                    p { class: "desk-muted pad", "No articles yet. Create the first draft." }
                },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead {
                            tr {
                                th { "Title" }
                                th { "State" }
                                th { "Kind" }
                                th { "Byline" }
                                th { "Updated" }
                                th { "Actions" }
                            }
                        }
                        tbody {
                            for a in v {
                                DeskRow { key: "{a.id}", a, articles, busy, err }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ComplaintsPanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskComplaint>>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);
    let mut slug = use_signal(String::new);
    let mut who = use_signal(String::new);
    let mut body = use_signal(String::new);

    use_resource(move || async move {
        match desk_complaints().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_log_complaint(slug(), who(), body()).await {
                Ok(v) => {
                    items.set(Some(v));
                    slug.set(String::new());
                    who.set(String::new());
                    body.set(String::new());
                    show_new.set(false);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let rows = items.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Complaints" }
                button {
                    class: "desk-btn sm",
                    onclick: move |_| {
                        let open = show_new();
                        show_new.set(!open);
                    },
                    if show_new() { "Close" } else { "Log a complaint" }
                }
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Article slug (optional)", value: "{slug}", oninput: move |e| slug.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Complainant (optional)", value: "{who}", oninput: move |e| who.set(e.value()) }
                    }
                    textarea { class: "desk-in full", rows: "3", placeholder: "What is the complaint?", value: "{body}", oninput: move |e| body.set(e.value()) }
                    button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Record complaint" }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) if v.is_empty() => rsx! { p { class: "desk-muted pad", "No complaints on record." } },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead { tr { th { "Complaint" } th { "Re" } th { "From" } th { "Status" } th { "Logged" } } }
                        tbody {
                            for c in v {
                                ComplaintRow { key: "{c.id}", c, items, busy, err }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ComplaintRow(
    c: DeskComplaint,
    mut items: Signal<Option<Vec<DeskComplaint>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let id = c.id;
    // The next statuses an editor can move this complaint to.
    let nexts: Vec<(&str, &str)> = match c.status.as_str() {
        "received" => vec![("under_review", "Review")],
        "under_review" => vec![("upheld", "Uphold"), ("rejected", "Reject")],
        _ => vec![],
    };
    let re = if c.article_slug.is_empty() {
        "—".to_string()
    } else {
        c.article_slug.clone()
    };
    let from = if c.complainant.is_empty() {
        "—".to_string()
    } else {
        c.complainant.clone()
    };
    rsx! {
        tr {
            td { class: "desk-wrap", "{c.body}" }
            td { class: "desk-muted", "{re}" }
            td { class: "desk-muted", "{from}" }
            td {
                span { class: "desk-state s-c-{c.status}", "{complaint_label(&c.status)}" }
            }
            td { class: "desk-muted",
                div { "{ymd(c.ts)}" }
                div { class: "desk-actions",
                    for (to, label) in nexts {
                        button {
                            key: "{to}",
                            class: "desk-act",
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match desk_complaint_status(id, to.to_string()).await {
                                        Ok(v) => items.set(Some(v)),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CorrectionsPanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskCorrection>>::None);
    let mut articles = use_signal(Vec::<DeskArticle>::new);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);
    let mut article_id = use_signal(|| 0i64);
    let mut original = use_signal(String::new);
    let mut corrected = use_signal(String::new);
    let mut reason = use_signal(String::new);

    use_resource(move || async move {
        match desk_corrections().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
        if let Ok(list) = desk_articles().await {
            articles.set(list);
        }
    });

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            if article_id() == 0 {
                err.set(Some("choose the article to correct".to_string()));
                busy.set(false);
                return;
            }
            match desk_add_correction(article_id(), original(), corrected(), reason()).await {
                Ok(v) => {
                    items.set(Some(v));
                    original.set(String::new());
                    corrected.set(String::new());
                    reason.set(String::new());
                    show_new.set(false);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let rows = items.read().clone();
    let arts = articles.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Corrections" }
                button {
                    class: "desk-btn sm",
                    onclick: move |_| {
                        let open = show_new();
                        show_new.set(!open);
                    },
                    if show_new() { "Close" } else { "Add correction" }
                }
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    select {
                        class: "desk-in full",
                        value: "{article_id}",
                        onchange: move |e| article_id.set(e.value().parse().unwrap_or(0)),
                        option { value: "0", "— choose article —" }
                        for a in arts {
                            option { key: "{a.id}", value: "{a.id}", "{a.title}" }
                        }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Original wording", value: "{original}", oninput: move |e| original.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Corrected wording", value: "{corrected}", oninput: move |e| corrected.set(e.value()) }
                    }
                    input { class: "desk-in full", r#type: "text", placeholder: "Reason / what was wrong", value: "{reason}", oninput: move |e| reason.set(e.value()) }
                    button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Publish correction" }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) if v.is_empty() => rsx! { p { class: "desk-muted pad", "No corrections published." } },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead { tr { th { "Was" } th { "Now" } th { "Reason" } th { "Date" } } }
                        tbody {
                            for c in v {
                                tr { key: "{c.id}",
                                    td { class: "desk-wrap desk-muted", "{c.original}" }
                                    td { class: "desk-wrap", "{c.corrected}" }
                                    td { class: "desk-wrap desk-muted", "{c.reason}" }
                                    td { class: "desk-muted", "{ymd(c.ts)}" }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn SettingsPanel() -> Element {
    let mut current = use_signal(String::new);
    let mut newpw = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut ok = use_signal(|| Option::<String>::None);
    let mut err = use_signal(|| Option::<String>::None);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            ok.set(None);
            err.set(None);
            if newpw() != confirm() {
                err.set(Some("the new passwords do not match".to_string()));
                return;
            }
            busy.set(true);
            match staff_change_password(current(), newpw()).await {
                Ok(()) => {
                    ok.set(Some("Password changed.".to_string()));
                    current.set(String::new());
                    newpw.set(String::new());
                    confirm.set(String::new());
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Settings" }
            }
            form { class: "desk-new", onsubmit: submit,
                p { class: "desk-muted", style: "margin:0 0 14px;", "Change your password." }
                input {
                    class: "desk-in full",
                    r#type: "password",
                    autocomplete: "current-password",
                    placeholder: "Current password",
                    value: "{current}",
                    oninput: move |e| current.set(e.value()),
                }
                input {
                    class: "desk-in full",
                    r#type: "password",
                    autocomplete: "new-password",
                    placeholder: "New password (at least 8 characters)",
                    value: "{newpw}",
                    oninput: move |e| newpw.set(e.value()),
                }
                input {
                    class: "desk-in full",
                    r#type: "password",
                    autocomplete: "new-password",
                    placeholder: "Confirm new password",
                    value: "{confirm}",
                    oninput: move |e| confirm.set(e.value()),
                }
                if let Some(m) = ok() {
                    p { class: "desk-ok", "{m}" }
                }
                if let Some(e) = err() {
                    p { class: "desk-error", "{e}" }
                }
                button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Change password" }
            }
        }
    }
}

fn complaint_label(status: &str) -> &str {
    match status {
        "received" => "Received",
        "under_review" => "Under review",
        "upheld" => "Upheld",
        "rejected" => "Rejected",
        _ => status,
    }
}

#[component]
fn DeskRow(
    a: DeskArticle,
    mut articles: Signal<Option<Vec<DeskArticle>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let id = a.id;
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
            td { class: "desk-actions",
                if a.state == "published" || a.state == "corrected" {
                    a {
                        class: "desk-act",
                        href: "/news/{a.slug}",
                        target: "_blank",
                        rel: "noopener",
                        "View ↗"
                    }
                } else {
                    a {
                        class: "desk-act",
                        href: "/desk/preview/{a.id}",
                        target: "_blank",
                        rel: "noopener",
                        "Preview ↗"
                    }
                }
                if a.actions.is_empty() {
                    span { class: "desk-muted", "—" }
                }
                for act in a.actions.clone() {
                    button {
                        key: "{act.to}",
                        class: if act.to == "retracted" { "desk-act danger" } else { "desk-act" },
                        disabled: busy(),
                        onclick: {
                            let to = act.to.clone();
                            move |_| {
                                let to = to.clone();
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match desk_transition(id, to).await {
                                        Ok(list) => articles.set(Some(list)),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            }
                        },
                        "{act.label}"
                    }
                }
            }
        }
    }
}

#[component]
fn NewDraftForm(mut articles: Signal<Option<Vec<DeskArticle>>>, mut busy: Signal<bool>) -> Element {
    let mut title = use_signal(String::new);
    let mut summary = use_signal(String::new);
    let mut body = use_signal(String::new);
    let mut kind = use_signal(|| "Court report".to_string());
    let mut section = use_signal(|| "Crime".to_string());
    let mut err = use_signal(|| Option::<String>::None);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_create(title(), summary(), kind(), section(), body()).await {
                Ok(list) => {
                    articles.set(Some(list));
                    title.set(String::new());
                    summary.set(String::new());
                    body.set(String::new());
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    // Writer-first editor: a calm, focused writing surface (Ghost/Medium feel) —
    // big headline, a light meta row, the standfirst, then a roomy body.
    rsx! {
        form { class: "editor", onsubmit: submit,
            input {
                class: "editor-title",
                r#type: "text",
                placeholder: "Headline",
                value: "{title}",
                oninput: move |e| title.set(e.value()),
                autofocus: true,
            }
            div { class: "editor-meta",
                label {
                    span { "Section" }
                    select { value: "{section}", onchange: move |e| section.set(e.value()),
                        option { value: "Crime", "Crime" }
                        option { value: "Courts", "Courts" }
                        option { value: "Local", "Local" }
                        option { value: "Community", "Community" }
                    }
                }
                label {
                    span { "Format" }
                    select { value: "{kind}", onchange: move |e| kind.set(e.value()),
                        option { value: "Court report", "Court report" }
                        option { value: "Investigation", "Investigation" }
                        option { value: "Explainer", "Explainer" }
                        option { value: "Announcement", "Announcement" }
                        option { value: "News", "News" }
                    }
                }
            }
            input {
                class: "editor-sub",
                r#type: "text",
                placeholder: "Standfirst — the one-line summary readers see first",
                value: "{summary}",
                oninput: move |e| summary.set(e.value()),
            }
            textarea {
                class: "editor-body",
                rows: "14",
                placeholder: "Write the story. Leave a blank line — or a new line — between paragraphs.",
                value: "{body}",
                oninput: move |e| body.set(e.value()),
            }
            if let Some(e) = err() {
                p { class: "desk-error", "{e}" }
            }
            div { class: "editor-actions",
                span { class: "editor-hint", "Saved as a draft \u{2014} it goes live only after editorial + legal review." }
                button { class: "desk-btn", r#type: "submit", disabled: busy(), "Create draft" }
            }
        }
    }
}

/// Staff-only draft preview at /desk/preview/:id — fetches the article in ANY
/// state and renders it in the reading layout with a "preview" banner. noindex.
#[component]
pub fn DeskPreview(id: i64) -> Element {
    let res = use_resource(move || async move { desk_preview(id).await });
    let guard = res.read();
    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Preview · Predator Hunters" }
        div { class: "desk-root",
            match guard.as_ref() {
                None => rsx! { div { class: "desk-loading", "Loading preview…" } },
                Some(Ok(Some(a))) => rsx! { PreviewBody { a: a.clone() } },
                _ => rsx! {
                    div { class: "desk-login",
                        div { class: "desk-card",
                            h1 { class: "desk-title", "Not available" }
                            p { class: "desk-sub", "Sign in to the desk to preview drafts." }
                            Link { class: "desk-btn", to: Route::Desk {}, "Go to the desk" }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn PreviewBody(a: PreviewArticle) -> Element {
    rsx! {
        div { class: "preview-banner",
            span { class: "preview-tag", "Preview" }
            span { class: "preview-state", "{state_label(&a.state)} — not the public version" }
            Link { class: "preview-back", to: Route::Desk {}, "← Back to the desk" }
        }
        article {
            header { class: "page-head",
                div { class: "wrap", style: "max-width:760px;",
                    p { class: "eyebrow", "{a.section} · {a.kind}" }
                    h1 { "{a.title}" }
                    p { class: "lede", "{a.summary}" }
                    div { style: "margin-top:14px; font-family:var(--mono); font-size:.72rem; letter-spacing:.12em; text-transform:uppercase; color:var(--muted);",
                        "By {a.byline} · {a.iso_date}"
                    }
                }
            }
            section { class: "section", style: "padding-top:clamp(14px,2.5vh,30px);",
                div { class: "wrap", style: "max-width:760px;",
                    if a.body.is_empty() {
                        p { class: "desk-muted", "No body written yet." }
                    }
                    div { class: "prose",
                        for para in a.body.iter() {
                            p { "{para}" }
                        }
                    }
                }
            }
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
