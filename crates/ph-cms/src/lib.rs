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

pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM staff_user")
            .fetch_one(pool)
            .await?,
    )
}

/// Create a staff user. The very first user must be an admin (bootstrap gate);
/// after that, only an existing admin should call this (enforce at the API layer).
pub async fn create_user(
    pool: &SqlitePool,
    username: &str,
    display_name: &str,
    role: Role,
    password: &str,
) -> Result<i64> {
    if count_users(pool).await? == 0 && role != Role::Admin {
        return Err(CmsError::Forbidden(
            "the first user must be an admin".into(),
        ));
    }
    let hash = auth::hash_password(password)?;
    let res = sqlx::query(
        "INSERT INTO staff_user (username, display_name, role, password_hash, created_at) VALUES (?,?,?,?,?)",
    )
    .bind(username)
    .bind(display_name)
    .bind(role.as_str())
    .bind(hash)
    .bind(now())
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

pub async fn create_article(
    pool: &SqlitePool,
    slug: &str,
    title: &str,
    summary: &str,
    body: &str,
    byline: &str,
    kind: &str,
) -> Result<i64> {
    let t = now();
    let res = sqlx::query(
        "INSERT INTO article (slug, title, summary, body, byline, kind, state, created_at, updated_at) VALUES (?,?,?,?,?,?,?,?,?)",
    )
    .bind(slug)
    .bind(title)
    .bind(summary)
    .bind(body)
    .bind(byline)
    .bind(kind)
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

/// Create a Draft from a title (slug derived + de-duplicated), audited. `byline`
/// is the article's credit; `actor` is the stable username recorded in the audit
/// chain (so creation is attributable even if a display name later changes). The
/// body is a JSON array of paragraph strings (same shape the public renderer reads).
pub async fn create_draft(
    pool: &SqlitePool,
    title: &str,
    summary: &str,
    body: &str,
    byline: &str,
    kind: &str,
    actor: &str,
) -> Result<i64> {
    let base = slugify(title);
    let mut slug = base.clone();
    let mut n = 2;
    while slug_exists(pool, &slug).await? {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let id = create_article(pool, &slug, title, summary, body, byline, kind).await?;
    append_audit(pool, actor, "article.create", &slug, "draft created").await?;
    Ok(id)
}

pub async fn get_article(pool: &SqlitePool, id: i64) -> Result<Option<Article>> {
    Ok(
        sqlx::query_as::<_, Article>("SELECT * FROM article WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Append a record to the hash-chained audit log (reads the current tip first).
pub async fn append_audit(
    pool: &SqlitePool,
    actor: &str,
    action: &str,
    subject: &str,
    detail: &str,
) -> Result<()> {
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
    pub body: String,
    pub status: String,
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

/// Log a reader complaint (kept on record) + audit it. Recorded by staff however
/// the complaint arrived (the public route is currently email).
pub async fn log_complaint(
    pool: &SqlitePool,
    article_slug: &str,
    complainant: &str,
    body: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO complaint (article_slug, complainant, body, status, ts) VALUES (?,?,?,'received',?)",
    )
    .bind(article_slug)
    .bind(complainant)
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

/// Valid complaint statuses (a simple documented workflow for IMPRESS).
pub const COMPLAINT_STATUSES: [&str; 4] = ["received", "under_review", "upheld", "rejected"];

/// Update a complaint's status, audited under `actor`. Returns true if a row changed.
pub async fn set_complaint_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    actor: &str,
) -> Result<bool> {
    if !COMPLAINT_STATUSES.contains(&status) {
        return Err(CmsError::Bad(format!("complaint status: {status}")));
    }
    let res = sqlx::query("UPDATE complaint SET status=? WHERE id=?")
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
    create_user(pool, username, display_name, Role::Admin, password).await?;
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
    pub published_at: i64,
}

/// Idempotently insert seed articles as Published (by slug). Returns how many
/// were newly inserted. Safe to call on every boot.
pub async fn seed_articles(pool: &SqlitePool, items: &[ArticleSeed<'_>]) -> Result<u64> {
    let mut inserted = 0u64;
    for a in items {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO article (slug, title, summary, body, byline, kind, state, created_at, updated_at, published_at) VALUES (?,?,?,?,?,?, 'published', ?, ?, ?)",
        )
        .bind(a.slug)
        .bind(a.title)
        .bind(a.summary)
        .bind(a.body)
        .bind(a.byline)
        .bind(a.kind)
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
    admin_pass: &str,
    reset_admin: bool,
    seeds: &[ArticleSeed<'_>],
) -> Result<Db> {
    let pool = connect(url).await?;
    init(&pool).await?;
    let created = bootstrap_admin(&pool, admin_user, admin_display, admin_pass).await?;
    // Never overwrite the admin on a normal deploy. Only reset the password when
    // the operator explicitly asks (PH_ADMIN_RESET), i.e. a deliberate takeover.
    if !created && reset_admin && !reset_password(&pool, admin_user, admin_pass).await? {
        eprintln!("[ph-cms] PH_ADMIN_RESET set but no user '{admin_user}' to reset");
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
    async fn db_lifecycle_end_to_end() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();

        // bootstrap gate: first user must be admin
        assert!(create_user(&pool, "w", "Writer", Role::Writer, "pw")
            .await
            .is_err());
        let _admin = create_user(&pool, "admin", "Admin", Role::Admin, "pw")
            .await
            .unwrap();
        create_user(&pool, "jordan", "Jordan Upton", Role::Editor, "pw1")
            .await
            .unwrap();
        create_user(&pool, "scott", "Scott Taylor", Role::Legal, "pw2")
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
        log_complaint(&pool, "test-case", "anon", "you got X wrong")
            .await
            .unwrap();
        assert_eq!(list_complaints(&pool).await.unwrap().len(), 1);
        let cid = list_complaints(&pool).await.unwrap()[0].id;
        assert!(set_complaint_status(&pool, cid, "under_review", "editor")
            .await
            .unwrap());
        assert!(set_complaint_status(&pool, cid, "bogus", "editor")
            .await
            .is_err());
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
                published_at: 1000,
            },
            ArticleSeed {
                slug: "b",
                title: "B",
                summary: "s",
                body: "[]",
                byline: "x",
                kind: "Court report",
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
            "admin",
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
            "admin",
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
}
