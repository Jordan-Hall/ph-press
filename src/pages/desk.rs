//! `/desk` — the staff editorial console. Deliberately NOT at `/admin`; it is
//! noindex, unlinked from the public site, and absent from the sitemap. Renders a
//! login form until authenticated, then the editorial dashboard: create drafts
//! and move articles through the role-gated lifecycle (the gate — publish only via
//! legal sign-off — is enforced server-side; the UI only shows allowed actions).

use dioxus::prelude::*;

use crate::api::{
    desk_add_correction, desk_add_staff, desk_add_watch, desk_articles, desk_audit,
    desk_complaint_note, desk_complaint_reply, desk_complaint_status, desk_complaint_thread,
    desk_complaints, desk_convictions, desk_corrections, desk_courtwatch,
    desk_courtwatch_update, desk_create, desk_create_conviction, desk_dismiss_lead, desk_leads,
    desk_log_complaint, desk_poll_now, desk_preview, desk_promote_lead,
    desk_promote_lead_conviction, desk_regenerate_draft, desk_removal_decide,
    desk_removal_requests, desk_set_conviction_status, desk_sources,
    desk_staff, desk_transition, desk_update, regulator_registered, set_regulator_registered,
    staff_change_password, staff_forgot_password,
    staff_install, staff_login, staff_logout, staff_me, staff_needs_install, staff_profile,
    staff_reset_password, staff_totp_begin, staff_totp_disable, staff_totp_enable,
    staff_totp_status, DeskArticle, DeskComplaint, DeskComplaintMessage, DeskConviction,
    DeskCorrection, DeskLead, DeskRemovalRequest, DeskSession, DeskSource, DeskWatch,
    PreviewArticle, StaffMember,
};
use crate::app::Route;
// Native-Rust WYSIWYG editor (the "Visual" mode in the article editor). The
// bridge keeps markdown canonical; TainoEditor is an empty host off-wasm (SSG).
use taino_edit_dx::{
    markdown_to_doc, newsroom_keymap, newsroom_schema, state_to_markdown, EditorState, KeymapProp,
    TainoEditor,
};
// The Source mode's live preview uses crate::md::block_html — the same renderer
// as the public article page — so authors see the real published look as they type.

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
    /// Fresh deploy with no users yet — show the first-run install screen.
    NeedsInstall,
    Out,
    In(DeskSession),
}

#[component]
pub fn Desk() -> Element {
    let mut auth = use_signal(|| Auth::Loading);

    // Resolve state once on load: an existing session wins; otherwise, if no users
    // exist yet, show first-run install; else the login form.
    use_resource(move || async move {
        if let Some(me) = staff_me().await.ok().flatten() {
            auth.set(Auth::In(me));
        } else if staff_needs_install().await.unwrap_or(false) {
            auth.set(Auth::NeedsInstall);
        } else {
            auth.set(Auth::Out);
        }
    });

    let state = auth.read().clone();
    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Editorial desk · Predator Hunters" }
        div { class: "desk-root",
            match state {
                Auth::Loading => rsx! { div { class: "desk-loading", "Loading the desk…" } },
                Auth::NeedsInstall => rsx! { InstallPanel { auth } },
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
    // TOTP code — always sent; empty when user has no 2FA enrolled.
    let mut code = use_signal(String::new);
    // Whether the server has indicated this account requires a TOTP code.
    let mut need_totp = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            error.set(None);
            match staff_login(username(), password(), code()).await {
                Ok(user) => auth.set(Auth::In(user)),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("two-factor") {
                        need_totp.set(true);
                        if msg.contains("required") {
                            error.set(Some(
                                "This account has two-factor authentication. \
                                 Enter your 6-digit authenticator code."
                                    .to_string(),
                            ));
                        } else {
                            error.set(Some("Invalid authenticator code — please try again.".to_string()));
                        }
                    } else {
                        need_totp.set(false);
                        error.set(Some("Invalid username or password.".to_string()));
                    }
                }
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
                if need_totp() {
                    label { class: "desk-field",
                        span { "Authenticator code" }
                        input {
                            r#type: "text",
                            autocomplete: "one-time-code",
                            inputmode: "numeric",
                            pattern: "[0-9]*",
                            maxlength: 6,
                            placeholder: "6-digit code",
                            value: "{code}",
                            oninput: move |e| code.set(e.value()),
                        }
                    }
                }
                if let Some(msg) = error() {
                    p { class: "desk-error", "{msg}" }
                }
                button { class: "desk-btn", r#type: "submit", disabled: busy(),
                    if busy() { "Signing in…" } else { "Sign in" }
                }
                Link { class: "desk-forgot", to: Route::DeskForgot {}, "Forgot password?" }
            }
        }
    }
}

/// First-run install: shown only when the deployment has no users yet. Creates
/// the first administrator account (and signs them in) — there is no default
/// password. The server re-checks "no users" so it can't run twice.
#[component]
fn InstallPanel(auth: Signal<Auth>) -> Element {
    let mut username = use_signal(String::new);
    let mut display = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            error.set(None);
            if password().chars().count() < 8 {
                error.set(Some("Use at least 8 characters.".to_string()));
                return;
            }
            if password() != confirm() {
                error.set(Some("The two passwords don't match.".to_string()));
                return;
            }
            busy.set(true);
            match staff_install(username(), display(), email(), password()).await {
                Ok(user) => auth.set(Auth::In(user)),
                Err(e) => error.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    rsx! {
        div { class: "desk-login",
            form { class: "desk-card", onsubmit: submit,
                p { class: "desk-eyebrow", "Predator Hunters" }
                h1 { class: "desk-title", "Set up the newsroom" }
                p { class: "desk-sub", "Create the administrator account for this site." }
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
                    span { "Display name" }
                    input {
                        r#type: "text",
                        autocomplete: "name",
                        value: "{display}",
                        oninput: move |e| display.set(e.value()),
                    }
                }
                label { class: "desk-field",
                    span { "Email (for password recovery)" }
                    input {
                        r#type: "email",
                        autocomplete: "email",
                        value: "{email}",
                        oninput: move |e| email.set(e.value()),
                    }
                }
                label { class: "desk-field",
                    span { "Password" }
                    input {
                        r#type: "password",
                        autocomplete: "new-password",
                        value: "{password}",
                        oninput: move |e| password.set(e.value()),
                    }
                }
                label { class: "desk-field",
                    span { "Confirm password" }
                    input {
                        r#type: "password",
                        autocomplete: "new-password",
                        value: "{confirm}",
                        oninput: move |e| confirm.set(e.value()),
                    }
                }
                if let Some(msg) = error() {
                    p { class: "desk-error", "{msg}" }
                }
                button { class: "desk-btn", r#type: "submit", disabled: busy(),
                    if busy() { "Creating…" } else { "Create administrator" }
                }
            }
        }
    }
}

