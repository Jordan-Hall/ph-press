# Intake → AI-drafted articles with SEO + figure slot

**Date:** 2026-06-18
**Status:** Approved design — ready for implementation planning
**Branch context:** `feat/crawler-enhancements`

## Problem

The crawler files unverified leads into **Intake**. Today, promoting a lead
(`ph_cms::ingest::promote_lead`) creates a near-empty Draft: a "write this from
the court record" banner plus the source link, deliberately containing **no
source prose**. The editor then writes the whole report from scratch.

We want promotion to land in the editor with a **real draft to fine-tune**: an
original (never-copied) article body, pre-filled SEO, and a figure slot — while
keeping the editorial/legal firewall intact.

## Non-negotiables we are preserving

These are deliberate, documented decisions in the existing code. The design keeps
all of them:

1. **No source prose is copied.** The AI writes *original* connective prose from
   the facts; the source's wording is never carried into our article.
2. **Source images are never republished.** We add our own figure slot / OG
   image; the crawled `image_url` stays "reference only".
3. **Legal gate is authoritative.** A promoted draft is a normal Draft and still
   goes Draft → Submitted → Editorial → Legal → Published. Nothing auto-publishes.
4. **AI-assisted is labelled** (IMPRESS Clause 2) — promoted drafts keep
   `is_ai_assisted = 1`.
5. **No surprise outbound traffic.** AI is OFF by default, gated by env, exactly
   like the crawler. With it off, promote behaves precisely as it does today.

## The accuracy guardrail (core constraint)

The AI only receives the **lead title + ~300-char snippet + offence
classification + (for caselaw) citation/court** — **not** the court record. An AI
that writes a confident full article from a headline will fabricate names, dates,
and sentences. That is the exact failure the firewall exists to prevent.

Therefore the AI produces a **guarded scaffold**, not a finished article:

- Uses **only** facts present in the supplied input.
- Writes neutral, house-style original prose to connect those facts.
- Inserts explicit `**[VERIFY: …]**` / `**[FROM RECORD: …]**` placeholders
  wherever a fact must be confirmed before publish (names, ages, dates,
  sentence length, court, plea).
- **Never** invents names, ages, dates, or sentence details.
- For `child` / `sexual` / `id_risk` leads, is additionally instructed to avoid
  any victim-identifying detail (automatic anonymity duties).

The existing "write from the court record" banner remains the first paragraph.

## Architecture

```
crawler → Intake leads → [Promote] → src/cms.rs orchestrates:
                                        1. build LeadFacts from the lead
                                        2. ph-ai: draft(facts) -> AiDraft   (if PH_AI_ENABLED)
                                        3. ph-cms: promote_lead_with_draft(...) writes the Draft
                                        4. navigate to /desk editor (existing)
                                      on AI disabled/error -> fall back to today's banner Draft
```

### New crate: `ph-ai` (server-only)

One job: turn structured lead facts into a typed draft via the Anthropic
Messages API. No DB, no Dioxus, no `ph-cms` dependency. Mirrors `ph-crawl`'s
isolation. Owns its `reqwest` dependency. **Cargo wiring:** added as
`optional = true` and pulled in only by the `server` feature (exactly like
`ph-crawl`), so the wasm/web build never compiles it.

```rust
pub struct AiConfig {
    pub api_key: String,
    pub model: String,        // default "claude-sonnet-4-6"
    pub base_url: String,     // default "https://api.anthropic.com"
    pub max_tokens: u32,
    pub timeout: Duration,
}

pub struct LeadFacts {
    pub title: String,
    pub snippet: String,
    pub offence_category: String, // sexual | child | other | unknown
    pub source_key: String,
    pub source_url: String,
    pub citation: String,         // from extracted_json (caselaw); may be empty
    pub court: String,            // from extracted_json (caselaw); may be empty
    pub kind: String,             // Court report | Investigation | ...
    pub section: String,          // Crime | Courts | ...
    pub id_risk: bool,            // stronger anonymity prompt
}

pub struct AiDraft {
    pub summary: String,           // standfirst
    pub meta_description: String,  // ~155 chars
    pub slug: String,              // suggestion; main crate still de-dupes
    pub tags: Vec<String>,
    pub body_paragraphs: Vec<String>, // banner + figure placeholder + scaffold + [VERIFY] markers
    pub figure_caption: String,
}

pub async fn draft(facts: &LeadFacts, cfg: &AiConfig) -> Result<AiDraft, AiError>;
```

