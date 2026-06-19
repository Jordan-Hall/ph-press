//! The PUBLIC ingest pipeline: a registry of crawled sources (`ingest_source`),
//! a queue of crawled LEADS awaiting editorial triage (`ingest_item`), and the
//! database-backed public conviction database (`conviction`).
//!
//! This module is the post-conviction / public side of the
//! active-proceedings firewall. It NEVER reads the private [`crate::courtwatch`]
//! store: a live or upcoming hearing can only enter here as a fresh
//! post-conviction lead after the case concludes. Promotion turns a lead into a
//! Draft article through the ordinary legal-gated lifecycle — nothing here
//! publishes automatically.

use crate::{append_audit, create_draft_with_slug, now, CmsError, Result, Role, StaffUser, State};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;

/// Offence categories the crawler tags a lead with (machine-classified, treated
/// as unverified until an editor confirms). `child` covers crimes against
/// children beyond sexual offences.
pub const OFFENCE_CATEGORIES: [&str; 4] = ["sexual", "child", "other", "unknown"];

/// Lead triage states.
pub const LEAD_STATUSES: [&str; 4] = ["new", "triaged", "promoted", "dismissed"];

/// Conviction-entry lifecycle (public DB). `published` requires a linked,
/// already-published report (our own court report).
pub const CONVICTION_STATUSES: [&str; 3] = ["draft", "published", "retracted"];

// ===================== sources =====================

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct IngestSource {
    pub id: i64,
    pub key: String,
    pub kind: String,
    pub label: String,
    pub url: String,
    pub enabled: bool,
    pub last_polled_at: Option<i64>,
}

/// Register or update a source by its stable `key` (idempotent — safe to call on
/// every boot from the configured source list). Returns the row id.
pub async fn upsert_source(
    pool: &SqlitePool,
    key: &str,
    kind: &str,
    label: &str,
    url: &str,
) -> Result<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO ingest_source (key, kind, label, url, enabled) VALUES (?,?,?,?,1)
         ON CONFLICT(key) DO UPDATE SET kind=excluded.kind, label=excluded.label, url=excluded.url
         RETURNING id",
    )
    .bind(key)
    .bind(kind)
    .bind(label)
    .bind(url)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Every configured source (for the desk + the runner).
pub async fn list_sources(pool: &SqlitePool) -> Result<Vec<IngestSource>> {
    Ok(
        sqlx::query_as::<_, IngestSource>("SELECT * FROM ingest_source ORDER BY kind, key")
            .fetch_all(pool)
            .await?,
    )
}

/// Enabled sources of a given kind (`caselaw` | `news` | `courtwatch`) — the
/// runner polls these.
pub async fn enabled_sources(pool: &SqlitePool, kind: &str) -> Result<Vec<IngestSource>> {
    Ok(sqlx::query_as::<_, IngestSource>(
        "SELECT * FROM ingest_source WHERE enabled = 1 AND kind = ? ORDER BY key",
    )
    .bind(kind)
    .fetch_all(pool)
    .await?)
}