/// `/desk/forgot` — request a password-reset link by email. Always reports the
/// same neutral confirmation, so it can't be used to discover which emails have
/// accounts. No auth required: this is the locked-out path.
#[component]
pub fn DeskForgot() -> Element {
    let mut email = use_signal(String::new);
    let mut sent = use_signal(|| false);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            // Result intentionally ignored — same outcome whether or not it matched.
            let _ = staff_forgot_password(email()).await;
            sent.set(true);
            busy.set(false);
        });
    };

    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Reset password · Predator Hunters" }
        div { class: "desk-root",
            div { class: "desk-login",
                if sent() {
                    div { class: "desk-card",
                        p { class: "desk-eyebrow", "Predator Hunters" }
                        h1 { class: "desk-title", "Check your email" }
                        p { class: "desk-sub",
                            "If an account is registered with that email, we've sent it a link to reset the password. The link expires in an hour."
                        }
                        Link { class: "desk-btn ghost", to: Route::Desk {}, "← Back to sign in" }
                    }
                } else {
                    form { class: "desk-card", onsubmit: submit,
                        p { class: "desk-eyebrow", "Predator Hunters" }
                        h1 { class: "desk-title", "Reset your password" }
                        p { class: "desk-sub", "Enter your account email and we'll send a reset link." }
                        label { class: "desk-field",
                            span { "Email" }
                            input {
                                r#type: "email",
                                autocomplete: "email",
                                value: "{email}",
                                oninput: move |e| email.set(e.value()),
                                autofocus: true,
                            }
                        }
                        button { class: "desk-btn", r#type: "submit", disabled: busy(),
                            if busy() { "Sending…" } else { "Send reset link" }
                        }
                        Link { class: "desk-forgot", to: Route::Desk {}, "← Back to sign in" }
                    }
                }
            }
        }
    }
}

