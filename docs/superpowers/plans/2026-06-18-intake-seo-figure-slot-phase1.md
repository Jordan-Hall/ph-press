# Intake AI-draft — Phase 1: SEO fields + figure slot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every CMS article editable SEO fields (meta description, social/OG image, tags) and an editable, lifecycle-gated URL slug, surfaced in the `/desk` editor and rendered into the public article `<head>`.

**Architecture:** Add three columns to the `article` table via a sqlx migration, thread them through the `ph-cms` storage functions and the `api.rs` DTOs/endpoints, add the editor fields in `desk.rs`, and emit the new `<meta>`/`og:image`/keywords tags in `article.rs`. No AI in this phase — promote behaviour is unchanged; this only makes the fields exist, editable, and rendered. Phase 2 (the `ph-ai` crate) will later populate them on promote.

**Tech Stack:** Rust, Dioxus 0.8 (fullstack: wasm web + server), sqlx (SQLite), tokio. Server-only code is gated behind the `server` feature; `ph-cms` is storage-only.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-06-18-intake-ai-draft-seo-design.md`. This plan implements **Phase 1 only**.
- **`ph-cms` stays storage-only** — no network, no Dioxus, no serde_json-for-Vec at the storage layer. `tags` crosses the `ph-cms` boundary as an already-serialised JSON **string**; Vec<String> ↔ JSON conversion happens in `api.rs`.
- **Slug edits are gated to pre-publish states.** `update_article` must refuse a slug *change* when the article state `is_public()` (published/corrected). Other SEO fields stay editable on live articles.
- **`og:image` must be emitted as an ABSOLUTE URL.** If the stored value is a relative `/assets/...` path, prepend `BASE` at render time (matches the seed renderer's `format!("{BASE}{}", …)`).
- **No behaviour change to promote, the legal lifecycle, or the crawler.** New columns default to empty; existing rows and seeds are unaffected.
- **Migrations are append-only** — add `0005_…`, never edit an existing migration file.
- **Tags storage:** JSON array of strings, column default `'[]'`. Meta/og default `''`.
- **Verification commands:**
  - `ph-cms` unit tests: `cargo test -p ph-cms`
  - Server build type-check: `cargo check --no-default-features --features server`
  - Web build type-check: `cargo check`
  - Full canonical build (final gate): `dx build --fullstack --ssg`

---

### Task 1: Migration + `Article` struct fields

**Files:**
- Create: `crates/ph-cms/migrations/0005_article_seo.sql`
- Modify: `crates/ph-cms/src/lib.rs` (the `Article` struct ~216-230; add a test in the `tests` module at end of file)
- Test: `crates/ph-cms/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: existing `create_draft`, `get_article`, `connect`, `init`.
- Produces: `Article { …, meta_description: String, og_image_url: String, tags: String }` — three new public fields other tasks read/write.

- [ ] **Step 1: Write the migration file**

`crates/ph-cms/migrations/0005_article_seo.sql`:

```sql
-- SEO + social fields for articles created/edited in /desk: a search-result
-- meta description (distinct from the on-page standfirst), a social/OG image
-- URL, and topic tags (JSON array of strings). Existing rows default to empty;
-- the public renderer falls back to the summary when meta_description is blank.
ALTER TABLE article ADD COLUMN meta_description TEXT NOT NULL DEFAULT '';
ALTER TABLE article ADD COLUMN og_image_url     TEXT NOT NULL DEFAULT '';
ALTER TABLE article ADD COLUMN tags             TEXT NOT NULL DEFAULT '[]';
```

- [ ] **Step 2: Add the fields to the `Article` struct**

In `crates/ph-cms/src/lib.rs`, in `pub struct Article` (after `pub section: String,`):

```rust
    pub section: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: String, // JSON array of strings
    pub state: String,
```

(Insert the three lines between `section` and `state`. sqlx `FromRow` maps by column name, so field order is not significant, but keep it readable.)

- [ ] **Step 3: Write the failing test**

Add to the `tests` module at the bottom of `crates/ph-cms/src/lib.rs`:

```rust
    #[tokio::test]
    async fn article_carries_seo_columns_defaulting_empty() {
        let pool = connect("sqlite::memory:").await.unwrap();
        init(&pool).await.unwrap();
        let id = create_draft(
            &pool, "Title", "sum", "[]", "By", "Court report", "Crime", "admin",
        )
        .await
        .unwrap();
        let a = get_article(&pool, id).await.unwrap().unwrap();
        assert_eq!(a.meta_description, "");
        assert_eq!(a.og_image_url, "");
        assert_eq!(a.tags, "[]");
    }
```

