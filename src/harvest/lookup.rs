use anyhow::Result;
use std::hash::{Hash, Hasher};

use crate::db::{queries, Database};
use crate::http::RateLimitedClient;
use crate::models::LookupResponse;

const BATCH_SIZE: usize = 200;

pub async fn enrich_apps(db: &Database, client: &RateLimitedClient) -> Result<()> {
    let unenriched = queries::get_unenriched_app_ids(db)?;

    if unenriched.is_empty() {
        tracing::info!("All apps already enriched");
        return Ok(());
    }

    tracing::info!("{} apps need enrichment", unenriched.len());

    for batch in unenriched.chunks(BATCH_SIZE) {
        let ids_param = batch.join(",");
        let url = format!(
            "https://itunes.apple.com/lookup?id={}&country=us",
            ids_param
        );
        // Key on the whole ID list: batches sharing a prefix used to collide
        // and serve each other's responses. DefaultHasher isn't stable across
        // Rust releases, which only costs a cache miss.
        let mut hasher = std::hash::DefaultHasher::new();
        ids_param.hash(&mut hasher);
        let cache_key = format!("lookup_{}_{:016x}", batch.len(), hasher.finish());

        tracing::info!("Looking up batch of {} apps", batch.len());

        let response: LookupResponse = match client
            .get_json(&url, Some(&cache_key), Some(24))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Lookup batch failed: {}", e);
                continue;
            }
        };

        let mut enriched = 0;
        for result in &response.results {
            if let Err(e) = queries::enrich_app_from_lookup(db, result) {
                tracing::warn!("Failed to enrich app: {}", e);
            } else {
                enriched += 1;
            }
        }

        tracing::info!("  Enriched {}/{} apps", enriched, batch.len());
    }

    Ok(())
}