/// Record that a source was just polled.
pub async fn mark_source_polled(pool: &SqlitePool, id: i64) -> Result<()> {
    sqlx::query("UPDATE ingest_source SET last_polled_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ===================== leads =====================

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct IngestItem {
    pub id: i64,
    pub source_id: i64,
    pub source_key: String,
    pub external_id: String,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub offence_category: String,
    pub extracted_json: String,
    pub image_url: String,
    pub image_attribution: String,
    pub status: String,
    pub promoted_article_id: Option<i64>,
    pub created_at: i64,
}

/// A new crawled lead, produced by an adapter. All fields are UNVERIFIED machine
/// output; `snippet` must be a short extract only (never the source's full body).
#[derive(Debug, Clone, Default)]
pub struct NewLead {
    pub source_id: i64,
    pub source_key: String,
    pub external_id: String,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub offence_category: String,
    pub extracted_json: String,
    pub image_url: String,
    pub image_attribution: String,
}

/// Insert a crawled lead, de-duplicated by `(source_id, external_id)`. Returns
/// `Some(id)` if a new lead was stored, `None` if it was already present (a
/// re-crawl). Only a genuinely new lead is audited.
pub async fn insert_lead(pool: &SqlitePool, lead: &NewLead) -> Result<Option<i64>> {
    let res = sqlx::query(
        "INSERT OR IGNORE INTO ingest_item
         (source_id, source_key, external_id, url, title, snippet, offence_category,
          extracted_json, image_url, image_attribution, status, created_at)
         VALUES (?,?,?,?,?,?,?,?,?,?, 'new', ?)",
    )
    .bind(lead.source_id)
    .bind(&lead.source_key)
    .bind(&lead.external_id)
    .bind(&lead.url)
    .bind(&lead.title)
    .bind(&lead.snippet)
    .bind(if lead.offence_category.is_empty() {
        "unknown"
    } else {
        &lead.offence_category
    })
    .bind(if lead.extracted_json.is_empty() {
        "{}"
    } else {
        &lead.extracted_json
    })
    .bind(&lead.image_url)
    .bind(&lead.image_attribution)
    .bind(now())
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Ok(None);
    }
    let id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM ingest_item WHERE source_id = ? AND external_id = ?",
    )
    .bind(lead.source_id)
    .bind(&lead.external_id)
    .fetch_one(pool)
    .await?;
    append_audit(
        pool,
        "crawler",
        "ingest.lead",
        &lead.source_key,
        &lead.title,
    )
    .await?;
    Ok(Some(id))
}

