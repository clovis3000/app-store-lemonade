#![allow(dead_code)]

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod api;
mod cli;
mod config;
mod db;
mod diagnose;
mod harvest;
mod http;
mod index;
mod models;
mod openai;
mod output;
mod profile;
mod research;
mod score;
mod text;

use cli::{Cli, Commands, ProfileAction};
use profile::Profile;
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                if cli.verbose {
                    EnvFilter::new("lemonade=debug")
                } else {
                    EnvFilter::new("lemonade=info")
                }
            }),
        )
        .init();

    let config = AppConfig::load(cli.config.as_deref())?;
    let database = db::Database::open(&config.paths.db)?;
    let client = http::RateLimitedClient::new(&config)?;

    match cli.command {
        Commands::Harvest { rss_only, enrich_only, search_only, countries } => {
            harvest::run_harvest(&database, &client, &config, &countries, rss_only, enrich_only, search_only).await?;
        }
        Commands::Score { limit, recalculate } => {
            score::run_scoring(&database, &config, limit, recalculate)?;
        }
        Commands::Diagnose { app_id, limit, reviews_only, profile, max_reviews } => {
            let profile = Profile::load(&config, &profile)?;
            diagnose::run_diagnosis(&database, &client, &config, app_id.as_deref(), limit, reviews_only, &profile, max_reviews)
                .await?;
        }
        Commands::Research(args) => {
            research::dispatch(&database, &client, &config, args).await?;
        }
        Commands::Profile { action } => {
            profile_cmd(&database, &config, action)?;
        }
        Commands::Index { profile, app_id, search, top, calibrate, batch_size, concurrency } => {
            let profile = Profile::load(&config, &profile)?;
            let apps = research::resolve_apps(&database, &profile, app_id.as_deref(), search.as_deref(), top)?;
            let options = index::IndexOptions { calibrate, batch_size, concurrency };
            index::run(&database, &client, &config, &profile, &apps, options).await?;
        }
        Commands::Query { query, limit, app_id, profile, json } => {
            index::query_semantic(&database, &client, &config, &query, limit, app_id.as_deref(), &profile, json).await?;
        }
        Commands::Serve { port, host } => {
            api::serve(database, &config.root, host, port).await?;
        }
        Commands::List { filter, limit } => {
            let apps = db::queries::list_apps(&database, &filter, limit)?;
            output::table::print_app_table(&apps, &filter);
        }
        Commands::Card { app_id, json, export } => {
            output::card::show_card(&database, &app_id, json, export.as_deref())?;
        }
    }

    Ok(())
}

fn profile_cmd(db: &db::Database, config: &AppConfig, action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            for (name, description) in profile::list(config)? {
                println!("{:<24} {}", name, description);
            }
        }
        ProfileAction::Show { name } => {
            let p = Profile::load(config, &name)?;
            println!("profile:     {}", p.name);
            if let Some(dir) = &p.dir {
                println!("folder:      {}", dir.display());
            }
            if !p.manifest.description.is_empty() {
                println!("description: {}", p.manifest.description);
            }
            let apps = &p.manifest.apps;
            println!("apps:        search={:?} ids={:?} top={:?}", apps.search, apps.ids, apps.top);
            for file in profile::TAILORABLE {
                let source = if p.overrides().contains(file) { "tailored" } else { "general" };
                println!("  {:<28} {:<9} {}", file, source, p.path_of(file).display());
            }
            let labelled: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM review_labels WHERE profile = ?1",
                [&p.name],
                |row| row.get(0),
            )?;
            println!("labelled reviews: {}", labelled);
        }
        ProfileAction::New { name, from, description, search, app_ids, top } => {
            let apps = profile::AppSelection { search, ids: app_ids, top };
            let dir = profile::create(config, &name, &from, &description, apps)?;
            println!("Created {}", dir.display());
            println!("Edit the copied prompts there to tailor the research; delete any you leave unchanged");
            println!("so they keep tracking the general versions. Other files in prompts/ can be copied in to override them.");
        }
    }
    Ok(())
}
