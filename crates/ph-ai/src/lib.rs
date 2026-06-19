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
