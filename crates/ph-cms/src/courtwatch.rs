//! The PRIVATE court-watch store: upcoming / appeal / listing hearings the
//! newsroom wants to attend or request a transcript for.
//!
//! This is the LIVE-proceedings side of the active-proceedings firewall. It is
//! deliberately isolated from the public pipeline in [`crate::ingest`]: this
//! module has no function that writes an `ingest_item` or a `conviction`, and the
//! public side never reads `court_watch`. Court-watch data is never published. A
//! case that concludes is re-discovered by the post-conviction adapters as a
//! fresh lead — it is never "promoted" across the firewall. (Contempt of Court
//! Act 1981.)

use crate::{append_audit, now, CmsError, Result, StaffUser};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;

/// Hearing kinds a watch entry can carry.
pub const HEARING_TYPES: [&str; 4] = ["trial", "appeal", "sentencing", "listing"];

/// Court-watch workflow states.
pub const WATCH_STATUSES: [&str; 4] = ["watching", "attending", "transcript_requested", "closed"];

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct CourtWatch {
    pub id: i64,
    pub court: String,
    pub case_ref: String,
    pub hearing_date: String,
    pub hearing_type: String,
    pub offence_category: String,
    pub source_key: String,
    pub external_id: String,
    pub source_url: String,
    pub notes: String,
    pub status: String,
    pub created_at: i64,
}

/// A new court-watch entry (from a listing adapter or added by hand).
#[derive(Debug, Clone, Default)]
pub struct NewWatch {
    pub court: String,
    pub case_ref: String,
    pub hearing_date: String,
    pub hearing_type: String,
    pub offence_category: String,
    pub source_key: String,
    pub external_id: String,
    pub source_url: String,
    pub notes: String,
}

/// Insert a court-watch entry, de-duplicated by `(source_key, external_id)`.
/// Returns `Some(id)` for a new entry, `None` if already present. Audited on
/// insert only.
pub async fn insert_watch(pool: &SqlitePool, w: &NewWatch) -> Result<Option<i64>> {
    let hearing_type = if HEARING_TYPES.contains(&w.hearing_type.as_str()) {
        w.hearing_type.as_str()
    } else {
        "listing"
    };
    let res = sqlx::query(
        "INSERT OR IGNORE INTO court_watch
         (court, case_ref, hearing_date, hearing_type, offence_category, source_key,
          external_id, source_url, notes, status, created_at)
         VALUES (?,?,?,?,?,?,?,?,?, 'watching', ?)",
    )
    .bind(w.court.trim())
    .bind(w.case_ref.trim())
    .bind(w.hearing_date.trim())
    .bind(hearing_type)
    .bind(if w.offence_category.is_empty() {
        "unknown"
    } else {
        &w.offence_category
    })
    .bind(w.source_key.trim())
    .bind(w.external_id.trim())
    .bind(w.source_url.trim())
    .bind(w.notes.trim())
    .bind(now())
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Ok(None);
    }
    let id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM court_watch WHERE source_key = ? AND external_id = ?",
    )
    .bind(w.source_key.trim())
    .bind(w.external_id.trim())
    .fetch_one(pool)
    .await?;
    append_audit(
        pool,
        "crawler",
        "courtwatch.add",
        &w.source_key,
        &w.case_ref,
    )
    .await?;
    Ok(Some(id))
}

/// Court-watch entries, by hearing date (soonest first). `status` filters when
/// `Some`.
pub async fn list_watch(pool: &SqlitePool, status: Option<&str>) -> Result<Vec<CourtWatch>> {
    let q = match status {
        Some(_) => "SELECT * FROM court_watch WHERE status = ? ORDER BY hearing_date ASC",
        None => "SELECT * FROM court_watch ORDER BY hearing_date ASC",
    };
    let mut query = sqlx::query_as::<_, CourtWatch>(q);
    if let Some(s) = status {
        query = query.bind(s);
    }
    Ok(query.fetch_all(pool).await?)
}

/// Update a court-watch entry's status (attending / transcript requested /
/// closed) and optionally append a note. Gated to any authenticated staff;
/// audited.
pub async fn set_watch_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    note: &str,
    actor: &StaffUser,
) -> Result<()> {
    // Any staff role may manage the watch list (it is purely internal).
    let _ = actor.role()?;
    if !WATCH_STATUSES.contains(&status) {
        return Err(CmsError::Bad(format!("watch status: {status}")));
    }
    if note.trim().is_empty() {
        sqlx::query("UPDATE court_watch SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("UPDATE court_watch SET status = ?, notes = ? WHERE id = ?")
            .bind(status)
            .bind(note.trim())
            .bind(id)
            .execute(pool)
            .await?;
    }
    append_audit(
        pool,
        &actor.username,
        "courtwatch.status",
        &id.to_string(),
        status,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest;
    use crate::{create_user, init, Role};

    #[tokio::test]
    async fn firewall_courtwatch_never_reaches_the_public_pipeline() {
        let pool = crate::connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();

        // An upcoming (LIVE) hearing lands in the private court-watch store.
        let w = NewWatch {
            court: "Leicester Crown Court".into(),
            case_ref: "T2026-99".into(),
            hearing_date: "2026-07-01".into(),
            hearing_type: "trial".into(),
            offence_category: "child".into(),
            source_key: "courtwatch-gov".into(),
            external_id: "lcc-t2026-99".into(),
            ..Default::default()
        };
        assert!(insert_watch(&pool, &w).await.unwrap().is_some());
        // de-dupes on re-crawl
        assert!(insert_watch(&pool, &w).await.unwrap().is_none());
        assert_eq!(list_watch(&pool, None).await.unwrap().len(), 1);

        // INVARIANT: nothing crossed the firewall into the public pipeline.
        assert_eq!(ingest::list_leads(&pool, None).await.unwrap().len(), 0);
        assert_eq!(
            ingest::list_convictions(&pool, None).await.unwrap().len(),
            0
        );
        assert_eq!(ingest::published_convictions(&pool).await.unwrap().len(), 0);

        assert!(crate::audit_chain(&pool).await.unwrap().verify().is_ok());
    }

    #[tokio::test]
    async fn watch_status_workflow() {
        let pool = crate::connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        create_user(&pool, "admin", "Admin", Role::Admin, "pw", "")
            .await
            .unwrap();
        let staff = crate::find_user(&pool, "admin").await.unwrap().unwrap();

        let id = insert_watch(
            &pool,
            &NewWatch {
                court: "RCJ".into(),
                source_key: "k".into(),
                external_id: "e".into(),
                hearing_type: "appeal".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .unwrap();
        set_watch_status(&pool, id, "attending", "Scott to attend", &staff)
            .await
            .unwrap();
        assert!(set_watch_status(&pool, id, "bogus", "", &staff)
            .await
            .is_err());
        let row = &list_watch(&pool, Some("attending")).await.unwrap()[0];
        assert_eq!(row.notes, "Scott to attend");
        assert_eq!(row.hearing_type, "appeal");
    }
}
