use anyhow::{bail, Context, Result};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use super::{claude_code, templates};
use crate::config::{AppConfig, ModelRole, Provider, RoleSettings};
use crate::http::RateLimitedClient;
use crate::models::{App, CompetitiveResearchResult, CrossAppSynthesisResult, DiagnosisResult, FollowupResult, Review};
use crate::openai;
use crate::pricing;
use crate::profile::Profile;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// System prompt for providers that take one. The Anthropic path sends the
/// rendered template alone, as it always has.
pub const SYSTEM_PROMPT: &str =
    "You are an expert analyst of app markets and App Store reviews. Answer only with the requested JSON.";

#[derive(Debug, Clone, Default)]
pub struct LlmUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
    /// What was billed, priced per call at the model that served it.
    pub cost_usd: f64,
    /// For subscription calls: what the same tokens would cost at API list
    /// price. Billed nothing, but it shows how much of the plan a run used.
    pub notional_usd: f64,
}

impl LlmUsage {
    pub fn add(&mut self, other: &LlmUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.calls += other.calls;
        self.cost_usd += other.cost_usd;
        self.notional_usd += other.notional_usd;
    }
}

/// Fails early, before any slow scraping, when the configured provider
/// can't be called, or would bill something other than what it promises.
pub async fn preflight(config: &AppConfig) -> Result<()> {
    match config.llm.provider {
        Provider::Anthropic => anthropic_key().map(|_| ()),
        Provider::Openai => openai_key().map(|_| ()),
        Provider::ClaudeCode => claude_code::preflight().await,
    }
}

fn anthropic_key() -> Result<String> {
    env_key("ANTHROPIC_API_KEY")
}

fn openai_key() -> Result<String> {
    env_key("OPENAI_API_KEY")
}

fn env_key(name: &str) -> Result<String> {
    std::env::var(name).ok().filter(|k| !k.is_empty()).with_context(|| format!("{name} is not set"))
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

/// One structured-output call on the configured provider. `Ok(None)` means
/// this call failed (logged); callers decide whether a partial result is
/// still worth keeping.
async fn call_llm<T: DeserializeOwned + JsonSchema>(
    client: &RateLimitedClient,
    config: &AppConfig,
    role: ModelRole,
    prompt: &str,
) -> Result<(Option<T>, LlmUsage)> {
    let settings = config.llm.for_role(role);
    let schema = output_schema::<T>();
    let (outcome, usage) = match config.llm.provider {
        Provider::Anthropic => call_anthropic(client, &settings, prompt, &schema).await?,
        Provider::ClaudeCode => claude_code::call(&settings, prompt, &schema).await,
        Provider::Openai => call_openai(client, &settings, prompt, &schema).await?,
    };

    let parsed = outcome.and_then(|value| {
        serde_json::from_value::<T>(value.clone()).with_context(|| {
            format!("Response did not match the schema: {}", crate::text::truncate(&value.to_string(), 200))
        })
    });
    match parsed {
        Ok(t) => Ok((Some(t), usage)),
        Err(e) => {
            tracing::error!("  {:?} call to {} failed: {:#}", config.llm.provider, settings.model, e);
            Ok((None, usage))
        }
    }
}

async fn call_anthropic(
    client: &RateLimitedClient,
    settings: &RoleSettings,
    prompt: &str,
    schema: &serde_json::Value,
) -> Result<(Result<serde_json::Value>, LlmUsage)> {
    let mut body = serde_json::json!({
        "model": settings.model,
        "max_tokens": settings.max_tokens,
        "messages": [{ "role": "user", "content": prompt }],
        "output_config": { "format": { "type": "json_schema", "schema": schema } }
    });
    if let Some(effort) = &settings.effort {
        body["output_config"]["effort"] = serde_json::json!(effort);
    }

    let mut headers = vec![
        ("x-api-key".to_string(), anthropic_key()?),
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
        Err(e) => return Ok((Err(e), LlmUsage::default())),
    };
    let usage = anthropic_usage(&response, &settings.model);
    Ok((parse_anthropic(&response), usage))
}

fn parse_anthropic(response: &serde_json::Value) -> Result<serde_json::Value> {
    match response["stop_reason"].as_str() {
        // Retrying the identical request would truncate identically.
        Some("max_tokens") => bail!("output hit max_tokens; lower --batch-size or raise max_tokens in lemonade.toml"),
        Some("refusal") => bail!(
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
    serde_json::from_str(text).with_context(|| format!("Response was not JSON: {}", crate::text::truncate(text, 200)))
}

fn supports_default_fallback(model: &str) -> bool {
    ["claude-fable-5-1", "claude-opus-5", "claude-sonnet-5-5"].iter().any(|prefix| model.starts_with(prefix))
}

fn anthropic_usage(response: &serde_json::Value, model: &str) -> LlmUsage {
    let input = response["usage"]["input_tokens"].as_u64().unwrap_or(0);
    let output = response["usage"]["output_tokens"].as_u64().unwrap_or(0);
    // A fallback may have served the call; price it at whichever model answered.
    let served_by = response["model"].as_str().unwrap_or(model);
    LlmUsage { input_tokens: input, output_tokens: output, calls: 1, cost_usd: priced(served_by, input, output), notional_usd: 0.0 }
}

async fn call_openai(
    client: &RateLimitedClient,
    settings: &RoleSettings,
    prompt: &str,
    schema: &serde_json::Value,
) -> Result<(Result<serde_json::Value>, LlmUsage)> {
    let api_key = openai_key()?;
    let result = openai::chat_structured(
        &client.raw_client(),
        &settings.model,
        settings.max_tokens,
        &api_key,
        SYSTEM_PROMPT,
        prompt,
        schema,
        "lemonade_result",
        settings.effort.as_deref(),
    )
    .await;
    Ok(match result {
        Ok((text, u)) => {
            let usage = LlmUsage {
                input_tokens: u.input_tokens,
                output_tokens: u.output_tokens,
                calls: 1,
                cost_usd: priced(&settings.model, u.input_tokens, u.output_tokens),
                notional_usd: 0.0,
            };
            let value = serde_json::from_str(&text)
                .with_context(|| format!("Response was not JSON: {}", crate::text::truncate(&text, 200)));
            (value, usage)
        }
        Err(e) => (Err(e), LlmUsage::default()),
    })
}

pub fn priced(model: &str, input: u64, output: u64) -> f64 {
    pricing::cost(model, input, output).unwrap_or_else(|| {
        tracing::warn!("No price known for {}; cost not counted", model);
        0.0
    })
}

/// JSON schema for `T` in the subset structured outputs accept (Anthropic,
/// OpenAI strict mode and Claude Code alike): every object closed and fully
/// required, no numeric formats or bounds.
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
    fn truncated_and_refused_responses_are_errors() {
        let truncated = serde_json::json!({"stop_reason": "max_tokens", "content": []});
        assert!(parse_anthropic(&truncated).is_err());
        let refused = serde_json::json!({"stop_reason": "refusal", "stop_details": {"category": "cyber"}});
        assert!(parse_anthropic(&refused).is_err());
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
        let parsed: FollowupResult = serde_json::from_value(parse_anthropic(&response).unwrap()).unwrap();
        assert_eq!(parsed.answer, "a");
    }
}