- [ ] **Step 4: Run the test to verify it fails (before the migration/struct compile)**

Run: `cargo test -p ph-cms article_carries_seo_columns_defaulting_empty`
Expected: FAILS to compile until Steps 1-2 are in place (unknown fields), then PASSES. If you wrote Steps 1-2 first, instead temporarily comment the migration to see the `no such column` failure, then restore it.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p ph-cms article_carries_seo_columns_defaulting_empty`
Expected: PASS.

- [ ] **Step 6: Run the full ph-cms suite (no regressions)**

Run: `cargo test -p ph-cms`
Expected: all tests PASS (existing `SELECT *` reads now include the new columns).

- [ ] **Step 7: Commit**

```bash
git add crates/ph-cms/migrations/0005_article_seo.sql crates/ph-cms/src/lib.rs
git commit -m "feat(cms): add article SEO columns (meta_description, og_image_url, tags)"
```

---

### Task 2: SEO fields on the create path (`create_article` / `create_draft`)

**Files:**
- Modify: `crates/ph-cms/src/lib.rs` (`create_article` ~466-493, `create_draft` ~530-550)
- Modify call sites: `crates/ph-cms/src/ingest.rs:271` (`create_draft`), `crates/ph-cms/src/lib.rs:1080` and the `create_draft` test calls (~1245, 1257), `crates/ph-cms/src/ingest.rs:700` (`create_article`)
- Test: `crates/ph-cms/src/lib.rs` (`tests` module)

**Interfaces:**
- Produces:
  - `create_article(pool, slug, title, summary, body, byline, kind, section, meta_description, og_image_url, tags) -> Result<i64>` (three new `&str` params appended).
  - `create_draft(pool, title, summary, body, byline, kind, section, actor, meta_description, og_image_url, tags) -> Result<i64>` (three new `&str` params appended; `tags` is JSON).
- Consumes: `Article` fields from Task 1.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/ph-cms/src/lib.rs`:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ph-cms create_draft_persists_seo_fields`
Expected: FAIL — `create_draft` takes 8 args, not 11 (compile error).

- [ ] **Step 3: Extend `create_article`**

Replace the body of `create_article` in `crates/ph-cms/src/lib.rs`:

```rust
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
```

- [ ] **Step 4: Extend `create_draft`**

Replace the `create_draft` signature + the internal `create_article` call in `crates/ph-cms/src/lib.rs`:

```rust
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
    let base = slugify(title);
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
```

- [ ] **Step 5: Update the in-crate call sites to pass empty SEO defaults**

`crates/ph-cms/src/ingest.rs:271` — the `promote_lead` call to `create_draft`. Append three empty-string args:

```rust
    let article_id = create_draft(
        pool,
        &lead.title,
        &summary,
        &body_json,
        &actor.display_name,
        kind,
        section,
        &actor.username,
        "",   // meta_description (Phase 2 fills this)
        "",   // og_image_url
        "[]", // tags
    )
    .await?;
```

`crates/ph-cms/src/ingest.rs:700` (test) — the `crate::create_article(&pool, "jane-doe", "Jane Doe", "s", "[]", "Ed", "Court report", "Crime")` call. Append `, "", "", "[]"` before `.await`.

`crates/ph-cms/src/lib.rs:1080` (test) — the `create_article(...)` call. Append `, "", "", "[]"` to its argument list.

`crates/ph-cms/src/lib.rs:1245` and `:1257` (the two `create_draft(...)` calls in `create_draft_dedupes_slug_and_lists_actions`) — append `, "", "", "[]"` to each argument list (after the `"admin"` actor arg).

The **Task 1 test** `article_carries_seo_columns_defaulting_empty` also calls the now-changed `create_draft` with 8 args — append `, "", "", "[]"` to its call too (its assertions `meta_description == ""`, `og_image_url == ""`, `tags == "[]"` still hold).

- [ ] **Step 6: Run the new test + full suite**

Run: `cargo test -p ph-cms create_draft_persists_seo_fields` → Expected: PASS.
Run: `cargo test -p ph-cms` → Expected: all PASS (all call sites updated).

- [ ] **Step 7: Commit**

```bash
git add crates/ph-cms/src/lib.rs crates/ph-cms/src/ingest.rs
git commit -m "feat(cms): thread SEO fields through create_article/create_draft"
```

---

### Task 3: SEO fields + editable/gated slug on the update path (`update_article`)

**Files:**
- Modify: `crates/ph-cms/src/lib.rs` (`update_article` ~555-590; add a `slug_taken_by_other` helper near `slug_exists` ~518)
- Test: `crates/ph-cms/src/lib.rs` (`tests` module)

**Interfaces:**
- Produces: `update_article(pool, id, title, summary, body, kind, section, actor, meta_description, og_image_url, tags, slug) -> Result<()>` — four new params appended (`tags` is JSON; `slug` empty = keep current).
- Consumes: `Article::state()`, `State::is_public`, `slugify`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/ph-cms/src/lib.rs`:

