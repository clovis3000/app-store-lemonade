use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::cache::ResponseCache;
use crate::config::AppConfig;

pub const MAX_ATTEMPTS: u32 = 5;

pub struct RateLimitedClient {
    client: Client,
    cache: ResponseCache,
    /// Earliest instant the next request to each domain may start.
    next_slot: Mutex<HashMap<String, Instant>>,
    /// delay per domain in ms
    domain_delays: HashMap<String, u64>,
}

impl RateLimitedClient {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let client = Client::builder()
            // Opus at high effort can think for minutes before a long synthesis.
            .timeout(Duration::from_secs(600))
            .user_agent(concat!("lemonade/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to build HTTP client")?;

        let cache = ResponseCache::new(&config.paths.cache_dir)?;

        let mut domain_delays = HashMap::new();
        domain_delays.insert("rss.marketingtools.apple.com".into(), config.rate_limits.rss_delay_ms);
        domain_delays.insert("itunes.apple.com".into(), config.rate_limits.itunes_delay_ms);
        domain_delays.insert("api.anthropic.com".into(), config.rate_limits.llm_delay_ms.unwrap_or(250));
        domain_delays.insert("api.openai.com".into(), config.rate_limits.openai_delay_ms.unwrap_or(50));

        Ok(Self {
            client,
            cache,
            next_slot: Mutex::new(HashMap::new()),
            domain_delays,
        })
    }

    /// Get the underlying reqwest::Client for direct use (bypasses rate limiting).
    /// Use for concurrent operations where a semaphore controls concurrency instead.
    pub fn raw_client(&self) -> Client {
        self.client.clone()
    }

    fn domain_from_url(url: &str) -> String {
        url::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default()
    }

    /// Reserves the domain's next start slot under the lock, then sleeps until
    /// it. Checking and stamping in two steps let concurrent callers read the
    /// same timestamp and all fire together.
    async fn wait_for_rate_limit(&self, url: &str) {
        let domain = Self::domain_from_url(url);
        let delay = Duration::from_millis(self.domain_delays.get(&domain).copied().unwrap_or(1000));

        let start_at = {
            let mut slots = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let start_at = slots.get(&domain).map_or(now, |&next| next.max(now));
            slots.insert(domain.clone(), start_at + delay);
            start_at
        };

        let wait = start_at.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            tracing::debug!("Rate limiting: sleeping {:?} for {}", wait, domain);
            tokio::time::sleep(wait).await;
        }
    }

    /// Sends a request with rate limiting, retrying throttling (429), overload
    /// (529), server errors and dropped connections with backoff. Other
    /// statuses are returned for the caller to judge.
    async fn send(&self, url: &str, build: impl Fn() -> RequestBuilder) -> Result<Response> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            self.wait_for_rate_limit(url).await;
            tracing::debug!("{} (attempt {})", url, attempt);

            let wait = match build().send().await {
                Ok(response) if !is_retryable(response.status()) => return Ok(response),
                Ok(response) => {
                    let status = response.status();
                    if attempt >= MAX_ATTEMPTS {
                        let body = response.text().await.unwrap_or_default();
                        anyhow::bail!("HTTP {} for {} after {} attempts: {}", status, url, attempt, body);
                    }
                    let wait = retry_after(&response).unwrap_or_else(|| backoff(attempt));
                    tracing::warn!("HTTP {} from {}; retry {} in {:?}", status, url, attempt, wait);
                    wait
                }
                Err(e) if attempt < MAX_ATTEMPTS && (e.is_connect() || e.is_timeout()) => {
                    tracing::warn!("{} for {}; retry {}", e, url, attempt);
                    backoff(attempt)
                }
                Err(e) => return Err(e).with_context(|| format!("HTTP request failed: {}", url)),
            };
            tokio::time::sleep(wait).await;
        }
    }

    /// GET with rate limiting and optional caching
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        cache_key: Option<&str>,
        max_age_hours: Option<u64>,
    ) -> Result<T> {
        if let Some(key) = cache_key {
            if let Some(cached) = self.cache.get(key, max_age_hours.unwrap_or(24))? {
                tracing::debug!("Cache hit: {}", key);
                let parsed: T = serde_json::from_str(&cached)
                    .with_context(|| format!("Failed to parse cached response for {}", key))?;
                return Ok(parsed);
            }
        }

        let body = self.get_text(url).await?;

        if let Some(key) = cache_key {
            self.cache.put(key, &body)?;
        }

        serde_json::from_str(&body).with_context(|| format!("Failed to parse JSON from {}", url))
    }

    /// GET raw text with rate limiting (no caching)
    pub async fn get_text(&self, url: &str) -> Result<String> {
        let response = self.send(url, || self.client.get(url)).await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("HTTP {} for {}", status, url);
        }
        response
            .text()
            .await
            .with_context(|| format!("Failed to read response body from {}", url))
    }

    /// POST JSON with rate limiting and retries (for LLM APIs)
    pub async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &serde_json::Value,
        headers: &[(String, String)],
    ) -> Result<T> {
        let response = self
            .send(url, || {
                headers
                    .iter()
                    .fold(self.client.post(url).json(body), |req, (k, v)| req.header(k, v))
            })
            .await?;

        let status = response.status();
        let text = response
            .text()
            .await
            .with_context(|| format!("Failed to read response body from {}", url))?;
        if !status.is_success() {
            anyhow::bail!("HTTP {} for {}: {}", status, url, text);
        }
        serde_json::from_str(&text).with_context(|| format!("Failed to parse JSON from POST {}", url))
    }
}

pub fn is_retryable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.as_u16() == 529 || status.is_server_error()
}

/// The server's `retry-after` (seconds), when it sends one.
pub fn retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// 2s, 4s, 8s, 16s, capped at 30s.
pub fn backoff(attempt: u32) -> Duration {
    Duration::from_secs((1u64 << attempt.min(5)).min(30))
}
