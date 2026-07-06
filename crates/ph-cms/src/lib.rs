//! Editorial CMS core for PH Press: a SQLite store, staff auth (Argon2id), the
//! article lifecycle state machine with a mandatory legal sign-off before
//! publish, and a hash-chained audit trail ([`ph_audit`]) of every action.
//!
//! SERVER-ONLY. `sqlx` does not compile to wasm, so the web/SSG build never
//! depends on this crate; ph-press pulls it behind its `server` feature.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

/// The database handle (an sqlx SQLite pool). Re-exported so callers need not
/// depend on sqlx directly.
pub type Db = SqlitePool;

/// The PUBLIC crawler-ingest pipeline (sources, leads, conviction database) and
/// the PRIVATE court-watch store live in their own modules. They are split so the
/// active-proceedings firewall is a module boundary: `courtwatch` (live/upcoming
/// proceedings) never writes into `ingest` (post-conviction / public), and
/// `ingest` never reads `courtwatch`.
pub mod courtwatch;
pub mod ingest;

#[derive(Debug, thiserror::Error)]
pub enum CmsError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("authentication failed")]
    Auth,
    #[error("invalid lifecycle transition: {from} -> {to} (role {role})")]
    Transition {
        from: String,
        to: String,
        role: String,
    },
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("bad value: {0}")]
    Bad(String),
}
pub type Result<T> = std::result::Result<T, CmsError>;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ===================== roles =====================
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Writer,
    SubEditor,
    Editor,
    Legal,
    Admin,
}
impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Writer => "writer",
            Role::SubEditor => "sub_editor",
            Role::Editor => "editor",
            Role::Legal => "legal",
            Role::Admin => "admin",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "writer" => Role::Writer,
            "sub_editor" => Role::SubEditor,
            "editor" => Role::Editor,
            "legal" => Role::Legal,
            "admin" => Role::Admin,
            _ => return Err(CmsError::Bad(format!("role: {s}"))),
        })
    }
}

// ===================== lifecycle =====================
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    Draft,
    Submitted,
    EditorialReview,
    LegalReview,
    Scheduled,
    Published,
    Corrected,
    Retracted,
}
impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Draft => "draft",
            State::Submitted => "submitted",
            State::EditorialReview => "editorial_review",
            State::LegalReview => "legal_review",
            State::Scheduled => "scheduled",
            State::Published => "published",
            State::Corrected => "corrected",
            State::Retracted => "retracted",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "draft" => State::Draft,
            "submitted" => State::Submitted,
            "editorial_review" => State::EditorialReview,
            "legal_review" => State::LegalReview,
            "scheduled" => State::Scheduled,
            "published" => State::Published,
            "corrected" => State::Corrected,
            "retracted" => State::Retracted,
            _ => return Err(CmsError::Bad(format!("state: {s}"))),
        })
    }
    /// Visible on the public site?
    pub fn is_public(self) -> bool {
        matches!(self, State::Published | State::Corrected)
    }
    /// Every lifecycle state, for enumerating valid transitions.
    pub const ALL: [State; 8] = [
        State::Draft,
        State::Submitted,
        State::EditorialReview,
        State::LegalReview,
        State::Scheduled,
        State::Published,
        State::Corrected,
        State::Retracted,
    ];
}

/// The gated transition table. Crucially, `Published` is only reachable via
/// `LegalReview` (a legal sign-off must have happened) — never straight from a
/// draft or editorial review. Admin can perform any *defined* transition, but
/// undefined transitions are always refused (no skipping legal).
pub fn can_transition(from: State, to: State, role: Role) -> bool {
    use Role::*;
    use State::*;
    let admin = matches!(role, Admin);
    match (from, to) {
        (Draft, Submitted) => matches!(role, Writer | SubEditor | Editor) || admin,
        (Submitted, EditorialReview) => matches!(role, SubEditor | Editor) || admin,
        (Submitted, Draft) => matches!(role, SubEditor | Editor) || admin,
        (EditorialReview, LegalReview) => matches!(role, Editor) || admin,
        (EditorialReview, Draft) => matches!(role, Editor) || admin,
        // legal sign-off:
        (LegalReview, Scheduled) => matches!(role, Legal) || admin,
        (LegalReview, Published) => matches!(role, Legal) || admin,
        (LegalReview, EditorialReview) => matches!(role, Legal | Editor) || admin,
        (Scheduled, Published) => matches!(role, Editor) || admin,
        (Published, Corrected) => matches!(role, Editor) || admin,
        (Corrected, Corrected) => matches!(role, Editor) || admin,
        (Published, Retracted) => matches!(role, Editor | Admin),
        (Corrected, Retracted) => matches!(role, Editor | Admin),
        _ => false,
    }
}

/// The states this `role` may move an article to FROM `from` (excluding `from`
/// itself). Drives the dashboard's per-article action buttons — the gate stays
/// authoritative in `can_transition`, so the UI can never offer an illegal move.
pub fn allowed_transitions(from: State, role: Role) -> Vec<State> {
    State::ALL
        .into_iter()
        .filter(|&to| to != from && can_transition(from, to, role))
        .collect()
}

// ===================== auth =====================
pub mod auth {
    use crate::{CmsError, Result};
    use argon2::password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    };
    use argon2::Argon2;

    pub fn hash_password(pw: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(pw.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| CmsError::Auth)
    }
    pub fn verify_password(hash: &str, pw: &str) -> bool {
        PasswordHash::new(hash)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(pw.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false)
    }
}

// ===================== models =====================
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StaffUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub created_at: i64,
    /// Contact email for password recovery (and, later, notifications). Optional:
    /// accounts created before the recovery feature have none until set.
    pub email: Option<String>,
}
impl StaffUser {
    pub fn role(&self) -> Result<Role> {
        Role::parse(&self.role)
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Article {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub body: String, // JSON array of paragraphs
    pub byline: String,
    pub kind: String,
    pub section: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: String, // JSON array of strings
    pub state: String,
    pub is_ai_assisted: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: Option<i64>,
}
impl Article {
    pub fn state(&self) -> Result<State> {
        State::parse(&self.state)
    }
}

// ===================== database =====================
pub async fn connect(url: &str) -> Result<SqlitePool> {
    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await?)
}

/// Apply pending schema migrations (versioned in ./migrations, tracked in
/// _sqlx_migrations). Each runs exactly once, so deploys never recreate or wipe
/// existing data. Add schema changes as new migration files.
pub async fn init(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| CmsError::Bad(format!("migration failed: {e}")))?;
    Ok(())
}

/// Deliberately reset the named admin/user's password (operator takeover via
/// deploy, gated by PH_ADMIN_RESET). Returns true if a row was updated.
pub async fn reset_password(pool: &SqlitePool, username: &str, new_password: &str) -> Result<bool> {
    let hash = auth::hash_password(new_password)?;
    let res = sqlx::query("UPDATE staff_user SET password_hash = ? WHERE username = ?")
        .bind(hash)
        .bind(username)
        .execute(pool)
        .await?;
    let changed = res.rows_affected() > 0;
    if changed {
        append_audit(
            pool,
            "system",
            "admin.password_reset",
            username,
            "via deploy",
        )
        .await?;
    }
    Ok(changed)
}

/// Self-service password change: verify the current password, then set the new
/// one. Returns `CmsError::Auth` if the current password is wrong. Audited.
pub async fn change_password(
    pool: &SqlitePool,
    username: &str,
    current: &str,
    new_password: &str,
) -> Result<()> {
    let user = find_user(pool, username).await?.ok_or(CmsError::Auth)?;
    if !auth::verify_password(&user.password_hash, current) {
        return Err(CmsError::Auth);
    }
    let hash = auth::hash_password(new_password)?;
    sqlx::query("UPDATE staff_user SET password_hash = ? WHERE username = ?")
        .bind(hash)
        .bind(username)
        .execute(pool)
        .await?;
    append_audit(
        pool,
        username,
        "staff.password_change",
        username,
        "self-service",
    )
    .await?;
    Ok(())
}