**Structured output:** the request defines a single tool (JSON schema matching
`AiDraft`) and forces tool use, so the model returns validated JSON rather than
free text we have to parse loosely. Headers: `x-api-key`, `anthropic-version`,
`content-type`. The system prompt encodes the guardrails above; the user message
carries `LeadFacts` as JSON. **At Phase 2 implementation, confirm the exact
Messages API mechanics** (the `anthropic-version` value, tool-use request/response
shape, and current model id) **against the `claude-api` skill** before coding —
this spec asserts them from memory.

**Purity for testing:** the HTTP call and the response→`AiDraft` mapping are
separate functions. `parse_tool_response(json) -> Result<AiDraft, AiError>` is a
pure function unit-tested with canned API responses; the networked `draft()` is
exercised manually / behind an ignored integration test.

### `ph-cms` changes (storage only — no network deps)

1. **Migration `0005_article_seo.sql`:**
   ```sql
   ALTER TABLE article ADD COLUMN meta_description TEXT NOT NULL DEFAULT '';
   ALTER TABLE article ADD COLUMN og_image_url     TEXT NOT NULL DEFAULT '';
   ALTER TABLE article ADD COLUMN tags             TEXT NOT NULL DEFAULT '[]'; -- JSON array
   ```
2. **`Article` struct** gains `meta_description`, `og_image_url`, `tags` (stored
   string; DTOs expose `Vec<String>`).
3. **`create_article` / `create_draft`** extended to accept the SEO fields
   (default empty), so a promoted draft is written in one insert. Existing
   callers (seed insertion uses raw SQL and is unaffected; in-crate tests updated).
4. **`update_article`** gains `meta_description`, `og_image_url`, `tags`, **and an
   editable `slug`** (today slug is auto-only). Slug is `slugify`'d, validated,
   and de-duplicated against other rows (excluding self); empty input keeps the
   current slug. **Slug edits are gated to pre-publish states** (Draft /
   Submitted / Editorial / Legal). `update_article` refuses a slug *change* once
   the article is `published`/`corrected`, because changing a live URL 404s
   inbound links and drops search ranking — the opposite of this feature's goal.
   (The other SEO fields remain editable on a live article.)
