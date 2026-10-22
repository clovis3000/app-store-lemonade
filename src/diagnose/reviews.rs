use anyhow::Result;
use chrono::Utc;

use crate::db::{queries, Database};
use crate::http::RateLimitedClient;
use crate::models::{Review, ReviewFeedEntry, ReviewFeedResponse};

const PAGES_PER_COUNTRY: u32 = 10; // 50 reviews per page, 10 pages = 500 max

/// Scrapes reviews and returns how many the app now has stored.
pub async fn scrape_reviews(
    db: &Database,
    client: &RateLimitedClient,
    app_id: &str,
    countries: &[String],
) -> Result<i64> {
    scrape_reviews_with_staleness(db, client, app_id, countries, None).await
}

/// Scrapes each country not scraped within `max_age_days` (None: ever) and
/// returns how many reviews the app now has stored.
pub async fn scrape_reviews_with_staleness(
    db: &Database,
    client: &RateLimitedClient,
    app_id: &str,
    countries: &[String],
    max_age_days: Option<u32>,
) -> Result<i64> {
    for country in countries {
        if queries::has_fresh_scrape(db, app_id, "rss_reviews", country, max_age_days)? {
            tracing::debug!("  Reviews fresh for {} ({})", app_id, country);
            continue;
        }

        let mut new_reviews = 0;
        let mut failure: Option<String> = None;

        for page in 1..=PAGES_PER_COUNTRY {
            let url = format!(
                "https://itunes.apple.com/{}/rss/customerreviews/page={}/id={}/sortby=mostrecent/json",
                country, page, app_id
            );

            // Apple ends the feed with an empty 200, so an HTTP error within
            // pages 1-10 is a real failure (rate limit, outage), not the end.
            let response: ReviewFeedResponse = match client.get_json(&url, None, None).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("  Review page {} ({}) for {} failed: {:#}", page, country, app_id, e);
                    failure = Some(format!("page {page}: {e:#}"));
                    break;
                }
            };

            let entries = match response.feed.entry {
                Some(entries) if !entries.is_empty() => entries,
                _ => break,
            };

            let now = Utc::now().to_rfc3339();
            let tx = db.conn.unchecked_transaction()?;
            for entry in &entries {
                if queries::insert_review(db, &review_from_entry(entry, app_id, country, &now))? {
                    new_reviews += 1;
                }
            }
            tx.commit()?;

            // A full page holds 50; anything well short of that is the last one.
            if entries.len() < 40 {
                break;
            }
        }

        // A failed country is logged as such so the next run retries it.
        queries::log_scrape(db, app_id, "rss_reviews", country, new_reviews, failure.is_none(), failure.as_deref())?;
        tracing::debug!("  {} ({}): {} new reviews", app_id, country, new_reviews);
    }

    queries::get_review_count(db, app_id)
}

fn review_from_entry(entry: &ReviewFeedEntry, app_id: &str, country: &str, now: &str) -> Review {
    Review {
        review_id: entry.id.label.clone(),
        app_id: app_id.to_string(),
        platform: "ios".to_string(),
        rating: entry.rating.label.parse().unwrap_or(0),
        title: Some(entry.title.label.clone()),
        body: entry.content.as_ref().and_then(|c| c.label.clone()),
        author: entry.author.as_ref().map(|a| a.name.label.clone()),
        date: entry.updated.as_ref().map(|u| u.label.clone()),
        country: Some(country.to_string()),
        version: entry.version.as_ref().map(|v| v.label.clone()),
        scraped_at: Some(now.to_string()),
    }
}
