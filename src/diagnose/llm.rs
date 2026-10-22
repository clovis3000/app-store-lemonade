use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use super::templates;
use crate::config::{AppConfig, ModelRole};
use crate::http::RateLimitedClient;
use crate::models::{App, CompetitiveResearchResult, CrossAppSynthesisResult, DiagnosisResult, FollowupResult, Review};
use crate::profile::Profile;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

#[derive(Debug, Clone, Default)]
pub struct LlmUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
    /// Priced per call at that call's model, since roles use different models.
    pub cost_usd: f64,
}

impl LlmUsage {
    pub fn add(&mut self, other: &LlmUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.calls += other.calls;
        self.cost_usd += other.cost_usd;
    }
}

/// (input, output) USD per million tokens, first-party API rates.
/// Dated IDs like `claude-haiku-4-5-20251001` match by prefix.
pub fn price_per_mtok(model: &str) -> Option<(f64, f64)> {
    const PRICES: &[(&str, f64, f64)] = &[
        ("claude-fable-5-1", 10.0, 50.0),
        ("claude-fable-5", 10.0, 50.0),
        ("claude-opus-5-5", 4.0, 20.0),
        ("claude-opus-5", 5.0, 25.0),
        ("claude-opus-4", 5.0, 25.0),
        ("claude-sonnet-5", 2.0, 10.0),
        ("claude-sonnet-4-6", 3.0, 15.0),
        ("claude-haiku-5-5", 0.10, 0.50),
        ("claude-haiku-4-5", 1.0, 5.0),
    ];
    PRICES
        .iter()
        .find(|(prefix, _, _)| model.starts_with(prefix))
        .map(|&(_, input, output)| (input, output))
}

/// Fails early, before any slow scraping, when there is no key to analyse with.
pub fn require_api_key() -> Result<String> {
    std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .context("ANTHROPIC_API_KEY is not set")
}

pub async fn analyse_reviews(
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app: &App,
    reviews: &[Review],
) -> Result<(Option<DiagnosisResult>, LlmUsage)> {
    let prompt = templates::render_review_prompt(profile, app, reviews)?;
    call_llm(client, config, ModelRole::Analysis, &prompt).await
}

pub async fn competitive_research(
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app: &App,
    reviews: &[Review],
) -> Result<(Option<CompetitiveResearchResult>, LlmUsage)> {
    let prompt = templates::render_competitive_prompt(profile, app, reviews)?;
    call_llm(client, config, ModelRole::Analysis, &prompt).await
}

pub async fn cross_app_synthesis(
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app_summaries: &[serde_json::Value],
) -> Result<(Option<CrossAppSynthesisResult>, LlmUsage)> {
    let prompt = templates::render_cross_app_synthesis_prompt(profile, app_summaries)?;
    call_llm(client, config, ModelRole::Synthesis, &prompt).await
}

pub async fn synthesize_batches(
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app: &App,
    raw_merged_json: &str,
    batch_count: usize,
) -> Result<(Option<CompetitiveResearchResult>, LlmUsage)> {
    let prompt = templates::render_synthesis_prompt(profile, app, raw_merged_json, batch_count)?;
    call_llm(client, config, ModelRole::Analysis, &prompt).await
}

pub async fn followup_analysis(
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app: &App,
    reviews: &[Review],
    parent_analysis_json: &str,
    question: &str,
) -> Result<(Option<FollowupResult>, LlmUsage)> {
    let prompt = templates::render_followup_prompt(profile, app, reviews, parent_analysis_json, question)?;
    call_llm(client, config, ModelRole::Synthesis, &prompt).await
}

/// One structured-output call. `Ok(None)` means this call failed (logged);
/// callers decide whether a partial result is still worth keeping.
async fn call_llm<T: DeserializeOwned + JsonSchema>(
    client: &RateLimitedClient,
    config: &AppConfig,
    role: ModelRole,
    prompt: &str,
) -> Result<(Option<T>, LlmUsage)> {
    let api_key = require_api_key()?;
    let settings = config.llm.for_role(role);

    let mut body = serde_json::json!({
        "model": settings.model,
        "max_tokens": settings.max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
        "output_config": {
            "format": { "type": "json_schema", "schema": output_schema::<T>() }
        }
    });
    if let Some(effort) = &settings.effort {
        body["output_config"]["effort"] = serde_json::json!(effort);
    }

    let mut headers = vec![
        ("x-api-key".to_string(), api_key),
        ("anthropic-version".to_string(), "2023-06-01".to_string()),
    ];
    // Models with server-side safety classifiers can decline with
    // stop_reason "refusal"; let the API re-route to a fallback model.
    if supports_default_fallback(&settings.model) {
        body["fallbacks"] = serde_json::json!("default");
        headers.push(("anthropic-beta".to_string(), "server-side-fallback-2026-07-01".to_string()));
    }

    let response: serde_json::Value = match client.post_json(ANTHROPIC_API_URL, &body, &headers).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("  LLM call to {} failed: {:#}", settings.model, e);
            return Ok((None, LlmUsage::default()));
        }
    };

    let usage = extract_usage(&response, &settings.model);
    match parse_response::<T>(&response) {
        Ok(parsed) => Ok((Some(parsed), usage)),
        Err(e) => {
            tracing::error!("  LLM response from {} unusable: {:#}", settings.model, e);
            Ok((None, usage))
        }
    }
}

