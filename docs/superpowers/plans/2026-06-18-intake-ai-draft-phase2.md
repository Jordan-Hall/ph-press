# Intake AI-draft — Phase 2: AI draft on promote Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When an editor promotes an Intake lead, generate a guarded AI draft (original prose + `[VERIFY]` markers + SEO + figure slot) via the Anthropic API, falling back to today's banner draft whenever AI is disabled or fails.

**Architecture:** A new server-only `ph-ai` crate turns structured `LeadFacts` into a typed `AiDraft` by calling the Anthropic Messages API with `reqwest` (forced tool use for validated JSON). `src/cms.rs` orchestrates: it generates the draft content once and threads it into a new content-driven `ph-cms` primitive used by BOTH the article-only and article+conviction promote paths. `ph-cms` stays storage-only.

**Tech Stack:** Rust, reqwest (blocking-free async, rustls), serde/serde_json, tokio, sqlx (SQLite), Dioxus 0.8 fullstack. Server-only code behind the `server` feature.

## Global Constraints

- **Spec:** `docs/superpowers/specs/2026-06-18-intake-ai-draft-seo-design.md`. This plan is **Phase 2**. Phase 1 (SEO + slot) is already merged (commits `79878a6..378a628`).
- **Builds on Phase 1.** `create_article`/`create_draft` already take `meta_description, og_image_url, tags`; `update_article` adds an editable gated `slug`; the editor and public head render SEO. Phase 2 fills these via AI on promote.
- **Accuracy guardrail (core):** the AI gets only the lead title + ~300-char snippet + offence classification + (caselaw) citation/court. It MUST produce a *guarded scaffold*, not a finished article: use only supplied facts; write neutral original prose (never the source's wording); insert explicit `**[VERIFY: …]**` / `**[FROM RECORD: …]**` markers wherever a fact must be confirmed; NEVER invent names, ages, dates, or sentences; for `child`/`sexual`/`id_risk` leads, avoid any victim-identifying detail. The existing "DRAFT FROM AN EXTERNAL LEAD" banner stays the first paragraph.
- **Firewall preserved:** promoted drafts stay `is_ai_assisted = 1`, remain normal Drafts subject to the full legal gate, and never carry copied source prose or republished source images.
- **No surprise outbound traffic:** AI is OFF by default. `ai_config()` returns `Some` only when `PH_AI_ENABLED` is truthy AND `PH_AI_API_KEY` is non-empty. Off/failed → banner draft. **Promote NEVER fails because of AI.**
- **`ph-cms` stays storage-only** — no network/Dioxus/AI deps. `ph-ai` has no `ph-cms`/Dioxus dep.
- **`ph-ai` Cargo wiring:** added to the workspace AND to the main crate as `optional = true`, pulled in only by the `server` feature (exactly like `ph-crawl`), so the wasm/web build never compiles it.
- **Anthropic API contract (confirmed — see memory `anthropic-api-for-ph-ai`):**
  - `POST https://api.anthropic.com/v1/messages`; headers `x-api-key`, `anthropic-version: 2023-06-01`, `content-type: application/json`.
  - Body: `model`, `max_tokens` (~4000), `system` (the guardrails), `messages` (one user message carrying `LeadFacts` as JSON text), `tools: [emit_draft]` (one tool whose `input_schema` is the `AiDraft` shape), `tool_choice: {"type":"tool","name":"emit_draft"}`. No `thinking`, no streaming (small output).
  - Response success: `stop_reason == "tool_use"` and a `content[]` block with `"type":"tool_use"` whose `.input` is the `AiDraft` JSON. Anything else (incl. `"refusal"`, network/non-200, JSON parse failure) → `AiError` → banner fallback.
  - Model default `claude-sonnet-4-6` (cost-sensitive, approved), env override `PH_AI_MODEL`. Use exact id strings, never date-suffixed.
- **Verification commands:**
  - `ph-ai` unit tests: `cargo test -p ph-ai`
  - `ph-cms` unit tests: `cargo test -p ph-cms`
  - Server build: `cargo check --no-default-features --features server`
  - Web build: `cargo check`
  - Full build (final gate): `dx build --fullstack --ssg`

---

### Task 1: `ph-ai` crate scaffold + types + request builder

**Files:**
- Create: `crates/ph-ai/Cargo.toml`, `crates/ph-ai/src/lib.rs`
- Modify: `Cargo.toml` (root `[workspace] members` — add `"crates/ph-ai"`)
- Test: `crates/ph-ai/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  ```rust
  pub struct AiConfig { pub api_key: String, pub model: String, pub base_url: String, pub max_tokens: u32, pub timeout_secs: u64 }
  pub struct LeadFacts { pub title: String, pub snippet: String, pub offence_category: String, pub source_key: String, pub source_url: String, pub citation: String, pub court: String, pub kind: String, pub section: String, pub id_risk: bool }
  pub struct AiDraft { pub summary: String, pub meta_description: String, pub slug: String, pub tags: Vec<String>, pub body_paragraphs: Vec<String>, pub figure_caption: String }
  pub enum AiError { Disabled, Http(String), Status(u16), Parse(String), NoToolUse }
  pub fn system_prompt(facts: &LeadFacts) -> String;     // guardrails; extra anonymity text when id_risk/child/sexual
  pub fn build_request_body(facts: &LeadFacts, cfg: &AiConfig) -> serde_json::Value;  // pure
  ```

- [ ] **Step 1: Create the crate manifest**

`crates/ph-ai/Cargo.toml`:

```toml
[package]
name = "ph-ai"
version = "0.0.0"
edition = "2021"
publish = false
description = "Anthropic Messages API client for guarded AI drafting of promoted leads (server-only)."

[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
```

- [ ] **Step 2: Add the crate to the workspace**

In the root `Cargo.toml`, add `"crates/ph-ai"` to `[workspace] members`:

```toml
members = ["crates/ph-audit", "crates/ph-ai", "crates/ph-cms", "crates/ph-crawl", "crates/taino-edit-dx", "crates/web-framework-markdown", "crates/dioxus-markdown"]
```

- [ ] **Step 3: Write the types + `system_prompt` + `build_request_body` (lib.rs)**

`crates/ph-ai/src/lib.rs`:

```rust
//! Anthropic Messages API client for guarded AI drafting (server-only). No DB,
//! no Dioxus. Turns structured lead facts into a typed draft via forced tool use.
//! Every value is UNVERIFIED machine output; the draft is a scaffold the editor
//! rewrites from the court record before it can be published.

use serde::{Deserialize, Serialize};

/// Runtime config (resolved from env by the caller).
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub timeout_secs: u64,
}

/// The facts handed to the model (all UNVERIFIED).
#[derive(Debug, Clone, Default, Serialize)]
pub struct LeadFacts {
    pub title: String,
    pub snippet: String,
    pub offence_category: String,
    pub source_key: String,
    pub source_url: String,
    pub citation: String,
    pub court: String,
    pub kind: String,
    pub section: String,
    pub id_risk: bool,
}

/// The typed draft the model returns (via the emit_draft tool).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct AiDraft {
    pub summary: String,
    pub meta_description: String,
    pub slug: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub body_paragraphs: Vec<String>,
    #[serde(default)]
    pub figure_caption: String,
}

