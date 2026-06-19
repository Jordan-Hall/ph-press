# Phase 2 REVISION — multi-backend ph-ai (local OpenAI-compatible + Anthropic)

> **Supersedes** the single-backend assumption in `2026-06-18-intake-ai-draft-phase2.md`. That plan's **Task 1 (Anthropic backend + types)** is built (commit `4eddced`) and becomes the `anthropic` backend. Its **Task 4 (ph-cms `PromotedDraft` primitive)** is unchanged. This revision changes the **request/parse/draft** layer to be backend-pluggable and adds the **local OpenAI-compatible** backend, then updates orchestration (the original Task 5).

**Why:** the user wants a cheap, small **local model on a small/CPU EC2 box**, env-selectable, with Bedrock as a possible cloud option. A local model keeps sensitive court data on-prem and removes per-article cost. Accuracy guardrail caveat stands: small local models fabricate more, so the `[VERIFY]`-marker scaffold discipline matters even more — the editor must catch more.

## Backend model

`PH_AI_BACKEND` selects the wire protocol (default — and whole feature — OFF unless `PH_AI_ENABLED`):

| `PH_AI_BACKEND` | Client | Reaches |
|---|---|---|
| `local` (default when AI enabled) | OpenAI-compatible `POST {base_url}/v1/chat/completions` | llama.cpp `llama-server`, Ollama, vLLM, LM Studio — and **Bedrock** via an OpenAI-compatible gateway (AWS Bedrock Access Gateway / LiteLLM) |
| `anthropic` | Anthropic Messages API (built in Task 1) | api.anthropic.com (Claude; cheapest is Haiku) |

Env (server-only, off by default):
- `PH_AI_ENABLED` — master switch.
- `PH_AI_BACKEND` — `local` (default) | `anthropic`.
- `PH_AI_BASE_URL` — local: e.g. `http://127.0.0.1:8080`; anthropic: `https://api.anthropic.com`.
- `PH_AI_API_KEY` — anthropic: required; local: optional bearer token (many local servers ignore it).
- `PH_AI_MODEL` — local: the served model name (e.g. `llama-3.2-3b-instruct`); anthropic: `claude-sonnet-4-6` (or `claude-haiku-4-5`).
- `PH_AI_TIMEOUT_SECS` — optional; default 120 (CPU local inference is slow).

## Global-constraint deltas vs the original Phase 2 plan

- The "Anthropic API contract" constraint still holds **for the `anthropic` backend only**.
- New **local backend contract**: `POST {base_url}/v1/chat/completions`, header `content-type: application/json` (+ `authorization: Bearer <key>` only if `PH_AI_API_KEY` set). Body: `{ model, max_tokens, temperature: 0, messages: [{role:"system", content:<guardrails>}, {role:"user", content:<facts JSON + "Reply with ONLY a JSON object matching: {summary, meta_description, slug, tags[], body_paragraphs[], figure_caption}">}], response_format: {"type":"json_object"} }`. Response: `choices[0].message.content` is a JSON string → parse leniently into `AiDraft`. Forced tool/function calling is NOT used (small local models are unreliable at it).
- `temperature: 0` for determinism/conservatism on the local model.
- Both backends share `LeadFacts`, `AiDraft`, `system_prompt`, and the fallback behaviour.

---

### Task R1: Backend enum + extend `AiConfig` + OpenAI-compatible backend

**Files:**
- Modify: `crates/ph-ai/src/lib.rs`
- Test: `crates/ph-ai/src/lib.rs` (`tests`)

**Interfaces:**
- Produces:
  ```rust
  pub enum Backend { Local, Anthropic }
  // AiConfig gains: pub backend: Backend
  pub fn build_openai_body(facts: &LeadFacts, cfg: &AiConfig) -> serde_json::Value;     // pure
  pub fn parse_openai_response(resp: &serde_json::Value) -> Result<AiDraft, AiError>;    // pure, lenient
  ```
- Consumes: `LeadFacts`, `AiDraft`, `AiError`, `system_prompt`, `slugify` (Task 1/2).

- [ ] **Step 1: Add `Backend` and extend `AiConfig`**