// ===================== password recovery =====================
/// Lifetime of a password-reset link (1 hour).
pub const RESET_TTL_SECS: i64 = 60 * 60;

/// Set (or clear, when `email` is blank) a staff account's contact email. Stored
/// lowercased so recovery lookups are case-insensitive. Idempotent — used by the
/// deploy bootstrap (PH_ADMIN_EMAIL) and the admin Staff tab. Returns true if a
/// row was updated.
pub async fn set_user_email(pool: &SqlitePool, username: &str, email: &str) -> Result<bool> {
    let email = email.trim().to_lowercase();
    let res = sqlx::query("UPDATE staff_user SET email = ? WHERE username = ?")
        .bind(if email.is_empty() { None } else { Some(email) })
        .bind(username)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

/// Find a staff account by contact email (case-insensitive). None if unset/unknown.
pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<StaffUser>> {
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Ok(None);
    }
    Ok(
        sqlx::query_as::<_, StaffUser>("SELECT * FROM staff_user WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?,
    )
}

/// Mint a single-use password-reset token for the account at `email`, valid for
/// `ttl_secs`. Returns the RAW token (put it in the reset link) and the account;
/// only the token's SHA-256 is persisted, so a DB leak yields no usable link. Any
/// earlier token for the account is dropped first, so only the newest link works.
///
/// Returns `None` when no account has that email. Callers MUST report the same
/// "if that account exists, we've sent a link" message either way, so the
/// endpoint never reveals which emails are registered.
pub async fn create_password_reset(
    pool: &SqlitePool,
    email: &str,
    ttl_secs: i64,
) -> Result<Option<(String, StaffUser)>> {
    let Some(user) = find_user_by_email(pool, email).await? else {
        return Ok(None);
    };
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let token = hex(&raw);
    let t = now();
    // One active link per account: invalidate any prior tokens before issuing.
    sqlx::query("DELETE FROM password_reset_token WHERE user_id = ?")
        .bind(user.id)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO password_reset_token (user_id, token_hash, created_at, expires_at) VALUES (?,?,?,?)",
    )
    .bind(user.id)
    .bind(token_hash(&token))
    .bind(t)
    .bind(t + ttl_secs)
    .execute(pool)
    .await?;
    append_audit(
        pool,
        "system",
        "staff.password_reset_requested",
        &user.username,
        "reset link issued",
    )
    .await?;
    Ok(Some((token, user)))
}

/// Redeem a reset token: if it is known, unused and unexpired, set the account's
/// new password, mark the token used, and destroy ALL of that user's sessions (so
/// a stolen session can't outlive the reset). Returns the account on success,
/// `CmsError::Auth` for an invalid/expired/already-used token. The caller is
/// responsible for password-strength validation (as elsewhere in this crate).
pub async fn consume_password_reset(
    pool: &SqlitePool,
    token: &str,
    new_password: &str,
) -> Result<StaffUser> {
    let h = token_hash(token);
    let row = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
        "SELECT user_id, expires_at, used_at FROM password_reset_token WHERE token_hash = ?",
    )
    .bind(&h)
    .fetch_optional(pool)
    .await?;
    let Some((user_id, expires_at, used_at)) = row else {
        return Err(CmsError::Auth);
    };
    if used_at.is_some() || expires_at <= now() {
        return Err(CmsError::Auth);
    }
    let user = sqlx::query_as::<_, StaffUser>("SELECT * FROM staff_user WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(CmsError::Auth)?;
    let hash = auth::hash_password(new_password)?;
    sqlx::query("UPDATE staff_user SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(user_id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE password_reset_token SET used_at = ? WHERE token_hash = ?")
        .bind(now())
        .bind(&h)
        .execute(pool)
        .await?;
    // A reset invalidates every existing session for the account.
    sqlx::query("DELETE FROM session WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    append_audit(
        pool,
        &user.username,
        "staff.password_reset",
        &user.username,
        "via reset link",
    )
    .await?;
    Ok(user)
}

pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM staff_user")
            .fetch_one(pool)
            .await?,
    )
}

/// True when there are no staff users yet — the `/desk` first-run install screen
/// shows until the first administrator account is created.
pub async fn needs_install(pool: &SqlitePool) -> Result<bool> {
    Ok(count_users(pool).await? == 0)
}

/// Create a staff user. The very first user must be an admin (bootstrap gate);
/// after that, only an existing admin should call this (enforce at the API layer).
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    display_name: &str,
    role: Role,
    password: &str,
    email: &str,
) -> Result<i64> {
    if count_users(pool).await? == 0 && role != Role::Admin {
        return Err(CmsError::Forbidden(
            "the first user must be an admin".into(),
        ));
    }
    let hash = auth::hash_password(password)?;
    let email = email.trim().to_lowercase();
    let email_val: Option<String> = if email.is_empty() { None } else { Some(email) };
    let res = sqlx::query(
        "INSERT INTO staff_user (username, display_name, role, password_hash, created_at, email) VALUES (?,?,?,?,?,?)",
    )
    .bind(username)
    .bind(display_name)
    .bind(role.as_str())
    .bind(hash)
    .bind(now())
    .bind(email_val)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn find_user(pool: &SqlitePool, username: &str) -> Result<Option<StaffUser>> {
    Ok(
        sqlx::query_as::<_, StaffUser>("SELECT * FROM staff_user WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await?,
    )
}

/// All staff users, oldest first — for the admin Staff tab + the public team page.
/// Callers strip the password hash / TOTP secret before returning to the client.
pub async fn list_staff(pool: &SqlitePool) -> Result<Vec<StaffUser>> {
    Ok(
        sqlx::query_as::<_, StaffUser>("SELECT * FROM staff_user ORDER BY id")
            .fetch_all(pool)
            .await?,
    )
}

/// Verify a username + password. (TOTP 2FA is checked separately at the API layer.)
pub async fn authenticate(pool: &SqlitePool, username: &str, password: &str) -> Result<StaffUser> {
    let user = find_user(pool, username).await?.ok_or(CmsError::Auth)?;
    if auth::verify_password(&user.password_hash, password) {
        Ok(user)
    } else {
        Err(CmsError::Auth)
    }
}

// ===================== sessions =====================
/// A validated staff session — what the API layer trusts after checking the cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
}
impl Session {
    pub fn role(&self) -> Result<Role> {
        Role::parse(&self.role)
    }
}

/// Default editorial session lifetime (12 hours).
pub const SESSION_TTL_SECS: i64 = 12 * 60 * 60;

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(token.as_bytes()))
}

/// Mint a session for an authenticated user. Returns the RAW token (put it in an
/// HttpOnly cookie); only its SHA-256 is persisted, so a DB leak yields nothing.
pub async fn create_session(pool: &SqlitePool, user: &StaffUser, ttl_secs: i64) -> Result<String> {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut raw = [0u8; 32];
    let mut rng = OsRng;
    rng.fill_bytes(&mut raw);
    let token = hex(&raw);
    let t = now();
    sqlx::query(
        "INSERT INTO session (token_hash, user_id, username, display_name, role, created_at, expires_at) VALUES (?,?,?,?,?,?,?)",
    )
    .bind(token_hash(&token))
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.display_name)
    .bind(&user.role)
    .bind(t)
    .bind(t + ttl_secs)
    .execute(pool)
    .await?;
    Ok(token)
}

/// Validate a raw session token: returns the session if present and unexpired,
/// prunes and returns None when expired, None for unknown tokens.
pub async fn validate_session(pool: &SqlitePool, token: &str) -> Result<Option<Session>> {
    let h = token_hash(token);
    let row = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT user_id, username, display_name, role, expires_at FROM session WHERE token_hash = ?",
    )
    .bind(&h)
    .fetch_optional(pool)
    .await?;
    let Some((user_id, username, display_name, role, expires_at)) = row else {
        return Ok(None);
    };
    if expires_at <= now() {
        sqlx::query("DELETE FROM session WHERE token_hash = ?")
            .bind(&h)
            .execute(pool)
            .await?;
        return Ok(None);
    }
    Ok(Some(Session {
        user_id,
        username,
        display_name,
        role,
    }))
}

