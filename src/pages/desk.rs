//! `/desk` — the staff editorial console. Deliberately NOT at `/admin`; it is
//! noindex, unlinked from the public site, and absent from the sitemap. Renders a
//! login form until authenticated, then the editorial dashboard: create drafts
//! and move articles through the role-gated lifecycle (the gate — publish only via
//! legal sign-off — is enforced server-side; the UI only shows allowed actions).

use dioxus::prelude::*;

use crate::api::{
    desk_add_correction, desk_add_staff, desk_add_watch, desk_articles, desk_audit,
    desk_complaint_status, desk_complaints, desk_convictions, desk_corrections, desk_courtwatch,
    desk_courtwatch_update, desk_create, desk_create_conviction, desk_dismiss_lead, desk_leads,
    desk_log_complaint, desk_poll_now, desk_preview, desk_promote_lead,
    desk_promote_lead_conviction, desk_set_conviction_status, desk_sources, desk_staff,
    desk_transition, desk_update, staff_change_password, staff_login, staff_logout, staff_me,
    DeskArticle, DeskComplaint, DeskConviction, DeskCorrection, DeskLead, DeskSession, DeskSource,
    DeskWatch, PreviewArticle, StaffMember,
};
use crate::app::Route;
// Native-Rust WYSIWYG editor (the "Visual" mode in the article editor). The
// bridge keeps markdown canonical; TainoEditor is an empty host off-wasm (SSG).
use taino_edit_dx::{
    markdown_to_doc, newsroom_keymap, newsroom_schema, state_to_markdown, EditorState, KeymapProp,
    TainoEditor,
};
// The Source mode's live preview is rendered by the dioxus-markdown crate (not our
// md.rs). preserve_html=false so staff markdown can't inject raw HTML.
use dioxus_markdown::Markdown;

/// Which editor surface the writer is using.
#[derive(Clone, Copy, PartialEq)]
enum EdMode {
    /// The native-Rust WYSIWYG editor (taino-edit).
    Visual,
    /// The raw markdown source editor (toolbar + live preview).
    Markdown,
}

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
    Intake,
    Database,
    Complaints,
    Corrections,
    Staff,
    Audit,
    CourtWatch,
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
                button { class: tab_class(Tab::Intake), onclick: move |_| tab.set(Tab::Intake), "Intake" }
                button { class: tab_class(Tab::Database), onclick: move |_| tab.set(Tab::Database), "Database" }
                button { class: tab_class(Tab::Complaints), onclick: move |_| tab.set(Tab::Complaints), "Complaints" }
                button { class: tab_class(Tab::Corrections), onclick: move |_| tab.set(Tab::Corrections), "Corrections" }
                if user.role == "admin" {
                    button { class: tab_class(Tab::Staff), onclick: move |_| tab.set(Tab::Staff), "Staff" }
                    button { class: tab_class(Tab::Audit), onclick: move |_| tab.set(Tab::Audit), "Audit" }
                }
                button { class: tab_class(Tab::CourtWatch), onclick: move |_| tab.set(Tab::CourtWatch), "Court watch" }
                button { class: tab_class(Tab::Settings), onclick: move |_| tab.set(Tab::Settings), "Settings" }
            }
        }
        main { class: "desk-main",
            match active {
                Tab::Articles => rsx! { ArticlesPanel { user: user.clone() } },
                Tab::Intake => rsx! { IntakePanel {} },
                Tab::Database => rsx! { DatabasePanel {} },
                Tab::Complaints => rsx! { ComplaintsPanel {} },
                Tab::Corrections => rsx! { CorrectionsPanel {} },
                Tab::Staff => rsx! { StaffPanel {} },
                Tab::Audit => rsx! { AuditPanel {} },
                Tab::CourtWatch => rsx! { CourtWatchPanel {} },
                Tab::Settings => rsx! { SettingsPanel {} },
            }
        }
    }
}