In `crates/ph-ai/src/lib.rs`, add the enum and field:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    /// OpenAI-compatible /v1/chat/completions (local model, or Bedrock via gateway).
    Local,
    /// Anthropic Messages API.
    Anthropic,
}
```

Add `pub backend: Backend,` to `AiConfig` (update the Task 1 test's `cfg()` helper to set `backend: Backend::Anthropic`).

- [ ] **Step 2: Write the failing tests**

```rust
    #[test]
    fn openai_body_requests_json_object_and_carries_facts() {
        let mut c = cfg();
        c.backend = Backend::Local;
        c.model = "llama-3.2-3b-instruct".into();
        let body = build_openai_body(&facts(), &c);
        assert_eq!(body["model"], "llama-3.2-3b-instruct");
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["temperature"], 0);
        // system carries the guardrails, user carries the facts
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("guarded scaffold"));
        assert!(body["messages"][1]["content"].as_str().unwrap().contains("R v Smith"));
    }

    #[test]
    fn parse_openai_extracts_json_from_message_content() {
        // content is a JSON string (lenient: tolerate surrounding prose / code fences)
        let resp = serde_json::json!({"choices": [{"message": {"content":
            "Here you go:\n```json\n{\"summary\":\"S\",\"meta_description\":\"M\",\"slug\":\"r-v-smith\",\"tags\":[\"a\"],\"body_paragraphs\":[\"P **[VERIFY: age]**\"],\"figure_caption\":\"C\"}\n```"
        }}]});
        let d = parse_openai_response(&resp).unwrap();
        assert_eq!(d.slug, "r-v-smith");
        assert_eq!(d.body_paragraphs.len(), 1);
    }

    #[test]
    fn parse_openai_no_json_is_an_error() {
        let resp = serde_json::json!({"choices": [{"message": {"content": "sorry, I can't."}}]});
        assert!(matches!(parse_openai_response(&resp), Err(AiError::Parse(_))));
    }
```

- [ ] **Step 3: Implement the OpenAI-compatible builder + lenient parser**

```rust
/// Build an OpenAI-compatible chat-completions body (local / gateway). Pure.
pub fn build_openai_body(facts: &LeadFacts, cfg: &AiConfig) -> serde_json::Value {
    let user = serde_json::to_string(facts).unwrap_or_else(|_| "{}".to_string());
    let instruction = format!(
        "Draft a scaffold from these UNVERIFIED lead facts (JSON):\n{user}\n\n\
         Reply with ONLY a JSON object, no prose, matching exactly: \
         {{\"summary\":string, \"meta_description\":string, \"slug\":string, \
         \"tags\":[string], \"body_paragraphs\":[string], \"figure_caption\":string}}"
    );
    serde_json::json!({
        "model": cfg.model,
        "max_tokens": cfg.max_tokens,
        "temperature": 0,
        "response_format": {"type": "json_object"},
        "messages": [
            {"role": "system", "content": system_prompt(facts)},
            {"role": "user", "content": instruction}
        ]
    })
}

/// Parse the assistant message content as JSON into AiDraft. Lenient: strips code
/// fences and extracts the first balanced {...} object if there's surrounding prose.
pub fn parse_openai_response(resp: &serde_json::Value) -> Result<AiDraft, AiError> {
    let content = resp
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .ok_or(AiError::NoToolUse)?;
    let slice = extract_json_object(content).ok_or_else(|| AiError::Parse("no JSON object in reply".into()))?;
    let mut draft: AiDraft =
        serde_json::from_str(slice).map_err(|e| AiError::Parse(e.to_string()))?;
    if draft.body_paragraphs.is_empty() || draft.summary.trim().is_empty() {
        return Err(AiError::Parse("empty draft body or summary".into()));
    }
    draft.slug = slugify(&draft.slug);
    Ok(draft)
}

/// Return the first balanced top-level {...} substring (brace-counting, ignoring
/// braces inside strings). Handles plain JSON, ```json fences, or prose+JSON.
fn extract_json_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = s.find('{')?;
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    for i in start..bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p ph-ai` → all PASS (Task 1/2 tests + the 3 new ones; the Task 1 `cfg()` now sets `backend`).

- [ ] **Step 5: Commit**

```bash
git add crates/ph-ai/src/lib.rs
git commit -m "feat(ai): OpenAI-compatible local backend (json-mode + lenient parse) + Backend enum

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task R2: `draft()` dispatches on backend