fn parse_response<T: DeserializeOwned>(response: &serde_json::Value) -> Result<T> {
    match response["stop_reason"].as_str() {
        // Retrying the identical request would truncate identically.
        Some("max_tokens") => anyhow::bail!(
            "output hit max_tokens; lower --batch-size or raise max_tokens in lemonade.toml"
        ),
        Some("refusal") => anyhow::bail!(
            "model declined ({})",
            response["stop_details"]["category"].as_str().unwrap_or("no category")
        ),
        _ => {}
    }

    // Thinking models put thinking blocks first; the answer is the text block.
    let text = response["content"]
        .as_array()
        .and_then(|blocks| blocks.iter().find(|b| b["type"] == "text"))
        .and_then(|b| b["text"].as_str())
        .context("No text block in LLM response")?;

    serde_json::from_str(text).with_context(|| {
        format!("Response did not match the schema: {}", crate::text::truncate(text, 200))
    })
}

fn supports_default_fallback(model: &str) -> bool {
    ["claude-fable-5-1", "claude-opus-5", "claude-sonnet-5-5"]
        .iter()
        .any(|prefix| model.starts_with(prefix))
}

fn extract_usage(response: &serde_json::Value, model: &str) -> LlmUsage {
    let input = response["usage"]["input_tokens"].as_u64().unwrap_or(0);
    let output = response["usage"]["output_tokens"].as_u64().unwrap_or(0);
    // A fallback may have served the call; price it at whichever model answered.
    let served_by = response["model"].as_str().unwrap_or(model);
    let cost_usd = match price_per_mtok(served_by) {
        Some((i, o)) => (input as f64 * i + output as f64 * o) / 1_000_000.0,
        None => {
            tracing::warn!("No price known for {}; cost not counted", served_by);
            0.0
        }
    };
    LlmUsage { input_tokens: input, output_tokens: output, calls: 1, cost_usd }
}

/// JSON schema for `T` in the subset structured outputs accepts: every object
/// closed and fully required, no numeric formats or bounds.
pub fn output_schema<T: JsonSchema>() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).expect("schema serializes");
    if let Some(obj) = schema.as_object_mut() {
        obj.remove("$schema");
        obj.remove("title");
    }
    close_objects(&mut schema);
    schema
}

fn close_objects(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["format", "minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "default"] {
                // Only strip schema keywords, never a property that happens to share the name.
                if map.get(key).is_some_and(|v| !v.is_object()) {
                    map.remove(key);
                }
            }
            if let Some(props) = map.get("properties").and_then(|p| p.as_object()) {
                let required: Vec<serde_json::Value> = props.keys().cloned().map(Into::into).collect();
                map.insert("required".into(), required.into());
                map.insert("additionalProperties".into(), false.into());
            }
            map.values_mut().for_each(close_objects);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(close_objects),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(v: &serde_json::Value, check: &mut impl FnMut(&serde_json::Map<String, serde_json::Value>)) {
        match v {
            serde_json::Value::Object(m) => {
                check(m);
                m.values().for_each(|c| walk(c, check));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|c| walk(c, check)),
            _ => {}
        }
    }

    #[test]
    fn schemas_are_closed_and_fully_required() {
        for schema in [
            output_schema::<CompetitiveResearchResult>(),
            output_schema::<DiagnosisResult>(),
            output_schema::<CrossAppSynthesisResult>(),
            output_schema::<FollowupResult>(),
        ] {
            walk(&schema, &mut |m| {
                if let Some(props) = m.get("properties").and_then(|p| p.as_object()) {
                    assert_eq!(m["additionalProperties"], false);
                    assert_eq!(m["required"].as_array().unwrap().len(), props.len());
                }
                assert!(!m.contains_key("format") || m["format"].is_object());
            });
        }
    }

    #[test]
    fn diagnosis_schema_leaves_competitive_scan_to_the_code() {
        let schema = output_schema::<DiagnosisResult>();
        assert!(schema["properties"].get("competitive_scan").is_none());
    }

    /// Writes each output schema to $SCHEMA_DUMP_DIR for checking against the live API.
    #[test]
    #[ignore]
    fn dump_schemas() {
        let dir = std::env::var("SCHEMA_DUMP_DIR").expect("set SCHEMA_DUMP_DIR");
        for (name, schema) in [
            ("competitive", output_schema::<CompetitiveResearchResult>()),
            ("diagnosis", output_schema::<DiagnosisResult>()),
            ("cross_app", output_schema::<CrossAppSynthesisResult>()),
            ("followup", output_schema::<FollowupResult>()),
        ] {
            std::fs::write(format!("{dir}/{name}.json"), schema.to_string()).unwrap();
        }
    }

    #[test]
    fn prices_match_dated_ids_and_reject_unknown() {
        assert_eq!(price_per_mtok("claude-haiku-4-5-20251001"), Some((1.0, 5.0)));
        assert_eq!(price_per_mtok("claude-opus-5-5"), Some((4.0, 20.0)));
        assert_eq!(price_per_mtok("gpt-5-mini"), None);
    }

    #[test]
    fn truncated_and_refused_responses_are_errors() {
        let truncated = serde_json::json!({"stop_reason": "max_tokens", "content": []});
        assert!(parse_response::<FollowupResult>(&truncated).is_err());
        let refused = serde_json::json!({"stop_reason": "refusal", "stop_details": {"category": "cyber"}});
        assert!(parse_response::<FollowupResult>(&refused).is_err());
    }

    #[test]
    fn reads_the_text_block_after_thinking() {
        let response = serde_json::json!({
            "stop_reason": "end_turn",
            "content": [
                {"type": "thinking", "thinking": ""},
                {"type": "text", "text": r#"{"question":"q","answer":"a","evidence":[],"further_questions":[]}"#}
            ]
        });
        let parsed: FollowupResult = parse_response(&response).unwrap();
        assert_eq!(parsed.answer, "a");
    }
}