/// One lead by id.
pub async fn get_lead(pool: &SqlitePool, id: i64) -> Result<Option<IngestItem>> {
    Ok(
        sqlx::query_as::<_, IngestItem>("SELECT * FROM ingest_item WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Leads for the intake desk, newest first. `status` filters when `Some`.
pub async fn list_leads(pool: &SqlitePool, status: Option<&str>) -> Result<Vec<IngestItem>> {
    let q = match status {
        Some(_) => "SELECT * FROM ingest_item WHERE status = ? ORDER BY created_at DESC",
        None => "SELECT * FROM ingest_item ORDER BY created_at DESC",
    };
    let mut query = sqlx::query_as::<_, IngestItem>(q);
    if let Some(s) = status {
        query = query.bind(s);
    }
    Ok(query.fetch_all(pool).await?)
}

/// Count of leads still awaiting triage (status = 'new') — the desk badge.
pub async fn count_new_leads(pool: &SqlitePool) -> Result<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ingest_item WHERE status = 'new'")
            .fetch_one(pool)
            .await?,
    )
}

/// Set a lead's triage status (`new`/`triaged`/`dismissed` — `promoted` is set by
/// [`promote_lead`]). Audited.
pub async fn set_lead_status(pool: &SqlitePool, id: i64, status: &str, actor: &str) -> Result<()> {
    if !LEAD_STATUSES.contains(&status) || status == "promoted" {
        return Err(CmsError::Bad(format!("lead status: {status}")));
    }
    sqlx::query("UPDATE ingest_item SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    append_audit(pool, actor, "ingest.status", &id.to_string(), status).await?;
    Ok(())
}

/// Pre-generated draft content threaded into a promotion (AI output, or the banner fallback).
#[derive(Debug, Clone)]
pub struct PromotedDraft {
    pub summary: String,
    pub body_json: String,      // JSON array of paragraph strings
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: String,           // JSON array of strings
    pub slug_base: String,      // AI-suggested slug base (empty = derive from title)
}

/// The banner-only fallback content (today's behaviour) for a lead.
pub fn banner_draft(lead: &IngestItem) -> PromotedDraft {
    let banner = "DRAFT FROM AN EXTERNAL LEAD — unverified. Write this report from the \
                  public court record. Clear reporting restrictions (complainant / child \
                  anonymity) and confirm the conviction before it can be published. Use \
                  the source for context only; do not copy its wording.";
    let paras = vec![
        banner.to_string(),
        format!("Source ({}): {}", lead.source_key, lead.url),
    ];
    PromotedDraft {
        summary: "(unverified lead — write a standfirst from the court record)".to_string(),
        body_json: serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string()),
        meta_description: String::new(),
        og_image_url: String::new(),
        tags: "[]".to_string(),
        slug_base: String::new(),
    }
}

fn authoring_role_ok(actor: &StaffUser) -> Result<()> {
    if !matches!(actor.role()?, Role::Writer | Role::SubEditor | Role::Editor | Role::Admin) {
        return Err(CmsError::Forbidden(
            "your role cannot promote a lead into a draft".into(),
        ));
    }
    Ok(())
}

/// Promote a lead into a Draft using pre-generated content. The single primitive
/// both promote paths route through. Flags AI-assisted, marks the lead promoted,
/// audits. Returns the new article id.
pub async fn promote_lead_with_draft(
    pool: &SqlitePool,
    lead_id: i64,
    actor: &StaffUser,
    kind: &str,
    section: &str,
    draft: &PromotedDraft,
) -> Result<i64> {
    authoring_role_ok(actor)?;
    let lead = get_lead(pool, lead_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no lead {lead_id}")))?;
    if lead.status == "promoted" {
        return Err(CmsError::Forbidden("this lead is already promoted".into()));
    }
    let article_id = create_draft_with_slug(
        pool,
        &draft.slug_base,
        &lead.title,
        &draft.summary,
        &draft.body_json,
        &actor.display_name,
        kind,
        section,
        &actor.username,
        &draft.meta_description,
        &draft.og_image_url,
        &draft.tags,
    )
    .await?;
    sqlx::query("UPDATE article SET is_ai_assisted = 1 WHERE id = ?")
        .bind(article_id)
        .execute(pool)
        .await?;
    sqlx::query("UPDATE ingest_item SET status = 'promoted', promoted_article_id = ? WHERE id = ?")
        .bind(article_id)
        .bind(lead_id)
        .execute(pool)
        .await?;
    append_audit(
        pool,
        &actor.username,
        "ingest.promote",
        &lead.external_id,
        &format!("lead {lead_id} -> draft article {article_id}"),
    )
    .await?;
    Ok(article_id)
}

/// Banner-only promote (today's behaviour) — used when AI is off/failed.
pub async fn promote_lead(
    pool: &SqlitePool,
    lead_id: i64,
    actor: &StaffUser,
    kind: &str,
    section: &str,
) -> Result<i64> {
    let lead = get_lead(pool, lead_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no lead {lead_id}")))?;
    let draft = banner_draft(&lead);
    promote_lead_with_draft(pool, lead_id, actor, kind, section, &draft).await
}

/// Promote a lead into BOTH a draft article and a linked draft conviction, using
/// pre-generated draft content. Returns (article_id, conviction_id).
pub async fn promote_lead_to_conviction_with_draft(
    pool: &SqlitePool,
    lead_id: i64,
    actor: &StaffUser,
    kind: &str,
    section: &str,
    draft: &PromotedDraft,
) -> Result<(i64, i64)> {
    let lead = get_lead(pool, lead_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no lead {lead_id}")))?;
    let offence = match lead.offence_category.as_str() {
        "child" => "Offence against a child",
        "sexual" => "Sexual offence",
        _ => "Offence",
    }
    .to_string();
    let name = lead.title.clone();
    let source_url = lead.url.clone();
    let source_name = lead.source_key.clone();

    let article_id = promote_lead_with_draft(pool, lead_id, actor, kind, section, draft).await?;
    let article_slug = crate::get_article(pool, article_id)
        .await?
        .map(|a| a.slug)
        .unwrap_or_default();

    let conv = NewConviction {
        name,
        offence,
        article_id: Some(article_id),
        article_slug,
        source_url,
        source_name,
        ..Default::default()
    };
    let conviction_id = create_conviction(pool, &conv, actor).await?;
    Ok((article_id, conviction_id))
}

/// Banner-only conviction promote (today's behaviour).
pub async fn promote_lead_to_conviction(
    pool: &SqlitePool,
    lead_id: i64,
    actor: &StaffUser,
    kind: &str,
    section: &str,
) -> Result<(i64, i64)> {
    let lead = get_lead(pool, lead_id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no lead {lead_id}")))?;
    let draft = banner_draft(&lead);
    promote_lead_to_conviction_with_draft(pool, lead_id, actor, kind, section, &draft).await
}

// ===================== convictions (public DB) =====================

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Conviction {
    pub id: i64,
    pub name: String,
    pub area: String,
    pub offence: String,
    pub outcome: String,
    pub date: String,
    pub iso_date: String,
    pub lat: f64,
    pub lng: f64,
    pub article_id: Option<i64>,
    pub article_slug: String,
    pub source_url: String,
    pub source_name: String,
    pub status: String,
    pub created_at: i64,
    pub published_at: Option<i64>,
}

/// A new conviction entry (created as a draft).
#[derive(Debug, Clone, Default)]
pub struct NewConviction {
    pub name: String,
    pub area: String,
    pub offence: String,
    pub outcome: String,
    pub date: String,
    pub iso_date: String,
    pub lat: f64,
    pub lng: f64,
    pub article_id: Option<i64>,
    pub article_slug: String,
    pub source_url: String,
    pub source_name: String,
}

/// Resolve the linked report id: use the explicit `article_id` if given, else
/// look it up from `article_slug` (so an editor can link by the slug they already
/// know from the URL, without hunting for a numeric id).
async fn resolve_article_id(pool: &SqlitePool, c: &NewConviction) -> Result<Option<i64>> {
    if c.article_id.is_some() {
        return Ok(c.article_id);
    }
    let slug = c.article_slug.trim();
    if slug.is_empty() {
        return Ok(None);
    }
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT id FROM article WHERE slug = ?")
            .bind(slug)
            .fetch_optional(pool)
            .await?,
    )
}

/// Create a draft conviction entry. Gated to authoring roles; audited.
pub async fn create_conviction(
    pool: &SqlitePool,
    c: &NewConviction,
    actor: &StaffUser,
) -> Result<i64> {
    if !matches!(
        actor.role()?,
        Role::Writer | Role::SubEditor | Role::Editor | Role::Admin
    ) {
        return Err(CmsError::Forbidden(
            "your role cannot create a conviction entry".into(),
        ));
    }
    if c.name.trim().is_empty() || c.offence.trim().is_empty() {
        return Err(CmsError::Bad("name and offence are required".into()));
    }
    let article_id = resolve_article_id(pool, c).await?;
    let res = sqlx::query(
        "INSERT INTO conviction
         (name, area, offence, outcome, date, iso_date, lat, lng, article_id, article_slug,
          source_url, source_name, status, created_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?, 'draft', ?)",
    )
    .bind(c.name.trim())
    .bind(c.area.trim())
    .bind(c.offence.trim())
    .bind(c.outcome.trim())
    .bind(c.date.trim())
    .bind(c.iso_date.trim())
    .bind(c.lat)
    .bind(c.lng)
    .bind(article_id)
    .bind(c.article_slug.trim())
    .bind(c.source_url.trim())
    .bind(c.source_name.trim())
    .bind(now())
    .execute(pool)
    .await?;
    let id = res.last_insert_rowid();
    append_audit(pool, &actor.username, "conviction.create", &c.name, "draft").await?;
    Ok(id)
}

/// One conviction by id.
pub async fn get_conviction(pool: &SqlitePool, id: i64) -> Result<Option<Conviction>> {
    Ok(
        sqlx::query_as::<_, Conviction>("SELECT * FROM conviction WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Conviction entries for the desk, newest first. `status` filters when `Some`.
pub async fn list_convictions(pool: &SqlitePool, status: Option<&str>) -> Result<Vec<Conviction>> {
    let q = match status {
        Some(_) => "SELECT * FROM conviction WHERE status = ? ORDER BY created_at DESC",
        None => "SELECT * FROM conviction ORDER BY created_at DESC",
    };
    let mut query = sqlx::query_as::<_, Conviction>(q);
    if let Some(s) = status {
        query = query.bind(s);
    }
    Ok(query.fetch_all(pool).await?)
}

/// Published conviction entries, newest first — the PUBLIC `/database` read.
pub async fn published_convictions(pool: &SqlitePool) -> Result<Vec<Conviction>> {
    Ok(sqlx::query_as::<_, Conviction>(
        "SELECT * FROM conviction WHERE status = 'published' \
         ORDER BY COALESCE(published_at, created_at) DESC",
    )
    .fetch_all(pool)
    .await?)
}

/// Edit a DRAFT conviction's fields. Refuses once published (publishing is the
/// committed public record; corrections to a live entry go through review).
/// Gated to authoring roles; audited.
#[allow(clippy::too_many_arguments)]
pub async fn update_conviction(
    pool: &SqlitePool,
    c: &NewConviction,
    id: i64,
    actor: &StaffUser,
) -> Result<()> {
    if !matches!(
        actor.role()?,
        Role::Writer | Role::SubEditor | Role::Editor | Role::Admin
    ) {
        return Err(CmsError::Forbidden(
            "your role cannot edit a conviction entry".into(),
        ));
    }
    let existing = get_conviction(pool, id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no conviction {id}")))?;
    if existing.status != "draft" {
        return Err(CmsError::Forbidden(
            "only a draft conviction can be edited".into(),
        ));
    }
    let article_id = resolve_article_id(pool, c).await?;
    sqlx::query(
        "UPDATE conviction SET name=?, area=?, offence=?, outcome=?, date=?, iso_date=?,
         lat=?, lng=?, article_id=?, article_slug=?, source_url=?, source_name=? WHERE id=?",
    )
    .bind(c.name.trim())
    .bind(c.area.trim())
    .bind(c.offence.trim())
    .bind(c.outcome.trim())
    .bind(c.date.trim())
    .bind(c.iso_date.trim())
    .bind(c.lat)
    .bind(c.lng)
    .bind(article_id)
    .bind(c.article_slug.trim())
    .bind(c.source_url.trim())
    .bind(c.source_name.trim())
    .bind(id)
    .execute(pool)
    .await?;
    append_audit(pool, &actor.username, "conviction.edit", &existing.name, "").await?;
    Ok(())
}

/// Move a conviction to `published` or `retracted`. Publishing REQUIRES a linked
/// report (`article_id`) that is itself already public (published / corrected) —
/// the "only after we publish our own report" safeguard. Gated to Editor / Legal
/// / Admin; audited.
pub async fn set_conviction_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    actor: &StaffUser,
) -> Result<()> {
    if !matches!(actor.role()?, Role::Editor | Role::Legal | Role::Admin) {
        return Err(CmsError::Forbidden(
            "only an editor, legal or admin may change a conviction's status".into(),
        ));
    }
    if !CONVICTION_STATUSES.contains(&status) || status == "draft" {
        return Err(CmsError::Bad(format!("conviction status: {status}")));
    }
    let c = get_conviction(pool, id)
        .await?
        .ok_or_else(|| CmsError::Bad(format!("no conviction {id}")))?;
    if status == "published" {
        let article_id = c.article_id.ok_or_else(|| {
            CmsError::Forbidden(
                "a published report must be linked before this conviction can go public".into(),
            )
        })?;
        // The linked report must itself be public (our own legal-gated report).
        let state: Option<String> = sqlx::query_scalar("SELECT state FROM article WHERE id = ?")
            .bind(article_id)
            .fetch_optional(pool)
            .await?;
        let public = state
            .and_then(|s| State::parse(&s).ok())
            .map(State::is_public)
            .unwrap_or(false);
        if !public {
            return Err(CmsError::Forbidden(
                "the linked report is not published yet".into(),
            ));
        }
    }
    let published_at = if status == "published" && c.published_at.is_none() {
        Some(now())
    } else {
        c.published_at
    };
    sqlx::query("UPDATE conviction SET status = ?, published_at = ? WHERE id = ?")
        .bind(status)
        .bind(published_at)
        .bind(id)
        .execute(pool)
        .await?;
    append_audit(
        pool,
        &actor.username,
        &format!("conviction.{status}"),
        &c.name,
        "",
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_user, init, transition};

    async fn mempool() -> SqlitePool {
        let pool = crate::connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        pool
    }

    async fn user(pool: &SqlitePool, name: &str, role: Role) -> StaffUser {
        // first user must be admin (bootstrap gate)
        if crate::count_users(pool).await.unwrap() == 0 && role != Role::Admin {
            create_user(pool, "admin", "Admin", Role::Admin, "pw")
                .await
                .unwrap();
        }
        create_user(pool, name, name, role, "pw").await.unwrap();
        crate::find_user(pool, name).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn lead_dedupes_and_promotes_into_legal_gated_draft() {
        let pool = mempool().await;
        let editor = user(&pool, "ed", Role::Editor).await;
        let legal = user(&pool, "lg", Role::Legal).await;
        let src = upsert_source(&pool, "caselaw", "caselaw", "Find Case Law", "https://x")
            .await
            .unwrap();

        let lead = NewLead {
            source_id: src,
            source_key: "caselaw".into(),
            external_id: "ewca-2026-1".into(),
            url: "https://caselaw/ewca-2026-1".into(),
            title: "R v Smith".into(),
            snippet: "Sentenced for offences against a child.".into(),
            offence_category: "child".into(),
            ..Default::default()
        };
        assert!(insert_lead(&pool, &lead).await.unwrap().is_some());
        // re-crawl is de-duplicated
        assert!(insert_lead(&pool, &lead).await.unwrap().is_none());
        assert_eq!(count_new_leads(&pool).await.unwrap(), 1);

        let lead_id = list_leads(&pool, Some("new")).await.unwrap()[0].id;
        let article_id = promote_lead(&pool, lead_id, &editor, "Court report", "Crime")
            .await
            .unwrap();

        // the lead is now promoted (not re-promotable)
        assert_eq!(list_leads(&pool, Some("new")).await.unwrap().len(), 0);
        assert!(
            promote_lead(&pool, lead_id, &editor, "Court report", "Crime")
                .await
                .is_err()
        );

        // the promoted draft is a NORMAL draft: it still cannot skip the legal gate
        let a = crate::get_article(&pool, article_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.state, "draft");
        assert!(a.is_ai_assisted);
        assert!(transition(&pool, article_id, State::Published, &editor, "")
            .await
            .is_err());
        // proper path still requires legal sign-off
        transition(&pool, article_id, State::Submitted, &editor, "")
            .await
            .unwrap();
        transition(&pool, article_id, State::EditorialReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, article_id, State::LegalReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, article_id, State::Published, &legal, "ok")
            .await
            .unwrap();
        assert!(crate::audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn promote_with_draft_sets_seo_and_marks_ai_assisted() {
        let pool = mempool().await;
        let editor = user(&pool, "ed", Role::Editor).await;
        let src = upsert_source(&pool, "caselaw", "caselaw", "Find Case Law", "https://x").await.unwrap();
        let lead = NewLead { source_id: src, source_key: "caselaw".into(), external_id: "e1".into(), url: "https://c/e1".into(), title: "R v Smith".into(), offence_category: "child".into(), ..Default::default() };
        insert_lead(&pool, &lead).await.unwrap();
        let lead_id = list_leads(&pool, Some("new")).await.unwrap()[0].id;

        let draft = PromotedDraft {
            summary: "A standfirst.".into(),
            body_json: serde_json::to_string(&vec!["Para **[VERIFY: age]**."]).unwrap(),
            meta_description: "Search desc.".into(),
            og_image_url: String::new(),
            tags: r#"["grooming"]"#.into(),
            slug_base: String::new(),
        };
        let aid = promote_lead_with_draft(&pool, lead_id, &editor, "Court report", "Crime", &draft).await.unwrap();

        let a = crate::get_article(&pool, aid).await.unwrap().unwrap();
        assert_eq!(a.state, "draft");
        assert!(a.is_ai_assisted);
        assert_eq!(a.summary, "A standfirst.");
        assert_eq!(a.meta_description, "Search desc.");
        assert_eq!(a.tags, r#"["grooming"]"#);
        // lead is now promoted + not re-promotable
        assert_eq!(list_leads(&pool, Some("new")).await.unwrap().len(), 0);
        assert!(promote_lead_with_draft(&pool, lead_id, &editor, "Court report", "Crime", &draft).await.is_err());
    }

    #[tokio::test]
    async fn promote_with_draft_uses_ai_slug_base() {
        let pool = mempool().await;
        let editor = user(&pool, "ed", Role::Editor).await;
        let src = upsert_source(&pool, "caselaw", "caselaw", "Find Case Law", "https://x").await.unwrap();
        let lead = NewLead {
            source_id: src,
            source_key: "caselaw".into(),
            external_id: "slug-test-1".into(),
            url: "https://c/slug-test-1".into(),
            title: "R v Jones".into(),
            offence_category: "child".into(),
            ..Default::default()
        };
        insert_lead(&pool, &lead).await.unwrap();
        let lead_id = list_leads(&pool, Some("new")).await.unwrap()[0].id;

        let draft = PromotedDraft {
            summary: "A standfirst.".into(),
            body_json: serde_json::to_string(&vec!["Para one."]).unwrap(),
            meta_description: String::new(),
            og_image_url: String::new(),
            tags: "[]".into(),
            slug_base: "custom-ai-slug".into(),
        };
        let aid = promote_lead_with_draft(&pool, lead_id, &editor, "Court report", "Crime", &draft)
            .await
            .unwrap();

        let a = crate::get_article(&pool, aid).await.unwrap().unwrap();
        // Free slug — no suffix expected
        assert_eq!(a.slug, "custom-ai-slug");
    }

    #[tokio::test]
    async fn conviction_publishes_only_with_a_published_report() {
        let pool = mempool().await;
        let editor = user(&pool, "ed", Role::Editor).await;
        let legal = user(&pool, "lg", Role::Legal).await;

        let conv = NewConviction {
            name: "Jane Doe".into(),
            offence: "Child cruelty".into(),
            outcome: "2 years".into(),
            ..Default::default()
        };
        let cid = create_conviction(&pool, &conv, &editor).await.unwrap();
        // cannot publish without a linked report
        assert!(set_conviction_status(&pool, cid, "published", &editor)
            .await
            .is_err());

        // create + publish a report, link it, then publish the conviction
        let aid = crate::create_article(
            &pool,
            "jane-doe",
            "Jane Doe",
            "s",
            "[]",
            "Ed",
            "Court report",
            "Crime",
            "",
            "",
            "[]",
        )
        .await
        .unwrap();
        transition(&pool, aid, State::Submitted, &editor, "")
            .await
            .unwrap();
        transition(&pool, aid, State::EditorialReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, aid, State::LegalReview, &editor, "")
            .await
            .unwrap();
        transition(&pool, aid, State::Published, &legal, "")
            .await
            .unwrap();

        // Link by SLUG only (no numeric id): the engine resolves the id from the
        // slug, so an editor never has to find a numeric article id.
        let _ = aid;
        let linked = NewConviction {
            article_slug: "jane-doe".into(),
            source_url: "https://caselaw/jane".into(),
            source_name: "Find Case Law".into(),
            ..conv
        };
        update_conviction(&pool, &linked, cid, &editor)
            .await
            .unwrap();
        set_conviction_status(&pool, cid, "published", &editor)
            .await
            .unwrap();
        assert_eq!(published_convictions(&pool).await.unwrap().len(), 1);
        // a writer cannot publish a conviction
        let writer = user(&pool, "wr", Role::Writer).await;
        assert!(set_conviction_status(&pool, cid, "retracted", &writer)
            .await
            .is_err());
    }
}
