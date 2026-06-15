//! Editorial CMS core for PH Press: a SQLite store, staff auth (Argon2id), the
//! article lifecycle state machine with a mandatory legal sign-off before
//! publish, and a hash-chained audit trail ([`ph_audit`]) of every action.
//!
//! SERVER-ONLY. `sqlx` does not compile to wasm, so the web/SSG build never
//! depends on this crate; ph-press pulls it behind its `server` feature.

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

#[derive(Debug, thiserror::Error)]
pub enum CmsError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("authentication failed")]
    Auth,
    #[error("invalid lifecycle transition: {from} -> {to} (role {role})")]
    Transition { from: String, to: String, role: String },
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
            .map(|parsed| Argon2::default().verify_password(pw.as_bytes(), &parsed).is_ok())
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
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS staff_user (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  role TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  totp_secret TEXT,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS article (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  slug TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  body TEXT NOT NULL,
  byline TEXT NOT NULL,
  kind TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'draft',
  is_ai_assisted INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  published_at INTEGER
);
CREATE TABLE IF NOT EXISTS review_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_id INTEGER NOT NULL,
  from_state TEXT NOT NULL,
  to_state TEXT NOT NULL,
  actor TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  ts INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS correction (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_id INTEGER NOT NULL,
  original TEXT NOT NULL,
  corrected TEXT NOT NULL,
  reason TEXT NOT NULL DEFAULT '',
  ts INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS complaint (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_slug TEXT NOT NULL DEFAULT '',
  complainant TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'received',
  ts INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS audit (
  seq INTEGER PRIMARY KEY,
  ts INTEGER NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  subject TEXT NOT NULL,
  detail TEXT NOT NULL,
  prev_hash TEXT NOT NULL,
  hash TEXT NOT NULL
);
"#;

pub async fn connect(url: &str) -> Result<SqlitePool> {
    Ok(SqlitePoolOptions::new().max_connections(5).connect(url).await?)
}

pub async fn init(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(SCHEMA).execute(pool).await?;
    Ok(())
}

pub async fn count_users(pool: &SqlitePool) -> Result<i64> {
    Ok(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM staff_user")
        .fetch_one(pool)
        .await?)
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
        return Err(CmsError::Forbidden("the first user must be an admin".into()));
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

pub async fn get_article(pool: &SqlitePool, id: i64) -> Result<Option<Article>> {
    Ok(sqlx::query_as::<_, Article>("SELECT * FROM article WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?)
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
        .map(|(seq, ts, actor, action, subject, detail, prev_hash, hash)| ph_audit::Entry {
            seq: seq as u64,
            ts,
            actor,
            action,
            subject,
            detail,
            prev_hash,
            hash,
        })
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

/// Public text search over published articles (title/summary/body), newest first.
pub async fn search_articles(pool: &SqlitePool, q: &str) -> Result<Vec<Article>> {
    let like = format!("%{}%", q.replace('%', "").replace('_', ""));
    Ok(sqlx::query_as::<_, Article>(
        "SELECT * FROM article WHERE state IN ('published','corrected') AND (title LIKE ? OR summary LIKE ? OR body LIKE ?) ORDER BY COALESCE(published_at, updated_at) DESC",
    )
    .bind(&like)
    .bind(&like)
    .bind(&like)
    .fetch_all(pool)
    .await?)
}

/// Record a published correction (both versions kept) + audit it.
pub async fn add_correction(
    pool: &SqlitePool,
    article_id: i64,
    original: &str,
    corrected: &str,
    reason: &str,
) -> Result<i64> {
    let res = sqlx::query(
        "INSERT INTO correction (article_id, original, corrected, reason, ts) VALUES (?,?,?,?,?)",
    )
    .bind(article_id)
    .bind(original)
    .bind(corrected)
    .bind(reason)
    .bind(now())
    .execute(pool)
    .await?;
    append_audit(pool, "system", "article.correction", &article_id.to_string(), reason).await?;
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

/// Log a reader complaint (kept on record) + audit it.
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
    append_audit(pool, "system", "bootstrap.admin", username, "first admin created").await?;
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
        append_audit(pool, "system", "seed.articles", &format!("{inserted} seeded"), "").await?;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_gates_publish_behind_legal() {
        // A writer cannot leap a draft to published.
        assert!(!can_transition(State::Draft, State::Published, Role::Writer));
        assert!(!can_transition(State::EditorialReview, State::Published, Role::Editor));
        // Publish is only reachable from LegalReview (legal sign-off) or Scheduled.
        assert!(can_transition(State::LegalReview, State::Published, Role::Legal));
        assert!(!can_transition(State::LegalReview, State::Published, Role::Writer));
        assert!(can_transition(State::Scheduled, State::Published, Role::Editor));
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
        assert!(create_user(&pool, "w", "Writer", Role::Writer, "pw").await.is_err());
        let _admin = create_user(&pool, "admin", "Admin", Role::Admin, "pw").await.unwrap();
        create_user(&pool, "jordan", "Jordan Upton", Role::Editor, "pw1").await.unwrap();
        create_user(&pool, "scott", "Scott Taylor", Role::Legal, "pw2").await.unwrap();

        assert!(authenticate(&pool, "jordan", "pw1").await.is_ok());
        assert!(authenticate(&pool, "jordan", "nope").await.is_err());

        let editor = find_user(&pool, "jordan").await.unwrap().unwrap();
        let legal = find_user(&pool, "scott").await.unwrap().unwrap();

        let id = create_article(&pool, "test-case", "Test case", "summary", "[]", "Jordan Upton", "Court report").await.unwrap();

        // editor cannot publish directly
        assert!(transition(&pool, id, State::Published, &editor, "").await.is_err());

        // proper path: editor moves through review, legal signs off + publishes
        transition(&pool, id, State::Submitted, &editor, "").await.unwrap();
        transition(&pool, id, State::EditorialReview, &editor, "").await.unwrap();
        transition(&pool, id, State::LegalReview, &editor, "").await.unwrap();
        transition(&pool, id, State::Published, &legal, "signed off").await.unwrap();

        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.state().unwrap(), State::Published);
        assert!(a.published_at.is_some());

        // audit chain records every step + verifies
        let chain = audit_chain(&pool).await.unwrap();
        assert!(chain.entries().len() >= 4);
        assert!(chain.verify().is_ok());

        // corrections archive, complaints, public listing + search
        add_correction(&pool, id, "old text", "new text", "fixed a detail").await.unwrap();
        assert_eq!(list_corrections(&pool).await.unwrap().len(), 1);
        log_complaint(&pool, "test-case", "anon", "you got X wrong").await.unwrap();
        assert_eq!(published_articles(&pool).await.unwrap().len(), 1);
        assert_eq!(search_articles(&pool, "Test").await.unwrap().len(), 1);
        assert_eq!(search_articles(&pool, "zzznomatch").await.unwrap().len(), 0);
        assert!(audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn bootstrap_and_seed_are_idempotent() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        assert!(bootstrap_admin(&pool, "admin", "Admin", "pw").await.unwrap());
        // second call is a no-op once an admin exists
        assert!(!bootstrap_admin(&pool, "admin2", "Admin2", "pw").await.unwrap());
        assert!(authenticate(&pool, "admin", "pw").await.is_ok());

        let seeds = [
            ArticleSeed { slug: "a", title: "A", summary: "s", body: "[]", byline: "x", kind: "Court report", published_at: 1000 },
            ArticleSeed { slug: "b", title: "B", summary: "s", body: "[]", byline: "x", kind: "Court report", published_at: 2000 },
        ];
        assert_eq!(seed_articles(&pool, &seeds).await.unwrap(), 2);
        assert_eq!(seed_articles(&pool, &seeds).await.unwrap(), 0); // idempotent
        assert_eq!(published_articles(&pool).await.unwrap().len(), 2);
        assert!(audit_chain(&pool).await.unwrap().verify().is_ok());
    }
}