#[derive(Debug)]
pub enum AiError {
    Disabled,
    Http(String),
    Status(u16),
    Parse(String),
    NoToolUse,
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::Disabled => write!(f, "AI disabled"),
            AiError::Http(e) => write!(f, "AI http error: {e}"),
            AiError::Status(c) => write!(f, "AI status {c}"),
            AiError::Parse(e) => write!(f, "AI parse error: {e}"),
            AiError::NoToolUse => write!(f, "AI returned no tool_use block"),
        }
    }
}

/// The guardrail system prompt. Extra anonymity instruction for sensitive leads.
pub fn system_prompt(facts: &LeadFacts) -> String {
    let mut s = String::from(
        "You draft court-reporting SCAFFOLDS for a UK newsroom from UNVERIFIED leads. \
         You receive only a headline, a short snippet, and a machine offence-category — \
         NOT the court record. Produce a guarded scaffold, never a finished article:\n\
         - Use ONLY facts present in the input. Never invent names, ages, dates, places, \
         pleas, or sentences.\n\
         - Write neutral, original house-style prose to connect the known facts. NEVER copy \
         or paraphrase the source's wording.\n\
         - Wherever a fact must be confirmed against the court record, insert an explicit \
         marker like **[VERIFY: defendant age]** or **[FROM RECORD: sentence length]**.\n\
         - Keep it short (3-6 short paragraphs). The editor will rewrite it from the record.\n\
         - summary is a one-line standfirst; meta_description is ~155 chars for search; \
         slug is a lowercase hyphenated URL slug; tags are 2-5 short topic tags; \
         figure_caption describes a non-identifying illustrative image (the editor supplies the image).",
    );
    if facts.id_risk || matches!(facts.offence_category.as_str(), "child" | "sexual") {
        s.push_str(
            "\n- ANONYMITY: this case carries automatic reporting restrictions. Do NOT include \
             any detail that could identify a victim (relationships, schools, locations, ages of \
             children). Flag anonymity in a [VERIFY: reporting restrictions] marker.",
        );
    }
    s
}

