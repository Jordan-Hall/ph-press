//! Anthropic Messages API client for guarded AI drafting (server-only). No DB,
//! no Dioxus. Turns structured lead facts into a typed draft via forced tool use.
//! Every value is UNVERIFIED machine output; the draft is a scaffold the editor
//! rewrites from the court record before it can be published.

use serde::{Deserialize, Serialize};

/// Which wire protocol to use when calling the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    /// OpenAI-compatible /v1/chat/completions (local model, or Bedrock via gateway).
    Local,
    /// Anthropic Messages API.
    Anthropic,
}

/// Runtime config (resolved from env by the caller).
#[derive(Debug, Clone)]
pub struct AiConfig {
    pub backend: Backend,
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
            let base = base.strip_suffix("/v1").unwrap_or(base);
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
        AiConfig { backend: Backend::Anthropic, api_key: "k".into(), model: "claude-sonnet-4-6".into(), base_url: "https://api.anthropic.com".into(), max_tokens: 4000, timeout_secs: 30 }
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

    /// Live integration test — Anthropic backend. Requires a real API key.
    /// Run with: PH_AI_API_KEY=sk-ant-… cargo test -p ph-ai -- --ignored live_draft
    #[tokio::test]
    #[ignore]
    async fn live_draft() {
        let key = std::env::var("PH_AI_API_KEY").unwrap_or_default();
        if key.trim().is_empty() {
            eprintln!("live_draft: PH_AI_API_KEY not set — skipped");
            return;
        }
        let mut c = cfg();
        c.backend = Backend::Anthropic;
        c.api_key = key;
        let d = draft(&facts(), &c).await.expect("draft should succeed");
        assert!(!d.summary.is_empty());
        assert!(!d.body_paragraphs.is_empty());
    }

    /// Live integration test — Local OpenAI-compatible backend.
    /// Run with: PH_AI_BASE_URL=http://127.0.0.1:8080 PH_AI_MODEL=my-model cargo test -p ph-ai -- --ignored live_local_draft
    #[tokio::test]
    #[ignore]
    async fn live_local_draft() {
        let base = std::env::var("PH_AI_BASE_URL").unwrap_or_default();
        if base.trim().is_empty() {
            eprintln!("live_local_draft: PH_AI_BASE_URL not set — skipped");
            return;
        }
        let model = std::env::var("PH_AI_MODEL").unwrap_or_else(|_| "local-model".into());
        let c = AiConfig {
            backend: Backend::Local,
            api_key: std::env::var("PH_AI_API_KEY").unwrap_or_default(),
            model,
            base_url: base,
            max_tokens: 4000,
            timeout_secs: 120,
        };
        let d = draft(&facts(), &c).await.expect("draft should succeed");
        assert!(!d.summary.is_empty());
        assert!(!d.body_paragraphs.is_empty());
    }
}
