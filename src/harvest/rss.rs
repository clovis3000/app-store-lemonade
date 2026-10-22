use anyhow::Result;

use crate::db::{queries, Database};
use crate::http::RateLimitedClient;
use crate::models::RssFeedResponse;

const CHART_TYPES: &[&str] = &["top-free", "top-paid"];
const LIMIT: u32 = 100; // Apple's RSS API returns 500 for limit>100

pub async fn fetch_top_charts(
    db: &Database,
    client: &RateLimitedClient,
    countries: &[String],
) -> Result<()> {
    let mut total_inserted = 0;

    for country in countries {
        for chart in CHART_TYPES {
            let url = format!(
                "https://rss.marketingtools.apple.com/api/v2/{}/apps/{}/{}/apps.json",
                country, chart, LIMIT
            );
            let cache_key = format!("rss_{}_{}_{}", country, chart, LIMIT);

            tracing::info!("Fetching RSS: {} {} (limit {})", country, chart, LIMIT);

            let feed: RssFeedResponse = match client
                .get_json(&url, Some(&cache_key), Some(12))
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("Failed to fetch RSS {} {}: {}", country, chart, e);
                    continue;
                }
            };

            let count = feed.feed.results.len();
            for entry in &feed.feed.results {
                if let Err(e) = queries::upsert_app_from_rss(db, entry, country) {
                    tracing::warn!("Failed to upsert app {}: {}", entry.id, e);
                }
            }

            tracing::info!("  {} {}: {} apps", country, chart, count);
            total_inserted += count;
        }
    }

    tracing::info!("RSS harvest: {} total entries processed", total_inserted);
    Ok(())
}