/// Destroy a session (logout). Idempotent — unknown tokens are a no-op.
pub async fn destroy_session(pool: &SqlitePool, token: &str) -> Result<()> {
    sqlx::query("DELETE FROM session WHERE token_hash = ?")
        .bind(token_hash(token))
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_article(
    pool: &SqlitePool,
    slug: &str,
    title: &str,
    summary: &str,
    body: &str,
    byline: &str,
    kind: &str,
    section: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
) -> Result<i64> {
    let t = now();
    let res = sqlx::query(
        "INSERT INTO article (slug, title, summary, body, byline, kind, section, meta_description, og_image_url, tags, state, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(slug)
    .bind(title)
    .bind(summary)
    .bind(body)
    .bind(byline)
    .bind(kind)
    .bind(section)
    .bind(meta_description)
    .bind(og_image_url)
    .bind(if tags.trim().is_empty() { "[]" } else { tags })
    .bind(State::Draft.as_str())
    .bind(t)
    .bind(t)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

/// URL-safe slug from a title: lowercase ASCII alphanumerics, single dashes.
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

async fn slug_exists(pool: &SqlitePool, slug: &str) -> Result<bool> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM article WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

/// Is `slug` already used by a DIFFERENT article (for slug edits on update)?
async fn slug_taken_by_other(pool: &SqlitePool, slug: &str, id: i64) -> Result<bool> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM article WHERE slug = ? AND id != ?")
            .bind(slug)
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

/// Create a Draft with an explicit slug base. When `slug_base` is non-empty,
/// the base slug is `slugify(slug_base)`; otherwise the base slug is
/// `slugify(title)`. The same de-dupe loop applies either way. `byline` is the
/// article's credit; `actor` is the stable username recorded in the audit chain.
/// The body is a JSON array of paragraph strings.
#[allow(clippy::too_many_arguments)]
pub async fn create_draft_with_slug(
    pool: &SqlitePool,
    slug_base: &str,
    title: &str,
    summary: &str,
    body: &str,
    byline: &str,
    kind: &str,
    section: &str,
    actor: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
) -> Result<i64> {
    let base = if slug_base.trim().is_empty() {
        slugify(title)
    } else {
        slugify(slug_base)
    };
    let mut slug = base.clone();
    let mut n = 2;
    while slug_exists(pool, &slug).await? {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let id = create_article(
        pool, &slug, title, summary, body, byline, kind, section,
        meta_description, og_image_url, tags,
    )
    .await?;
    append_audit(pool, actor, "article.create", &slug, "draft created").await?;
    Ok(id)
}

/// Create a Draft from a title (slug derived + de-duplicated), audited. `byline`
/// is the article's credit; `actor` is the stable username recorded in the audit
/// chain (so creation is attributable even if a display name later changes). The
/// body is a JSON array of paragraph strings (same shape the public renderer reads).
#[allow(clippy::too_many_arguments)]
pub async fn create_draft(
    pool: &SqlitePool,
    title: &str,
    summary: &str,
    body: &str,
    byline: &str,
    kind: &str,
    section: &str,
    actor: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
) -> Result<i64> {
    create_draft_with_slug(pool, "", title, summary, body, byline, kind, section, actor, meta_description, og_image_url, tags).await
}

/// Update an article's content + SEO, audited; lifecycle state is unchanged. Any
/// story EXCEPT a retracted one is editable. The SLUG may only be changed while
/// the article is pre-publish (changing a live URL would 404 inbound links); an
/// empty `slug` keeps the current one. A changed slug is slugified + de-duplicated.
#[allow(clippy::too_many_arguments)]
pub async fn update_article(
    pool: &SqlitePool,
    id: i64,
    title: &str,
    summary: &str,
    body: &str,
    kind: &str,
    section: &str,
    actor: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
    slug: &str,
) -> Result<()> {
    let article = get_article(pool, id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no article {id}")))?;
    if article.state()? == State::Retracted {
        return Err(CmsError::Forbidden(
            "a retracted story can't be edited".into(),
        ));
    }
    // Resolve the final slug. Empty input keeps the current slug. A real change is
    // gated to pre-publish states and de-duplicated against other rows.
    let new_slug = if slug.trim().is_empty() {
        article.slug.clone()
    } else {
        let wanted = slugify(slug);
        if wanted == article.slug {
            article.slug.clone()
        } else {
            if article.state()?.is_public() {
                return Err(CmsError::Forbidden(
                    "a published article's URL can't be changed".into(),
                ));
            }
            let mut candidate = wanted.clone();
            let mut n = 2;
            while slug_taken_by_other(pool, &candidate, id).await? {
                candidate = format!("{wanted}-{n}");
                n += 1;
            }
            candidate
        }
    };
    let tags = if tags.trim().is_empty() { "[]" } else { tags };
    sqlx::query(
        "UPDATE article SET slug = ?, title = ?, summary = ?, body = ?, kind = ?, section = ?, meta_description = ?, og_image_url = ?, tags = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&new_slug)
    .bind(title)
    .bind(summary)
    .bind(body)
    .bind(kind)
    .bind(section)
    .bind(meta_description)
    .bind(og_image_url)
    .bind(tags)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
    append_audit(pool, actor, "article.edit", &new_slug, "").await?;
    Ok(())
}

pub async fn get_article(pool: &SqlitePool, id: i64) -> Result<Option<Article>> {
    Ok(
        sqlx::query_as::<_, Article>("SELECT * FROM article WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Serialises the read-then-insert in [`append_audit`]. Without it, two
/// concurrent appends on different pooled connections (the pool allows several)
/// can read the same chain tip and collide on the `seq` primary key — now
/// reachable because the background crawler appends audit rows concurrently with
/// request handlers and the public complaint form. The whole product is one
/// process/container and the audit path is low-frequency, so a process-wide async
/// lock held across the SELECT+INSERT is sufficient and cheap.
static AUDIT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Append a record to the hash-chained audit log (reads the current tip first).
pub async fn append_audit(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    subject: &str,
    detail: &str,
) -> Result<()> {
    // Hold the lock across the read-then-insert so the chain tip can't be read by
    // two writers at once (see AUDIT_LOCK).
    let _guard = AUDIT_LOCK.lock().await;
    let (seq, prev_hash): (i64, String) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq)+1,0), COALESCE((SELECT hash FROM audit ORDER BY seq DESC LIMIT 1), ?) FROM audit",
    )
    .bind(ph_audit::GENESIS)
    .fetch_one(pool)
    .await?;
    let mut e = ph_audit::Entry {
        seq: seq as u64,
        ts: now(),
        actor: actor.to_string(),
        action: action.to_string(),
        subject: subject.to_string(),
        detail: detail.to_string(),
        prev_hash,
        hash: String::new(),
    };
    e.hash = e.compute_hash();
    sqlx::query("INSERT INTO audit (seq, ts, actor, action, subject, detail, prev_hash, hash) VALUES (?,?,?,?,?,?,?,?)")
        .bind(e.seq as i64)
        .bind(e.ts)
        .bind(&e.actor)
        .bind(&e.action)
        .bind(&e.subject)
        .bind(&e.detail)
        .bind(&e.prev_hash)
        .bind(&e.hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Load + verify the whole audit chain.
pub async fn audit_chain(pool: &SqlitePool) -> Result<ph_audit::AuditChain> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, String, String, String, String)>(
        "SELECT seq, ts, actor, action, subject, detail, prev_hash, hash FROM audit ORDER BY seq",
    )
    .fetch_all(pool)
    .await?;
    let entries = rows
        .into_iter()
        .map(
            |(seq, ts, actor, action, subject, detail, prev_hash, hash)| ph_audit::Entry {
                seq: seq as u64,
                ts,
                actor,
                action,
                subject,
                detail,
                prev_hash,
                hash,
            },
        )
        .collect();
    ph_audit::AuditChain::from_entries(entries)
        .map_err(|e| CmsError::Bad(format!("audit chain invalid: {e}")))
}

// ===================== settings (operator key/value) =====================

/// Key for the "we are a registered member of our press regulator" flag.
pub const SETTING_REGULATOR_REGISTERED: &str = "regulator_registered";

/// Read a raw setting value (None if unset).
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM setting WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(v,)| v))
}

/// Upsert a setting value (stamps `updated_at`).
pub async fn set_setting(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO setting (key, value, updated_at) VALUES (?, ?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

/// Whether we are a registered member of our press regulator. Defaults to `false`
/// (the cautious, never-over-claim state) when unset or unrecognised.
pub async fn regulator_registered(pool: &SqlitePool) -> Result<bool> {
    Ok(get_setting(pool, SETTING_REGULATOR_REGISTERED)
        .await?
        .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false))
}

/// Set the regulator-registered flag, writing an audit record — a change to a
/// public legal claim belongs on the tamper-evident chain.
///
/// The value write and the audit append are two statements (the same
/// mutate-then-audit shape used across this crate — transitions, complaints,
/// removals — none wrap the pair in one transaction). A transient failure of the
/// audit append after the value commits would change the flag without an audit
/// row. That gap is benign in the only direction that matters: `registered=true`
/// only ever commits on a deliberate admin flip (so the public claim still matches
/// intent), and `registered=false` reverts to the cautious wording (an under-claim,
/// which is safe). It never produces a false over-claim.
pub async fn set_regulator_registered(
    pool: &SqlitePool,
    registered: bool,
    actor: &str,
) -> Result<()> {
    set_setting(
        pool,
        SETTING_REGULATOR_REGISTERED,
        if registered { "1" } else { "0" },
    )
    .await?;
    append_audit(
        pool,
        actor,
        "setting.regulator_registered",
        SETTING_REGULATOR_REGISTERED,
        if registered { "registered" } else { "not registered" },
    )
    .await?;
    Ok(())
}

/// Move an article to a new state, enforcing the lifecycle gate for the actor's
/// role, logging the review, and writing to the audit chain.
pub async fn transition(
    pool: &SqlitePool,
    article_id: i64,
    to: State,
    actor: &StaffUser,
    note: &str,
) -> Result<()> {
    let article = get_article(pool, article_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no article {article_id}")))?;
    let from = article.state()?;
    let role = actor.role()?;
    if !can_transition(from, to, role) {
        return Err(CmsError::Transition {
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
            role: role.as_str().to_string(),
        });
    }
    let t = now();
    let published_at = if to == State::Published && article.published_at.is_none() {
        Some(t)
    } else {
        article.published_at
    };
    sqlx::query("UPDATE article SET state = ?, updated_at = ?, published_at = ? WHERE id = ?")
        .bind(to.as_str())
        .bind(t)
        .bind(published_at)
        .bind(article_id)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO review_log (article_id, from_state, to_state, actor, note, ts) VALUES (?,?,?,?,?,?)")
        .bind(article_id)
        .bind(from.as_str())
        .bind(to.as_str())
        .bind(&actor.username)
        .bind(note)
        .bind(t)
        .execute(pool)
        .await?;
    append_audit(
        pool,
        &actor.username,
        &format!("article.{}", to.as_str()),
        &article.slug,
        note,
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Correction {
    pub id: i64,
    pub article_id: i64,
    pub original: String,
    pub corrected: String,
    pub reason: String,
    pub ts: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Complaint {
    pub id: i64,
    pub article_slug: String,
    pub complainant: String,
    pub complainant_email: String,
    /// The IMPRESS Standards Code clause the complaint concerns (free text/key).
    pub category: String,
    pub body: String,
    pub status: String,
    /// When the complaint was acknowledged / resolved (IMPRESS 7-day / 21-day
    /// targets are measured from `ts`). None until reached.
    pub acknowledged_at: Option<i64>,
    pub resolved_at: Option<i64>,
    pub ts: i64,
}

/// One message in a complaint's handling thread: a staff-only internal note, or a
/// reply that was emailed to the complainant. Both are recorded for the IMPRESS
/// audit record of how the complaint was handled.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ComplaintMessage {
    pub id: i64,
    pub complaint_id: i64,
    pub author: String,
    pub channel: String, // 'internal' | 'reply'
    pub body: String,
    pub ts: i64,
}

/// Publicly visible articles (Published or Corrected), newest first.
pub async fn published_articles(pool: &SqlitePool) -> Result<Vec<Article>> {
    Ok(sqlx::query_as::<_, Article>(
        "SELECT * FROM article WHERE state IN ('published','corrected') ORDER BY COALESCE(published_at, updated_at) DESC",
    )
    .fetch_all(pool)
    .await?)
}

/// A single publicly-visible article by slug (published or corrected) — for the
/// public detail page when the slug is not a compile-time seed.
pub async fn published_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<Article>> {
    Ok(sqlx::query_as::<_, Article>(
        "SELECT * FROM article WHERE slug = ? AND state IN ('published','corrected')",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?)
}

/// Every article regardless of state — for the staff editorial dashboard, newest first.
pub async fn all_articles(pool: &SqlitePool) -> Result<Vec<Article>> {
    Ok(sqlx::query_as::<_, Article>(
        "SELECT * FROM article ORDER BY COALESCE(published_at, updated_at) DESC",
    )
    .fetch_all(pool)
    .await?)
}

/// Public text search over published articles (title/summary/body), newest first.
pub async fn search_articles(pool: &SqlitePool, q: &str) -> Result<Vec<Article>> {
    let like = format!("%{}%", q.replace(['%', '_'], ""));
    Ok(sqlx::query_as::<_, Article>(
        "SELECT * FROM article WHERE state IN ('published','corrected') AND (title LIKE ? OR summary LIKE ? OR body LIKE ?) ORDER BY COALESCE(published_at, updated_at) DESC",
    )
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .fetch_all(pool)
    .await?)
}

/// Record a published correction (both versions kept) + audit it under the actor.
/// Publishing a correction is an editorial act, gated to Editor/Admin (mirrors the
/// `Published -> Corrected` authority). On a live article it also moves the article
/// to Corrected — kept public, and logged in the review trail like any other
/// lifecycle move (equal-prominence, IMPRESS Clause).
pub async fn add_correction(
    pool: &SqlitePool,
    article_id: i64,
    original: &str,
    corrected: &str,
    reason: &str,
    actor: &StaffUser,
) -> Result<i64> {
    let article = get_article(pool, article_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no article {article_id}")))?;
    if !matches!(actor.role()?, Role::Editor | Role::Admin) {
        return Err(CmsError::Forbidden(
            "only an editor or admin may publish a correction".into(),
        ));
    }
    let t = now();
    let res = sqlx::query(
        "INSERT INTO correction (article_id, original, corrected, reason, ts) VALUES (?,?,?,?,?)",
    )
    .bind(article_id)
    .bind(original)
    .bind(corrected)
    .bind(reason)
    .bind(t)
    .execute(pool)
    .await?;
    // A correction on a live article marks it Corrected (kept public) + review-logged.
    if article.state()? == State::Published {
        sqlx::query("UPDATE article SET state = 'corrected', updated_at = ? WHERE id = ?")
            .bind(t)
            .bind(article_id)
            .execute(pool)
            .await?;
        sqlx::query("INSERT INTO review_log (article_id, from_state, to_state, actor, note, ts) VALUES (?,?,?,?,?,?)")
            .bind(article_id)
            .bind("published")
            .bind("corrected")
            .bind(&actor.username)
            .bind(reason)
            .bind(t)
            .execute(pool)
            .await?;
    }
    append_audit(
        pool,
        &actor.username,
        "article.correction",
        &article.slug,
        reason,
    )
    .await?;
    Ok(res.last_insert_rowid())
}

/// The published corrections archive, newest first.
pub async fn list_corrections(pool: &SqlitePool) -> Result<Vec<Correction>> {
    Ok(
        sqlx::query_as::<_, Correction>("SELECT * FROM correction ORDER BY ts DESC")
            .fetch_all(pool)
            .await?,
    )
}

/// Log a reader complaint (kept on record) + audit it. Submitted from the
/// per-article complaint form, or recorded by staff if it arrived another way.
pub async fn log_complaint(
    pool: &SqlitePool,
    article_slug: &str,
    complainant: &str,
    complainant_email: &str,
    category: &str,
    body: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO complaint (article_slug, complainant, complainant_email, category, body, status, ts) \
         VALUES (?,?,?,?,?,'received',?)",
    )
    .bind(article_slug)
    .bind(complainant)
    .bind(complainant_email.trim().to_lowercase())
    .bind(category)
    .bind(body)
    .bind(now())
    .execute(pool)
    .await?;
    append_audit(pool, "system", "complaint.received", article_slug, "").await?;
    Ok(res.last_insert_rowid())
}

/// Every logged complaint, newest first (the staff inbox).
pub async fn list_complaints(pool: &SqlitePool) -> Result<Vec<Complaint>> {
    Ok(
        sqlx::query_as::<_, Complaint>("SELECT * FROM complaint ORDER BY ts DESC")
            .fetch_all(pool)
            .await?,
    )
}

/// A single complaint by id.
pub async fn get_complaint(pool: &SqlitePool, id: i64) -> Result<Option<Complaint>> {
    Ok(
        sqlx::query_as::<_, Complaint>("SELECT * FROM complaint WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// The handling thread for a complaint (internal notes + replies), oldest first.
pub async fn list_complaint_messages(
    pool: &SqlitePool,
    complaint_id: i64,
) -> Result<Vec<ComplaintMessage>> {
    Ok(sqlx::query_as::<_, ComplaintMessage>(
        "SELECT * FROM complaint_message WHERE complaint_id = ? ORDER BY ts",
    )
    .bind(complaint_id)
    .fetch_all(pool)
    .await?)
}

/// Add a message to a complaint's thread. `channel` is `internal` (staff-only) or
/// `reply` (a message emailed to the complainant). Audited.
pub async fn add_complaint_message(
    pool: &SqlitePool,
    complaint_id: i64,
    author: &str,
    channel: &str,
    body: &str,
) -> Result<i64> {
    if !matches!(channel, "internal" | "reply") {
        return Err(CmsError::Bad(format!("complaint message channel: {channel}")));
    }
    let res = sqlx::query(
        "INSERT INTO complaint_message (complaint_id, author, channel, body, ts) VALUES (?,?,?,?,?)",
    )
    .bind(complaint_id)
    .bind(author)
    .bind(channel)
    .bind(body)
    .bind(now())
    .execute(pool)
    .await?;
    append_audit(pool, author, "complaint.message", &complaint_id.to_string(), channel).await?;
    Ok(res.last_insert_rowid())
}

/// The IMPRESS-aligned complaint workflow.
pub const COMPLAINT_STATUSES: [&str; 8] = [
    "received",
    "acknowledged",
    "under_investigation",
    "upheld",
    "partly_upheld",
    "not_upheld",
    "closed",
    "escalated",
];

/// A terminal (resolved) outcome — stamps `resolved_at`.
fn is_resolved_status(status: &str) -> bool {
    matches!(status, "upheld" | "partly_upheld" | "not_upheld" | "closed")
}

/// Update a complaint's status, audited under `actor`. Stamps `acknowledged_at`
/// the first time it leaves `received`, and `resolved_at` on a terminal outcome,
/// so the IMPRESS 7-day / 21-day targets can be measured. True if a row changed.
pub async fn set_complaint_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    actor: &str,
) -> Result<bool> {
    if !COMPLAINT_STATUSES.contains(&status) {
        return Err(CmsError::Bad(format!("complaint status: {status}")));
    }
    let t = now();
    if status != "received" {
        sqlx::query("UPDATE complaint SET acknowledged_at = ? WHERE id = ? AND acknowledged_at IS NULL")
            .bind(t)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if is_resolved_status(status) {
        sqlx::query("UPDATE complaint SET resolved_at = ? WHERE id = ? AND resolved_at IS NULL")
            .bind(t)
            .bind(id)
            .execute(pool)
            .await?;
    }
    let res = sqlx::query("UPDATE complaint SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    let changed = res.rows_affected() > 0;
    if changed {
        append_audit(pool, actor, "complaint.status", &id.to_string(), status).await?;
    }
    Ok(changed)
}

// ==================== removal requests (right-to-erasure review) ====================

/// A request from a member of the public to remove a conviction-database entry.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RemovalRequest {
    pub id: i64,
    pub target_ref: String,
    pub requester_name: String,
    pub requester_email: String,
    pub reason: String,
    pub status: String,
    pub created_at: i64,
    pub decided_at: Option<i64>,
    pub decision_note: String,
    pub decided_by: String,
}

/// Valid statuses for a removal request workflow.
pub const REMOVAL_STATUSES: [&str; 4] = [
    "received",
    "under_review",
    "upheld_removed",
    "rejected",
];

/// Is this a terminal (decided) status?
fn is_decided_status(status: &str) -> bool {
    matches!(status, "upheld_removed" | "rejected")
}

/// Log a public removal request. Returns the new id.
pub async fn create_removal_request(
    pool: &SqlitePool,
    target_ref: &str,
    requester_name: &str,
    requester_email: &str,
    reason: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO removal_request \
         (target_ref, requester_name, requester_email, reason, status, created_at) \
         VALUES (?,?,?,?,'received',?)",
    )
    .bind(target_ref.trim())
    .bind(requester_name.trim())
    .bind(requester_email.trim().to_lowercase())
    .bind(reason.trim())
    .bind(now())
    .execute(pool)
    .await?;
    append_audit(pool, "system", "removal.received", target_ref, "").await?;
    Ok(res.last_insert_rowid())
}

/// Every removal request, newest first (the staff inbox).
pub async fn list_removal_requests(pool: &SqlitePool) -> Result<Vec<RemovalRequest>> {
    Ok(
        sqlx::query_as::<_, RemovalRequest>(
            "SELECT * FROM removal_request ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?,
    )
}

/// A single removal request by id.
pub async fn get_removal_request(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<RemovalRequest>> {
    Ok(
        sqlx::query_as::<_, RemovalRequest>(
            "SELECT * FROM removal_request WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?,
    )
}

/// Advance a removal request's status, audited under `actor`. On `upheld_removed`
/// also hides the conviction (see `hide_conviction`). Stamps decided_at / decided_by
/// / decision_note on a terminal decision. Returns true if a row changed.
pub async fn set_removal_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    actor: &str,
    decision_note: &str,
) -> Result<bool> {
    if !REMOVAL_STATUSES.contains(&status) {
        return Err(CmsError::Bad(format!("removal request status: {status}")));
    }
    let t = now();
    if is_decided_status(status) {
        sqlx::query(
            "UPDATE removal_request SET decided_at = ?, decided_by = ?, decision_note = ? \
             WHERE id = ? AND decided_at IS NULL",
        )
        .bind(t)
        .bind(actor)
        .bind(decision_note.trim())
        .bind(id)
        .execute(pool)
        .await?;
    }
    let res = sqlx::query("UPDATE removal_request SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    let changed = res.rows_affected() > 0;
    if changed {
        append_audit(pool, actor, "removal.status", &id.to_string(), status).await?;
        if status == "upheld_removed" {
            // Fetch the target_ref and hide it.
            if let Some(req) = get_removal_request(pool, id).await? {
                hide_conviction(pool, &req.target_ref, id, actor).await?;
            }
        }
    }
    Ok(changed)
}

/// Hide a conviction entry (both compile-time and DB-backed) from the public
/// database. Safe to call multiple times — INSERT OR IGNORE.
pub async fn hide_conviction(
    pool: &SqlitePool,
    target_ref: &str,
    removal_request_id: i64,
    actor: &str,
) -> Result<()> {
    let t = now();
    sqlx::query(
        "INSERT OR IGNORE INTO hidden_conviction \
         (target_ref, removal_request_id, hidden_at, hidden_by) VALUES (?,?,?,?)",
    )
    .bind(target_ref)
    .bind(removal_request_id)
    .bind(t)
    .bind(actor)
    .execute(pool)
    .await?;
    append_audit(pool, actor, "conviction.hidden", target_ref, "").await?;
    Ok(())
}

/// Unhide a conviction entry (removes from the hidden set; the row is untouched).
pub async fn unhide_conviction(pool: &SqlitePool, target_ref: &str, actor: &str) -> Result<()> {
    sqlx::query("DELETE FROM hidden_conviction WHERE target_ref = ?")
        .bind(target_ref)
        .execute(pool)
        .await?;
    append_audit(pool, actor, "conviction.unhidden", target_ref, "").await?;
    Ok(())
}

/// Every currently hidden conviction target_ref.
pub async fn list_hidden_refs(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT target_ref FROM hidden_conviction")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(r,)| r).collect())
}

/// First-run setup. If there are no staff users yet, create the first admin
/// (the only way a first admin can be made; after this, an existing admin invites
/// others). Idempotent: returns true only if it created the admin this call.
pub async fn bootstrap_admin(
    pool: &SqlitePool,
    username: &str,
    display_name: &str,
    password: &str,
) -> Result<bool> {
    if count_users(pool).await? > 0 {
        return Ok(false);
    }
    create_user(pool, username, display_name, Role::Admin, password, "").await?;
    append_audit(
        pool,
        "system",
        "bootstrap.admin",
        username,
        "first admin created",
    )
    .await?;
    Ok(true)
}

/// A seed article (the compile-time `content.rs` data, migrated into the DB).
pub struct ArticleSeed<'a> {
    pub slug: &'a str,
    pub title: &'a str,
    pub summary: &'a str,
    pub body: &'a str, // JSON array of paragraphs
    pub byline: &'a str,
    pub kind: &'a str,
    pub section: &'a str,
    pub published_at: i64,
}

/// Idempotently insert seed articles as Published (by slug). Returns how many
/// were newly inserted. Safe to call on every boot.
pub async fn seed_articles(pool: &SqlitePool, items: &[ArticleSeed<'_>]) -> Result<u64> {
    let mut inserted = 0u64;
    for a in items {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO article (slug, title, summary, body, byline, kind, section, state, created_at, updated_at, published_at) VALUES (?,?,?,?,?,?,?, 'published', ?, ?, ?)",
        )
        .bind(a.slug)
        .bind(a.title)
        .bind(a.summary)
        .bind(a.body)
        .bind(a.byline)
        .bind(a.kind)
        .bind(a.section)
        .bind(a.published_at)
        .bind(a.published_at)
        .bind(a.published_at)
        .execute(pool)
        .await?;
        inserted += res.rows_affected();
    }
    if inserted > 0 {
        append_audit(
            pool,
            "system",
            "seed.articles",
            &format!("{inserted} seeded"),
            "",
        )
        .await?;
    }
    Ok(inserted)
}

/// Open the database, run schema init, ensure a first admin exists, and seed the
/// starter articles. The one call a server makes on boot. Idempotent.
pub async fn open_and_setup(
    url: &str,
    admin_user: &str,
    admin_display: &str,
    admin_pass: Option<&str>,
    reset_admin: bool,
    seeds: &[ArticleSeed<'_>],
) -> Result<Db> {
    let pool = connect(url).await?;
    init(&pool).await?;
    // With a password supplied (PH_ADMIN_PASS), keep the env-seeded bootstrap +
    // operator reset (PH_ADMIN_RESET). Without one, do NOT auto-create an admin —
    // a fresh deploy is completed via the first-run install screen, so there is no
    // default password anywhere. An EXISTING deployment already has users, so this
    // changes nothing for it (bootstrap_admin no-ops once count_users > 0).
    if let Some(pass) = admin_pass {
        let created = bootstrap_admin(&pool, admin_user, admin_display, pass).await?;
        if !created && reset_admin && !reset_password(&pool, admin_user, pass).await? {
            eprintln!("[ph-cms] PH_ADMIN_RESET set but no user '{admin_user}' to reset");
        }
    } else if reset_admin {
        eprintln!("[ph-cms] PH_ADMIN_RESET set but PH_ADMIN_PASS is empty — skipping reset");
    }
    seed_articles(&pool, seeds).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_gates_publish_behind_legal() {
        // A writer cannot leap a draft to published.
        assert!(!can_transition(
            State::Draft,
            State::Published,
            Role::Writer
        ));
        assert!(!can_transition(
            State::EditorialReview,
            State::Published,
            Role::Editor
        ));
        // Publish is only reachable from LegalReview (legal sign-off) or Scheduled.
        assert!(can_transition(
            State::LegalReview,
            State::Published,
            Role::Legal
        ));
        assert!(!can_transition(
            State::LegalReview,
            State::Published,
            Role::Writer
        ));
        assert!(can_transition(
            State::Scheduled,
            State::Published,
            Role::Editor
        ));
        // normal early steps
        assert!(can_transition(State::Draft, State::Submitted, Role::Writer));
    }

    #[test]
    fn password_hash_roundtrip() {
        let h = auth::hash_password("correct horse").unwrap();
        assert!(auth::verify_password(&h, "correct horse"));
        assert!(!auth::verify_password(&h, "wrong"));
    }

    #[tokio::test]
    async fn password_reset_flow() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        create_user(&pool, "admin", "Admin", Role::Admin, "old-pass", "")
            .await
            .unwrap();

        // No email set yet → no reset issued, but the call still succeeds (None),
        // so the endpoint can't be used to probe which emails are registered.
        assert!(create_password_reset(&pool, "admin@example.com", RESET_TTL_SECS)
            .await
            .unwrap()
            .is_none());

        // Link the account to an email; lookups are case-insensitive.
        assert!(set_user_email(&pool, "admin", "Admin@Example.com")
            .await
            .unwrap());
        let u = find_user_by_email(&pool, "ADMIN@example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(u.username, "admin");
        assert!(find_user_by_email(&pool, "nobody@example.com")
            .await
            .unwrap()
            .is_none());

        // Issue a link, mint a session, then redeem the link.
        let (token, user) = create_password_reset(&pool, "admin@example.com", RESET_TTL_SECS)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.username, "admin");
        let session = create_session(&pool, &user, SESSION_TTL_SECS).await.unwrap();
        assert!(validate_session(&pool, &session).await.unwrap().is_some());

        let reset = consume_password_reset(&pool, &token, "new-pass")
            .await
            .unwrap();
        assert_eq!(reset.username, "admin");
        // New password works, old one no longer does.
        assert!(authenticate(&pool, "admin", "new-pass").await.is_ok());
        assert!(authenticate(&pool, "admin", "old-pass").await.is_err());
        // The reset destroyed the pre-existing session.
        assert!(validate_session(&pool, &session).await.unwrap().is_none());
        // Token is single-use.
        assert!(consume_password_reset(&pool, &token, "another")
            .await
            .is_err());

        // An expired token is rejected.
        let (token2, user2) = create_password_reset(&pool, "admin@example.com", RESET_TTL_SECS)
            .await
            .unwrap()
            .unwrap();
        sqlx::query("UPDATE password_reset_token SET expires_at = ? WHERE user_id = ?")
            .bind(now() - 1)
            .bind(user2.id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(consume_password_reset(&pool, &token2, "whatever")
            .await
            .is_err());

        // Issuing a fresh link invalidates the prior one (one active link per account).
        let (token_a, _) = create_password_reset(&pool, "admin@example.com", RESET_TTL_SECS)
            .await
            .unwrap()
            .unwrap();
        let (token_b, _) = create_password_reset(&pool, "admin@example.com", RESET_TTL_SECS)
            .await
            .unwrap()
            .unwrap();
        assert!(consume_password_reset(&pool, &token_a, "stale")
            .await
            .is_err());
        assert!(consume_password_reset(&pool, &token_b, "valid-pass")
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn install_gate_password_optional() {
        // No password → NO admin auto-seeded; the first-run install screen shows.
        let pool = open_and_setup("sqlite::memory:", "admin", "Admin", None, false, &[])
            .await
            .unwrap();
        assert_eq!(count_users(&pool).await.unwrap(), 0);
        assert!(needs_install(&pool).await.unwrap());

        // A supplied password → the first admin IS env-seeded (operator opt-in).
        let pool2 = open_and_setup("sqlite::memory:", "admin", "Admin", Some("install-pw"), false, &[])
            .await
            .unwrap();
        assert_eq!(count_users(&pool2).await.unwrap(), 1);
        assert!(!needs_install(&pool2).await.unwrap());
        assert!(authenticate(&pool2, "admin", "install-pw").await.is_ok());
    }

    #[tokio::test]
    async fn db_lifecycle_end_to_end() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();

        // bootstrap gate: first user must be admin
        assert!(create_user(&pool, "w", "Writer", Role::Writer, "pw", "")
            .await
            .is_err());
        let _admin = create_user(&pool, "admin", "Admin", Role::Admin, "pw", "")
            .await
            .unwrap();
        create_user(&pool, "jordan", "Jordan Upton", Role::Editor, "pw1", "")
            .await
            .unwrap();
        create_user(&pool, "scott", "Scott Taylor", Role::Legal, "pw2", "")
            .await
            .unwrap();

        assert!(authenticate(&pool, "jordan", "pw1").await.is_ok());
        assert!(authenticate(&pool, "jordan", "nope").await.is_err());

        let editor = find_user(&pool, "jordan").await.unwrap().unwrap();
        let legal = find_user(&pool, "scott").await.unwrap().unwrap();

        let id = create_article(
            &pool,
            "test-case",
            "Test case",
            "summary",
            "[]",
            "Jordan Upton",
            "Court report",
            "Crime",
            "",
            "",
            "[]",
        )
        .await
        .unwrap();

        // editor cannot publish directly
        assert!(transition(&pool, id, State::Published, &editor, "")
            .await
            .is_err());

        // proper path: editor moves through review, legal signs off + publishes
        transition(&pool, id, State::Submitted, &editor, "")
            .await
            .unwrap();
        transition(&pool, id, State::EditorialReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, id, State::LegalReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, id, State::Published, &legal, "signed off")
            .await
            .unwrap();

        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.state().unwrap(), State::Published);
        assert!(a.published_at.is_some());

        // audit chain records every step + verifies
        let chain = audit_chain(&pool).await.unwrap();
        assert!(chain.entries().len() >= 4);
        assert!(chain.verify().is_ok());

        // corrections archive, complaints, public listing + search
        add_correction(&pool, id, "old text", "new text", "fixed a detail", &editor)
            .await
            .unwrap();
        assert_eq!(list_corrections(&pool).await.unwrap().len(), 1);
        log_complaint(
            &pool,
            "test-case",
            "anon",
            "anon@example.com",
            "accuracy",
            "you got X wrong",
        )
        .await
        .unwrap();
        assert_eq!(list_complaints(&pool).await.unwrap().len(), 1);
        let cid = list_complaints(&pool).await.unwrap()[0].id;
        assert!(set_complaint_status(&pool, cid, "under_investigation", "editor")
            .await
            .unwrap());
        assert!(set_complaint_status(&pool, cid, "bogus", "editor")
            .await
            .is_err());
        // Leaving 'received' stamps acknowledged_at.
        assert!(get_complaint(&pool, cid)
            .await
            .unwrap()
            .unwrap()
            .acknowledged_at
            .is_some());
        // Handling thread: internal note + reply recorded; bad channel rejected.
        add_complaint_message(&pool, cid, "editor", "internal", "looking into it")
            .await
            .unwrap();
        add_complaint_message(&pool, cid, "editor", "reply", "thanks for getting in touch")
            .await
            .unwrap();
        assert!(add_complaint_message(&pool, cid, "editor", "bogus", "x")
            .await
            .is_err());
        assert_eq!(list_complaint_messages(&pool, cid).await.unwrap().len(), 2);
        // A terminal outcome stamps resolved_at.
        set_complaint_status(&pool, cid, "not_upheld", "editor")
            .await
            .unwrap();
        assert!(get_complaint(&pool, cid)
            .await
            .unwrap()
            .unwrap()
            .resolved_at
            .is_some());
        assert_eq!(published_articles(&pool).await.unwrap().len(), 1);
        assert_eq!(search_articles(&pool, "Test").await.unwrap().len(), 1);
        assert_eq!(search_articles(&pool, "zzznomatch").await.unwrap().len(), 0);
        assert!(audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn bootstrap_and_seed_are_idempotent() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        assert!(bootstrap_admin(&pool, "admin", "Admin", "pw")
            .await
            .unwrap());
        // second call is a no-op once an admin exists
        assert!(!bootstrap_admin(&pool, "admin2", "Admin2", "pw")
            .await
            .unwrap());
        assert!(authenticate(&pool, "admin", "pw").await.is_ok());

        let seeds = [
            ArticleSeed {
                slug: "a",
                title: "A",
                summary: "s",
                body: "[]",
                byline: "x",
                kind: "Court report",
                section: "Crime",
                published_at: 1000,
            },
            ArticleSeed {
                slug: "b",
                title: "B",
                summary: "s",
                body: "[]",
                byline: "x",
                kind: "Court report",
                section: "Crime",
                published_at: 2000,
            },
        ];
        assert_eq!(seed_articles(&pool, &seeds).await.unwrap(), 2);
        assert_eq!(seed_articles(&pool, &seeds).await.unwrap(), 0); // idempotent
        assert_eq!(published_articles(&pool).await.unwrap().len(), 2);
        assert!(audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn admin_password_reset_is_deliberate() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        assert!(bootstrap_admin(&pool, "admin", "Admin", "old")
            .await
            .unwrap());
        // a later boot never overwrites the admin, even with a different password
        assert!(!bootstrap_admin(&pool, "admin", "Admin", "different")
            .await
            .unwrap());
        assert!(authenticate(&pool, "admin", "old").await.is_ok());
        // a deliberate reset (PH_ADMIN_RESET) does change it
        assert!(reset_password(&pool, "admin", "new").await.unwrap());
        assert!(authenticate(&pool, "admin", "old").await.is_err());
        assert!(authenticate(&pool, "admin", "new").await.is_ok());

        // self-service change needs the correct current password
        assert!(change_password(&pool, "admin", "wrong", "newer123")
            .await
            .is_err());
        assert!(authenticate(&pool, "admin", "new").await.is_ok()); // unchanged after a bad attempt
        change_password(&pool, "admin", "new", "newer123")
            .await
            .unwrap();
        assert!(authenticate(&pool, "admin", "new").await.is_err());
        assert!(authenticate(&pool, "admin", "newer123").await.is_ok());
        assert!(audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn session_roundtrip_and_expiry() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        bootstrap_admin(&pool, "ed", "Editor", "pw").await.unwrap();
        let user = authenticate(&pool, "ed", "pw").await.unwrap();

        let token = create_session(&pool, &user, SESSION_TTL_SECS)
            .await
            .unwrap();
        let s = validate_session(&pool, &token)
            .await
            .unwrap()
            .expect("valid session");
        assert_eq!(s.username, "ed");
        assert_eq!(s.role().unwrap(), Role::Admin);
        // unknown token -> None, never an error
        assert!(validate_session(&pool, "deadbeef").await.unwrap().is_none());
        // an already-expired session validates to None and is pruned
        let expired = create_session(&pool, &user, -1).await.unwrap();
        assert!(validate_session(&pool, &expired).await.unwrap().is_none());
        // logout invalidates
        destroy_session(&pool, &token).await.unwrap();
        assert!(validate_session(&pool, &token).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn create_draft_dedupes_slug_and_lists_actions() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();

        let id1 = create_draft(
            &pool,
            "Court Report: R v Smith",
            "s",
            "[]",
            "Jordan",
            "Court report",
            "Crime",
            "admin",
            "",
            "",
            "[]",
        )
        .await
        .unwrap();
        let id2 = create_draft(
            &pool,
            "Court Report: R v Smith",
            "s",
            "[]",
            "Jordan",
            "Court report",
            "Crime",
            "admin",
            "",
            "",
            "[]",
        )
        .await
        .unwrap();
        assert_ne!(id1, id2);
        let a1 = get_article(&pool, id1).await.unwrap().unwrap();
        let a2 = get_article(&pool, id2).await.unwrap().unwrap();
        assert_eq!(a1.slug, "court-report-r-v-smith");
        assert_eq!(a2.slug, "court-report-r-v-smith-2");
        assert_eq!(a1.state, "draft");

        // A writer can only submit a draft; a legal reviewer cannot act on a draft.
        assert_eq!(
            allowed_transitions(State::Draft, Role::Writer),
            vec![State::Submitted]
        );
        assert!(allowed_transitions(State::Draft, Role::Legal).is_empty());
        // Publishing is reachable only from legal review, and only by legal/admin.
        assert!(allowed_transitions(State::LegalReview, Role::Legal).contains(&State::Published));
        assert!(
            !allowed_transitions(State::EditorialReview, Role::Editor).contains(&State::Published)
        );
    }

    #[tokio::test]
    async fn article_carries_seo_columns_defaulting_empty() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        let id = create_draft(
            &pool, "Title", "sum", "[]", "By", "Court report", "Crime", "admin",
            "", "", "[]",
        )
        .await
        .unwrap();
        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.meta_description, "");
        assert_eq!(a.og_image_url, "");
        assert_eq!(a.tags, "[]");
    }

    #[tokio::test]
    async fn create_draft_persists_seo_fields() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        let id = create_draft(
            &pool, "Title", "sum", "[]", "By", "Court report", "Crime", "admin",
            "A search description.", "/assets/og.png", r#"["grooming","crown court"]"#,
        )
        .await
        .unwrap();
        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.meta_description, "A search description.");
        assert_eq!(a.og_image_url, "/assets/og.png");
        assert_eq!(a.tags, r#"["grooming","crown court"]"#);
    }

    #[tokio::test]
    async fn update_article_sets_seo_and_edits_slug_pre_publish() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        let id = create_draft(
            &pool, "Old Title", "s", "[]", "By", "Court report", "Crime", "admin",
            "", "", "[]",
        )
        .await
        .unwrap();
        // edit SEO + change the slug while still a draft
        update_article(
            &pool, id, "Old Title", "s", "[]", "Court report", "Crime", "admin",
            "New meta desc.", "/assets/x.png", r#"["tag-a"]"#, "my-custom-slug",
        )
        .await
        .unwrap();
        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.meta_description, "New meta desc.");
        assert_eq!(a.og_image_url, "/assets/x.png");
        assert_eq!(a.tags, r#"["tag-a"]"#);
        assert_eq!(a.slug, "my-custom-slug");
    }

    #[tokio::test]
    async fn update_article_dedupes_changed_slug() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        let _a = create_draft(
            &pool, "Taken", "s", "[]", "By", "Court report", "Crime", "admin", "", "", "[]",
        ).await.unwrap();
        let b = create_draft(
            &pool, "Other", "s", "[]", "By", "Court report", "Crime", "admin", "", "", "[]",
        ).await.unwrap();
        // try to move b onto a's slug "taken" -> de-duped to "taken-2"
        update_article(
            &pool, b, "Other", "s", "[]", "Court report", "Crime", "admin",
            "", "", "[]", "Taken",
        ).await.unwrap();
        let b2 = get_article(&pool, b).await.unwrap().unwrap();
        assert_eq!(b2.slug, "taken-2");
    }

    #[tokio::test]
    async fn update_article_refuses_slug_change_when_published() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        bootstrap_admin(&pool, "admin", "Admin", "pw").await.unwrap();
        let admin = find_user(&pool, "admin").await.unwrap().unwrap();
        let id = create_draft(
            &pool, "Live Story", "s", "[]", "By", "Court report", "Crime", "admin", "", "", "[]",
        ).await.unwrap();
        // drive it to Published via the legal-gated lifecycle
        transition(&pool, id, State::Submitted, &admin, "").await.unwrap();
        transition(&pool, id, State::EditorialReview, &admin, "").await.unwrap();
        transition(&pool, id, State::LegalReview, &admin, "").await.unwrap();
        transition(&pool, id, State::Published, &admin, "").await.unwrap();
        let original = get_article(&pool, id).await.unwrap().unwrap().slug;
        // changing the slug of a live article is refused...
        assert!(update_article(
            &pool, id, "Live Story", "s", "[]", "Court report", "Crime", "admin",
            "", "", "[]", "a-different-slug",
        ).await.is_err());
        // ...but editing other SEO fields with the SAME slug is allowed
        update_article(
            &pool, id, "Live Story", "s", "[]", "Court report", "Crime", "admin",
            "Edited meta", "", "[]", &original,
        ).await.unwrap();
        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.slug, original);
        assert_eq!(a.meta_description, "Edited meta");
    }

    #[tokio::test]
    async fn removal_request_lifecycle() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();

        // Create a request.
        let id = create_removal_request(
            &pool,
            "kieron-willans-guilty",
            "Test Requester",
            "test@example.com",
            "Spent conviction under ROA",
        )
        .await
        .unwrap();
        assert!(id > 0);

        // Fetch it back.
        let req = get_removal_request(&pool, id).await.unwrap().unwrap();
        assert_eq!(req.status, "received");
        assert_eq!(req.target_ref, "kieron-willans-guilty");
        assert!(req.decided_at.is_none());

        // Advance to under_review.
        assert!(set_removal_status(&pool, id, "under_review", "admin", "").await.unwrap());
        let req2 = get_removal_request(&pool, id).await.unwrap().unwrap();
        assert_eq!(req2.status, "under_review");
        assert!(req2.decided_at.is_none());

        // Uphold — this should also hide the conviction.
        assert!(
            set_removal_status(&pool, id, "upheld_removed", "admin", "ROA applies")
                .await
                .unwrap()
        );
        let req3 = get_removal_request(&pool, id).await.unwrap().unwrap();
        assert_eq!(req3.status, "upheld_removed");
        assert!(req3.decided_at.is_some());
        assert_eq!(req3.decided_by, "admin");
        assert_eq!(req3.decision_note, "ROA applies");

        // The conviction is now in the hidden set.
        let hidden = list_hidden_refs(&pool).await.unwrap();
        assert!(hidden.contains(&"kieron-willans-guilty".to_string()));

        // Unhide it.
        unhide_conviction(&pool, "kieron-willans-guilty", "admin")
            .await
            .unwrap();
        let hidden2 = list_hidden_refs(&pool).await.unwrap();
        assert!(!hidden2.contains(&"kieron-willans-guilty".to_string()));

        // The removal_request row is NOT deleted.
        assert!(get_removal_request(&pool, id).await.unwrap().is_some());

        // Reject a different request (no hide).
        let id2 = create_removal_request(
            &pool,
            "jamie-wallace-guilty",
            "Another",
            "b@example.com",
            "No basis",
        )
        .await
        .unwrap();
        set_removal_status(&pool, id2, "rejected", "admin", "Not spent")
            .await
            .unwrap();
        let hidden3 = list_hidden_refs(&pool).await.unwrap();
        assert!(!hidden3.contains(&"jamie-wallace-guilty".to_string()));

        // list_removal_requests returns both.
        let all = list_removal_requests(&pool).await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
