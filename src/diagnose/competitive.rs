use anyhow::Result;

use crate::http::RateLimitedClient;
use crate::models::{CompetitiveAlternative, CompetitiveScan, LookupResponse};

const MIN_ALTERNATIVE_RATING: f64 = 4.0;
const MIN_ALTERNATIVE_REVIEWS: i64 = 1000;

pub async fn scan(
    client: &RateLimitedClient,
    aso_keywords: &[String],
    category: &str,
    exclude_app_id: &str,
) -> Result<CompetitiveScan> {
    let mut alternatives: Vec<CompetitiveAlternative> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    seen_ids.insert(exclude_app_id.to_string());

    for keyword in aso_keywords.iter().take(5) {
        let url = format!(
            "https://itunes.apple.com/search?term={}&entity=software&country=us&limit=20",
            urlencoding::encode(keyword)
        );

        let response: LookupResponse = match client.get_json(&url, None, None).await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Competitive search '{}' failed: {}", keyword, e);
                continue;
            }
        };

        for result in &response.results {
            let app_id = match result.track_id {
                Some(id) => id.to_string(),
                None => continue,
            };

            if seen_ids.contains(&app_id) {
                continue;
            }
            seen_ids.insert(app_id.clone());

            let rating = result.average_user_rating.unwrap_or(0.0);
            let rating_count = result.user_rating_count.unwrap_or(0);
            let genre = result.primary_genre_name.as_deref().unwrap_or("");

            if rating >= MIN_ALTERNATIVE_RATING
                && rating_count >= MIN_ALTERNATIVE_REVIEWS
                && (category.is_empty() || genre == category)
            {
                alternatives.push(CompetitiveAlternative {
                    name: result.track_name.clone().unwrap_or_default(),
                    app_id,
                    rating,
                    rating_count,
                });
            }
        }
    }

    let best = alternatives
        .iter()
        .max_by(|a, b| a.rating.partial_cmp(&b.rating).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();

    Ok(CompetitiveScan {
        gap_open: alternatives.is_empty(),
        alternatives_count: alternatives.len() as i32,
        best_alternative: best,
    })
}