5. **Shared content-driven primitive — `ingest::promote_lead_with_draft`**: takes
   the **pre-generated** draft content (summary, body JSON, meta, tags, og, slug
   suggestion). It creates the Draft with those fields, marks
   `is_ai_assisted = 1`, flips the lead to `promoted`, and audits. Both promotion
   paths route through this one primitive so AI content reaches **both**:
   - article-only promote → calls `promote_lead_with_draft` directly;
   - **article + conviction** → `promote_lead_to_conviction` is refactored to
     accept the same pre-generated content, create the article via
     `promote_lead_with_draft`, then build the linked draft conviction from the
     lead facts. It must **not** generate or build the article internally as it
     does today (that internal `promote_lead` call is what would bypass the AI).
   Banner-only content (today's text) is just one possible value of the
   pre-generated content the orchestrator passes when AI is off/failed.

### Orchestration (`src/cms.rs`, server-only)

A single private helper generates the draft content **once**, and both promote
entry points thread that content into the matching `ph-cms` primitive. The AI
call lives here (the only crate layer with network access); `ph-cms` only ever
receives finished content.

- `generate_promo_content(pool, lead, kind, section) -> PromoContent`:
  1. Build `LeadFacts` from the lead (pull `citation`/`court`/`id_risk` from
     `extracted_json`).
  2. If `ai_config()` is `Some` (env enabled + key present): call `ph_ai::draft`.
     On `Ok(d)` build content from `d` (prepend the existing banner paragraph;
     append the figure placeholder `![{figure_caption}](  )`). On `Err`, log and
     fall through to banner.
  3. If AI is off or failed: today's banner-only content.
- `promote_lead(actor, id, kind, section)`: load lead → `generate_promo_content`
  → `ph_cms::ingest::promote_lead_with_draft(content)`.
- `promote_lead_to_conviction(actor, id, kind, section)`: load lead →
  `generate_promo_content` → `ph_cms::ingest::promote_lead_to_conviction(content)`
  (the refactored, content-accepting variant from §5).
- **Promote never fails because of AI** — a failed/disabled AI just yields the
  banner draft.
- `ai_config()` reads `PH_AI_ENABLED`, `PH_AI_API_KEY`, `PH_AI_MODEL`
  (default `claude-sonnet-4-6`), optional `PH_AI_BASE_URL`. Returns `None` unless
  enabled + key present.

### DTO + API (`src/api.rs`)

- `PreviewArticle` and `PublicArticle` gain `meta_description`, `og_image_url`,
  `tags: Vec<String>`.
- `desk_create` / `desk_update` signatures gain `meta_description`,
  `og_image_url`, `tags`, and `slug` (update only). `tags` crosses the wire as a
  `Vec<String>`.

### Editor UI (`src/pages/desk.rs` `EditorForm`)

New controls under the existing meta row:
- **Meta description** textarea with a length meter (sweet spot ~120–160).
- **URL slug** input showing the resulting permalink; editable while the article
  is pre-publish (on a brand-new draft it stays auto from the title). Once the
  article is published/corrected the slug field is shown read-only (the server
  also enforces this — see §4), so a live URL can't be changed by accident.
- **Tags** comma-separated input → `Vec<String>`.
- **Social / OG image URL** input (paste a self-hosted/asset path).
  **Image upload is out of scope** (no storage layer added).

`WriteLoad`/`desk_preview` load the new fields; the figure slot is just a
markdown image already inserted into `body` by the AI, edited inline like any
other image.

The promote button already shows a busy label ("Opening…"); with synchronous AI
it simply stays busy a few seconds longer. No new promote UI is required.

### Public rendering (`src/pages/article.rs` `LiveArticleBody`)

- `description` / `og:description` / `twitter:description` use
  `meta_description`, falling back to `summary` when empty.
- **Add `og:image` + `twitter:image`** from `og_image_url` (live CMS articles
  currently emit none). When empty, omit (as today). **The emitted URL must be
  absolute:** if the editor stored a relative `/assets/...` path, prepend `BASE`
  on render (social scrapers reject relative og:image). Easiest to normalise at
  render time, matching the seed renderer's `format!("{BASE}{}", …)`.
- Add `keywords` meta and the `keywords` field of the NewsArticle JSON-LD from
  `tags`.

## Error handling

| Condition | Behaviour |
|-----------|-----------|
| `PH_AI_ENABLED` off / no key | No AI call; banner draft created (today's behaviour). |
| API timeout / network / non-200 | Log; banner draft created; promote still succeeds. |
| Tool-response JSON invalid | `AiError::Parse`; banner draft created. |
| AI returns empty body | Treated as failure → banner fallback. |
| Slug collision on update | Auto-suffix `-2`, `-3`, … (existing `create_draft` pattern). |

Outbound-data note: only the lead's **title + snippet** (already public
court/news material) and classification are sent to Anthropic, and only when an
editor explicitly promotes with AI enabled.

## Testing

- **`ph-ai`:** unit-test the prompt/tool builder and `parse_tool_response` with
  canned Anthropic JSON (valid, missing-field, malformed). Assert guardrail
  text is present in the system prompt for `id_risk`/child/sexual facts.
  Networked `draft()` behind an ignored integration test (needs a key).
- **`ph-cms`:** migration applies cleanly on an existing DB; create/update with
  SEO fields round-trips; editable-slug de-dupes and excludes self;
  `promote_lead_with_draft` sets fields, marks `is_ai_assisted`, flips the lead
  to `promoted`, and the draft still cannot skip the legal gate (extend the
  existing `lead_dedupes_and_promotes_into_legal_gated_draft` test).
- **Public render:** `meta_description` fallback to summary; `og:image` present
  when set, omitted when empty.
- **Manual:** promote with AI **off** → banner draft (unchanged); promote with
  AI **on** → scaffold draft with `[VERIFY]` markers, SEO fields, figure slot;
  edit + walk through the legal gate to publish; confirm the live page meta.

## Phasing (one spec, sequenced)

1. **Phase 1 — SEO + figure slot (no AI):** migration `0005`, `ph-cms` field
   threading + editable slug, `api.rs` DTO/endpoint changes, editor fields,
   public `<meta>`/`og:image`/keywords. Independently valuable and low-risk.
2. **Phase 2 — AI draft on promote:** `ph-ai` crate, `ai_config()`,
   orchestration in `cms.rs`, `promote_lead_with_draft`, guardrails, fallback.
3. **Phase 3 — AI illustration (DEFERRED):** a "generate illustration" button.
   Blocked on an image-provider decision (Anthropic has no image API:
   OpenAI Images / Stability / Replicate / Google). Not built in this spec.

## Out of scope

- Image **upload**/hosting (OG image is a pasted URL/asset path).
- AI image generation (Phase 3, separate decision).
- Auto-drafting at crawl time (we draft on-demand at promote only).
- Changing the crawler's extraction or the legal lifecycle.
