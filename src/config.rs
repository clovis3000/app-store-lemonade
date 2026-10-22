use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "lemonade.toml";

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// Directory holding the config file; relative paths resolve against it.
    #[serde(skip)]
    pub root: PathBuf,

    #[serde(default = "default_countries")]
    pub countries: Vec<String>,

    #[serde(default = "default_categories")]
    pub categories: HashMap<String, String>,

    #[serde(default)]
    pub keywords: Vec<String>,

    #[serde(default)]
    pub scoring: ScoringConfig,

    #[serde(default)]
    pub rate_limits: RateLimitConfig,

    #[serde(default)]
    pub paths: PathsConfig,

    #[serde(default)]
    pub llm: LlmConfig,

    #[serde(default)]
    pub research: ResearchConfig,

    #[serde(default)]
    pub openai: OpenaiConfig,
}

#[derive(Debug, Deserialize)]
pub struct ScoringConfig {
    #[serde(default = "default_min_rating_count")]
    pub min_rating_count: i64,

    #[serde(default = "default_max_rating")]
    pub max_rating: f64,

    #[serde(default = "default_skip_developers")]
    pub skip_developers: Vec<String>,

    #[serde(default = "default_skip_categories")]
    pub skip_categories: Vec<String>,

    #[serde(default = "default_min_months_since_update")]
    pub min_months_since_update: f64,
}

#[derive(Debug, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rss_delay_ms")]
    pub rss_delay_ms: u64,

    #[serde(default = "default_itunes_delay_ms")]
    pub itunes_delay_ms: u64,

    #[serde(default = "default_reviews_delay_ms")]
    pub reviews_delay_ms: u64,

    pub llm_delay_ms: Option<u64>,

    pub openai_delay_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_db_path")]
    pub db: String,

    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,

    #[serde(default = "default_prompts_dir")]
    pub prompts_dir: String,

    #[serde(default = "default_profiles_dir")]
    pub profiles_dir: String,
}

/// Where research calls go. The roles and prompts are the same on every provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    /// Anthropic API, billed to ANTHROPIC_API_KEY.
    #[default]
    Anthropic,
    /// Headless Claude Code (`claude -p`), billed to the logged-in Claude subscription.
    ClaudeCode,
    /// OpenAI API, billed to OPENAI_API_KEY.
    Openai,
}

/// Two roles: bulk reading of review batches, and the few judgement-heavy
/// calls (cross-app synthesis, follow-up questions) where a stronger model
/// earns its price.
#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    #[serde(default)]
    pub provider: Provider,

    /// Unset picks the provider's default, so switching provider never sends
    /// one vendor's model ID to another.
    #[serde(default, alias = "model")]
    pub analysis_model: Option<String>,

    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// low | medium | high | xhigh | max; unset keeps the model's default.
    #[serde(default)]
    pub analysis_effort: Option<String>,

    #[serde(default)]
    pub synthesis_model: Option<String>,

    #[serde(default = "default_synthesis_max_tokens")]
    pub synthesis_max_tokens: u32,

    #[serde(default = "default_synthesis_effort")]
    pub synthesis_effort: Option<String>,

    /// Review batches sent to the model at once.
    #[serde(default = "default_llm_concurrency")]
    pub concurrency: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum ModelRole {
    Analysis,
    Synthesis,
}

pub struct RoleSettings {
    pub model: String,
    pub max_tokens: u32,
    pub effort: Option<String>,
}