**Files:**
- Modify: `crates/ph-ai/src/lib.rs` (the `draft()` from the original Task 3)

- [ ] **Step 1: Replace `draft()` to dispatch on `cfg.backend`**

```rust
/// Call the configured backend and return a typed draft. Networked.
pub async fn draft(facts: &LeadFacts, cfg: &AiConfig) -> Result<AiDraft, AiError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(cfg.timeout_secs))
        .build()
        .map_err(|e| AiError::Http(e.to_string()))?;
    let base = cfg.base_url.trim_end_matches('/');
    match cfg.backend {
        Backend::Anthropic => {
            if cfg.api_key.trim().is_empty() {
                return Err(AiError::Disabled);
            }
            let body = build_request_body(facts, cfg);
            let resp = client
                .post(format!("{base}/v1/messages"))
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
            let json: serde_json::Value =
                resp.json().await.map_err(|e| AiError::Http(e.to_string()))?;
            parse_tool_response(&json)
        }
        Backend::Local => {
            let body = build_openai_body(facts, cfg);
            let mut req = client
                .post(format!("{base}/v1/chat/completions"))
                .header("content-type", "application/json");
            if !cfg.api_key.trim().is_empty() {
                req = req.header("authorization", format!("Bearer {}", cfg.api_key));
            }
            let resp = req
                .json(&body)
                .send()
                .await
                .map_err(|e| AiError::Http(e.to_string()))?;
            let status = resp.status();
            if !status.is_success() {
                return Err(AiError::Status(status.as_u16()));
            }
            let json: serde_json::Value =
                resp.json().await.map_err(|e| AiError::Http(e.to_string()))?;
            parse_openai_response(&json)
        }
    }
}
```

