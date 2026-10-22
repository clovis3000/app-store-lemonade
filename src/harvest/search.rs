use anyhow::Result;

use crate::config::AppConfig;
use crate::db::{queries, Database};
use crate::http::RateLimitedClient;
use crate::models::LookupResponse;

pub async fn search_keywords(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    countries: &[String],
) -> Result<()> {
    if config.keywords.is_empty() {
        tracing::info!("No keywords configured, skipping search");
        return Ok(());
    }

    let mut total_new = 0;

    // Search only first country to avoid excessive duplication
    let country = countries.first().map(|s| s.as_str()).unwrap_or("us");

    for keyword in &config.keywords {
        let url = format!(
            "https://itunes.apple.com/search?term={}&entity=software&country={}&limit=200",
            urlencoding::encode(keyword),
            country
        );
        let cache_key = format!("search_{}_{}", country, keyword.replace(' ', "_"));

        tracing::info!("Searching: '{}' ({})", keyword, country);

        let response: LookupResponse = match client
            .get_json(&url, Some(&cache_key), Some(24))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Search failed for '{}': {}", keyword, e);
                continue;
            }
        };

        let mut new_apps = 0;
        for result in &response.results {
            let app_id = match result.track_id {
                Some(id) => id.to_string(),
                None => continue,
            };

            let is_new = !queries::app_exists(db, &app_id)?;
            if let Err(e) = queries::enrich_app_from_lookup(db, result) {
                tracing::warn!("Failed to insert search result {}: {}", app_id, e);
            } else if is_new {
                new_apps += 1;
            }
        }

        tracing::info!("  '{}': {} results, {} new", keyword, response.results.len(), new_apps);
        total_new += new_apps;
    }

    tracing::info!("Search harvest: {} new apps discovered", total_new);
    Ok(())
}