/// The emit_draft tool's JSON schema (forced tool use → validated JSON output).
fn tool_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "emit_draft",
        "description": "Emit the guarded article-draft scaffold.",
        "input_schema": {
            "type": "object",
            "properties": {
                "summary": {"type": "string", "description": "One-line standfirst."},
                "meta_description": {"type": "string", "description": "~155-char search description."},
                "slug": {"type": "string", "description": "lowercase-hyphenated URL slug."},
                "tags": {"type": "array", "items": {"type": "string"}},
                "body_paragraphs": {"type": "array", "items": {"type": "string"}, "description": "Scaffold paragraphs with [VERIFY] markers."},
                "figure_caption": {"type": "string"}
            },
            "required": ["summary", "meta_description", "slug", "body_paragraphs"]
        }
    })
}

/// Build the Messages API request body. Pure — no network.
pub fn build_request_body(facts: &LeadFacts, cfg: &AiConfig) -> serde_json::Value {
    let user = serde_json::to_string(facts).unwrap_or_else(|_| "{}".to_string());
    serde_json::json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "system": system_prompt(facts),
        "tools": [tool_schema()],
        "tool_choice": {"type": "tool", "name": "emit_draft"},
        "messages": [
            {"role": "user", "content": format!("Draft a scaffold from these UNVERIFIED lead facts (JSON):\n{user}")}
        ]
    })
}
```

- [ ] **Step 4: Write the failing test**

Add to `crates/ph-ai/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> LeadFacts {
        LeadFacts {
            title: "R v Smith".into(),
            snippet: "Jailed for offences against a child.".into(),
            offence_category: "child".into(),
            kind: "Court report".into(),
            section: "Crime".into(),
            ..Default::default()
        }
    }

    fn cfg() -> AiConfig {
        AiConfig { api_key: "k".into(), model: "claude-sonnet-4-6".into(), base_url: "https://api.anthropic.com".into(), max_tokens: 4000, timeout_secs: 30 }
    }

    #[test]
    fn request_body_forces_the_emit_draft_tool() {
        let body = build_request_body(&facts(), &cfg());
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tool_choice"]["name"], "emit_draft");
        assert_eq!(body["tools"][0]["name"], "emit_draft");
        // user message carries the facts as JSON
        let content = body["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("R v Smith"));
    }

    #[test]
    fn child_case_system_prompt_adds_anonymity_rule() {
        let p = system_prompt(&facts());
        assert!(p.contains("ANONYMITY"));
        assert!(p.contains("[VERIFY"));
        // a non-sensitive lead does not get the anonymity block
        let other = LeadFacts { offence_category: "other".into(), id_risk: false, ..facts() };
        assert!(!system_prompt(&other).contains("ANONYMITY"));
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p ph-ai`
Expected: 2 tests PASS (after Steps 1-3 compile). If you wrote tests first, they fail to compile until the types/functions exist, then pass.

- [ ] **Step 6: Commit**

```bash
git add crates/ph-ai/Cargo.toml crates/ph-ai/src/lib.rs Cargo.toml
git commit -m "feat(ai): ph-ai crate scaffold — LeadFacts/AiDraft + guarded request builder

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `ph-ai` response parser (`parse_tool_response`)

**Files:**
- Modify: `crates/ph-ai/src/lib.rs`
- Test: `crates/ph-ai/src/lib.rs` (`tests`)

**Interfaces:**
- Produces: `pub fn parse_tool_response(resp: &serde_json::Value) -> Result<AiDraft, AiError>` — extracts the `tool_use` block's `.input` into `AiDraft`.
- Consumes: `AiDraft`, `AiError` (Task 1).

- [ ] **Step 1: Write the failing tests**

Add to `crates/ph-ai/src/lib.rs` `tests`:

```rust
    fn ok_response() -> serde_json::Value {
        serde_json::json!({
            "stop_reason": "tool_use",
            "content": [
                {"type": "text", "text": "ignored"},
                {"type": "tool_use", "name": "emit_draft", "input": {
                    "summary": "A standfirst.",
                    "meta_description": "A search description.",
                    "slug": "r-v-smith",
                    "tags": ["grooming", "crown court"],
                    "body_paragraphs": ["Para one **[VERIFY: age]**.", "Para two."],
                    "figure_caption": "Court building exterior."
                }}
            ]
        })
    }

    #[test]
    fn parses_tool_use_input_into_draft() {
        let d = parse_tool_response(&ok_response()).unwrap();
        assert_eq!(d.slug, "r-v-smith");
        assert_eq!(d.tags, vec!["grooming", "crown court"]);
        assert_eq!(d.body_paragraphs.len(), 2);
    }

    #[test]
    fn no_tool_use_block_is_an_error() {
        let resp = serde_json::json!({"stop_reason": "refusal", "content": [{"type": "text", "text": "no"}]});
        assert!(matches!(parse_tool_response(&resp), Err(AiError::NoToolUse)));
    }

    #[test]
    fn malformed_input_is_a_parse_error() {
        let resp = serde_json::json!({"content": [{"type": "tool_use", "name": "emit_draft", "input": {"summary": 5}}]});
        assert!(matches!(parse_tool_response(&resp), Err(AiError::Parse(_))));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p ph-ai parse` → Expected: FAIL (function not defined).

- [ ] **Step 3: Implement `parse_tool_response`**

Add to `crates/ph-ai/src/lib.rs` (above `mod tests`):

```rust
/// Extract the emit_draft tool_use input from a Messages API response. Pure.
pub fn parse_tool_response(resp: &serde_json::Value) -> Result<AiDraft, AiError> {
    let content = resp.get("content").and_then(|c| c.as_array());
    let input = content
        .and_then(|blocks| {
            blocks
                .iter()
                .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        })
        .and_then(|b| b.get("input"))
        .ok_or(AiError::NoToolUse)?;
    let mut draft: AiDraft =
        serde_json::from_value(input.clone()).map_err(|e| AiError::Parse(e.to_string()))?;
    if draft.body_paragraphs.is_empty() || draft.summary.trim().is_empty() {
        return Err(AiError::Parse("empty draft body or summary".into()));
    }
    // Normalise the slug defensively (the editor can still edit it).
    draft.slug = slugify(&draft.slug);
    Ok(draft)
}

/// Minimal slug normaliser (lowercase ascii alphanumerics, single dashes).
fn slugify(s: &str) -> String {
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
    out
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ph-ai` → Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ph-ai/src/lib.rs
git commit -m "feat(ai): parse_tool_response — extract validated AiDraft from tool_use

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: `ph-ai` networked `draft()`

**Files:**
- Modify: `crates/ph-ai/src/lib.rs`
- Test: `crates/ph-ai/src/lib.rs` (an `#[ignore]` integration test — needs a real key)

**Interfaces:**
- Produces: `pub async fn draft(facts: &LeadFacts, cfg: &AiConfig) -> Result<AiDraft, AiError>`
- Consumes: `build_request_body`, `parse_tool_response`, `AiConfig`, `AiError`.

- [ ] **Step 1: Implement `draft()`**

Add to `crates/ph-ai/src/lib.rs`:

```rust
/// Call the Anthropic Messages API and return a typed draft. Networked.
pub async fn draft(facts: &LeadFacts, cfg: &AiConfig) -> Result<AiDraft, AiError> {
    if cfg.api_key.trim().is_empty() {
        return Err(AiError::Disabled);
    }
    let body = build_request_body(facts, cfg);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
        .build()
        .map_err(|e| AiError::Http(e.to_string()))?;
    let url = format!("{}/v1/messages", cfg.base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AiError::Http(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AiError::Status(status.as_u16()));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| AiError::Http(e.to_string()))?;
    parse_tool_response(&json)
}
```

- [ ] **Step 2: Add an ignored integration test**

Add to `crates/ph-ai/src/lib.rs` `tests`:

```rust
    // Networked — run manually with a real key:
    //   PH_AI_API_KEY=sk-ant-... cargo test -p ph-ai live_draft -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn live_draft() {
        let key = std::env::var("PH_AI_API_KEY").unwrap_or_default();
        if key.is_empty() {
            return;
        }
        let cfg = AiConfig { api_key: key, model: "claude-sonnet-4-6".into(), base_url: "https://api.anthropic.com".into(), max_tokens: 4000, timeout_secs: 60 };
        let d = draft(&facts(), &cfg).await.unwrap();
        assert!(!d.body_paragraphs.is_empty());
        assert!(d.body_paragraphs.iter().any(|p| p.contains("[VERIFY")));
        println!("{d:#?}");
    }
```

- [ ] **Step 3: Verify build + unit tests (the ignored test is skipped)**

Run: `cargo test -p ph-ai` → Expected: all PASS, `live_draft` reported as ignored.

- [ ] **Step 4: Commit**

```bash
git add crates/ph-ai/src/lib.rs
git commit -m "feat(ai): networked draft() over the Anthropic Messages API (reqwest)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: `ph-cms` content-driven promote primitive

**Files:**
- Modify: `crates/ph-cms/src/ingest.rs` (`promote_lead` ~234-301; `promote_lead_to_conviction` ~307-345)
- Test: `crates/ph-cms/src/ingest.rs` (`tests`)

**Interfaces:**
- Produces:
  ```rust
  pub struct PromotedDraft { pub summary: String, pub body_json: String, pub meta_description: String, pub og_image_url: String, pub tags: String /* JSON */ }
  pub async fn promote_lead_with_draft(pool, lead_id: i64, actor: &StaffUser, kind: &str, section: &str, draft: &PromotedDraft) -> Result<i64>;
  pub async fn promote_lead_to_conviction_with_draft(pool, lead_id, actor, kind, section, draft: &PromotedDraft) -> Result<(i64, i64)>;
  ```
- The existing `promote_lead` / `promote_lead_to_conviction` keep their signatures and become thin wrappers that build the **banner** `PromotedDraft` and delegate, so all current callers/tests keep working.

- [ ] **Step 1: Write the failing test**

Add to `crates/ph-cms/src/ingest.rs` `tests`:

```rust
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
```

(`mempool`, `user`, `upsert_source`, `insert_lead`, `list_leads`, `NewLead` are already in the ingest test module.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p ph-cms promote_with_draft` → Expected: FAIL (types/functions not defined).

- [ ] **Step 3: Add `PromotedDraft` + the content-driven primitives, refactor the wrappers**

Replace the existing `promote_lead` and `promote_lead_to_conviction` in `crates/ph-cms/src/ingest.rs` with:

```rust
/// Pre-generated draft content threaded into a promotion (AI output, or the banner fallback).
#[derive(Debug, Clone)]
pub struct PromotedDraft {
    pub summary: String,
    pub body_json: String,      // JSON array of paragraph strings
    pub meta_description: String,
    pub og_image_url: String,
    pub tags: String,           // JSON array of strings
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
    let article_id = create_draft(
        pool,
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
```

(Keep the existing `use` imports; `create_draft` now takes the Phase-1 SEO params, which this code supplies.)

- [ ] **Step 4: Run the new test + full ph-cms suite**

Run: `cargo test -p ph-cms promote_with_draft` → PASS.
Run: `cargo test -p ph-cms` → all PASS (the existing `lead_dedupes_and_promotes_into_legal_gated_draft` test still exercises the banner wrapper).

- [ ] **Step 5: Commit**

```bash
git add crates/ph-cms/src/ingest.rs
git commit -m "feat(cms): content-driven promote primitive (PromotedDraft) for both promote paths

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Orchestration in `src/cms.rs` (ai_config + generate + rewire)

**Files:**
- Modify: `src/cms.rs` (`promote_lead` ~445-451, `promote_lead_to_conviction` ~752-764; add helpers + `use`)
- Modify: `Cargo.toml` (main — add `ph-ai` optional dep + `dep:ph-ai` to `server` feature)
- Verify: `cargo check --no-default-features --features server`, `cargo test -p ph-cms`, `cargo test -p ph-ai`

**Interfaces:**
- Consumes: `ph_ai::{AiConfig, LeadFacts, draft}`, `ph_cms::ingest::{PromotedDraft, banner_draft, promote_lead_with_draft, promote_lead_to_conviction_with_draft, get_lead}`.
- Produces: unchanged `cms::promote_lead` / `cms::promote_lead_to_conviction` signatures (api.rs is untouched), now AI-driven with banner fallback.

- [ ] **Step 1: Wire `ph-ai` into the main crate Cargo**

In the root `Cargo.toml` `[dependencies]`, add:

```toml
# AI drafting client (server-only). Pulls reqwest; behind `server`, like ph-crawl.
ph-ai = { path = "crates/ph-ai", optional = true }
```

And add `"dep:ph-ai"` to the `server` feature:

```toml
server = ["dioxus/server", "dep:ph-cms", "dep:ph-crawl", "dep:ph-ai", "dep:tokio", "dep:serde_json"]
```

- [ ] **Step 2: Add `ai_config()`, `lead_facts()`, and `generate_promo_content()` to `src/cms.rs`**

Add near the crawler-boot helpers in `src/cms.rs`:

```rust
/// Resolve the AI config from env, or None when disabled / unconfigured.
/// OFF by default — no surprise outbound traffic.
fn ai_config() -> Option<ph_ai::AiConfig> {
    if !env_flag("PH_AI_ENABLED") {
        return None;
    }
    let api_key = std::env::var("PH_AI_API_KEY").ok().filter(|s| !s.is_empty())?;
    let model = std::env::var("PH_AI_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
    let base_url = std::env::var("PH_AI_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.anthropic.com".to_string());
    Some(ph_ai::AiConfig { api_key, model, base_url, max_tokens: 4000, timeout_secs: 45 })
}

/// Build LeadFacts from a stored lead (pull citation/court/id_risk from extracted_json).
fn lead_facts(lead: &ph_cms::ingest::IngestItem, kind: &str, section: &str) -> ph_ai::LeadFacts {
    let v: serde_json::Value =
        serde_json::from_str(&lead.extracted_json).unwrap_or(serde_json::Value::Null);
    let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    ph_ai::LeadFacts {
        title: lead.title.clone(),
        snippet: lead.snippet.clone(),
        offence_category: lead.offence_category.clone(),
        source_key: lead.source_key.clone(),
        source_url: lead.url.clone(),
        citation: get("citation"),
        court: get("court"),
        kind: kind.to_string(),
        section: section.to_string(),
        id_risk: v.get("identification_risk").and_then(|b| b.as_bool()).unwrap_or(false),
    }
}

/// Generate the promote content ONCE. AI when enabled + succeeds, else the banner.
/// The banner paragraph is always prepended; a figure placeholder is appended.
async fn generate_promo_content(
    lead: &ph_cms::ingest::IngestItem,
    kind: &str,
    section: &str,
) -> ph_cms::ingest::PromotedDraft {
    let banner = ph_cms::ingest::banner_draft(lead);
    let Some(cfg) = ai_config() else {
        return banner;
    };
    let facts = lead_facts(lead, kind, section);
    match ph_ai::draft(&facts, &cfg).await {
        Ok(d) => {
            // Prepend the provenance banner; append a figure placeholder slot.
            let banner_para = "DRAFT FROM AN EXTERNAL LEAD — unverified. Write this report \
                from the public court record; clear reporting restrictions and confirm the \
                conviction before publishing. Source for context only — do not copy its wording.";
            let mut paras = vec![banner_para.to_string()];
            paras.extend(d.body_paragraphs);
            if !d.figure_caption.trim().is_empty() {
                paras.push(format!("![{}](  )", d.figure_caption.trim()));
            }
            paras.push(format!("Source ({}): {}", lead.source_key, lead.url));
            let body_json = serde_json::to_string(&paras).unwrap_or_else(|_| "[]".to_string());
            let tags = serde_json::to_string(&d.tags).unwrap_or_else(|_| "[]".to_string());
            ph_cms::ingest::PromotedDraft {
                summary: d.summary,
                body_json,
                meta_description: d.meta_description,
                og_image_url: String::new(),
                tags,
            }
        }
        Err(e) => {
            eprintln!("[ph-press] AI draft failed ({e}); using banner draft");
            banner
        }
    }
}
```

- [ ] **Step 3: Rewire `cms::promote_lead` + `cms::promote_lead_to_conviction`**

Replace the two functions in `src/cms.rs`:

```rust
/// Promote a lead into a Draft article — AI-drafted when enabled, banner otherwise.
pub async fn promote_lead(actor: &str, id: i64, kind: &str, section: &str) -> Result<i64, String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let lead = ph_cms::ingest::get_lead(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no lead {id}"))?;
    let content = generate_promo_content(&lead, kind, section).await;
    ph_cms::ingest::promote_lead_with_draft(pool, id, &user, kind, section, &content)
        .await
        .map_err(|e| e.to_string())
}

/// Promote a lead into a draft article + a linked draft conviction (AI or banner).
pub async fn promote_lead_to_conviction(
    actor: &str,
    id: i64,
    kind: &str,
    section: &str,
) -> Result<(), String> {
    let pool = db().await.map_err(|e| e.to_string())?;
    let user = actor_user(pool, actor).await?;
    let lead = ph_cms::ingest::get_lead(pool, id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no lead {id}"))?;
    let content = generate_promo_content(&lead, kind, section).await;
    ph_cms::ingest::promote_lead_to_conviction_with_draft(pool, id, &user, kind, section, &content)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
```

(`get_lead` must be `pub` in `ph_cms::ingest` — it already is.)

- [ ] **Step 4: Verify**

Run: `cargo test -p ph-cms` → all PASS.
Run: `cargo test -p ph-ai` → all PASS.
Run: `cargo check --no-default-features --features server` → compiles clean (with AI off, this path is unchanged behaviour).
Run: `cargo check` → web build still compiles (ph-ai is server-only; not pulled here).

- [ ] **Step 5: Commit**

```bash
git add src/cms.rs Cargo.toml
git commit -m "feat(desk): AI-drafted promote with banner fallback; ph-ai wired server-only

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Full-build gate, env docs, manual verification

**Files:**
- Modify: `README.md` (document `PH_AI_ENABLED` / `PH_AI_API_KEY` / `PH_AI_MODEL` / `PH_AI_BASE_URL` next to the `PH_CRAWL_*` env docs)
- Verify: `dx build --fullstack --ssg`

- [ ] **Step 1: Document the env vars**

In `README.md`, near the existing crawler/identity env documentation, add a short block:

```markdown
**AI drafting (optional, OFF by default).** When promoting an Intake lead, the
desk can pre-fill an AI-drafted *scaffold* (original prose with `[VERIFY]`
markers, SEO, a figure slot) instead of the bare banner. It stays AI-assisted
and goes through the full legal gate. Enable with:

- `PH_AI_ENABLED=1`
- `PH_AI_API_KEY=sk-ant-…` (Anthropic API key)
- `PH_AI_MODEL` — optional, default `claude-sonnet-4-6` (cost-sensitive); set
  `claude-opus-4-8` for the most capable drafts.

With it unset, promote behaves exactly as before (banner draft). A failed or
disabled AI call never breaks promote — it falls back to the banner.
```

- [ ] **Step 2: Full canonical build**

Run: `dx build --fullstack --ssg`
Expected: build succeeds (the web bundle must NOT contain ph-ai/reqwest — server-only).

- [ ] **Step 3: Manual verification (controller/user)**

1. AI **off**: `dx serve --platform web`, promote an Intake lead → confirm the draft is the banner-only draft (unchanged), opens in the editor.
2. AI **on**: set `PH_AI_ENABLED=1` + `PH_AI_API_KEY=…`, restart, promote a lead → the draft opens with: the banner paragraph first, an AI scaffold body containing `**[VERIFY: …]**` markers, a pre-filled standfirst/meta-description/tags, a `![caption](  )` figure slot, and the source line last. `is_ai_assisted` shows the "AI-assisted" tag in the Articles list.
3. Walk the draft through Submitted → Editorial → Legal → Published; confirm the legal gate still applies and the public page renders the SEO from Phase 1.
4. AI **failure**: set `PH_AI_API_KEY` to an invalid key, promote → confirm it logs the failure and still produces the banner draft (promote did not error).

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: document PH_AI_* env vars for AI-drafted promote

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage (Phase 2):**
- New `ph-ai` crate (types, request builder, parser, networked draft) → Tasks 1-3. ✓
- Guarded-scaffold prompt (only-known-facts, `[VERIFY]`, anonymity for sensitive leads) → Task 1 `system_prompt`. ✓
- Forced tool use for validated JSON; banner prepend + figure slot append → Tasks 1/5. ✓
- Content-driven `ph-cms` primitive routing BOTH promote paths (the advisor's fix) → Task 4. ✓
- `cms.rs` orchestration: `ai_config` (off by default), generate-once, graceful fallback, promote never fails → Task 5. ✓
- `ph-ai` optional under the `server` feature; never in the wasm build → Tasks 1/5/6. ✓
- AI-assisted flag + full legal gate preserved → Task 4 (test asserts `is_ai_assisted` + draft state) + Task 6 manual. ✓
- Confirm Messages API mechanics against the `claude-api` skill → done; baked into Global Constraints + Task 1/3 code.

**Phase 3 (not here):** AI illustration generation (needs an image-provider decision).

**Placeholder scan:** none — every step has complete code/commands.

**Type consistency:** `AiConfig`/`LeadFacts`/`AiDraft`/`AiError` defined in Task 1 and consumed unchanged in Tasks 3/5; `PromotedDraft`/`promote_lead_with_draft`/`promote_lead_to_conviction_with_draft`/`banner_draft`/`get_lead` defined in Task 4 and consumed in Task 5; `create_draft`'s Phase-1 SEO params are supplied by Task 4. `cms::promote_lead` keeps its `(actor, id, kind, section)` signature so `api.rs` is untouched.

**Cross-task build note:** Tasks 1-3 are self-contained (`cargo test -p ph-ai`). Task 4 is `cargo test -p ph-cms`-green on its own. The main crate first compiles against the new `ph-cms`/`ph-ai` symbols at Task 5; that is where `cargo check --features server` goes green.