(Update the original Task 3 `#[ignore] live_draft` test's `cfg` to set `backend: Backend::Anthropic`, and optionally add a second ignored `live_local_draft` test pointing at `PH_AI_BASE_URL`.)

- [ ] **Step 2: Verify**

Run: `cargo test -p ph-ai` → all PASS (ignored live tests skipped).

- [ ] **Step 3: Commit**

```bash
git add crates/ph-ai/src/lib.rs
git commit -m "feat(ai): draft() dispatches on backend (local OpenAI-compatible / anthropic)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task R3: orchestration `ai_config()` reads the backend (replaces original Task 5's `ai_config`)

**Files:**
- Modify: `src/cms.rs` (`ai_config()` from the original Task 5)

The rest of the original Task 5 (`lead_facts`, `generate_promo_content`, the rewired `promote_lead` / `promote_lead_to_conviction`) and **Task 4** (`ph-cms` `PromotedDraft` primitive) are **unchanged** — they call `ph_ai::draft(&facts, &cfg)` which now dispatches internally.

- [ ] **Step 1: Replace `ai_config()` in `src/cms.rs`**

```rust
/// Resolve the AI config from env, or None when disabled / unconfigured.
/// OFF by default — no surprise outbound traffic.
fn ai_config() -> Option<ph_ai::AiConfig> {
    if !env_flag("PH_AI_ENABLED") {
        return None;
    }
    let backend = match std::env::var("PH_AI_BACKEND").unwrap_or_default().as_str() {
        "anthropic" => ph_ai::Backend::Anthropic,
        _ => ph_ai::Backend::Local, // default: local OpenAI-compatible
    };
    let api_key = std::env::var("PH_AI_API_KEY").ok().unwrap_or_default();
    // Anthropic requires a key; local does not.
    if backend == ph_ai::Backend::Anthropic && api_key.trim().is_empty() {
        eprintln!("[ph-press] PH_AI_BACKEND=anthropic but PH_AI_API_KEY is empty; AI disabled");
        return None;
    }
    let default_base = match backend {
        ph_ai::Backend::Anthropic => "https://api.anthropic.com",
        ph_ai::Backend::Local => "http://127.0.0.1:8080",
    };
    let base_url = std::env::var("PH_AI_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_base.to_string());
    let default_model = match backend {
        ph_ai::Backend::Anthropic => "claude-sonnet-4-6",
        ph_ai::Backend::Local => "local-model",
    };
    let model = std::env::var("PH_AI_MODEL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_model.to_string());
    let timeout_secs = std::env::var("PH_AI_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    Some(ph_ai::AiConfig { backend, api_key, model, base_url, max_tokens: 4000, timeout_secs })
}
```

- [ ] **Step 2: Verify** (after the original Task 4 + the rest of Task 5 are in place)

`cargo test -p ph-cms`, `cargo test -p ph-ai`, `cargo check --no-default-features --features server`, `cargo check`.

- [ ] **Step 3: Commit** (folded into the original Task 5 commit if done together).

---

### Task R4: README — local-model + EC2 + Bedrock setup (replaces original Task 6 Step 1 env block)

Document both backends, plus a minimal EC2 `llama-server` recipe:

```markdown
**AI drafting (optional, OFF by default).** Promoting an Intake lead can pre-fill
an AI-drafted *scaffold* (original prose with `[VERIFY]` markers, SEO, a figure
slot). It stays AI-assisted and goes through the full legal gate. Two backends:

*Local model (default, cheap, on-prem) — `PH_AI_BACKEND=local`:*
- Run an OpenAI-compatible server on the box, e.g. llama.cpp:
  `./llama-server -m models/Llama-3.2-3B-Instruct-Q4_K_M.gguf -c 4096 --host 127.0.0.1 --port 8080`
  (Ollama also works: `ollama serve`, then `PH_AI_BASE_URL=http://127.0.0.1:11434/v1`.)
- `PH_AI_ENABLED=1`, `PH_AI_BASE_URL=http://127.0.0.1:8080`, `PH_AI_MODEL=<served name>`.
- Small CPU box → expect tens of seconds per draft; `PH_AI_TIMEOUT_SECS` defaults to 120.
- Bedrock: point `PH_AI_BASE_URL` at an OpenAI-compatible gateway (AWS Bedrock
  Access Gateway / LiteLLM) and set `PH_AI_MODEL` to the gateway's model id; set
  `PH_AI_API_KEY` if the gateway requires one.

*Anthropic (highest quality) — `PH_AI_BACKEND=anthropic`:*
- `PH_AI_ENABLED=1`, `PH_AI_API_KEY=sk-ant-…`, `PH_AI_MODEL=claude-sonnet-4-6`
  (or `claude-haiku-4-5` for the cheapest Claude).

With it unset, promote behaves exactly as before (banner draft). A failed or
disabled AI call never breaks promote — it falls back to the banner.
```

---

## Revised task order (Phase 2)

1. Task 1 — Anthropic backend + types ✅ (done, `4eddced`)
2. **Task R1** — Backend enum + `AiConfig.backend` + OpenAI-compatible builder/parser
3. Task 2 (original) — Anthropic `parse_tool_response`
4. **Task R2** — `draft()` dispatch (replaces original Task 3 body; keep the ignored live tests)
5. Task 4 (original) — `ph-cms` `PromotedDraft` primitive (unchanged)
6. Task 5 (original) — orchestration, with **Task R3**'s `ai_config()`
7. Task 6 (original) — full build gate + **Task R4** README

## Self-review

- Backends env-selected (`PH_AI_BACKEND`), local default, both off unless `PH_AI_ENABLED` → R1/R3. ✓
- Local reaches small EC2 model AND Bedrock-via-gateway with one client → R1/R2/R4. ✓
- Guarded scaffold + fallback shared across backends (only the wire layer differs) → R1/R2. ✓
- No native AWS SigV4 in Rust (Bedrock via gateway) → R4 note. ✓
- ph-cms stays storage-only; ph-ai still optional under `server` → unchanged from original plan. ✓
- Accuracy caveat (small local models fabricate more) is documented; `temperature:0` + JSON-mode + lenient parse + `[VERIFY]` discipline mitigate but the editor remains the gate. ✓
