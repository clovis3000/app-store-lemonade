use anyhow::Result;
use chrono::Utc;

use crate::config::AppConfig;
use crate::db::{queries, Database};
use crate::models::{MonetisationModel, ScoreComponents};

pub fn run_scoring(
    db: &Database,
    config: &AppConfig,
    limit: Option<usize>,
    recalculate: bool,
) -> Result<()> {
    let apps = queries::get_scoreable_apps(db, config)?;

    if apps.is_empty() {
        tracing::info!("No apps eligible for scoring. Run `lemonade harvest` first.");
        return Ok(());
    }

    tracing::info!("Scoring {} eligible apps", apps.len());

    let mut scored = 0;
    for app in &apps {
        // Skip already-scored unless recalculating
        if !recalculate && app.opportunity_score.is_some() {
            continue;
        }

        let rating = match app.rating {
            Some(r) => r,
            None => continue,
        };
        let rating_count = match app.rating_count {
            Some(c) => c,
            None => continue,
        };

        // Calculate months since update
        let months_since_update = app
            .last_updated
            .as_ref()
            .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
            .map(|dt| {
                let now = Utc::now();
                let duration = now.signed_duration_since(dt);
                duration.num_days() as f64 / 30.44 // average days per month
            })
            .unwrap_or(0.0);

        let monetisation_bonus = app
            .monetisation_model
            .as_ref()
            .map(|m| MonetisationModel::from_str_lossy(m).scoring_bonus())
            .unwrap_or(1.0);

        let scores = ScoreComponents::compute(
            rating_count,
            rating,
            months_since_update,
            monetisation_bonus,
        );

        queries::update_scores(
            db,
            &app.app_id,
            scores.demand_signal,
            scores.dissatisfaction_signal,
            scores.neglect_signal,
            scores.opportunity_score,
        )?;

        scored += 1;
    }

    tracing::info!("Scored {} apps", scored);

    // Show top 5
    let top = queries::list_apps(db, "shortlist", limit.unwrap_or(5))?;
    if !top.is_empty() {
        tracing::info!("Top opportunities:");
        for (i, app) in top.iter().enumerate() {
            tracing::info!(
                "  {}. {} (score: {:.2}) - {} | rating: {:.1} ({} reviews)",
                i + 1,
                app.name,
                app.opportunity_score.unwrap_or(0.0),
                app.category.as_deref().unwrap_or("?"),
                app.rating.unwrap_or(0.0),
                app.rating_count.unwrap_or(0),
            );
        }
    }

    Ok(())
}