```rust
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
```

(`bootstrap_admin`, `find_user`, `transition`, `State` are already in scope in the test module — see the existing `bootstrap_and_seed_are_idempotent` and `create_draft_dedupes_slug_and_lists_actions` tests.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ph-cms update_article_`
Expected: FAIL — `update_article` takes 8 args, not 12 (compile error).

- [ ] **Step 3: Add the `slug_taken_by_other` helper**

In `crates/ph-cms/src/lib.rs`, next to `slug_exists` (~518):

```rust
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
```

- [ ] **Step 4: Extend `update_article`**

Replace `update_article` in `crates/ph-cms/src/lib.rs`:

```rust
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
```

- [ ] **Step 5: Run the new tests + full suite**

Run: `cargo test -p ph-cms update_article_` → Expected: 3 PASS.
Run: `cargo test -p ph-cms` → Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ph-cms/src/lib.rs
git commit -m "feat(cms): editable, lifecycle-gated slug + SEO fields on update_article"
```

---

### Task 4: API DTOs, server-fn signatures, and `cms.rs` glue

**Files:**
- Modify: `src/api.rs` (`PreviewArticle` ~132-142, `PublicArticle` ~86-96, `desk_create` ~437-466, `desk_update` ~763-792, `desk_preview` ~796-825, `public_article` ~829-857; add a small tags helper)
- Modify: `src/cms.rs` (`create_draft` ~208-241, `update_article` ~245-276)
- Verify: `cargo check --no-default-features --features server`, then `cargo test -p ph-cms`

**Interfaces:**
- Consumes: `ph-cms` `create_draft`/`update_article` (Tasks 2-3), `Article` fields (Task 1).
- Produces:
  - DTOs `PublicArticle` and `PreviewArticle` each gain `meta_description: String`, `og_image_url: String`, `tags: Vec<String>`.
  - `desk_create(title, summary, kind, section, body, meta_description, og_image_url, tags: Vec<String>)`.
  - `desk_update(id, title, summary, kind, section, body, meta_description, og_image_url, tags: Vec<String>, slug: String)`.
  - `cms::create_draft(...)` / `cms::update_article(...)` gain matching `&str` SEO params (+ `slug` on update); `tags` is a JSON string at the cms.rs↔ph-cms boundary.

- [ ] **Step 1: Add a tags JSON helper to `src/api.rs`**

Near the top of `src/api.rs` (after the imports), add a server-only helper pair:

```rust
/// tags Vec<String> -> JSON string for storage (server-side).
#[cfg(feature = "server")]
fn tags_to_json(tags: &[String]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string())
}

/// stored tags JSON string -> Vec<String> for the DTO (server-side).
#[cfg(feature = "server")]
fn tags_from_json(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}
```

(`serde_json` is already a server dep — see `Cargo.toml` `server` feature.)

- [ ] **Step 2: Extend the DTO structs**

`PublicArticle` (add three fields):

```rust
pub struct PublicArticle {
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub body: Vec<String>,
    pub kind: String,
    pub section: String,
    pub byline: String,
    pub iso_date: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: Vec<String>,
}
```

`PreviewArticle` (add three fields, keep existing `state`):

```rust
pub struct PreviewArticle {
    pub title: String,
    pub summary: String,
    pub body: Vec<String>,
    pub kind: String,
    pub section: String,
    pub byline: String,
    pub state: String,
    pub iso_date: String,
    pub slug: String,
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: Vec<String>,
}
```

(`PreviewArticle` gains `slug` too — the editor needs the current slug to show/edit. Map `a.slug` in `desk_preview`.)

- [ ] **Step 3: Map the new fields in `desk_preview` and `public_article`**

In `desk_preview`, extend the returned `PreviewArticle`:

```rust
        Ok(Some(PreviewArticle {
            title: a.title,
            summary: a.summary,
            body,
            kind: a.kind,
            section: a.section,
            byline: a.byline,
            state: a.state,
            iso_date,
            slug: a.slug,
            meta_description: a.meta_description,
            og_image_url: a.og_image_url,
            tags: tags_from_json(&a.tags),
        }))
```

In `public_article`, extend the returned `PublicArticle`:

```rust
        Ok(Some(PublicArticle {
            slug: a.slug,
            title: a.title,
            summary: a.summary,
            body,
            kind: a.kind,
            section: a.section,
            byline: a.byline,
            iso_date,
            meta_description: a.meta_description,
            og_image_url: a.og_image_url,
            tags: tags_from_json(&a.tags),
        }))
```

- [ ] **Step 4: Extend the `cms.rs` glue functions**

`src/cms.rs` `create_draft` — add SEO params and pass through:

```rust
#[allow(clippy::too_many_arguments)]
pub async fn create_draft(
    username: &str,
    byline: &str,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
) -> Result<i64, String> {
    if title.trim().is_empty() {
        return Err("a title is required".to_string());
    }
    let paras: Vec<&str> = body_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::create_draft(
        pool, title.trim(), summary.trim(), &body_json, byline, kind, section, username,
        meta_description.trim(), og_image_url.trim(), tags,
    )
    .await
    .map_err(|e| e.to_string())
}
```

`src/cms.rs` `update_article` — add SEO params + `slug`, pass through:

```rust
#[allow(clippy::too_many_arguments)]
pub async fn update_article(
    username: &str,
    id: i64,
    title: &str,
    summary: &str,
    kind: &str,
    section: &str,
    body_text: &str,
    meta_description: &str,
    og_image_url: &str,
    tags: &str,
    slug: &str,
) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("a title is required".to_string());
    }
    let paras: Vec<&str> = body_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
    let pool = db().await.map_err(|e| e.to_string())?;
    ph_cms::update_article(
        pool, id, title.trim(), summary.trim(), &body_json, kind, section, username,
        meta_description.trim(), og_image_url.trim(), tags, slug.trim(),
    )
    .await
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Extend the `desk_create` / `desk_update` server fns**

`src/api.rs` `desk_create` — new params + pass `tags_to_json`:

```rust
#[server(endpoint = "desk_create")]
pub async fn desk_create(
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
    meta_description: String,
    og_image_url: String,
    tags: Vec<String>,
) -> Result<Vec<DeskArticle>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::create_draft(
            &session.username,
            &session.display_name,
            &title,
            &summary,
            &kind,
            &section,
            &body,
            &meta_description,
            &og_image_url,
            &tags_to_json(&tags),
        )
        .await
        .map_err(ServerFnError::new)?;
        build_desk(&session.role).await
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (title, summary, kind, section, body, meta_description, og_image_url, tags);
        Err(ServerFnError::new("server only"))
    }
}
```

`src/api.rs` `desk_update` — new params incl. `slug`:

```rust
#[server(endpoint = "desk_update")]
#[allow(clippy::too_many_arguments)]
pub async fn desk_update(
    id: i64,
    title: String,
    summary: String,
    kind: String,
    section: String,
    body: String,
    meta_description: String,
    og_image_url: String,
    tags: Vec<String>,
    slug: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "server")]
    {
        let session = require_session().await?;
        crate::cms::update_article(
            &session.username,
            id,
            &title,
            &summary,
            &kind,
            &section,
            &body,
            &meta_description,
            &og_image_url,
            &tags_to_json(&tags),
            &slug,
        )
        .await
        .map_err(ServerFnError::new)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (id, title, summary, kind, section, body, meta_description, og_image_url, tags, slug);
        Err(ServerFnError::new("server only"))
    }
}
```

- [ ] **Step 6: Type-check the server build**

Run: `cargo check --no-default-features --features server`
Expected: compiles. (The `desk.rs` call sites still pass the old arg count and will be fixed in Task 5 — if `desk.rs` is compiled in this check it will error there; that is expected and resolved in Task 5. If you want a green checkpoint now, do Step 6 *after* Task 5 Step 4. Otherwise expect the only errors to be the two `desk_create`/`desk_update` call sites in `desk.rs`.)

- [ ] **Step 7: Confirm ph-cms tests still green**

Run: `cargo test -p ph-cms`
Expected: all PASS.

- [ ] **Step 8: Commit**

```bash
git add src/api.rs src/cms.rs
git commit -m "feat(api): SEO fields on article DTOs + desk_create/desk_update endpoints"
```

---

### Task 5: Editor UI — SEO fields + slug in `EditorForm`

**Files:**
- Modify: `src/pages/desk.rs` (`WriteArticle` ~1001-1034, `WriteLoad` ~1037-1057, `EditorForm` ~1061-1271)
- Verify: `cargo check` (web) and `cargo check --no-default-features --features server`

**Interfaces:**
- Consumes: `desk_create(..., meta_description, og_image_url, tags: Vec<String>)`, `desk_update(..., meta_description, og_image_url, tags: Vec<String>, slug)`, `PreviewArticle { slug, meta_description, og_image_url, tags, state, … }` (Task 4).
- Produces: an editor that round-trips all SEO fields and the slug.

- [ ] **Step 1: Pass the new init values from `WriteArticle` (new draft) and `WriteLoad` (edit)**

In `WriteArticle`, the `id == 0` branch — add the new init props (all empty, draft state):

```rust
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
```

In `WriteLoad`, the `Some(Ok(Some(a)))` branch — map the loaded fields:

```rust
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
```

- [ ] **Step 2: Add the new props + signals to `EditorForm`**

Extend the `EditorForm` props and the signal block. Replace the `#[component] fn EditorForm(...)` parameter list and the first signal lines:

```rust
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
```

- [ ] **Step 3: Update the `submit` closure to send the new fields**

Replace the `let res = if edit_id == 0 { … } else { … };` block inside `submit`:

```rust
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
```

- [ ] **Step 4: Add the SEO input controls + meta-description meter to the RSX**

Insert a new SEO block in the `EditorForm` RSX, immediately after the `editor-sub` standfirst input (the `input { class: "editor-sub", … }` element):

```rust
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
```

Then add a meta-description meter to the `editor-meters` row. Just before the closing of that `div { class: "editor-meters", … }`, compute and render:

```rust
            // (add near the other meter calcs, e.g. just after `sum_len`)
            let meta_len = meta_desc().chars().count();
            let meta_state = if meta_len == 0 || (120..=160).contains(&meta_len) {
                "meter"
            } else {
                "meter warn"
            };
```

and inside the `editor-meters` div:

```rust
                span { class: meta_state, "Meta " b { "{meta_len}" } " / ~155" }
```

(Place the `let meta_len`/`meta_state` bindings alongside the existing `title_len`/`sum_len` calculations above the `rsx!` block, not inside it.)

- [ ] **Step 5: Type-check both builds**

Run: `cargo check` → Expected: compiles (web/client).
Run: `cargo check --no-default-features --features server` → Expected: compiles (resolves the Task 4 Step 6 call-site note).