#[component]
fn ArticlesPanel(user: DeskSession) -> Element {
    let mut articles = use_signal(|| Option::<Vec<DeskArticle>>::None);
    let busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    // (article id, target state, button label) of a publish awaiting IMPRESS sign-off.
    let pending = use_signal(|| Option::<(i64, String, String)>::None);
    // Pipeline filter: "" = all, else a lifecycle state.
    let mut filter = use_signal(String::new);

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
                    Link { class: "desk-btn sm", to: Route::WriteArticle { id: 0 }, "Write a story" }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading articles…" } },
                Some(v) if v.is_empty() => rsx! {
                    p { class: "desk-muted pad", "No articles yet. Create the first draft." }
                },
                Some(v) => {
                    // Pipeline overview: a chip per lifecycle state present, with a
                    // count, so an editor sees drafts / in-review / awaiting-legal at
                    // a glance and can filter the queue.
                    let states = [
                        "draft",
                        "submitted",
                        "editorial_review",
                        "legal_review",
                        "scheduled",
                        "published",
                        "corrected",
                        "retracted",
                    ];
                    let f = filter();
                    let chips: Vec<(&str, usize)> = states
                        .iter()
                        .map(|&s| (s, v.iter().filter(|a| a.state == s).count()))
                        .filter(|(_, c)| *c > 0)
                        .collect();
                    let all_n = v.len();
                    let mine_n = v.iter().filter(|a| a.byline == user.display_name).count();
                    // Role-scoped review queue: what's awaiting THIS user's action.
                    let queue = queue_states(&user.role);
                    let queue_n = v
                        .iter()
                        .filter(|a| queue.contains(&a.state.as_str()))
                        .count();
                    let filtered: Vec<DeskArticle> = v
                        .iter()
                        .filter(|a| match f.as_str() {
                            "" => true,
                            "mine" => a.byline == user.display_name,
                            "queue" => queue.contains(&a.state.as_str()),
                            s => a.state == s,
                        })
                        .cloned()
                        .collect();
                    rsx! {
                        nav { class: "desk-filters", "aria-label": "Filter by stage",
                            button {
                                class: if f.is_empty() { "desk-fchip on" } else { "desk-fchip" },
                                onclick: move |_| filter.set(String::new()),
                                "All " span { class: "desk-fcount", "{all_n}" }
                            }
                            if !queue.is_empty() && queue_n > 0 {
                                button {
                                    class: if f == "queue" { "desk-fchip needs on" } else { "desk-fchip needs" },
                                    onclick: move |_| filter.set("queue".to_string()),
                                    "Needs you " span { class: "desk-fcount", "{queue_n}" }
                                }
                            }
                            if mine_n > 0 {
                                button {
                                    class: if f == "mine" { "desk-fchip on" } else { "desk-fchip" },
                                    onclick: move |_| filter.set("mine".to_string()),
                                    "Mine " span { class: "desk-fcount", "{mine_n}" }
                                }
                            }
                            for (s , c) in chips {
                                button {
                                    key: "{s}",
                                    class: if f == s { "desk-fchip on" } else { "desk-fchip" },
                                    onclick: move |_| filter.set(s.to_string()),
                                    "{state_label(s)} " span { class: "desk-fcount", "{c}" }
                                }
                            }
                        }
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
                                for a in filtered {
                                    DeskRow { key: "{a.id}", a, articles, busy, err, pending }
                                }
                            }
                        }
                    }
                }
            }
            PublishGate { pending, articles, busy, err }
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
fn AuditPanel() -> Element {
    let mut log = use_signal(|| Option::<crate::api::AuditLog>::None);
    let mut err = use_signal(|| Option::<String>::None);

    use_resource(move || async move {
        match desk_audit().await {
            Ok(v) => log.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let data = log.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Audit trail" }
                if let Some(d) = data.as_ref() {
                    if d.verified {
                        span { class: "desk-state s-published", "Chain verified ✓" }
                    } else {
                        span { class: "desk-state s-retracted", "Chain BROKEN" }
                    }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match data {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(d) if d.rows.is_empty() => rsx! { p { class: "desk-muted pad", "No activity recorded yet." } },
                Some(d) => rsx! {
                    p { class: "desk-muted pad", style: "padding-bottom:0;", "Every staff action is recorded in a tamper-evident hash chain." }
                    table { class: "desk-table",
                        thead { tr { th { "When" } th { "Who" } th { "Action" } th { "Subject" } } }
                        tbody {
                            for (i , r) in d.rows.iter().enumerate() {
                                tr { key: "{i}",
                                    td { class: "desk-muted", "{ymd(r.ts)}" }
                                    td { "{r.actor}" }
                                    td { span { class: "desk-row-title", "{audit_label(&r.action)}" }
                                        if !r.detail.is_empty() {
                                            div { class: "desk-muted", style: "font-size:12px;", "{r.detail}" }
                                        }
                                    }
                                    td { class: "desk-muted desk-wrap", "{r.subject}" }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

fn audit_label(action: &str) -> &str {
    match action {
        "staff.login" => "Signed in",
        "staff.password_change" => "Changed password",
        "staff.password_reset" | "admin.password_reset" => "Password reset",
        "article.create" => "Created draft",
        "article.edit" => "Edited article",
        "article.submitted" => "Submitted",
        "article.editorial_review" => "Sent to editorial review",
        "article.legal_review" => "Sent to legal",
        "article.scheduled" => "Scheduled",
        "article.published" => "Published",
        "article.corrected" => "Marked corrected",
        "article.retracted" => "Retracted",
        "article.correction" => "Published correction",
        "complaint.received" => "Complaint received",
        "complaint.status" => "Complaint status changed",
        "user.create" => "Staff added",
        "seed" => "Seeded content",
        other => other,
    }
}

#[component]
fn StaffPanel() -> Element {
    let mut staff = use_signal(|| Option::<Vec<StaffMember>>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);
    let mut username = use_signal(String::new);
    let mut display = use_signal(String::new);
    let mut role = use_signal(|| "writer".to_string());
    let mut password = use_signal(String::new);

    use_resource(move || async move {
        match desk_staff().await {
            Ok(v) => staff.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_add_staff(username(), display(), role(), password()).await {
                Ok(v) => {
                    staff.set(Some(v));
                    username.set(String::new());
                    display.set(String::new());
                    password.set(String::new());
                    show_new.set(false);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let rows = staff.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Staff" }
                button {
                    class: "desk-btn sm",
                    onclick: move |_| {
                        let open = show_new();
                        show_new.set(!open);
                    },
                    if show_new() { "Close" } else { "Add staff" }
                }
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Username (login)", value: "{username}", oninput: move |e| username.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Display name (byline)", value: "{display}", oninput: move |e| display.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        select { class: "desk-in", value: "{role}", onchange: move |e| role.set(e.value()),
                            option { value: "writer", "Writer" }
                            option { value: "sub_editor", "Sub-editor" }
                            option { value: "editor", "Editor" }
                            option { value: "legal", "Legal reviewer" }
                            option { value: "admin", "Admin" }
                        }
                        input { class: "desk-in", r#type: "password", autocomplete: "new-password", placeholder: "Temp password (8+ chars)", value: "{password}", oninput: move |e| password.set(e.value()) }
                    }
                    if let Some(e) = err() {
                        p { class: "desk-error", "{e}" }
                    }
                    div { class: "editor-actions",
                        span { class: "editor-hint", "They can change their password in Settings after signing in." }
                        button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Add staff" }
                    }
                }
            }
            if !show_new() {
                if let Some(e) = err() {
                    p { class: "desk-error pad", "{e}" }
                }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) if v.is_empty() => rsx! { p { class: "desk-muted pad", "No staff yet." } },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead { tr { th { "Name" } th { "Username" } th { "Role" } } }
                        tbody {
                            for m in v {
                                tr { key: "{m.username}",
                                    td { span { class: "desk-row-title", "{m.display_name}" } }
                                    td { class: "desk-muted", "{m.username}" }
                                    td { span { class: "desk-role", "{role_label(&m.role)}" } }
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
/// IMPRESS pre-publish checklist. Shown when an editor moves a story to Published
/// or Scheduled; the confirmations are recorded in the review log + audit trail as
/// the sign-off note. Publishing is blocked until all are ticked.
#[component]
fn PublishGate(
    mut pending: Signal<Option<(i64, String, String)>>,
    mut articles: Signal<Option<Vec<DeskArticle>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let mut c1 = use_signal(|| false);
    let mut c2 = use_signal(|| false);
    let mut c3 = use_signal(|| false);

    let p = pending.read().clone();
    let Some((id, to, label)) = p else {
        return rsx! {};
    };
    let ready = c1() && c2() && c3();

    let confirm = move |_| {
        if !(c1() && c2() && c3()) {
            return;
        }
        let to = to.clone();
        spawn(async move {
            busy.set(true);
            err.set(None);
            let note = "IMPRESS sign-off: case concluded (no active proceedings); public interest + accuracy checked; AI-assistance + pre-charge naming reviewed";
            match desk_transition(id, to, note.to_string()).await {
                Ok(list) => {
                    articles.set(Some(list));
                    pending.set(None);
                    c1.set(false);
                    c2.set(false);
                    c3.set(false);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "modal-scrim", onclick: move |_| pending.set(None),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                p { class: "desk-eyebrow", "Pre-publish checks" }
                h3 { class: "modal-title", "Going public: {label}" }
                p { class: "desk-muted", style: "margin:0 0 14px;", "Confirm our published standards before this story goes live:" }
                label { class: "modal-check",
                    input { r#type: "checkbox", checked: c1(), onchange: move |e| c1.set(e.checked()) }
                    span { "The case is concluded — no active proceedings (Contempt of Court Act 1981)." }
                }
                label { class: "modal-check",
                    input { r#type: "checkbox", checked: c2(), onchange: move |e| c2.set(e.checked()) }
                    span { "There is a clear public interest and the piece is accurate against the record." }
                }
                label { class: "modal-check",
                    input { r#type: "checkbox", checked: c3(), onchange: move |e| c3.set(e.checked()) }
                    span { "Any AI assistance is labelled; no one is named before charge without justification." }
                }
                div { class: "modal-actions",
                    button { class: "desk-btn ghost", onclick: move |_| pending.set(None), "Cancel" }
                    button { class: "desk-btn", disabled: !ready || busy(), onclick: confirm, "Confirm + publish" }
                }
            }
        }
    }
}

#[component]
fn DeskRow(
    a: DeskArticle,
    mut articles: Signal<Option<Vec<DeskArticle>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
    mut pending: Signal<Option<(i64, String, String)>>,
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
                if a.state != "retracted" {
                    Link { class: "desk-act", to: Route::WriteArticle { id: a.id }, "Edit ✎" }
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
                            let label = act.label.clone();
                            move |_| {
                                let to = to.clone();
                                let label = label.clone();
                                // Going public (publish/schedule) opens the IMPRESS
                                // pre-publish checklist; other moves apply at once.
                                if to == "published" || to == "scheduled" {
                                    pending.set(Some((id, to, label)));
                                } else {
                                    spawn(async move {
                                        busy.set(true);
                                        err.set(None);
                                        match desk_transition(id, to, String::new()).await {
                                            Ok(list) => articles.set(Some(list)),
                                            Err(e) => err.set(Some(e.to_string())),
                                        }
                                        busy.set(false);
                                    });
                                }
                            }
                        },
                        "{act.label}"
                    }
                }
            }
        }
    }
}

/// The editor page at /desk/edit/:id — id 0 = a new draft, else edit that article.
#[component]
pub fn WriteArticle(id: i64) -> Element {
    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Editor · Predator Hunters" }
        div { class: "desk-root",
            header { class: "desk-top",
                div { class: "desk-top-in",
                    div {
                        p { class: "desk-eyebrow", "Editorial desk" }
                        h1 { class: "desk-h1", if id == 0 { "Write a story" } else { "Edit story" } }
                    }
                    div { class: "desk-top-right",
                        Link { class: "desk-btn ghost", to: Route::Desk {}, "← Desk" }
                    }
                }
            }
            main { class: "desk-main",
                if id == 0 {
                    EditorForm {
                        edit_id: 0,
                        init_title: String::new(),
                        init_summary: String::new(),
                        init_kind: "Court report".to_string(),
                        init_section: "Crime".to_string(),
                        init_body: String::new(),
                        init_meta: String::new(),
                        init_og: String::new(),
                        init_tags: String::new(),
                        init_slug: String::new(),
                        init_state: "draft".to_string(),
                    }
                } else {
                    WriteLoad { id }
                }
            }
        }
    }
}

/// Loads an existing article (any state) into the editor for editing.
#[component]
fn WriteLoad(id: i64) -> Element {
    let res = use_resource(move || async move { desk_preview(id).await });
    let g = res.read();
    match g.as_ref() {
        None => rsx! { p { class: "desk-muted pad", "Loading…" } },
        Some(Ok(Some(a))) => rsx! {
            EditorForm {
                edit_id: id,
                init_title: a.title.clone(),
                init_summary: a.summary.clone(),
                init_kind: a.kind.clone(),
                init_section: a.section.clone(),
                init_body: a.body.join("\n"),
                init_meta: a.meta_description.clone(),
                init_og: a.og_image_url.clone(),
                init_tags: a.tags.join(", "),
                init_slug: a.slug.clone(),
                init_state: a.state.clone(),
            }
        },
        _ => rsx! {
            p { class: "desk-error pad", "Could not load this article, or you are not signed in." }
        },
    }
}

/// The shared writer-first editor (Ghost/Medium feel). Creates a draft when
/// edit_id is 0, otherwise saves changes to that article, then returns to /desk.
#[component]
#[allow(clippy::too_many_arguments)]
fn EditorForm(
    edit_id: i64,
    init_title: String,
    init_summary: String,
    init_kind: String,
    init_section: String,
    init_body: String,
    init_meta: String,
    init_og: String,
    init_tags: String,
    init_slug: String,
    init_state: String,
) -> Element {
    let mut title = use_signal(|| init_title.clone());
    let mut summary = use_signal(|| init_summary.clone());
    let mut body = use_signal(|| init_body.clone());
    let mut kind = use_signal(|| init_kind.clone());
    let mut section = use_signal(|| init_section.clone());
    let mut meta_desc = use_signal(|| init_meta.clone());
    let mut og_image = use_signal(|| init_og.clone());
    let mut tags = use_signal(|| init_tags.clone());
    let mut slug = use_signal(|| init_slug.clone());
    let mut err = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);
    let nav = navigator();
    // Slug is editable only while the article is pre-publish (published/corrected
    // URLs are locked — see update_article's server-side gate).
    let slug_locked = matches!(init_state.as_str(), "published" | "corrected");

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            // comma-separated tags -> Vec<String>, trimmed + de-blanked.
            let tag_vec: Vec<String> = tags()
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            let res = if edit_id == 0 {
                desk_create(
                    title(), summary(), kind(), section(), body(),
                    meta_desc(), og_image(), tag_vec,
                )
                .await
                .map(|_| ())
            } else {
                desk_update(
                    edit_id, title(), summary(), kind(), section(), body(),
                    meta_desc(), og_image(), tag_vec, slug(),
                )
                .await
            };
            match res {
                Ok(()) => {
                    nav.push(Route::Desk {});
                }
                Err(e) => {
                    err.set(Some(e.to_string()));
                    busy.set(false);
                }
            }
        });
    };

    // ---- editor mode: Visual (WYSIWYG) vs Markdown source ----
    let mut mode = use_signal(|| EdMode::Visual);
    // Built once: the article schema (shared by the editor, keymap + markdown
    // parser), the rich-editor document seeded from the markdown, and the keymap.
    // Markdown stays canonical — `body` is what is saved and publicly rendered.
    let schema = use_hook(newsroom_schema);
    let mut ed_state = use_signal({
        let schema = schema.clone();
        let init = init_body.clone();
        move || EditorState::new(markdown_to_doc(&schema, &init), schema.clone())
    });
    let keymap = use_hook({
        let schema = schema.clone();
        move || KeymapProp::new(newsroom_keymap(&schema))
    });
    // Visual-mode edits flow back into the canonical markdown `body` (which the
    // hidden #ed-body textarea + the save path read).
    use_effect(move || {
        if mode() == EdMode::Visual {
            body.set(state_to_markdown(&ed_state.read()));
        }
    });

    let save_label = if edit_id == 0 {
        "Create draft"
    } else {
        "Save changes"
    };

    // Newsroom writing meters (like a big-CMS editor): headline SEO length,
    // standfirst length, body word count + reading time.
    let title_len = title().chars().count();
    let sum_len = summary().chars().count();
    let words = body().split_whitespace().count();
    let mins = words.div_ceil(200).max(1);
    // SEO sweet spot for a headline is ~20–65 chars.
    let title_state = if title_len == 0 {
        "meter"
    } else if (20..=65).contains(&title_len) {
        "meter ok"
    } else {
        "meter warn"
    };
    let sum_state = if sum_len == 0 || (80..=200).contains(&sum_len) {
        "meter"
    } else {
        "meter warn"
    };
    let meta_len = meta_desc().chars().count();
    let meta_state = if meta_len == 0 || (120..=160).contains(&meta_len) {
        "meter"
    } else {
        "meter warn"
    };

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
            // ---- SEO + social (search/share metadata) ----
            div { class: "editor-meta",
                if edit_id != 0 {
                    label {
                        span { "URL slug" }
                        input {
                            r#type: "text",
                            value: "{slug}",
                            disabled: slug_locked,
                            oninput: move |e| slug.set(e.value()),
                            placeholder: "url-slug",
                        }
                    }
                }
                label {
                    span { "Tags (comma-separated)" }
                    input {
                        r#type: "text",
                        value: "{tags}",
                        oninput: move |e| tags.set(e.value()),
                        placeholder: "grooming, crown court",
                    }
                }
            }
            input {
                class: "editor-sub",
                r#type: "text",
                placeholder: "Social / OG image URL (e.g. /assets/og/your-image.jpg)",
                value: "{og_image}",
                oninput: move |e| og_image.set(e.value()),
            }
            textarea {
                class: "editor-body",
                rows: "2",
                placeholder: "Meta description — the ~155-char summary shown in search results (falls back to the standfirst if blank).",
                value: "{meta_desc}",
                oninput: move |e| meta_desc.set(e.value()),
            }
            // ---- editor mode toggle (Visual WYSIWYG ↔ Markdown source) ----
            div { class: "editor-modebar",
                button {
                    r#type: "button",
                    class: if mode() == EdMode::Visual { "em-tab on" } else { "em-tab" },
                    onclick: move |_| {
                        // Entering Visual: seed the editor from the latest markdown.
                        let s = schema.clone();
                        ed_state.set(EditorState::new(markdown_to_doc(&s, &body()), s));
                        mode.set(EdMode::Visual);
                    },
                    "\u{2726} Visual"
                }
                button {
                    r#type: "button",
                    class: if mode() == EdMode::Markdown { "em-tab on" } else { "em-tab" },
                    onclick: move |_| mode.set(EdMode::Markdown),
                    "\u{2261} Markdown"
                }
                span { class: "editor-hint2",
                    if mode() == EdMode::Visual {
                        "Visual editor \u{2014} format as you type (Ctrl/Cmd-B, -I, -Z). Saved as markdown."
                    } else {
                        "Markdown source \u{2014} **bold**, *italic*, [text](url), ![caption](img), ## heading, - bullet, ^ drop cap."
                    }
                }
            }

            // ---- Visual mode: native-Rust WYSIWYG. Mounted once (so it keeps its
            // keymap); hidden — not unmounted — when editing the markdown source. ----
            div {
                // Toggle visibility by CLASS, not inline style: Dioxus does not clear
                // a style attribute set back to "" (so a once-hidden wrapper would
                // never reappear). The editor stays mounted either way.
                class: if mode() == EdMode::Visual { "editor-rich" } else { "editor-rich is-hidden" },
                TainoEditor { state: ed_state, keymap: keymap.clone() }
            }

            // ---- Markdown mode: the source-insert toolbar ----
            if mode() == EdMode::Markdown {
                div { class: "editor-toolbar",
                    button { r#type: "button", class: "tb b", title: "Bold", onclick: move |_| { let _ = document::eval(&wrap_js("**", "**", "bold")); }, "B" }
                    button { r#type: "button", class: "tb i", title: "Italic", onclick: move |_| { let _ = document::eval(&wrap_js("*", "*", "italic")); }, "i" }
                    button { r#type: "button", class: "tb", title: "Link", onclick: move |_| { let _ = document::eval(&wrap_js("[", "](https://)", "link text")); }, "Link" }
                    button { r#type: "button", class: "tb", title: "Heading", onclick: move |_| { let _ = document::eval(&wrap_js("## ", "", "Heading")); }, "H" }
                    button { r#type: "button", class: "tb", title: "Image (paste a URL)", onclick: move |_| { let _ = document::eval(&wrap_js("![", "](https://)", "image caption")); }, "Image" }
                    button { r#type: "button", class: "tb", title: "Drop cap — large first letter on this paragraph", onclick: move |_| { let _ = document::eval(&wrap_js("^ ", "", "Lead paragraph")); }, "Drop cap" }
                }
            }

            // ---- The markdown source textarea: the editor in Markdown mode; the
            // hidden bridge to the canonical `body` (read by save) in Visual mode. ----
            div {
                class: if mode() == EdMode::Visual { "editor-split is-hidden" } else { "editor-split" },
                textarea {
                    id: "ed-body",
                    class: "editor-body",
                    rows: "16",
                    placeholder: "Write the story. Leave a blank line — or a new line — between paragraphs.",
                    value: "{body}",
                    oninput: move |e| body.set(e.value()),
                }
                div { class: "editor-preview prose",
                    span { class: "editor-prev-label", "Live preview · dioxus-markdown" }
                    Markdown { src: body, preserve_html: false }
                }
            }
            div { class: "editor-meters",
                span { class: title_state, "Headline " b { "{title_len}" } " / ~65" }
                span { class: sum_state, "Standfirst " b { "{sum_len}" } }
                span { class: "meter", "Body " b { "{words}" } " words · {mins} min read" }
                span { class: meta_state, "Meta " b { "{meta_len}" } " / ~155" }
            }
            if let Some(e) = err() {
                p { class: "desk-error", "{e}" }
            }
            div { class: "editor-actions",
                span { class: "editor-hint", "Saved as a draft \u{2014} it goes live only after editorial + legal review." }
                button { class: "desk-btn", r#type: "submit", disabled: busy(), "{save_label}" }
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
                Some(Ok(Some(a))) => rsx! { PreviewBody { a: a.clone(), id } },
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
fn PreviewBody(a: PreviewArticle, id: i64) -> Element {
    let editable = a.state != "retracted";
    rsx! {
        div { class: "preview-banner",
            span { class: "preview-tag", "Preview" }
            span { class: "preview-state", "{state_label(&a.state)} — not the public version" }
            div { class: "preview-actions",
                if editable {
                    Link { class: "preview-edit", to: Route::WriteArticle { id }, "Edit ✎" }
                }
                Link { class: "preview-back", to: Route::Desk {}, "← Back to the desk" }
            }
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
                            div { dangerous_inner_html: crate::md::block_html(para) }
                        }
                    }
                }
            }
        }
    }
}

/// JS that wraps the current selection in the `#ed-body` textarea with markdown
/// markers (or inserts a placeholder), then fires `input` so the Dioxus signal +
/// live preview update. Fire-and-forget via document::eval.
fn wrap_js(prefix: &str, suffix: &str, placeholder: &str) -> String {
    let p = prefix.len();
    format!(
        "(function(){{var t=document.getElementById('ed-body');if(!t)return;var s=t.selectionStart,e=t.selectionEnd,v=t.value;var sel=v.slice(s,e)||'{placeholder}';t.value=v.slice(0,s)+'{prefix}'+sel+'{suffix}'+v.slice(e);t.dispatchEvent(new Event('input',{{bubbles:true}}));t.focus();var c=s+{p}+sel.length;t.selectionStart=c;t.selectionEnd=c;}})();"
    )
}

/// The lifecycle states awaiting THIS role's action (the "Needs you" queue).
/// Writers act on their own drafts (covered by the "Mine" filter), so they get
/// no queue chip here.
fn queue_states(role: &str) -> &'static [&'static str] {
    match role {
        "legal" => &["legal_review"],
        "editor" | "sub_editor" => &["submitted", "editorial_review"],
        "admin" => &["submitted", "editorial_review", "legal_review", "scheduled"],
        _ => &[],
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

fn offence_label(cat: &str) -> &str {
    match cat {
        "child" => "Child",
        "sexual" => "Sexual",
        "other" => "Other",
        _ => "Unclassified",
    }
}

fn lead_status_label(status: &str) -> &str {
    match status {
        "new" => "New",
        "triaged" => "Triaged",
        "promoted" => "Promoted",
        "dismissed" => "Dismissed",
        _ => status,
    }
}

fn watch_status_label(status: &str) -> &str {
    match status {
        "watching" => "Watching",
        "attending" => "Attending",
        "transcript_requested" => "Transcript requested",
        "closed" => "Closed",
        _ => status,
    }
}

fn hearing_label(kind: &str) -> &str {
    match kind {
        "trial" => "Trial",
        "appeal" => "Appeal",
        "sentencing" => "Sentencing",
        _ => "Listing",
    }
}

// ===================== Intake (crawled leads) =====================

#[component]
fn IntakePanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskLead>>::None);
    let busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_handled = use_signal(|| false);
    let mut sources = use_signal(|| Option::<Vec<DeskSource>>::None);
    let mut polling = use_signal(|| false);

    use_resource(move || async move {
        match desk_leads().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });
    use_resource(move || async move {
        if let Ok(v) = desk_sources().await {
            sources.set(Some(v));
        }
    });

    let poll_now = move |_| {
        spawn(async move {
            polling.set(true);
            err.set(None);
            match desk_poll_now().await {
                Ok(()) => {
                    // Give the background pass a moment, then reload leads.
                    if let Ok(v) = desk_leads().await {
                        items.set(Some(v));
                    }
                    if let Ok(v) = desk_sources().await {
                        sources.set(Some(v));
                    }
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            polling.set(false);
        });
    };

    let rows = items.read().clone();
    let srcs = sources.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Intake — external sources" }
                div { class: "desk-actions",
                    button { class: "desk-btn sm", disabled: polling(), onclick: poll_now,
                        if polling() { "Polling…" } else { "Poll now" }
                    }
                    button {
                        class: "desk-btn sm",
                        onclick: move |_| {
                            let open = show_handled();
                            show_handled.set(!open);
                        },
                        if show_handled() { "Hide handled" } else { "Show handled" }
                    }
                }
            }
            p { class: "desk-muted pad",
                "Unverified leads crawled from court judgments and news. Promote a lead to start "
                "our own report (it enters the normal draft → legal → publish flow) or dismiss it. "
                "Always verify against the court record; never republish a source's text or photo."
            }
            if let Some(s) = srcs {
                p { class: "desk-muted pad", style: "font-family:var(--mono); font-size:.72rem;",
                    if s.is_empty() {
                        "No sources configured (set PH_CRAWL_ENABLED + feeds, or Poll now to seed presets)."
                    } else {
                        "Sources: "
                    }
                    for src in s.iter() {
                        span { key: "{src.key}", style: "margin-right:12px;",
                            "{src.key} ("
                            if let Some(t) = src.last_polled_at { "{ymd(t)}" } else { "never" }
                            ")"
                        }
                    }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) => {
                    let visible: Vec<DeskLead> = v
                        .into_iter()
                        .filter(|l| show_handled() || matches!(l.status.as_str(), "new" | "triaged"))
                        .collect();
                    if visible.is_empty() {
                        rsx! { div { class: "intake-empty", "\u{2713} No leads awaiting triage." } }
                    } else {
                        rsx! {
                            div { class: "intake-count", "{visible.len()} awaiting triage" }
                            div { class: "intake-list",
                                for l in visible {
                                    IntakeCard { key: "{l.id}", lead: l, items, busy, err }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Which promote flow the writer chose in step 1 (drives the inline step-2 form).
#[derive(Clone, Copy, PartialEq)]
enum PromoteMode {
    Draft,
    Db,
}

#[component]
fn IntakeCard(
    lead: DeskLead,
    mut items: Signal<Option<Vec<DeskLead>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let id = lead.id;
    let nav = navigator();
    let actionable = matches!(lead.status.as_str(), "new" | "triaged");
    // Restriction-sensitive leads get a red edge so triage can't miss them.
    let restricted = matches!(lead.offence_category.as_str(), "sexual" | "child") || lead.id_risk;

    // Two-step promote: None = the action row; Some(mode) = the inline
    // "as <kind> in <section> → Confirm" form for that mode.
    let mut step = use_signal(|| Option::<PromoteMode>::None);
    let mut kind = use_signal(|| "Court report".to_string());
    let mut section = use_signal(|| "Crime".to_string());

    // Step 2 confirm: promote (draft or +DB) then open the new draft in the editor.
    let confirm = move |_| {
        let mode = step();
        spawn(async move {
            busy.set(true);
            err.set(None);
            let res = if mode == Some(PromoteMode::Db) {
                desk_promote_lead_conviction(id, kind(), section()).await
            } else {
                desk_promote_lead(id, kind(), section()).await
            };
            match res {
                Ok(v) => {
                    let aid = v.iter().find(|l| l.id == id).and_then(|l| l.promoted_article_id);
                    items.set(Some(v));
                    busy.set(false);
                    if let Some(aid) = aid {
                        nav.push(Route::WriteArticle { id: aid });
                    }
                }
                Err(e) => {
                    err.set(Some(e.to_string()));
                    busy.set(false);
                }
            }
        });
    };

    let dismiss = move |_| {
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_dismiss_lead(id).await {
                Ok(v) => items.set(Some(v)),
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        article { class: if restricted { "intake-card restricted" } else { "intake-card" },
            div { class: "intake-card-head",
                div { class: "intake-tags",
                    span { class: "intake-cat", "{offence_label(&lead.offence_category)}" }
                    if matches!(lead.offence_category.as_str(), "sexual" | "child") {
                        span { class: "intake-warn", title: "Sexual-offence and child cases carry automatic anonymity duties — clear before publishing.", "\u{26a0} check restrictions" }
                    }
                    if lead.id_risk {
                        span { class: "intake-warn", title: "Wording suggests a victim could be identifiable.", "\u{26a0} ID risk" }
                    }
                }
                div { class: "intake-meta",
                    span { class: "intake-status", "{lead_status_label(&lead.status)}" }
                    span { class: "intake-src-key", "{lead.source_key}" }
                    span { "{ymd(lead.created_at)}" }
                }
            }
            // Click the headline to turn the lead into OUR draft and edit/preview
            // how it'll look on the site (opens the promote step below → editor).
            // The external source is a separate, clearly-labelled verify link — not
            // the primary click — so triage stays on our system.
            h3 {
                class: "intake-headline",
                onclick: move |_| {
                    if actionable && step().is_none() {
                        step.set(Some(PromoteMode::Draft));
                    }
                },
                "{lead.title}"
            }
            if !lead.snippet.is_empty() {
                p { class: "intake-snippet", "{lead.snippet}" }
            }
            if !lead.image_url.is_empty() {
                p { class: "intake-imgnote", "\u{2316} Source image (reference only — never republished): {lead.image_attribution}" }
            }
            a { class: "intake-source-link", href: "{lead.url}", target: "_blank", rel: "noopener noreferrer",
                "\u{2197} Source \u{2014} read + verify against the record"
            }
            if let Some(aid) = lead.promoted_article_id {
                Link { class: "intake-promoted", to: Route::WriteArticle { id: aid }, "\u{2192} opened as draft #{aid} \u{00b7} edit" }
            }
            if actionable {
                match step() {
                    None => rsx! {
                        div { class: "intake-foot",
                            button { class: "intake-btn primary", disabled: busy(), onclick: move |_| step.set(Some(PromoteMode::Draft)), "Promote \u{25b8}" }
                            button { class: "intake-btn", disabled: busy(), onclick: move |_| step.set(Some(PromoteMode::Db)), "Promote + DB \u{25b8}" }
                            button { class: "intake-btn ghost", disabled: busy(), onclick: dismiss, "Dismiss" }
                        }
                    },
                    Some(mode) => rsx! {
                        div { class: "intake-step2",
                            span { class: "intake-step2-lead",
                                if mode == PromoteMode::Db { "Promote + DB entry, as a" } else { "Promote as a" }
                            }
                            select { class: "intake-sel", value: "{kind}", oninput: move |e| kind.set(e.value()),
                                option { value: "Court report", "Court report" }
                                option { value: "Investigation", "Investigation" }
                                option { value: "Explainer", "Explainer" }
                                option { value: "News", "News" }
                            }
                            span { class: "intake-step2-in", "in" }
                            select { class: "intake-sel", value: "{section}", oninput: move |e| section.set(e.value()),
                                option { value: "Crime", "Crime" }
                                option { value: "Courts", "Courts" }
                                option { value: "Local", "Local" }
                                option { value: "Community", "Community" }
                            }
                            button { class: "intake-btn primary", disabled: busy(), onclick: confirm,
                                if busy() { "Opening\u{2026}" } else { "Confirm \u{2192} edit" }
                            }
                            button { class: "intake-btn ghost", disabled: busy(), onclick: move |_| step.set(None), "Cancel" }
                        }
                    },
                }
            }
        }
    }
}

// ===================== Database (conviction entries) =====================

#[component]
fn DatabasePanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskConviction>>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);
    let mut name = use_signal(String::new);
    let mut area = use_signal(String::new);
    let mut offence = use_signal(String::new);
    let mut outcome = use_signal(String::new);
    let mut date = use_signal(String::new);
    let mut iso_date = use_signal(String::new);
    let mut lat = use_signal(String::new);
    let mut lng = use_signal(String::new);
    let mut article_slug = use_signal(String::new);
    let mut article_id = use_signal(String::new);
    let mut source_url = use_signal(String::new);
    let mut source_name = use_signal(String::new);

    use_resource(move || async move {
        match desk_convictions().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            let aid = article_id().trim().parse::<i64>().ok();
            let latf = lat().trim().parse::<f64>().unwrap_or(0.0);
            let lngf = lng().trim().parse::<f64>().unwrap_or(0.0);
            match desk_create_conviction(
                name(),
                area(),
                offence(),
                outcome(),
                date(),
                iso_date(),
                latf,
                lngf,
                aid,
                article_slug(),
                source_url(),
                source_name(),
            )
            .await
            {
                Ok(v) => {
                    items.set(Some(v));
                    name.set(String::new());
                    area.set(String::new());
                    offence.set(String::new());
                    outcome.set(String::new());
                    date.set(String::new());
                    iso_date.set(String::new());
                    lat.set(String::new());
                    lng.set(String::new());
                    article_slug.set(String::new());
                    article_id.set(String::new());
                    source_url.set(String::new());
                    source_name.set(String::new());
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
                h2 { "Conviction database" }
                button {
                    class: "desk-btn sm",
                    onclick: move |_| {
                        let open = show_new();
                        show_new.set(!open);
                    },
                    if show_new() { "Close" } else { "Add entry" }
                }
            }
            p { class: "desk-muted pad",
                "Post-conviction entries for the public database. An entry only goes public once "
                "our own report on it is published, and it cites the court-record / news source."
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Name", value: "{name}", oninput: move |e| name.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Area (optional)", value: "{area}", oninput: move |e| area.set(e.value()) }
                    }
                    input { class: "desk-in full", r#type: "text", placeholder: "Offence", value: "{offence}", oninput: move |e| offence.set(e.value()) }
                    input { class: "desk-in full", r#type: "text", placeholder: "Outcome / sentence", value: "{outcome}", oninput: move |e| outcome.set(e.value()) }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Date e.g. May 2026", value: "{date}", oninput: move |e| date.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "ISO date e.g. 2026-05-21", value: "{iso_date}", oninput: move |e| iso_date.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Latitude (0 if unknown)", value: "{lat}", oninput: move |e| lat.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Longitude (0 if unknown)", value: "{lng}", oninput: move |e| lng.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Our report slug (links + enables publish)", value: "{article_slug}", oninput: move |e| article_slug.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Our report id (optional — slug is enough)", value: "{article_id}", oninput: move |e| article_id.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Source URL (court record / news)", value: "{source_url}", oninput: move |e| source_url.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Source name", value: "{source_name}", oninput: move |e| source_name.set(e.value()) }
                    }
                    button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Create draft entry" }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) if v.is_empty() => rsx! { p { class: "desk-muted pad", "No database entries yet." } },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead { tr { th { "Name" } th { "Offence" } th { "Area" } th { "Status" } th { "Source" } th { "Actions" } } }
                        tbody {
                            for c in v {
                                ConvictionRow { key: "{c.id}", c, items, busy, err }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ConvictionRow(
    c: DeskConviction,
    mut items: Signal<Option<Vec<DeskConviction>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let id = c.id;
    let nexts: Vec<(&str, &str)> = match c.status.as_str() {
        "draft" => vec![("published", "Publish")],
        "published" => vec![("retracted", "Retract")],
        _ => vec![],
    };
    let area = if c.area.is_empty() {
        "—".to_string()
    } else {
        c.area.clone()
    };
    rsx! {
        tr {
            td { class: "desk-wrap", "{c.name}" }
            td { class: "desk-muted", "{c.offence}" }
            td { class: "desk-muted", "{area}" }
            td { span { class: "desk-state", "{state_label(&c.status)}" } }
            td { class: "desk-muted",
                if c.source_url.is_empty() {
                    "—"
                } else {
                    a { href: "{c.source_url}", target: "_blank", rel: "noopener noreferrer", "{c.source_name}" }
                }
            }
            td { class: "desk-muted",
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
                                    match desk_set_conviction_status(id, to.to_string()).await {
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

// ===================== Court watch (PRIVATE) =====================

#[component]
fn CourtWatchPanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskWatch>>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let mut show_new = use_signal(|| false);
    let mut court = use_signal(String::new);
    let mut case_ref = use_signal(String::new);
    let mut hearing_date = use_signal(String::new);
    let mut hearing_type = use_signal(|| "listing".to_string());
    let mut offence_category = use_signal(|| "child".to_string());
    let mut source_url = use_signal(String::new);
    let mut notes = use_signal(String::new);

    use_resource(move || async move {
        match desk_courtwatch().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_add_watch(
                court(),
                case_ref(),
                hearing_date(),
                hearing_type(),
                offence_category(),
                source_url(),
                notes(),
            )
            .await
            {
                Ok(v) => {
                    items.set(Some(v));
                    court.set(String::new());
                    case_ref.set(String::new());
                    hearing_date.set(String::new());
                    source_url.set(String::new());
                    notes.set(String::new());
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
                h2 { "Court watch" }
                button {
                    class: "desk-btn sm",
                    onclick: move |_| {
                        let open = show_new();
                        show_new.set(!open);
                    },
                    if show_new() { "Close" } else { "Add hearing" }
                }
            }
            p { class: "desk-muted pad",
                "PRIVATE. Upcoming and appeal hearings to attend or request a transcript for. "
                "This is live-proceedings intelligence — it is never published and never feeds "
                "the public database."
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Court", value: "{court}", oninput: move |e| court.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Case reference", value: "{case_ref}", oninput: move |e| case_ref.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Hearing date", value: "{hearing_date}", oninput: move |e| hearing_date.set(e.value()) }
                        select {
                            class: "desk-in",
                            value: "{hearing_type}",
                            oninput: move |e: FormEvent| hearing_type.set(e.value()),
                            option { value: "listing", "Listing" }
                            option { value: "trial", "Trial" }
                            option { value: "appeal", "Appeal" }
                            option { value: "sentencing", "Sentencing" }
                        }
                        select {
                            class: "desk-in",
                            value: "{offence_category}",
                            oninput: move |e: FormEvent| offence_category.set(e.value()),
                            option { value: "child", "Child" }
                            option { value: "sexual", "Sexual" }
                            option { value: "other", "Other" }
                        }
                    }
                    input { class: "desk-in full", r#type: "text", placeholder: "Source / listing URL", value: "{source_url}", oninput: move |e| source_url.set(e.value()) }
                    textarea { class: "desk-in full", rows: "2", placeholder: "Notes (who is attending, transcript ref…)", value: "{notes}", oninput: move |e| notes.set(e.value()) }
                    button { class: "desk-btn sm", r#type: "submit", disabled: busy(), "Add to watch list" }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match rows {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some(v) if v.is_empty() => rsx! { p { class: "desk-muted pad", "Nothing on the watch list." } },
                Some(v) => rsx! {
                    table { class: "desk-table",
                        thead { tr { th { "Court / case" } th { "Hearing" } th { "Category" } th { "Status" } th { "Actions" } } }
                        tbody {
                            for w in v {
                                WatchRow { key: "{w.id}", w, items, busy, err }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn WatchRow(
    w: DeskWatch,
    mut items: Signal<Option<Vec<DeskWatch>>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let id = w.id;
    let nexts: Vec<(&str, &str)> = match w.status.as_str() {
        "watching" => vec![
            ("attending", "Attend"),
            ("transcript_requested", "Request transcript"),
            ("closed", "Close"),
        ],
        "attending" => vec![
            ("transcript_requested", "Request transcript"),
            ("closed", "Close"),
        ],
        "transcript_requested" => vec![("closed", "Close")],
        _ => vec![],
    };
    rsx! {
        tr {
            td { class: "desk-wrap",
                strong { "{w.court}" }
                if !w.case_ref.is_empty() {
                    div { class: "desk-muted", "{w.case_ref}" }
                }
                if !w.notes.is_empty() {
                    p { class: "desk-muted", "{w.notes}" }
                }
                if !w.source_url.is_empty() {
                    a { href: "{w.source_url}", target: "_blank", rel: "noopener noreferrer", "Listing" }
                }
            }
            td { class: "desk-muted",
                div { "{w.hearing_date}" }
                span { class: "desk-state", "{hearing_label(&w.hearing_type)}" }
            }
            td { span { class: "desk-state", "{offence_label(&w.offence_category)}" } }
            td { span { class: "desk-state", "{watch_status_label(&w.status)}" } }
            td { class: "desk-muted",
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
                                    match desk_courtwatch_update(id, to.to_string(), String::new()).await {
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