/// `/desk/reset/:token` — set a new password from a reset link. The token is the
/// URL path segment; an invalid/expired/used one is reported generically by the
/// server. On success every existing session for the account was already revoked.
#[component]
pub fn DeskReset(token: String) -> Element {
    let mut password = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut done = use_signal(|| false);
    let mut busy = use_signal(|| false);

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        let token = token.clone();
        spawn(async move {
            error.set(None);
            if password().chars().count() < 8 {
                error.set(Some("Use at least 8 characters.".to_string()));
                return;
            }
            if password() != confirm() {
                error.set(Some("The two passwords don't match.".to_string()));
                return;
            }
            busy.set(true);
            match staff_reset_password(token, password()).await {
                Ok(()) => done.set(true),
                Err(_) => error.set(Some(
                    "This reset link is invalid or has expired — request a new one.".to_string(),
                )),
            }
            busy.set(false);
        });
    };

    rsx! {
        document::Meta { name: "robots", content: "noindex, nofollow" }
        document::Title { "Set a new password · Predator Hunters" }
        div { class: "desk-root",
            div { class: "desk-login",
                if done() {
                    div { class: "desk-card",
                        p { class: "desk-eyebrow", "Predator Hunters" }
                        h1 { class: "desk-title", "Password updated" }
                        p { class: "desk-sub", "Your password has been reset. You can now sign in." }
                        Link { class: "desk-btn", to: Route::Desk {}, "Go to sign in" }
                    }
                } else {
                    form { class: "desk-card", onsubmit: submit,
                        p { class: "desk-eyebrow", "Predator Hunters" }
                        h1 { class: "desk-title", "Set a new password" }
                        p { class: "desk-sub", "Choose a new password for your account." }
                        label { class: "desk-field",
                            span { "New password" }
                            input {
                                r#type: "password",
                                autocomplete: "new-password",
                                value: "{password}",
                                oninput: move |e| password.set(e.value()),
                                autofocus: true,
                            }
                        }
                        label { class: "desk-field",
                            span { "Confirm password" }
                            input {
                                r#type: "password",
                                autocomplete: "new-password",
                                value: "{confirm}",
                                oninput: move |e| confirm.set(e.value()),
                            }
                        }
                        if let Some(msg) = error() {
                            p { class: "desk-error", "{msg}" }
                        }
                        button { class: "desk-btn", r#type: "submit", disabled: busy(),
                            if busy() { "Saving…" } else { "Set password" }
                        }
                    }
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
    RemovalRequests,
    Corrections,
    Staff,
    Audit,
    CourtWatch,
    Profile,
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
                button { class: tab_class(Tab::RemovalRequests), onclick: move |_| tab.set(Tab::RemovalRequests), "Removal reqs" }
                button { class: tab_class(Tab::Corrections), onclick: move |_| tab.set(Tab::Corrections), "Corrections" }
                if user.role == "admin" {
                    button { class: tab_class(Tab::Staff), onclick: move |_| tab.set(Tab::Staff), "Staff" }
                    button { class: tab_class(Tab::Audit), onclick: move |_| tab.set(Tab::Audit), "Audit" }
                }
                button { class: tab_class(Tab::CourtWatch), onclick: move |_| tab.set(Tab::CourtWatch), "Court watch" }
                button { class: tab_class(Tab::Profile), onclick: move |_| tab.set(Tab::Profile), "Profile" }
            }
        }
        main { class: "desk-main",
            match active {
                Tab::Articles => rsx! { ArticlesPanel { user: user.clone() } },
                Tab::Intake => rsx! { IntakePanel {} },
                Tab::Database => rsx! { DatabasePanel {} },
                Tab::Complaints => rsx! { ComplaintsPanel {} },
                Tab::RemovalRequests => rsx! { RemovalRequestsPanel {} },
                Tab::Corrections => rsx! { CorrectionsPanel {} },
                Tab::Staff => rsx! { StaffPanel {} },
                Tab::Audit => rsx! { AuditPanel {} },
                Tab::CourtWatch => rsx! { CourtWatchPanel {} },
                Tab::Profile => rsx! { ProfilePanel { user: user.clone() } },
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
    let mut selected = use_signal(|| Option::<i64>::None);
    let mut reload = use_signal(|| 0u32);
    let mut show_new = use_signal(|| false);
    // Live regulator status gates the IMPRESS-named "Escalated" label in the rows.
    let mut registered = use_signal(|| false);
    let mut slug = use_signal(String::new);
    let mut who = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut category = use_signal(String::new);
    let mut body = use_signal(String::new);

    use_resource(move || async move {
        reload(); // refetch the inbox when returning from a complaint detail
        match desk_complaints().await {
            Ok(v) => items.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });
    use_resource(move || async move {
        if let Ok(v) = regulator_registered().await {
            registered.set(v);
        }
    });

    // An open complaint shows its full detail/thread instead of the list.
    if let Some(cid) = selected() {
        return rsx! {
            ComplaintDetail {
                id: cid,
                on_back: move |_| {
                    selected.set(None);
                    reload.set(reload() + 1);
                },
            }
        };
    }

    let submit = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_log_complaint(slug(), who(), email(), category(), body()).await {
                Ok(v) => {
                    items.set(Some(v));
                    slug.set(String::new());
                    who.set(String::new());
                    email.set(String::new());
                    category.set(String::new());
                    body.set(String::new());
                    show_new.set(false);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    let rows = items.read().clone();
    let reg = registered();
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
            p { class: "desk-muted pad", style: "padding-top:0;",
                "Readers complain via each article's \u{201c}Make a complaint\u{201d} link. Open one to investigate, reply, and record the outcome — IMPRESS: acknowledge promptly, give a final response within 21 days."
            }
            if show_new() {
                form { class: "desk-new", onsubmit: submit,
                    p { class: "desk-muted", style: "margin:0 0 8px;", "Log a complaint that arrived another way (phone, post, email)." }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "text", placeholder: "Article slug", value: "{slug}", oninput: move |e| slug.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Complainant name", value: "{who}", oninput: move |e| who.set(e.value()) }
                    }
                    div { class: "desk-new-row",
                        input { class: "desk-in", r#type: "email", placeholder: "Complainant email", value: "{email}", oninput: move |e| email.set(e.value()) }
                        input { class: "desk-in", r#type: "text", placeholder: "Concerns (Code clause)", value: "{category}", oninput: move |e| category.set(e.value()) }
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
                                ComplaintRow { key: "{c.id}", c, selected, registered: reg }
                            }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ComplaintRow(c: DeskComplaint, mut selected: Signal<Option<i64>>, registered: bool) -> Element {
    let id = c.id;
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
        tr { class: "desk-rowlink", onclick: move |_| selected.set(Some(id)),
            td { class: "desk-wrap",
                span { class: "desk-row-title", "{c.body}" }
                if c.decision_overdue {
                    span { class: "desk-flag", "decision overdue" }
                } else if c.ack_overdue {
                    span { class: "desk-flag", "ack overdue" }
                }
            }
            td { class: "desk-muted", "{re}" }
            td { class: "desk-muted", "{from}" }
            td { span { class: "desk-state s-c-{c.status}", "{complaint_label(&c.status, registered)}" } }
            td { class: "desk-muted", "{ymd(c.ts)}" }
        }
    }
}

/// The investigate-and-communicate view for one complaint: the case, the IMPRESS
/// status workflow + timeline flags, the handling thread (internal notes + replies),
/// and forms to add a note or send the complainant an emailed reply.
#[component]
fn ComplaintDetail(id: i64, on_back: EventHandler<()>) -> Element {
    let mut data = use_signal(|| Option::<(DeskComplaint, Vec<DeskComplaintMessage>)>::None);
    let mut err = use_signal(|| Option::<String>::None);
    let mut busy = use_signal(|| false);
    let mut note = use_signal(String::new);
    let mut reply = use_signal(String::new);
    // Live regulator status gates the IMPRESS-named escalation labels below.
    let mut registered = use_signal(|| false);

    use_resource(move || async move {
        match desk_complaint_thread(id).await {
            Ok(d) => data.set(Some(d)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });
    use_resource(move || async move {
        if let Ok(v) = regulator_registered().await {
            registered.set(v);
        }
    });

    let reg = registered();
    let d = data.read().clone();
    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Complaint" }
                button { class: "desk-btn ghost", onclick: move |_| on_back.call(()), "← Back" }
            }
            if let Some(e) = err() {
                p { class: "desk-error pad", "{e}" }
            }
            match d {
                None => rsx! { p { class: "desk-muted pad", "Loading…" } },
                Some((c, thread)) => rsx! {
                    div { class: "desk-new",
                        p { class: "desk-muted", style: "margin:0 0 6px;",
                            "PH-C{c.id} · "
                            {complaint_label(&c.status, reg)}
                        }
                        if c.decision_overdue {
                            p { class: "desk-error", "Decision overdue — IMPRESS expects a final response within 21 days." }
                        } else if c.ack_overdue {
                            p { class: "desk-error", "Acknowledgement overdue — IMPRESS expects prompt acknowledgement (7 days)." }
                        }
                        table { class: "desk-table",
                            tbody {
                                tr {
                                    td { class: "desk-muted", "Article" }
                                    td { if c.article_slug.is_empty() { "—" } else { "{c.article_slug}" } }
                                }
                                tr {
                                    td { class: "desk-muted", "Name" }
                                    td { "{c.complainant}" }
                                }
                                tr {
                                    td { class: "desk-muted", "Email" }
                                    td { "{c.complainant_email}" }
                                }
                                if !c.category.is_empty() {
                                    tr {
                                        td { class: "desk-muted", "Concerns" }
                                        td { "{c.category}" }
                                    }
                                }
                                tr {
                                    td { class: "desk-muted", "Received" }
                                    td { "{ymd(c.ts)}" }
                                }
                            }
                        }
                        p { class: "desk-wrap", style: "margin-top:10px;white-space:pre-wrap;", "{c.body}" }
                    }
                    div { class: "desk-new", style: "margin-top:16px;",
                        p { class: "desk-muted", style: "margin:0 0 8px;", "Move status" }
                        div { class: "desk-actions",
                            for (to , label) in complaint_next_statuses(&c.status, reg) {
                                button {
                                    key: "{to}",
                                    class: "desk-act",
                                    disabled: busy(),
                                    onclick: move |_| {
                                        spawn(async move {
                                            busy.set(true);
                                            err.set(None);
                                            match desk_complaint_status(id, to.to_string()).await {
                                                Ok(_) => {
                                                    if let Ok(x) = desk_complaint_thread(id).await {
                                                        data.set(Some(x));
                                                    }
                                                }
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
                    div { class: "desk-new", style: "margin-top:16px;",
                        p { class: "desk-muted", style: "margin:0 0 8px;", "Handling notes & replies" }
                        if thread.is_empty() {
                            p { class: "desk-muted", "Nothing recorded yet." }
                        }
                        for (i , m) in thread.iter().enumerate() {
                            div { key: "{i}", class: "complaint-msg c-{m.channel}",
                                p { class: "desk-muted", style: "margin:0 0 2px;font-size:12px;",
                                    {format!("{} \u{00b7} {} \u{00b7} {}",
                                        if m.channel == "reply" { "\u{21a9} Reply to complainant" } else { "Internal note" },
                                        m.author, ymd(m.ts))}
                                }
                                p { class: "desk-wrap", style: "margin:0;white-space:pre-wrap;", "{m.body}" }
                            }
                        }
                    }
                    div { class: "desk-new", style: "margin-top:16px;",
                        p { class: "desk-muted", style: "margin:0 0 6px;", "Add internal note (staff only)" }
                        textarea { class: "desk-in full", rows: "2", value: "{note}", oninput: move |e| note.set(e.value()) }
                        button {
                            class: "desk-btn sm",
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match desk_complaint_note(id, note()).await {
                                        Ok(x) => {
                                            data.set(Some(x));
                                            note.set(String::new());
                                        }
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                            "Add note"
                        }
                    }
                    div { class: "desk-new", style: "margin-top:12px;",
                        p { class: "desk-muted", style: "margin:0 0 6px;", "Reply to the complainant (emailed + recorded)" }
                        textarea { class: "desk-in full", rows: "3", value: "{reply}", oninput: move |e| reply.set(e.value()) }
                        button {
                            class: "desk-btn sm",
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match desk_complaint_reply(id, reply()).await {
                                        Ok(x) => {
                                            data.set(Some(x));
                                            reply.set(String::new());
                                        }
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                            "Send reply"
                        }
                    }
                }
            }
        }
    }
}

// ---------- Removal requests panel ----------

fn removal_label(status: &str) -> &'static str {
    match status {
        "received" => "Received",
        "under_review" => "Under review",
        "upheld_removed" => "Upheld — removed",
        "rejected" => "Rejected",
        _ => "Unknown",
    }
}

fn removal_next_statuses(status: &str) -> &'static [&'static str] {
    match status {
        "received" => &["under_review", "upheld_removed", "rejected"],
        "under_review" => &["upheld_removed", "rejected"],
        _ => &[],
    }
}

#[component]
fn RemovalRequestsPanel() -> Element {
    let mut items = use_signal(|| Option::<Vec<DeskRemovalRequest>>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);
    let selected = use_signal(|| Option::<i64>::None);

    use_effect(move || {
        spawn(async move {
            busy.set(true);
            match desk_removal_requests().await {
                Ok(v) => {
                    items.set(Some(v));
                    err.set(None);
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    });

    if let Some(id) = selected() {
        if let Some(ref list) = *items.read() {
            if let Some(req) = list.iter().find(|r| r.id == id) {
                return rsx! { RemovalDetail {
                    req: req.clone(),
                    items: items,
                    selected: selected,
                    busy: busy,
                    err: err,
                } };
            }
        }
    }

    rsx! {
        div { class: "panel-head",
            h2 { "Removal requests" }
            if busy() { span { class: "badge badge-muted", "Loading\u{2026}" } }
        }
        if let Some(e) = err() {
            p { class: "desk-err", "{e}" }
        }
        match &*items.read() {
            None => rsx! { p { class: "desk-empty", "Loading\u{2026}" } },
            Some(list) if list.is_empty() => rsx! { p { class: "desk-empty", "No removal requests yet." } },
            Some(list) => rsx! {
                div { class: "complaint-list",
                    for req in list.iter() {
                        RemovalRow { req: req.clone(), selected: selected }
                    }
                }
            },
        }
    }
}

#[component]
fn RemovalRow(req: DeskRemovalRequest, mut selected: Signal<Option<i64>>) -> Element {
    let id = req.id;
    let label = removal_label(&req.status);
    let badge_class = match req.status.as_str() {
        "upheld_removed" => "badge badge-ok",
        "rejected" => "badge badge-warn",
        "under_review" => "badge badge-info",
        _ => "badge badge-muted",
    };
    let date = ymd(req.created_at);
    rsx! {
        button { class: "complaint-row", onclick: move |_| selected.set(Some(id)),
            div { class: "cr-meta",
                span { class: badge_class, "{label}" }
                span { class: "cr-ref", "PH-R{id}" }
                span { class: "cr-date", "{date}" }
            }
            div { class: "cr-subject", "{req.target_ref}" }
            div { class: "cr-from", "{req.requester_name}" }
        }
    }
}

#[component]
fn RemovalDetail(
    req: DeskRemovalRequest,
    mut items: Signal<Option<Vec<DeskRemovalRequest>>>,
    mut selected: Signal<Option<i64>>,
    mut busy: Signal<bool>,
    mut err: Signal<Option<String>>,
) -> Element {
    let mut note = use_signal(String::new);
    let id = req.id;
    let next = removal_next_statuses(&req.status);
    let decided_date = req.decided_at.map(ymd).unwrap_or_default();
    let created_date = ymd(req.created_at);
    let label = removal_label(&req.status);

    let badge_class = match req.status.as_str() {
        "upheld_removed" => "badge badge-ok",
        "rejected" => "badge badge-warn",
        "under_review" => "badge badge-info",
        _ => "badge badge-muted",
    };

    rsx! {
        div { class: "panel-head",
            button { class: "btn btn-ghost btn-sm", onclick: move |_| selected.set(None), "\u{2190} Back" }
            h2 { "PH-R{id}" }
        }
        div { class: "complaint-detail",
            div { class: "cd-header",
                span { class: badge_class, "{label}" }
                span { class: "cd-ref", "PH-R{id}" }
                span { class: "cd-date", "Received {created_date}" }
                if !req.decided_by.is_empty() {
                    span { class: "cd-date", "Decided {decided_date} by {req.decided_by}" }
                }
            }
            div { class: "cd-body",
                dl { class: "deflist",
                    div { class: "def", dt { "Entry" } dd { "{req.target_ref}" } }
                    div { class: "def", dt { "Requester" } dd { "{req.requester_name} ({req.requester_email})" } }
                    div { class: "def", dt { "Reason" } dd { "{req.reason}" } }
                    if !req.decision_note.is_empty() {
                        div { class: "def", dt { "Decision note" } dd { "{req.decision_note}" } }
                    }
                }
            }
            if !next.is_empty() {
                div { class: "cd-actions",
                    h4 { "Decision" }
                    textarea {
                        class: "cf-in cf-body",
                        rows: "3",
                        placeholder: "Decision note (required for upheld/rejected decisions)",
                        value: "{note}",
                        oninput: move |e| note.set(e.value()),
                    }
                    div { style: "display:flex; gap:10px; flex-wrap:wrap; margin-top:10px;",
                        for &s in next.iter() {
                            {
                                let note_val = note.clone();
                                let s_owned = s.to_string();
                                let btn_label = removal_label(s);
                                let btn_class = if s == "upheld_removed" {
                                    "btn btn-primary btn-sm"
                                } else {
                                    "btn btn-ghost btn-sm"
                                };
                                rsx! {
                                    button {
                                        class: btn_class,
                                        disabled: busy(),
                                        onclick: move |_| {
                                            let s2 = s_owned.clone();
                                            let n2 = note_val();
                                            spawn(async move {
                                                busy.set(true);
                                                match desk_removal_decide(id, s2, n2).await {
                                                    Ok(v) => {
                                                        items.set(Some(v));
                                                        selected.set(None);
                                                        err.set(None);
                                                    }
                                                    Err(e) => err.set(Some(e.to_string())),
                                                }
                                                busy.set(false);
                                            });
                                        },
                                        "{btn_label}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(e) = err() {
                p { class: "desk-err", "{e}" }
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
                                {
                                    let row_cat = audit_category(&r.action);
                                    let actor = r.actor.clone();
                                    let label = audit_label(&r.action);
                                    let detail = r.detail.clone();
                                    let subject = r.subject.clone();
                                    // Truncate long subjects for display; full value in title=
                                    let subject_short = if subject.chars().count() > 60 {
                                        let s: String = subject.chars().take(57).collect();
                                        format!("{s}\u{2026}")
                                    } else {
                                        subject.clone()
                                    };
                                    let when = ymd(r.ts);
                                    rsx! {
                                        tr { key: "{i}", class: "{row_cat}",
                                            td { class: "desk-muted", "{when}" }
                                            td { span { class: "pill", "{actor}" } }
                                            td {
                                                span { class: "desk-row-title", "{label}" }
                                                if !detail.is_empty() {
                                                    div { class: "desk-muted", style: "font-size:12px;", "{detail}" }
                                                }
                                            }
                                            td { class: "desk-muted desk-wrap",
                                                span { title: "{subject}", "{subject_short}" }
                                            }
                                        }
                                    }
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

/// Map an action string to a CSS category class for audit row color-coding.
fn audit_category(action: &str) -> &str {
    if action.starts_with("crawler.") || action.starts_with("lead.") {
        "a-crawler"
    } else if action.starts_with("article.create")
        || action.starts_with("article.edit")
        || action.starts_with("article.submitted")
    {
        "a-draft"
    } else if action.starts_with("article.published")
        || action.starts_with("article.corrected")
        || action.starts_with("article.correction")
        || action.starts_with("article.retracted")
        || action.starts_with("article.scheduled")
    {
        "a-publish"
    } else if action.starts_with("article.")
        || action.starts_with("complaint.")
    {
        "a-ingest"
    } else if action.starts_with("staff.") || action.starts_with("admin.") || action.starts_with("user.") {
        "a-system"
    } else {
        "a-system"
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
    let mut email = use_signal(String::new);

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
            match desk_add_staff(username(), display(), role(), password(), email()).await {
                Ok(v) => {
                    staff.set(Some(v));
                    username.set(String::new());
                    display.set(String::new());
                    password.set(String::new());
                    email.set(String::new());
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
                    input { class: "desk-in full", r#type: "email", autocomplete: "email", placeholder: "Email address (optional)", value: "{email}", oninput: move |e| email.set(e.value()) }
                    if let Some(e) = err() {
                        p { class: "desk-error", "{e}" }
                    }
                    div { class: "editor-actions",
                        span { class: "editor-hint", "They can change their password in Profile after signing in." }
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
                        thead { tr { th { "Name" } th { "Username" } th { "Role" } th { "Email" } } }
                        tbody {
                            for m in v {
                                tr { key: "{m.username}",
                                    td { span { class: "desk-row-title", "{m.display_name}" } }
                                    td { class: "desk-muted", "{m.username}" }
                                    td { span { class: "desk-role", "{role_label(&m.role)}" } }
                                    td { class: "desk-muted", if m.email.is_empty() { "—" } else { "{m.email}" } }
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
fn ProfilePanel(user: DeskSession) -> Element {
    let mut current = use_signal(String::new);
    let mut newpw = use_signal(String::new);
    let mut confirm = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut ok = use_signal(|| Option::<String>::None);
    let mut err = use_signal(|| Option::<String>::None);

    // Fetch the signed-in account's email from the server.
    let email_res = use_resource(move || async move { staff_profile().await });

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

    let email_display = match &*email_res.read() {
        Some(Ok(e)) if !e.is_empty() => e.clone(),
        Some(Ok(_)) => "—".to_string(),
        Some(Err(_)) => "—".to_string(),
        None => "Loading…".to_string(),
    };

    rsx! {
        section { class: "desk-panel",
            div { class: "desk-panel-head",
                h2 { "Profile" }
            }
            div { class: "desk-new",
                p { class: "desk-muted", style: "margin:0 0 6px;", "Account details" }
                table { class: "desk-table",
                    tbody {
                        tr {
                            td { class: "desk-muted", "Username" }
                            td { "{user.username}" }
                        }
                        tr {
                            td { class: "desk-muted", "Display name" }
                            td { "{user.display_name}" }
                        }
                        tr {
                            td { class: "desk-muted", "Role" }
                            td { span { class: "desk-role", "{role_label(&user.role)}" } }
                        }
                        tr {
                            td { class: "desk-muted", "Email" }
                            td { "{email_display}" }
                        }
                    }
                }
            }
            div { class: "desk-new", style: "margin-top:24px;",
                p { class: "desk-muted", style: "margin:0 0 14px;", "Change your password." }
                form { onsubmit: submit,
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
            TotpSection { username: user.username.clone() }
            if user.role == "admin" {
                RegulatoryStatusCard {}
            }
        }
    }
}

// ── Two-factor authentication section for ProfilePanel ────────────────────

/// The three states the 2FA section can be in.
#[derive(Clone, PartialEq)]
enum TotpState {
    /// Checking whether TOTP is enrolled.
    Loading,
    /// TOTP is not enrolled — show "Set up" button.
    Off,
    /// Enrolment started — show the secret/URI + a code field.
    Enrolling {
        secret: String,
        uri: String,
    },
    /// TOTP is enrolled — show "Disable" button.
    On,
}

#[component]
fn TotpSection(username: String) -> Element {
    let mut state = use_signal(|| TotpState::Loading);
    let mut totp_code = use_signal(String::new);
    let mut disable_pw = use_signal(String::new);
    let mut totp_busy = use_signal(|| false);
    let mut totp_ok = use_signal(|| Option::<String>::None);
    let mut totp_err = use_signal(|| Option::<String>::None);

    // On mount, check if TOTP is already enrolled.
    use_resource(move || async move {
        match staff_totp_status().await {
            Ok(true) => state.set(TotpState::On),
            Ok(false) => state.set(TotpState::Off),
            Err(_) => state.set(TotpState::Off),
        }
    });

    // "Set up two-factor authentication"
    let begin = move |_| {
        spawn(async move {
            totp_busy.set(true);
            totp_err.set(None);
            totp_ok.set(None);
            match staff_totp_begin().await {
                Ok((secret, uri)) => state.set(TotpState::Enrolling { secret, uri }),
                Err(e) => totp_err.set(Some(e.to_string())),
            }
            totp_busy.set(false);
        });
    };

    // "Confirm" — verifies the code and persists the pending secret.
    let confirm_enrol = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            totp_busy.set(true);
            totp_err.set(None);
            totp_ok.set(None);
            match staff_totp_enable(totp_code()).await {
                Ok(()) => {
                    state.set(TotpState::On);
                    totp_code.set(String::new());
                    totp_ok.set(Some("Two-factor authentication is now active.".to_string()));
                }
                Err(e) => totp_err.set(Some(e.to_string())),
            }
            totp_busy.set(false);
        });
    };

    // "Disable two-factor authentication"
    let disable = move |evt: FormEvent| {
        evt.prevent_default();
        spawn(async move {
            totp_busy.set(true);
            totp_err.set(None);
            totp_ok.set(None);
            match staff_totp_disable(disable_pw()).await {
                Ok(()) => {
                    state.set(TotpState::Off);
                    disable_pw.set(String::new());
                    totp_ok.set(Some("Two-factor authentication has been disabled.".to_string()));
                }
                Err(e) => totp_err.set(Some(e.to_string())),
            }
            totp_busy.set(false);
        });
    };

    let cur_state = state.read().clone();
    rsx! {
        div { class: "desk-new", style: "margin-top:24px;",
            p { class: "desk-muted", style: "margin:0 0 14px;", "Two-factor authentication (2FA)" }
            match cur_state {
                TotpState::Loading => rsx! {
                    p { class: "desk-muted", "Checking…" }
                },
                TotpState::Off => rsx! {
                    p { style: "margin:0 0 10px; font-size:.9rem;",
                        "2FA is not enabled. Add an extra layer of security by requiring \
                         an authenticator code at login."
                    }
                    if let Some(m) = totp_ok() {
                        p { class: "desk-ok", "{m}" }
                    }
                    if let Some(e) = totp_err() {
                        p { class: "desk-error", "{e}" }
                    }
                    button {
                        class: "desk-btn sm",
                        r#type: "button",
                        disabled: totp_busy(),
                        onclick: begin,
                        "Set up two-factor authentication"
                    }
                },
                TotpState::Enrolling { secret, uri } => rsx! {
                    p { style: "margin:0 0 8px; font-size:.9rem;",
                        "Scan the URI below with your authenticator app (Google Authenticator, \
                         Authy, 1Password, etc.), then enter the 6-digit code it shows to confirm."
                    }
                    p { class: "desk-muted", style: "margin:0 0 4px; font-size:.75rem;", "Secret key (manual entry)" }
                    p {
                        class: "desk-code",
                        style: "font-family:monospace; word-break:break-all; margin:0 0 8px; padding:6px 8px; \
                                background:var(--surface2,#f4f4f5); border-radius:4px; font-size:.85rem;",
                        "{secret}"
                    }
                    p { class: "desk-muted", style: "margin:0 0 4px; font-size:.75rem;", "otpauth URI" }
                    p {
                        style: "font-family:monospace; word-break:break-all; margin:0 0 12px; padding:6px 8px; \
                                background:var(--surface2,#f4f4f5); border-radius:4px; font-size:.75rem;",
                        "{uri}"
                    }
                    form { onsubmit: confirm_enrol,
                        input {
                            class: "desk-in",
                            r#type: "text",
                            autocomplete: "one-time-code",
                            inputmode: "numeric",
                            pattern: "[0-9]*",
                            maxlength: 6,
                            placeholder: "6-digit code from app",
                            value: "{totp_code}",
                            oninput: move |e| totp_code.set(e.value()),
                        }
                        if let Some(e) = totp_err() {
                            p { class: "desk-error", "{e}" }
                        }
                        button {
                            class: "desk-btn sm",
                            r#type: "submit",
                            disabled: totp_busy(),
                            "Confirm and enable 2FA"
                        }
                    }
                },
                TotpState::On => rsx! {
                    p { style: "margin:0 0 10px; font-size:.9rem;",
                        "Two-factor authentication is enabled. \
                         You will be asked for your authenticator code each time you sign in."
                    }
                    form { onsubmit: disable,
                        input {
                            class: "desk-in full",
                            r#type: "password",
                            autocomplete: "current-password",
                            placeholder: "Current password to confirm",
                            value: "{disable_pw}",
                            oninput: move |e| disable_pw.set(e.value()),
                        }
                        if let Some(m) = totp_ok() {
                            p { class: "desk-ok", "{m}" }
                        }
                        if let Some(e) = totp_err() {
                            p { class: "desk-error", "{e}" }
                        }
                        button {
                            class: "desk-btn sm danger",
                            r#type: "submit",
                            disabled: totp_busy(),
                            "Disable two-factor authentication"
                        }
                    }
                },
            }
        }
    }
}

/// Admin-only: flip whether we publicly claim to be registered with our press
/// regulator (IMPRESS). Off by default; turning it on publishes the "regulated by"
/// footer statement and enables complaint escalation to IMPRESS. Every change is
/// audited server-side. Live visitors see it immediately; crawlers / no-JS update
/// at the next deploy (an under-claim in the interim — never an over-claim).
#[component]
fn RegulatoryStatusCard() -> Element {
    // None = still loading the current value.
    let mut registered = use_signal(|| Option::<bool>::None);
    let mut busy = use_signal(|| false);
    let mut err = use_signal(|| Option::<String>::None);

    use_resource(move || async move {
        match regulator_registered().await {
            Ok(v) => registered.set(Some(v)),
            Err(e) => err.set(Some(e.to_string())),
        }
    });

    let cur = *registered.read();
    rsx! {
        div { class: "desk-new", style: "margin-top:24px;",
            p { class: "desk-muted", style: "margin:0 0 6px;", "Press regulation" }
            match cur {
                None => rsx! {
                    if let Some(e) = err() {
                        p { class: "desk-error", "{e}" }
                    } else {
                        p { class: "desk-muted", "Loading…" }
                    }
                },
                Some(is_reg) => rsx! {
                    p { style: "margin:0 0 10px;",
                        "Current status: "
                        strong {
                            if is_reg { "Registered with IMPRESS" } else { "Not registered — intend to seek registration" }
                        }
                    }
                    p { class: "desk-muted", style: "margin:0 0 14px; font-size:.82rem; line-height:1.5;",
                        "Only switch this on once IMPRESS registration is confirmed. It publishes the “regulated by IMPRESS” statement in the site footer and turns on complaint escalation to IMPRESS. Live visitors see the change at once; search engines and no-JS visitors catch up at the next site deploy. Every change is recorded in the audit log."
                    }
                    if let Some(e) = err() {
                        p { class: "desk-error", "{e}" }
                    }
                    if is_reg {
                        button {
                            class: "desk-btn ghost",
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match set_regulator_registered(false).await {
                                        Ok(()) => registered.set(Some(false)),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                            if busy() { "Saving…" } else { "Mark as NOT registered" }
                        }
                    } else {
                        button {
                            class: "desk-btn",
                            disabled: busy(),
                            onclick: move |_| {
                                spawn(async move {
                                    busy.set(true);
                                    err.set(None);
                                    match set_regulator_registered(true).await {
                                        Ok(()) => registered.set(Some(true)),
                                        Err(e) => err.set(Some(e.to_string())),
                                    }
                                    busy.set(false);
                                });
                            },
                            if busy() { "Saving…" } else { "Mark as registered with IMPRESS" }
                        }
                    }
                }
            }
        }
    }
}

fn complaint_label(status: &str, registered: bool) -> &str {
    match status {
        "received" => "Received",
        "acknowledged" => "Acknowledged",
        "under_investigation" => "Under investigation",
        "upheld" => "Upheld",
        "partly_upheld" => "Partly upheld",
        "not_upheld" => "Not upheld",
        "closed" => "Closed",
        "escalated" => {
            if registered { "Escalated to IMPRESS" } else { "Escalated" }
        }
        _ => status,
    }
}

/// The next statuses an editor can move a complaint to (the IMPRESS workflow).
/// `registered` gates the IMPRESS-named escalation label (see RegulatorStatus).
fn complaint_next_statuses(status: &str, registered: bool) -> Vec<(&'static str, &'static str)> {
    // Only claim an IMPRESS escalation route once we are actually registered.
    let escalate_label = if registered {
        "Escalate to IMPRESS"
    } else {
        "Escalate"
    };
    match status {
        "received" => vec![("acknowledged", "Acknowledge")],
        "acknowledged" => vec![("under_investigation", "Start investigation")],
        "under_investigation" => vec![
            ("upheld", "Uphold"),
            ("partly_upheld", "Partly uphold"),
            ("not_upheld", "Not upheld"),
        ],
        "upheld" | "partly_upheld" | "not_upheld" => {
            vec![("closed", "Close"), ("escalated", escalate_label)]
        }
        "closed" => vec![("escalated", escalate_label)],
        _ => vec![],
    }
}

/// IMPRESS pre-publish checklist. Shown when an editor moves a story to Published
/// or Scheduled; the confirmations are recorded in the review log + audit trail as
/// the sign-off note. Publishing is blocked until all four checkboxes are ticked AND:
///   a public-interest justification has been typed, AND
///   if the story names a person before charge, a separate documented
///   justification for that naming has been provided.
///
/// A fourth mandatory checkbox enforces victim-anonymity / reporting-restriction
/// confirmation (IMPRESS children+justice; IPSO Clauses 7 & 11). If the source
/// lead flagged `identification_risk` or `restrictions_review` a prominent warning
/// banner is also shown.
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
    let mut c4 = use_signal(|| false);
    // Public-interest justification (free text, required).
    let mut pi_just: Signal<String> = use_signal(|| String::new());
    // Editor attests the story names a person before charge.
    let mut pre_charge = use_signal(|| false);
    // Documented justification for naming before charge (required when pre_charge).
    let mut pc_just: Signal<String> = use_signal(|| String::new());

    let p = pending.read().clone();
    let Some((id, to, label)) = p else {
        return rsx! {};
    };

    // Look up the pending article's risk flags from the already-loaded article list.
    let (id_risk, restrictions_review) = articles
        .read()
        .as_ref()
        .and_then(|list| list.iter().find(|a| a.id == id))
        .map(|a| (a.id_risk, a.restrictions_review))
        .unwrap_or((false, false));

    let show_banner = id_risk || restrictions_review;

    // All gates must be satisfied before "Confirm + publish" is enabled.
    let pi_ok = !pi_just.read().trim().is_empty();
    let pc_ok = !pre_charge() || !pc_just.read().trim().is_empty();
    let ready = c1() && c2() && c3() && c4() && pi_ok && pc_ok;

    let confirm = move |_| {
        let pi_text = pi_just.read().trim().to_string();
        let pc_text = pc_just.read().trim().to_string();
        if !(c1() && c2() && c3() && c4()) || pi_text.is_empty() || (pre_charge() && pc_text.is_empty()) {
            return;
        }
        let to = to.clone();
        // Build the note dynamically so the audit trail records what was confirmed.
        let anon_clause = if id_risk {
            "; victim-anonymity + jigsaw-ID risk confirmed (IPSO Cl.7/11 — id_risk flag)"
        } else if restrictions_review {
            "; victim-anonymity confirmed (IPSO Cl.7/11 — sexual/child category)"
        } else {
            "; victim-anonymity / reporting-restriction confirmed (IPSO Cl.7/11)"
        };
        let pc_section = if pre_charge() {
            format!(" | PRE-CHARGE NAMING justification: {}", pc_text)
        } else {
            String::new()
        };
        let note = format!(
            "IMPRESS sign-off: case concluded (no active proceedings); public interest + accuracy checked; AI-assistance + pre-charge naming reviewed{anon_clause} | Public-interest justification: {}{}",
            pi_text, pc_section
        );
        spawn(async move {
            busy.set(true);
            err.set(None);
            match desk_transition(id, to, note).await {
                Ok(list) => {
                    articles.set(Some(list));
                    pending.set(None);
                    c1.set(false);
                    c2.set(false);
                    c3.set(false);
                    c4.set(false);
                    pi_just.set(String::new());
                    pre_charge.set(false);
                    pc_just.set(String::new());
                }
                Err(e) => err.set(Some(e.to_string())),
            }
            busy.set(false);
        });
    };

    // Precompute the pre_charge flag for use in RSX.
    let names_before_charge = pre_charge();

    rsx! {
        div { class: "modal-scrim", onclick: move |_| pending.set(None),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                p { class: "desk-eyebrow", "Pre-publish checks" }
                h3 { class: "modal-title", "Going public: {label}" }
                if show_banner {
                    div { class: "modal-warn-banner",
                        if id_risk {
                            span { class: "modal-warn-icon", "\u{26a0}\u{fe0f}" }
                            strong { "Identification risk flagged" }
                            p {
                                "The source lead indicates a victim or child may be identifiable. "
                                "Jigsaw identification is prohibited even when no single detail alone "
                                "names the person (IPSO Clause 7 — children; Clause 11 — sexual-assault victims; IMPRESS Standards)."
                            }
                        } else {
                            span { class: "modal-warn-icon", "\u{26a0}\u{fe0f}" }
                            strong { "Reporting restrictions apply" }
                            p {
                                "This article covers a sexual-offence or child case. "
                                "Automatic anonymity duties apply to victims and children under the Sexual Offences (Amendment) Act 1992 "
                                "and the Children and Young Persons Act 1933. Verify before publishing (IPSO Clauses 7 & 11; IMPRESS Standards)."
                            }
                        }
                    }
                }
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
                label { class: "modal-check modal-check-anon",
                    input { r#type: "checkbox", checked: c4(), onchange: move |e| c4.set(e.checked()) }
                    span {
                        "I confirm that publishing this does not breach any reporting restriction "
                        "and that no victim or child is identifiable, directly or by jigsaw "
                        "(IMPRESS; IPSO Clauses 7 \u{0026} 11)."
                    }
                }

                // Public-interest justification — required before publish.
                p { class: "desk-muted", style: "margin:14px 0 4px; font-weight:600;",
                    "Public-interest justification (required)"
                }
                p { class: "desk-muted", style: "margin:0 0 6px; font-size:0.85em;",
                    "Briefly state why publishing this story is in the public interest. This is recorded in the audit trail."
                }
                textarea {
                    style: "width:100%; min-height:72px; box-sizing:border-box; padding:6px 8px; font-size:0.9em; border:1px solid #ccc; border-radius:4px; resize:vertical;",
                    placeholder: "e.g. Informs the public of a convicted predator in the local community\u{2026}",
                    value: "{pi_just}",
                    oninput: move |e| pi_just.set(e.value()),
                }

                // Pre-charge naming — editor self-declares; hard-blocks publish if set without justification.
                p { class: "desk-muted", style: "margin:14px 0 4px; font-weight:600;",
                    "Pre-charge naming"
                }
                label { class: "modal-check",
                    input {
                        r#type: "checkbox",
                        checked: names_before_charge,
                        onchange: move |e| pre_charge.set(e.checked()),
                    }
                    span { "This story names a person who has not yet been charged." }
                }
                if names_before_charge {
                    div { class: "modal-warn-banner",
                        span { class: "modal-warn-icon", "\u{26a0}\u{fe0f}" }
                        strong { "Warning: naming a person before charge" }
                        p {
                            "Publishing a name before charge carries significant legal and ethical risk. "
                            "A documented public-interest justification for this specific naming decision "
                            "is required before this story can be published."
                        }
                        p { style: "margin:10px 0 4px; font-weight:600; font-size:0.9em;",
                            "Justification for naming before charge (required)"
                        }
                        textarea {
                            style: "width:100%; min-height:72px; box-sizing:border-box; padding:6px 8px; font-size:0.9em; border:1px solid var(--red); border-radius:4px; resize:vertical;",
                            placeholder: "State specifically why this naming is justified despite no charge having been made\u{2026}",
                            value: "{pc_just}",
                            oninput: move |e| pc_just.set(e.value()),
                        }
                    }
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
                if a.state == "draft" && a.is_ai_assisted {
                    button {
                        class: "desk-act",
                        disabled: busy(),
                        onclick: move |_| {
                            spawn(async move {
                                busy.set(true);
                                err.set(None);
                                match desk_regenerate_draft(id).await {
                                    Ok(()) => { if let Ok(list) = desk_articles().await { articles.set(Some(list)); } }
                                    Err(e) => err.set(Some(e.to_string())),
                                }
                                busy.set(false);
                            });
                        },
                        "Regenerate \u{27f3}"
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
                        init_ai_assisted: false,
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
                init_ai_assisted: a.is_ai_assisted,
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
    init_ai_assisted: bool,
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
    let verify_count = body().matches("[VERIFY").count() + body().matches("[FROM RECORD").count();

    rsx! {
        form { class: "editor", onsubmit: submit,
            if init_ai_assisted {
                div { class: "desk-error",
                    "\u{26a0} AI-assisted draft \u{2014} written by AI from an unverified lead. Verify every fact against the court record, clear reporting restrictions, and confirm the conviction before submitting."
                }
            }
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
                label {
                    span { "Social / OG image URL" }
                    input {
                        r#type: "text",
                        value: "{og_image}",
                        oninput: move |e| og_image.set(e.value()),
                        placeholder: "/assets/og/your-image.jpg",
                    }
                }
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
            // hidden bridge to the canonical `body` (read by save) in Visual mode.
            // In Markdown mode the right pane is a live preview using the SAME
            // md.rs renderer as the public article page so authors see the real
            // published look as they type. ----
            {
                // Precompute preview HTML outside rsx! to satisfy the Dioxus rule
                // that function calls must live in pure "{expr}" text nodes only.
                let preview_title = title();
                let preview_summary = summary();
                let preview_html: String = body()
                    .lines()
                    .map(|line| crate::md::block_html(line))
                    .collect::<Vec<_>>()
                    .join("\n");
                let preview_empty = preview_html.trim().is_empty();
                let compose_class = if mode() == EdMode::Visual {
                    "editor-panes is-hidden"
                } else {
                    "editor-panes"
                };
                rsx! {
                    div { class: "{compose_class}",
                        // ---- Left: compose column ----
                        div { class: "editor-compose",
                            textarea {
                                id: "ed-body",
                                class: "editor-body",
                                rows: "16",
                                placeholder: "Write the story. Leave a blank line or a new line between paragraphs.\n\n## Subheading\n- Bullet point\n^ Drop cap paragraph\n> Blockquote\n>> Pull quote",
                                value: "{body}",
                                oninput: move |e| body.set(e.value()),
                            }
                            // Formatting hints strip
                            div { class: "editor-fmt-hints",
                                span { class: "fmt-hint", code { "##" } " subhead" }
                                span { class: "fmt-hint", code { "-" } " bullet" }
                                span { class: "fmt-hint", code { "^" } " drop cap" }
                                span { class: "fmt-hint", code { ">" } " quote" }
                                span { class: "fmt-hint", code { ">>" } " pull quote" }
                                span { class: "fmt-hint", code { "![alt](url)" } " image" }
                            }
                        }
                        // ---- Right: live preview pane (real published look) ----
                        div { class: "editor-preview-pane",
                            span { class: "editor-prev-label", "Live preview \u{2014} published look" }
                            if !preview_title.is_empty() || !preview_summary.is_empty() || !preview_empty {
                                div { class: "art-page art-page--desk-preview",
                                    if !preview_title.is_empty() {
                                        h1 { class: "art-headline", "{preview_title}" }
                                    }
                                    if !preview_summary.is_empty() {
                                        p { class: "art-standfirst", "{preview_summary}" }
                                    }
                                    div {
                                        class: "art-prose",
                                        dangerous_inner_html: "{preview_html}",
                                    }
                                }
                            } else {
                                p { class: "editor-prev-empty", "Start writing to see the published preview\u{2026}" }
                            }
                        }
                    }
                }
            }
            div { class: "editor-meters",
                span { class: title_state, "Headline " b { "{title_len}" } " / ~65" }
                span { class: sum_state, "Standfirst " b { "{sum_len}" } }
                span { class: "meter", "Body " b { "{words}" } " words · {mins} min read" }
                span { class: meta_state, "Meta " b { "{meta_len}" } " / ~155" }
                if verify_count > 0 {
                    span { class: "meter warn", "\u{26a0} {verify_count} to verify" }
                    button {
                        r#type: "button",
                        class: "tb",
                        title: "Jump to the first [VERIFY] marker",
                        onclick: move |_| {
                            let _ = document::eval("(function(){var t=document.getElementById('ed-body');if(!t)return;var i=t.value.indexOf('[VERIFY');if(i<0)i=t.value.indexOf('[FROM RECORD');if(i<0)return;t.focus();t.setSelectionRange(i,i);var before=t.value.slice(0,i).split('\\n').length;t.scrollTop=Math.max(0,(before-2)*18);})();");
                        },
                        "Jump to \u{2192} [VERIFY]"
                    }
                }
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
                p { class: "intake-imgnote", "\u{2316} Source image \u{2014} carried into the draft; verify usage rights before publishing. {lead.image_attribution}" }
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
                                if busy() { "Drafting\u{2026}" } else { "Confirm \u{2192} edit" }
                            }
                            if busy() {
                                span { class: "intake-step2-in", "generating a draft\u{2026}" }
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
