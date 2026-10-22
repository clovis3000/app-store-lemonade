use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lemonade", version)]
#[command(about = "Find App Store apps with proven demand and unhappy users, and learn what to build instead")]
#[command(long_about = "Find App Store apps with proven demand and unhappy users, and learn what to build instead.\n\n\
Built to be driven by a coding agent (Claude Code or similar): commands print what they did, \
results land in SQLite, and the agent reads them with sqlite3. Typical loop:\n  \
lemonade harvest -> score -> research --plan -> research -> profile new -> research --profile -> index --profile -> query")]
pub struct Cli {
    /// Path to config file (default: nearest lemonade.toml in this directory or above)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Discover apps from App Store top charts and keyword search
    Harvest {
        /// Only fetch RSS top charts
        #[arg(long)]
        rss_only: bool,

        /// Only enrich existing apps via iTunes Lookup
        #[arg(long)]
        enrich_only: bool,

        /// Only run keyword search
        #[arg(long)]
        search_only: bool,

        /// Comma-separated country codes (e.g. us,au,gb)
        #[arg(long, value_delimiter = ',', default_values_t = vec!["us".to_string()])]
        countries: Vec<String>,
    },

    /// Score harvested apps by demand x dissatisfaction x neglect
    Score {
        /// Max number of apps to score
        #[arg(long)]
        limit: Option<usize>,

        /// Recalculate scores for all apps
        #[arg(long)]
        recalculate: bool,
    },

    /// Write a build brief for top-scored apps (reviews + LLM + competitor scan)
    Diagnose {
        /// Diagnose a specific app by ID
        #[arg(long)]
        app_id: Option<String>,

        /// Number of top apps to diagnose
        #[arg(long, default_value_t = 10)]
        limit: usize,

        /// Only scrape reviews, skip LLM analysis
        #[arg(long)]
        reviews_only: bool,

        /// Research profile whose prompts to use
        #[arg(long, default_value = "general")]
        profile: String,

        /// Max reviews to send to the LLM (most recent first)
        #[arg(long, default_value_t = 300)]
        max_reviews: usize,
    },

    /// List apps by stage
    List {
        /// Filter: all, shortlist, scored, diagnosed, actionable
        #[arg(default_value = "all")]
        filter: String,

        /// Max number of results
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },

    /// Start the web GUI server
    Serve {
        /// Port to listen on
        #[arg(long, default_value_t = 3001)]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },

    /// What users love, hate, wish for and pay for, per app and across a market
    Research(ResearchArgs),

    /// Create and inspect research profiles (tailored prompts and taxonomies)
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// Label every review with a profile's taxonomy, and embed reviews for `query`
    Index {
        /// Profile whose taxonomy to classify with; its [apps] pick the reviews
        #[arg(long, default_value = "general")]
        profile: String,

        /// Classify this app's reviews instead of the profile's apps
        #[arg(long)]
        app_id: Option<String>,

        /// Classify reviews of apps matching these terms instead (comma-separated)
        #[arg(long, value_delimiter = ',')]
        search: Option<Vec<String>>,

        /// Max apps when selecting by search terms
        #[arg(long)]
        top: Option<usize>,

        /// Try the taxonomy on ~200 stratified reviews and print label distributions,
        /// stored under "<profile>:calibration" so real labels are untouched
        #[arg(long)]
        calibrate: bool,

        /// Reviews per classification batch
        #[arg(long, default_value_t = 10)]
        batch_size: usize,

        /// Max concurrent API requests for classification
        #[arg(long, default_value_t = 100)]
        concurrency: usize,
    },

    /// Semantic search across embedded reviews
    Query {
        /// Natural language query text (embedded via OpenAI, nearest reviews by cosine distance)
        query: String,

        /// Max results to return
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Only search this app's reviews
        #[arg(long)]
        app_id: Option<String>,

        /// Show this profile's labels with each result
        #[arg(long, default_value = "general")]
        profile: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show detailed opportunity card for an app
    Card {
        /// App ID to display
        app_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Export to file path
        #[arg(long)]
        export: Option<PathBuf>,
    },
}

#[derive(Args)]
pub struct ResearchArgs {
    /// Research profile; its prompts frame the analysis and its [apps] are the default targets
    /// (default: general, or the parent's profile for --followup)
    #[arg(long)]
    pub profile: Option<String>,

    /// App ID to research (single app mode)
    #[arg(long)]
    pub app_id: Option<String>,

    /// Search terms to find apps in the DB (comma-separated, e.g. "habit tracker,streaks")
    #[arg(long, value_delimiter = ',')]
    pub search: Option<Vec<String>>,

    /// Max apps to research when selecting by search terms (default: profile's top, else 8)
    #[arg(long)]
    pub top: Option<usize>,

    /// Max reviews to analyse per app (most recent first)
    #[arg(long, default_value_t = 200)]
    pub max_reviews: usize,

    /// Reviews per LLM batch; 25 keeps the output schema well under max_tokens
    #[arg(long, default_value_t = 25)]
    pub batch_size: usize,

    /// Follow-up question on a previous analysis
    #[arg(long)]
    pub followup: Option<String>,

    /// Parent analysis ID (required with --followup)
    #[arg(long)]
    pub parent: Option<i64>,

    /// List previous analyses for an app (with --app-id) or all syntheses
    #[arg(long)]
    pub list: bool,

    /// Show cost/time estimate without running
    #[arg(long)]
    pub plan: bool,

    /// Name for the cross-app synthesis report
    #[arg(long)]
    pub name: Option<String>,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List profiles
    List,

    /// Show a profile's apps and which prompt files it overrides
    Show {
        name: String,
    },

    /// Create profiles/<name>/ with copies of the prompts worth tailoring
    New {
        /// Lowercase name, e.g. habit-trackers
        name: String,

        /// Profile to start from
        #[arg(long, default_value = "general")]
        from: String,

        /// One line on what this profile studies
        #[arg(long, default_value = "")]
        description: String,

        /// Search terms selecting this profile's apps (comma-separated)
        #[arg(long, value_delimiter = ',')]
        search: Vec<String>,

        /// Exact App Store IDs to include (comma-separated)
        #[arg(long, value_delimiter = ',')]
        app_ids: Vec<String>,

        /// How many search matches to take
        #[arg(long)]
        top: Option<usize>,
    },
}
