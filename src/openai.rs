use anyhow::{Context, Result};
use reqwest::Client;

use crate::config::AppConfig;
use crate::http::{backoff, is_retryable, retry_after, RateLimitedClient, MAX_ATTEMPTS};

const OPENAI_EMBEDDINGS_URL: &str = "https://api.openai.com/v1/embeddings";
const OPENAI_CHAT_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Debug, Clone, Default)]
pub struct OpenaiUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub calls: u64,
}

impl OpenaiUsage {
    pub fn add(&mut self, other: &OpenaiUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.calls += other.calls;
    }

    /// Calculate cost given per-million-token rates
    pub fn cost_usd(&self, input_rate: f64, output_rate: f64) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0) * input_rate
            + (self.output_tokens as f64 / 1_000_000.0) * output_rate
    }
}

fn get_api_key() -> Option<String> {
    match std::env::var("OPENAI_API_KEY") {
        Ok(key) if !key.is_empty() => Some(key),
        _ => None,
    }
}

fn auth_headers(api_key: &str) -> Vec<(String, String)> {
    // Note: Content-Type is set automatically by reqwest's .json() method
    vec![(
        "Authorization".to_string(),
        format!("Bearer {}", api_key),
    )]
}

/// Embed a batch of texts via OpenAI embeddings API.
/// Returns one embedding vector per input text (in input order) and usage.
pub async fn embed_batch(
    client: &RateLimitedClient,
    config: &AppConfig,
    texts: &[String],
) -> Result<(Vec<Vec<f32>>, OpenaiUsage)> {
    if texts.is_empty() {
        return Ok((Vec::new(), OpenaiUsage::default()));
    }

    let api_key = match get_api_key() {
        Some(k) => k,
        None => {
            anyhow::bail!("OPENAI_API_KEY not set");
        }
    };

    let body = serde_json::json!({
        "input": texts,
        "model": config.openai.embedding_model,
    });

    let headers = auth_headers(&api_key);
    let response: serde_json::Value = client
        .post_json(OPENAI_EMBEDDINGS_URL, &body, &headers)
        .await
        .context("OpenAI embeddings API call failed")?;

    let usage = OpenaiUsage {
        input_tokens: response["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
        output_tokens: 0,
        calls: 1,
    };

    let data = response["data"]
        .as_array()
        .context("No data array in embeddings response")?;

    let mut embeddings: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
    for item in data {
        let index = item["index"].as_u64().unwrap_or(0) as usize;
        let embedding: Vec<f32> = item["embedding"]
            .as_array()
            .context("No embedding array in response item")?
            .iter()
            .map(|v| v.as_f64().unwrap_or(0.0) as f32)
            .collect();
        embeddings.push((index, embedding));
    }

    // OpenAI returns items in order, but sort by index to be safe
    embeddings.sort_by_key(|(idx, _)| *idx);
    let result: Vec<Vec<f32>> = embeddings.into_iter().map(|(_, v)| v).collect();

    Ok((result, usage))
}

/// Call OpenAI chat completions directly with a raw reqwest::Client.
/// Bypasses rate limiting for use with concurrent semaphore-controlled requests.
/// Retries throttling, overload and server errors with backoff.
#[allow(clippy::too_many_arguments)]
pub async fn classify_direct(
    client: &Client,
    model: &str,
    max_tokens: u32,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    schema: &serde_json::Value,
    schema_name: &str,
) -> Result<(String, OpenaiUsage)> {
    let body = serde_json::json!({
        "model": model,
        "max_completion_tokens": max_tokens,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt },
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": schema_name,
                "strict": true,
                "schema": schema,
            }
        }
    });

    let headers = auth_headers(api_key);

    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let req = headers.iter().fold(client.post(OPENAI_CHAT_URL).json(&body), |req, (k, v)| req.header(k, v));
        let response = match req.send().await {
            Ok(r) => r,
            Err(e) if attempt < MAX_ATTEMPTS && (e.is_connect() || e.is_timeout()) => {
                tokio::time::sleep(backoff(attempt)).await;
                continue;
            }
            Err(e) => return Err(e).context("OpenAI classify_direct request failed"),
        };
        let status = response.status();

        if is_retryable(status) && attempt < MAX_ATTEMPTS {
            let wait = retry_after(&response).unwrap_or_else(|| backoff(attempt));
            tracing::warn!("OpenAI HTTP {}, retry {} in {:?}", status, attempt, wait);
            tokio::time::sleep(wait).await;
            continue;
        }

        if !status.is_success() {
            let err_body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI HTTP {} : {}", status, err_body);
        }

        let resp_body = response.text().await.context("Failed to read OpenAI response body")?;
        let response: serde_json::Value = serde_json::from_str(&resp_body)
            .context("Failed to parse OpenAI JSON response")?;

        let usage = OpenaiUsage {
            input_tokens: response["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            output_tokens: response["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            calls: 1,
        };

        if let Some(refusal) = response["choices"][0]["message"]["refusal"].as_str() {
            anyhow::bail!("OpenAI refused request: {}", refusal);
        }

        let content = response["choices"][0]["message"]["content"]
            .as_str()
            .context("No content in chat completion response")?
            .to_string();

        return Ok((content, usage));
    }
}

/// Convert a f32 slice to bytes for sqlite-vec insertion.
/// sqlite-vec expects little-endian f32 vectors as raw bytes.
pub fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}