- [ ] **Step 6: Commit**

```bash
git add src/pages/desk.rs
git commit -m "feat(desk): SEO + slug fields in the article editor"
```

---

### Task 6: Public rendering — meta description, og:image, keywords

**Files:**
- Modify: `src/pages/article.rs` (`LiveArticleBody` ~216-275; this is the renderer for CMS-published stories, i.e. promoted leads)
- Verify: `cargo check`, then `dx build --fullstack --ssg`

**Interfaces:**
- Consumes: `PublicArticle { meta_description, og_image_url, tags, … }` (Task 4).
- Produces: the public `<head>` for live articles now emits a real meta description, an absolute `og:image`/`twitter:image`, and a `keywords` meta.

- [ ] **Step 1: Compute the derived head values in `LiveArticleBody`**

In `src/pages/article.rs`, inside `fn LiveArticleBody(a: PublicArticle)`, after `let url = …` and before the `jsonld` string, add:

```rust
    // Meta description: explicit field, else fall back to the standfirst.
    let desc = if a.meta_description.trim().is_empty() {
        a.summary.clone()
    } else {
        a.meta_description.clone()
    };
    // OG image must be ABSOLUTE for social scrapers. Empty -> omitted.
    let og_image = if a.og_image_url.trim().is_empty() {
        String::new()
    } else if a.og_image_url.starts_with("http://") || a.og_image_url.starts_with("https://") {
        a.og_image_url.clone()
    } else {
        format!("{BASE}{}", a.og_image_url)
    };
    let has_og_image = !og_image.is_empty();
    let keywords = a.tags.join(", ");
    let has_keywords = !keywords.is_empty();
```

- [ ] **Step 2: Use the derived values in the head RSX**

Replace the existing head `Meta` block in `LiveArticleBody` (the `description`/`og:description`/`twitter:*` lines) with the version below, keeping the `Title`, `canonical`, `og:type`, `og:url`, `article:*`, and JSON-LD lines as they are:

```rust
        dioxus::document::Title { "{a.title} | Predator Hunters" }
        dioxus::document::Meta { name: "description", content: "{desc}" }
        if has_keywords {
            dioxus::document::Meta { name: "keywords", content: "{keywords}" }
        }
        dioxus::document::Link { rel: "canonical", href: "{url}" }
        dioxus::document::Meta { property: "og:type", content: "article" }
        dioxus::document::Meta { property: "og:title", content: "{a.title}" }
        dioxus::document::Meta { property: "og:description", content: "{desc}" }
        dioxus::document::Meta { property: "og:url", content: "{url}" }
        if has_og_image {
            dioxus::document::Meta { property: "og:image", content: "{og_image}" }
        }
        dioxus::document::Meta { property: "article:section", content: "{a.section}" }
        dioxus::document::Meta { property: "article:published_time", content: "{a.iso_date}" }
        dioxus::document::Meta { name: "twitter:card", content: "summary_large_image" }
        dioxus::document::Meta { name: "twitter:title", content: "{a.title}" }
        dioxus::document::Meta { name: "twitter:description", content: "{desc}" }
        if has_og_image {
            dioxus::document::Meta { name: "twitter:image", content: "{og_image}" }
        }
```

- [ ] **Step 3: Type-check the web build**

Run: `cargo check`
Expected: compiles.

- [ ] **Step 4: Full canonical build (Phase 1 gate)**

Run: `dx build --fullstack --ssg`
Expected: build succeeds.

- [ ] **Step 5: Manual verification**

1. `dx serve --platform web` (set `PH_DEV_INSECURE_COOKIE=1` per README dev notes if signing in over http).
2. Sign in at `/desk`, open a draft (or promote an Intake lead to get one), fill **meta description**, **tags**, **OG image URL** (e.g. `/assets/og/test.jpg`), and edit the **slug**; Save.
3. Re-open the editor → confirm all fields round-trip and the slug reflects your edit.
4. Drive the article to **Published** via the lifecycle, then `view-source` on `/news/<slug>` → confirm `<meta name="description">` is your meta text, `og:image`/`twitter:image` are **absolute** URLs, and `<meta name="keywords">` lists your tags.
5. Re-open the editor on the now-published article → confirm the **slug field is read-only**, but meta/tags/OG remain editable.

- [ ] **Step 6: Commit**

```bash
git add src/pages/article.rs
git commit -m "feat(web): render meta description, absolute og:image, and keywords for live articles"
```

---

## Self-Review

**Spec coverage (Phase 1 sections):**
- Schema `0005` (meta_description, og_image_url, tags) → Task 1. ✓
- `Article` struct + create/update threading → Tasks 1-3. ✓
- Editable slug, **gated to pre-publish** → Task 3 (server) + Task 5 (read-only UI). ✓
- DTOs (`PublicArticle`, `PreviewArticle`) + `desk_create`/`desk_update` + tags Vec↔JSON → Task 4. ✓
- Editor fields (meta w/ meter, slug, tags, OG image) → Task 5. ✓
- Public render: meta fallback, **absolute** og:image, keywords → Task 6. ✓
- Out of scope held: no image upload (OG is a pasted path), no AI, no crawl-time drafting, no lifecycle change. ✓

**Phase 2/3 (not in this plan):** `ph-ai` crate, `ai_config()`, orchestration in `cms.rs`, `promote_lead_with_draft`, guarded-scaffold prompt, AI illustration. These get a separate plan after Phase 1 is verified and the Anthropic Messages API details are confirmed against the `claude-api` skill.

**Placeholder scan:** No TBD/TODO; every code step shows complete code. ✓

**Type consistency:** `create_article`/`create_draft` gain `meta_description, og_image_url, tags: &str` (Tasks 1-2); `update_article` gains those plus `slug: &str` (Task 3); `cms.rs` mirrors with `&str` (Task 4); server fns use `tags: Vec<String>` + `tags_to_json`/`tags_from_json` (Task 4); editor sends `Vec<String>` + `slug: String` (Task 5); DTO field names `meta_description`/`og_image_url`/`tags` consistent across `Article`, `PublicArticle`, `PreviewArticle`, and the renderer (Task 6). ✓

**Cross-task build note:** Task 4 changes the server-fn arity that `desk.rs` calls; the fully-green `cargo check` for both targets lands at Task 5 Step 5. Each `ph-cms` task (1-3) is independently green via `cargo test -p ph-cms`.