impl LlmConfig {
    pub fn for_role(&self, role: ModelRole) -> RoleSettings {
        let (analysis, synthesis) = match self.provider {
            Provider::Anthropic | Provider::ClaudeCode => ("claude-haiku-5-5", "claude-opus-5-5"),
            Provider::Openai => ("gpt-6-luna", "gpt-6-luna"),
        };
        match role {
            ModelRole::Analysis => RoleSettings {
                model: self.analysis_model.clone().unwrap_or_else(|| analysis.into()),
                max_tokens: self.max_tokens,
                effort: self.analysis_effort.clone(),
            },
            ModelRole::Synthesis => RoleSettings {
                model: self.synthesis_model.clone().unwrap_or_else(|| synthesis.into()),
                max_tokens: self.synthesis_max_tokens,
                effort: self.synthesis_effort.clone(),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ResearchConfig {
    #[serde(default = "default_review_staleness_days")]
    pub review_staleness_days: u32,
}

#[derive(Debug, Deserialize)]
pub struct OpenaiConfig {
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    #[serde(default = "default_classification_model")]
    pub classification_model: String,

    #[serde(default = "default_openai_max_tokens")]
    pub max_tokens: u32,
}

// Defaults

fn default_countries() -> Vec<String> {
    vec!["us".into(), "au".into(), "gb".into(), "ca".into(), "nz".into()]
}

fn default_categories() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("6002".into(), "Utilities".into());
    m.insert("6007".into(), "Productivity".into());
    m.insert("6013".into(), "Health & Fitness".into());
    m.insert("6015".into(), "Finance".into());
    m.insert("6017".into(), "Education".into());
    m.insert("6001".into(), "Weather".into());
    m.insert("6006".into(), "Reference".into());
    m.insert("6008".into(), "Photo & Video".into());
    m
}

fn default_min_rating_count() -> i64 {
    500
}

fn default_max_rating() -> f64 {
    4.2
}

fn default_skip_developers() -> Vec<String> {
    vec![
        "Apple".into(),
        "Google LLC".into(),
        "Google".into(),
        "Meta Platforms, Inc.".into(),
        "Microsoft Corporation".into(),
        "Amazon".into(),
        "Amazon.com".into(),
    ]
}

fn default_skip_categories() -> Vec<String> {
    vec!["Games".into()]
}

fn default_min_months_since_update() -> f64 {
    0.0
}

fn default_rss_delay_ms() -> u64 {
    1000
}

fn default_itunes_delay_ms() -> u64 {
    3000
}

fn default_reviews_delay_ms() -> u64 {
    3000
}

fn default_db_path() -> String {
    "data/db/lemonade.db".into()
}

fn default_cache_dir() -> String {
    "data/cache".into()
}

fn default_prompts_dir() -> String {
    "prompts".into()
}

fn default_profiles_dir() -> String {
    "profiles".into()
}

fn default_review_staleness_days() -> u32 {
    14
}

// Thinking spends from this budget too; 25-review batches have hit 4096.
fn default_max_tokens() -> u32 {
    16000
}

fn default_synthesis_max_tokens() -> u32 {
    32000
}

fn default_synthesis_effort() -> Option<String> {
    Some("high".into())
}

fn default_llm_concurrency() -> usize {
    4
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".into()
}

fn default_classification_model() -> String {
    "gpt-5-mini".into()
}

fn default_openai_max_tokens() -> u32 {
    16384
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            min_rating_count: default_min_rating_count(),
            max_rating: default_max_rating(),
            skip_developers: default_skip_developers(),
            skip_categories: default_skip_categories(),
            min_months_since_update: default_min_months_since_update(),
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rss_delay_ms: default_rss_delay_ms(),
            itunes_delay_ms: default_itunes_delay_ms(),
            reviews_delay_ms: default_reviews_delay_ms(),
            llm_delay_ms: None,
            openai_delay_ms: None,
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            db: default_db_path(),
            cache_dir: default_cache_dir(),
            prompts_dir: default_prompts_dir(),
            profiles_dir: default_profiles_dir(),
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            analysis_model: None,
            max_tokens: default_max_tokens(),
            analysis_effort: None,
            synthesis_model: None,
            synthesis_max_tokens: default_synthesis_max_tokens(),
            synthesis_effort: default_synthesis_effort(),
            concurrency: default_llm_concurrency(),
        }
    }
}

impl AppConfig {
    /// Loads `explicit`, or the nearest `lemonade.toml` in the current
    /// directory or its ancestors. Never falls back to defaults: a missing
    /// config used to mean a fresh empty database wherever the command ran.
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        let path = match explicit {
            Some(p) => p.to_path_buf(),
            None => find_config()?,
        };
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;
        let mut config: AppConfig = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        let root = std::fs::canonicalize(&path)?
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        for p in [
            &mut config.paths.db,
            &mut config.paths.cache_dir,
            &mut config.paths.prompts_dir,
            &mut config.paths.profiles_dir,
        ] {
            *p = root.join(&*p).to_string_lossy().into_owned();
        }
        config.root = root;
        Ok(config)
    }
}

fn find_config() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    cwd.ancestors()
        .map(|dir| dir.join(CONFIG_FILE))
        .find(|p| p.is_file())
        .with_context(|| {
            format!(
                "No {CONFIG_FILE} found in {} or any parent. Run from the lemonade checkout or pass --config.",
                cwd.display()
            )
        })
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            review_staleness_days: default_review_staleness_days(),
        }
    }
}

impl Default for OpenaiConfig {
    fn default() -> Self {
        Self {
            embedding_model: default_embedding_model(),
            classification_model: default_classification_model(),
            max_tokens: default_openai_max_tokens(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root: PathBuf::new(),
            countries: default_countries(),
            categories: default_categories(),
            keywords: Vec::new(),
            scoring: ScoringConfig::default(),
            rate_limits: RateLimitConfig::default(),
            paths: PathsConfig::default(),
            llm: LlmConfig::default(),
            research: ResearchConfig::default(),
            openai: OpenaiConfig::default(),
        }
    }
}
