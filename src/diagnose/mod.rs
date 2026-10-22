mod claude_code;
mod competitive;
pub mod llm;
pub mod reviews;
pub mod templates;

use anyhow::Result;

use crate::config::AppConfig;
use crate::db::{queries, Database};
use crate::http::RateLimitedClient;
use crate::profile::Profile;

#[allow(clippy::too_many_arguments)]
pub async fn run_diagnosis(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    app_id: Option<&str>,
    limit: usize,
    reviews_only: bool,
    profile: &Profile,
    max_reviews: usize,
) -> Result<()> {
    let apps = if let Some(id) = app_id {
        match queries::get_app(db, id)? {
            Some(app) => vec![app],
            None => {
                anyhow::bail!("App not found: {}", id);
            }
        }
    } else {
        queries::get_top_undiagnosed(db, limit)?
    };

    if apps.is_empty() {
        tracing::info!("No apps to diagnose. Run `lemonade score` first.");
        return Ok(());
    }

    if !reviews_only {
        llm::preflight(config).await?;
    }

    tracing::info!("Diagnosing {} apps (profile: {})", apps.len(), profile.name);

    for app in &apps {
        tracing::info!(
            "─── {} ({}) score={:.2} ───",
            app.name,
            app.app_id,
            app.opportunity_score.unwrap_or(0.0)
        );

        // Step 1: Scrape reviews
        let review_count = reviews::scrape_reviews(db, client, &app.app_id, &config.countries).await?;
        tracing::info!("  {} reviews stored", review_count);

        if reviews_only {
            continue;
        }

        if review_count == 0 {
            tracing::warn!("  No reviews found, skipping LLM analysis");
            continue;
        }

        let mut all_reviews = queries::get_reviews(db, &app.app_id)?;
        all_reviews.truncate(max_reviews);
        tracing::info!("  Using {} of {} reviews", all_reviews.len(), review_count);

        let (mut diagnosis, _usage) = llm::analyse_reviews(client, config, profile, app, &all_reviews).await?;
        let Some(diag) = diagnosis.as_mut() else {
            tracing::warn!("  No diagnosis for {}; see the error above", app.app_id);
            continue;
        };

        let keywords = &diag.build_brief.aso_keywords;
        if !keywords.is_empty() {
            let scan = competitive::scan(client, keywords, app.category.as_deref().unwrap_or(""), &app.app_id).await?;
            tracing::info!("  Competitive scan: gap_open={}, {} alternatives", scan.gap_open, scan.alternatives_count);
            diag.competitive_scan = Some(scan);
        }

        let diag_json = serde_json::to_string(diag)?;
        queries::save_diagnosis(db, &app.app_id, &diag_json, diag.feasibility_tier, &diag.recommended_action)?;
        tracing::info!("  Diagnosis saved: tier={}, action={}", diag.feasibility_tier, diag.recommended_action);
    }

    Ok(())
}
