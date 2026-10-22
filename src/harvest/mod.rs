mod lookup;
mod rss;
mod search;

use anyhow::Result;

use crate::config::AppConfig;
use crate::db::Database;
use crate::http::RateLimitedClient;

pub async fn run_harvest(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    countries: &[String],
    rss_only: bool,
    enrich_only: bool,
    search_only: bool,
) -> Result<()> {
    let run_all = !rss_only && !enrich_only && !search_only;

    if rss_only || run_all {
        tracing::info!("=== Harvesting RSS top charts ===");
        rss::fetch_top_charts(db, client, countries).await?;
    }

    if search_only || run_all {
        tracing::info!("=== Harvesting keyword search ===");
        search::search_keywords(db, client, config, countries).await?;
    }

    if enrich_only || run_all {
        tracing::info!("=== Enriching apps via iTunes Lookup ===");
        lookup::enrich_apps(db, client).await?;
    }

    let total: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM apps", [], |r| r.get(0))?;
    let enriched: i64 = db
        .conn
        .query_row("SELECT COUNT(*) FROM apps WHERE rating IS NOT NULL", [], |r| r.get(0))?;

    tracing::info!("Harvest complete: {} apps total, {} enriched", total, enriched);

    Ok(())
}
