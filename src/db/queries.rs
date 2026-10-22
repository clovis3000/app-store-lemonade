use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashMap;

use crate::api::responses::{
    AppSummary, CategoryStat, PipelineFunnel, ReviewSummary, ScoreBucket,
};
use crate::db::Database;
use crate::models::{Analysis, App, LookupResult, MonetisationModel, Review, RssFeedResult, Synthesis};

impl App {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(App {
            app_id: row.get("app_id")?,
            platform: row.get("platform")?,
            name: row.get("name")?,
            developer: row.get("developer")?,
            category: row.get("category")?,
            subcategory: row.get("subcategory")?,
            rating: row.get("rating")?,
            rating_count: row.get("rating_count")?,
            price: row.get("price")?,
            formatted_price: row.get("formatted_price")?,
            monetisation_model: row.get("monetisation_model")?,
            last_updated: row.get("last_updated")?,
            original_release: row.get("original_release")?,
            version: row.get("version")?,
            description: row.get("description")?,
            app_store_url: row.get("app_store_url")?,
            file_size_bytes: row.get("file_size_bytes")?,
            current_version_rating: row.get("current_version_rating")?,
            scraped_at: row.get("scraped_at")?,
            demand_signal: row.get("demand_signal")?,
            dissatisfaction_signal: row.get("dissatisfaction_signal")?,
            neglect_signal: row.get("neglect_signal")?,
            opportunity_score: row.get("opportunity_score")?,
            diagnosis_json: row.get("diagnosis_json")?,
            diagnosed_at: row.get("diagnosed_at")?,
            feasibility_tier: row.get("feasibility_tier")?,
            recommended_action: row.get("recommended_action")?,
            starred: row.get::<_, i32>("starred").unwrap_or(0) != 0,
        })
    }
}

impl Review {
    /// Maps a row selected with `REVIEW_COLUMNS`.
    pub fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Review {
            review_id: row.get("review_id")?,
            app_id: row.get("app_id")?,
            platform: row.get("platform")?,
            rating: row.get("rating")?,
            title: row.get("title")?,
            body: row.get("body")?,
            author: row.get("author")?,
            date: row.get("date")?,
            country: row.get("country")?,
            version: row.get("version")?,
            scraped_at: row.get("scraped_at")?,
        })
    }
}

pub const REVIEW_COLUMNS: &str =
    "review_id, app_id, platform, rating, title, body, author, date, country, version, scraped_at";

impl Analysis {
    fn from_row(row: &rusqlite::Row) -> rusqlite::Result<Self> {
        Ok(Analysis {
            id: row.get("id")?,
            app_id: row.get("app_id")?,
            prompt_name: row.get("prompt_name")?,
            profile: row.get("profile")?,
            parent_id: row.get("parent_id")?,
            review_ids: row.get("review_ids")?,
            result_json: row.get("result_json")?,
            model: row.get("model")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Insert or update an app from RSS feed data
pub fn upsert_app_from_rss(db: &Database, entry: &RssFeedResult, country: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let category = entry
        .genres
        .as_ref()
        .and_then(|g| g.first())
        .map(|g| g.name.clone());

    db.conn.execute(
        "INSERT INTO apps (app_id, platform, name, developer, category, app_store_url, scraped_at)
         VALUES (?1, 'ios', ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(app_id) DO UPDATE SET
            name = COALESCE(excluded.name, apps.name),
            developer = COALESCE(excluded.developer, apps.developer),
            category = COALESCE(excluded.category, apps.category),
            app_store_url = COALESCE(excluded.app_store_url, apps.app_store_url),
            scraped_at = excluded.scraped_at",
        rusqlite::params![
            entry.id,
            entry.name,
            entry.artist_name,
            category,
            entry.url,
            now,
        ],
    ).with_context(|| format!("Failed to upsert app {} from RSS (country={})", entry.id, country))?;

    Ok(())
}

/// Enrich an existing app with iTunes Lookup data
pub fn enrich_app_from_lookup(db: &Database, result: &LookupResult) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let app_id = match result.track_id {
        Some(id) => id.to_string(),
        None => return Ok(()),
    };

    let file_size: Option<i64> = result
        .file_size_bytes
        .as_ref()
        .and_then(|s| s.parse().ok());

    let monetisation = result
        .price
        .map(|p| {
            let desc = result.description.as_deref().unwrap_or("");
            MonetisationModel::infer(p, desc).as_str().to_string()
        })
        .unwrap_or_else(|| "unknown".into());

    db.conn.execute(
        "INSERT INTO apps (app_id, platform, name, developer, category, rating, rating_count,
                          price, formatted_price, monetisation_model, last_updated, original_release,
                          version, description, app_store_url, file_size_bytes,
                          current_version_rating, scraped_at)
         VALUES (?1, 'ios', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
         ON CONFLICT(app_id) DO UPDATE SET
            name = COALESCE(excluded.name, apps.name),
            developer = COALESCE(excluded.developer, apps.developer),
            category = COALESCE(excluded.category, apps.category),
            rating = COALESCE(excluded.rating, apps.rating),
            rating_count = COALESCE(excluded.rating_count, apps.rating_count),
            price = COALESCE(excluded.price, apps.price),
            formatted_price = COALESCE(excluded.formatted_price, apps.formatted_price),
            monetisation_model = COALESCE(excluded.monetisation_model, apps.monetisation_model),
            last_updated = COALESCE(excluded.last_updated, apps.last_updated),
            original_release = COALESCE(excluded.original_release, apps.original_release),
            version = COALESCE(excluded.version, apps.version),
            description = COALESCE(excluded.description, apps.description),
            app_store_url = COALESCE(excluded.app_store_url, apps.app_store_url),
            file_size_bytes = COALESCE(excluded.file_size_bytes, apps.file_size_bytes),
            current_version_rating = COALESCE(excluded.current_version_rating, apps.current_version_rating),
            scraped_at = excluded.scraped_at",
        rusqlite::params![
            app_id,
            result.track_name,
            result.seller_name,
            result.primary_genre_name,
            result.average_user_rating,
            result.user_rating_count,
            result.price,
            result.formatted_price,
            monetisation,
            result.current_version_release_date,
            result.release_date,
            result.version,
            result.description,
            result.track_view_url,
            file_size,
            result.average_user_rating_for_current_version,
            now,
        ],
    ).with_context(|| format!("Failed to enrich app {}", app_id))?;

    Ok(())
}

/// Get all app IDs that need enrichment (have no rating data yet)
pub fn get_unenriched_app_ids(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db
        .conn
        .prepare("SELECT app_id FROM apps WHERE rating IS NULL ORDER BY app_id")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Get all app IDs in the database
pub fn get_all_app_ids(db: &Database) -> Result<Vec<String>> {
    let mut stmt = db.conn.prepare("SELECT app_id FROM apps ORDER BY app_id")?;
    let ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Check if an app already exists
pub fn app_exists(db: &Database, app_id: &str) -> Result<bool> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE app_id = ?1",
        [app_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Update scoring fields for an app
pub fn update_scores(
    db: &Database,
    app_id: &str,
    demand: f64,
    dissatisfaction: f64,
    neglect: f64,
    score: f64,
) -> Result<()> {
    db.conn.execute(
        "UPDATE apps SET demand_signal = ?1, dissatisfaction_signal = ?2,
         neglect_signal = ?3, opportunity_score = ?4 WHERE app_id = ?5",
        rusqlite::params![demand, dissatisfaction, neglect, score, app_id],
    )?;
    Ok(())
}

/// Get apps eligible for scoring
pub fn get_scoreable_apps(db: &Database, config: &crate::config::AppConfig) -> Result<Vec<App>> {
    let skip_devs_placeholders: String = config
        .scoring
        .skip_developers
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let skip_cats_placeholders: String = config
        .scoring
        .skip_categories
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "SELECT * FROM apps
         WHERE rating IS NOT NULL
           AND rating_count IS NOT NULL
           AND rating <= ?
           AND rating_count >= ?
           AND (category IS NULL OR category NOT IN ({skip_cats_placeholders}))
           AND (developer IS NULL OR developer NOT IN ({skip_devs_placeholders}))"
    );

    let mut stmt = db.conn.prepare(&sql)?;

    let mut param_idx = 1;
    stmt.raw_bind_parameter(param_idx, config.scoring.max_rating)?;
    param_idx += 1;
    stmt.raw_bind_parameter(param_idx, config.scoring.min_rating_count)?;
    param_idx += 1;

    for cat in &config.scoring.skip_categories {
        stmt.raw_bind_parameter(param_idx, cat.as_str())?;
        param_idx += 1;
    }
    for dev in &config.scoring.skip_developers {
        stmt.raw_bind_parameter(param_idx, dev.as_str())?;
        param_idx += 1;
    }

    let apps = stmt
        .raw_query()
        .mapped(App::from_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(apps)
}

/// Get top scored apps that haven't been diagnosed
pub fn get_top_undiagnosed(db: &Database, limit: usize) -> Result<Vec<App>> {
    let mut stmt = db.conn.prepare(
        "SELECT * FROM apps
         WHERE opportunity_score IS NOT NULL AND diagnosed_at IS NULL
         ORDER BY opportunity_score DESC
         LIMIT ?1",
    )?;
    let apps = stmt
        .query_map([limit as i64], App::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(apps)
}

/// Get a single app by ID
pub fn get_app(db: &Database, app_id: &str) -> Result<Option<App>> {
    let mut stmt = db.conn.prepare("SELECT * FROM apps WHERE app_id = ?1")?;
    let mut rows = stmt.query_map([app_id], App::from_row)?;
    match rows.next() {
        Some(Ok(app)) => Ok(Some(app)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// Insert a review, ignoring duplicates. Returns whether it was new.
pub fn insert_review(db: &Database, review: &Review) -> Result<bool> {
    let inserted = db.conn.execute(
        "INSERT OR IGNORE INTO reviews (review_id, app_id, platform, rating, title, body, author, date, country, version, scraped_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            review.review_id,
            review.app_id,
            review.platform,
            review.rating,
            review.title,
            review.body,
            review.author,
            review.date,
            review.country,
            review.version,
            review.scraped_at,
        ],
    )?;
    if inserted > 0 {
        db.conn.execute(
            "INSERT INTO reviews_fts(review_id, title, body) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                review.review_id,
                review.title.as_deref().unwrap_or(""),
                review.body.as_deref().unwrap_or("")
            ],
        )?;
    }
    Ok(inserted > 0)
}

/// Get reviews for an app
pub fn get_reviews(db: &Database, app_id: &str) -> Result<Vec<Review>> {
    let mut stmt = db.conn.prepare(
        "SELECT review_id, app_id, platform, rating, title, body, author, date, country, version, scraped_at
         FROM reviews WHERE app_id = ?1 ORDER BY date DESC",
    )?;
    let reviews = stmt
        .query_map([app_id], Review::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(reviews)
}

/// Log a scrape attempt
pub fn log_scrape(
    db: &Database,
    app_id: &str,
    source: &str,
    country: &str,
    review_count: i32,
    success: bool,
    error_message: Option<&str>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "INSERT INTO scrape_log (app_id, source, country, scraped_at, review_count, success, error_message)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            app_id,
            source,
            country,
            now,
            review_count,
            success as i32,
            error_message,
        ],
    )?;
    Ok(())
}

/// Check if we've scraped reviews for this app/source/country within the staleness window.
/// Returns true (fresh) if a successful scrape exists within `max_age_days`.
/// If `max_age_days` is None, any successful scrape counts as fresh (legacy behaviour).
pub fn has_fresh_scrape(db: &Database, app_id: &str, source: &str, country: &str, max_age_days: Option<u32>) -> Result<bool> {
    match max_age_days {
        Some(days) => {
            let cutoff = (Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
            let count: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM scrape_log WHERE app_id = ?1 AND source = ?2 AND country = ?3 AND success = 1 AND scraped_at > ?4",
                rusqlite::params![app_id, source, country, cutoff],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        }
        None => {
            let count: i64 = db.conn.query_row(
                "SELECT COUNT(*) FROM scrape_log WHERE app_id = ?1 AND source = ?2 AND country = ?3 AND success = 1",
                rusqlite::params![app_id, source, country],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        }
    }
}

/// Save diagnosis results
pub fn save_diagnosis(
    db: &Database,
    app_id: &str,
    diagnosis_json: &str,
    feasibility_tier: i32,
    recommended_action: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    db.conn.execute(
        "UPDATE apps SET diagnosis_json = ?1, diagnosed_at = ?2, feasibility_tier = ?3, recommended_action = ?4
         WHERE app_id = ?5",
        rusqlite::params![diagnosis_json, now, feasibility_tier, recommended_action, app_id],
    )?;
    Ok(())
}

/// List apps by filter
pub fn list_apps(db: &Database, filter: &str, limit: usize) -> Result<Vec<App>> {
    let sql = match filter {
        "shortlist" | "scored" => {
            "SELECT * FROM apps WHERE opportunity_score IS NOT NULL ORDER BY opportunity_score DESC LIMIT ?1"
        }
        "diagnosed" => {
            "SELECT * FROM apps WHERE diagnosed_at IS NOT NULL ORDER BY opportunity_score DESC LIMIT ?1"
        }
        "actionable" => {
            "SELECT * FROM apps WHERE recommended_action = 'build' ORDER BY opportunity_score DESC LIMIT ?1"
        }
        _ => "SELECT * FROM apps ORDER BY opportunity_score DESC NULLS LAST, rating_count DESC NULLS LAST LIMIT ?1",
    };

    let mut stmt = db.conn.prepare(sql)?;
    let apps = stmt
        .query_map([limit as i64], App::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(apps)
}

/// Get review count for an app
pub fn get_review_count(db: &Database, app_id: &str) -> Result<i64> {
    let count: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM reviews WHERE app_id = ?1",
        [app_id],
        |row| row.get(0),
    )?;
    Ok(count)
}

// --- Web API query functions ---

/// Paginated app list with filtering and sorting
#[allow(clippy::too_many_arguments)]
pub fn list_apps_paginated(
    db: &Database,
    filter: &str,
    sort: &str,
    order: &str,
    category: Option<&str>,
    search: Option<&str>,
    page: u32,
    per_page: u32,
) -> Result<(Vec<AppSummary>, u32)> {
    let where_clause = match filter {
        "scored" => "WHERE a.opportunity_score IS NOT NULL",
        "diagnosed" => "WHERE a.diagnosed_at IS NOT NULL",
        "actionable" => "WHERE a.recommended_action = 'build'",
        "starred" => "WHERE a.starred = 1",
        _ => "WHERE 1=1",
    };

    let mut conditions = vec![where_clause.to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(cat) = category {
        params.push(Box::new(cat.to_string()));
        conditions.push(format!("AND a.category = ?{}", params.len()));
    }
    if let Some(s) = search {
        params.push(Box::new(format!("%{s}%")));
        conditions.push(format!("AND a.name LIKE ?{}", params.len()));
    }

    let where_sql = conditions.join(" ");

    let sort_col = match sort {
        "rating" => "a.rating",
        "rating_count" => "a.rating_count",
        "name" => "a.name",
        "last_updated" => "a.last_updated",
        _ => "a.opportunity_score",
    };
    let order_dir = if order == "asc" { "ASC" } else { "DESC" };

    // Count query
    let count_sql = format!(
        "SELECT COUNT(*) FROM apps a {where_sql}"
    );
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: u32 = db.conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;

    // Data query
    let offset = (page - 1) * per_page;
    let data_sql = format!(
        "SELECT a.*, COALESCE(r.cnt, 0) as review_count
         FROM apps a
         LEFT JOIN (SELECT app_id, COUNT(*) as cnt FROM reviews GROUP BY app_id) r ON r.app_id = a.app_id
         {where_sql}
         ORDER BY {sort_col} {order_dir} NULLS LAST
         LIMIT ?{} OFFSET ?{}",
        params.len() + 1,
        params.len() + 2,
    );

    params.push(Box::new(per_page));
    params.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = db.conn.prepare(&data_sql)?;
    let apps = stmt
        .query_map(param_refs.as_slice(), |row| {
            let app = App::from_row(row)?;
            let review_count: i64 = row.get("review_count")?;
            Ok(AppSummary::from_app(&app, review_count))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok((apps, total))
}

/// Review breakdown by rating and country for an app
pub fn get_review_summary(db: &Database, app_id: &str) -> Result<ReviewSummary> {
    let total: i64 = db.conn.query_row(
        "SELECT COUNT(*) FROM reviews WHERE app_id = ?1",
        [app_id],
        |row| row.get(0),
    )?;

    let mut by_rating = HashMap::new();
    let mut stmt = db.conn.prepare(
        "SELECT rating, COUNT(*) FROM reviews WHERE app_id = ?1 GROUP BY rating"
    )?;
    let rows = stmt.query_map([app_id], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (rating, count) = row?;
        by_rating.insert(rating, count);
    }

    let mut by_country = HashMap::new();
    let mut stmt = db.conn.prepare(
        "SELECT COALESCE(country, 'unknown'), COUNT(*) FROM reviews WHERE app_id = ?1 GROUP BY country"
    )?;
    let rows = stmt.query_map([app_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    for row in rows {
        let (country, count) = row?;
        by_country.insert(country, count);
    }

    Ok(ReviewSummary {
        total,
        by_rating,
        by_country,
    })
}

/// Paginated reviews with filters
#[allow(clippy::too_many_arguments)]
pub fn get_reviews_paginated(
    db: &Database,
    app_id: &str,
    country: Option<&str>,
    rating: Option<i32>,
    sort: &str,
    order: &str,
    page: u32,
    per_page: u32,
) -> Result<(Vec<Review>, u32)> {
    let mut conditions = vec!["app_id = ?1".to_string()];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(app_id.to_string())];

    if let Some(c) = country {
        params.push(Box::new(c.to_string()));
        conditions.push(format!("country = ?{}", params.len()));
    }
    if let Some(r) = rating {
        params.push(Box::new(r));
        conditions.push(format!("rating = ?{}", params.len()));
    }

    let where_sql = conditions.join(" AND ");
    let sort_col = if sort == "rating" { "rating" } else { "date" };
    let order_dir = if order == "asc" { "ASC" } else { "DESC" };

    let count_sql = format!("SELECT COUNT(*) FROM reviews WHERE {where_sql}");
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: u32 = db.conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;

    let offset = (page - 1) * per_page;
    let data_sql = format!(
        "SELECT review_id, app_id, platform, rating, title, body, author, date, country, version, scraped_at
         FROM reviews WHERE {where_sql}
         ORDER BY {sort_col} {order_dir} NULLS LAST
         LIMIT ?{} OFFSET ?{}",
        params.len() + 1,
        params.len() + 2,
    );
    params.push(Box::new(per_page));
    params.push(Box::new(offset));

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let mut stmt = db.conn.prepare(&data_sql)?;
    let reviews = stmt
        .query_map(param_refs.as_slice(), Review::from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok((reviews, total))
}

/// Pipeline funnel counts
pub fn get_pipeline_stats(db: &Database) -> Result<PipelineFunnel> {
    let total_apps: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps", [], |row| row.get(0),
    )?;
    let scored: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE opportunity_score IS NOT NULL", [], |row| row.get(0),
    )?;
    let diagnosed: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE diagnosed_at IS NOT NULL", [], |row| row.get(0),
    )?;
    let actionable: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE recommended_action = 'build'", [], |row| row.get(0),
    )?;
    let total_reviews: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM reviews", [], |row| row.get(0),
    )?;
    let starred: u32 = db.conn.query_row(
        "SELECT COUNT(*) FROM apps WHERE starred = 1", [], |row| row.get(0),
    )?;

    Ok(PipelineFunnel {
        total_apps,
        scored,
        diagnosed,
        actionable,
        total_reviews,
        starred,
    })
}

/// Category breakdown with counts and averages
pub fn get_category_stats(db: &Database) -> Result<Vec<CategoryStat>> {
    let mut stmt = db.conn.prepare(
        "SELECT
            COALESCE(category, 'Unknown') as cat,
            COUNT(*) as cnt,
            COALESCE(AVG(rating), 0.0) as avg_r,
            SUM(CASE WHEN opportunity_score IS NOT NULL THEN 1 ELSE 0 END) as scored_cnt
         FROM apps
         GROUP BY category
         ORDER BY cnt DESC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CategoryStat {
            category: row.get(0)?,
            count: row.get(1)?,
            avg_rating: row.get(2)?,
            scored_count: row.get(3)?,
        })
    })?;
    let stats = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(stats)
}

/// Score distribution buckets
pub fn get_score_distribution(db: &Database) -> Result<Vec<ScoreBucket>> {
    let mut stmt = db.conn.prepare(
        "SELECT
            CASE
                WHEN opportunity_score < 0.5 THEN '0-0.5'
                WHEN opportunity_score < 1.0 THEN '0.5-1.0'
                WHEN opportunity_score < 1.5 THEN '1.0-1.5'
                WHEN opportunity_score < 2.0 THEN '1.5-2.0'
                WHEN opportunity_score < 2.5 THEN '2.0-2.5'
                WHEN opportunity_score < 3.0 THEN '2.5-3.0'
                ELSE '3.0+'
            END as range,
            COUNT(*) as cnt
         FROM apps
         WHERE opportunity_score IS NOT NULL
         GROUP BY range
         ORDER BY opportunity_score"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ScoreBucket {
            range: row.get(0)?,
            count: row.get(1)?,
        })
    })?;
    let buckets = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(buckets)
}

/// Top N apps by opportunity score (with review counts)
pub fn get_top_opportunities(db: &Database, limit: u32) -> Result<Vec<AppSummary>> {
    let mut stmt = db.conn.prepare(
        "SELECT a.*, COALESCE(r.cnt, 0) as review_count
         FROM apps a
         LEFT JOIN (SELECT app_id, COUNT(*) as cnt FROM reviews GROUP BY app_id) r ON r.app_id = a.app_id
         WHERE a.opportunity_score IS NOT NULL
         ORDER BY a.opportunity_score DESC
         LIMIT ?1"
    )?;
    let apps = stmt
        .query_map([limit], |row| {
            let app = App::from_row(row)?;
            let review_count: i64 = row.get("review_count")?;
            Ok(AppSummary::from_app(&app, review_count))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(apps)
}

// --- Analysis queries ---

/// Save an analysis result, returns the new row ID
#[allow(clippy::too_many_arguments)]
pub fn save_analysis(
    db: &Database,
    app_id: &str,
    kind: &str,
    profile: &str,
    parent_id: Option<i64>,
    review_ids: &[String],
    result_json: &str,
    model: &str,
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let review_ids_json = serde_json::to_string(review_ids)?;
    db.conn.execute(
        "INSERT INTO analyses (app_id, prompt_name, profile, parent_id, review_ids, result_json, model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![app_id, kind, profile, parent_id, review_ids_json, result_json, model, now],
    )?;
    Ok(db.conn.last_insert_rowid())
}

/// Get a single analysis by ID
pub fn get_analysis(db: &Database, id: i64) -> Result<Option<Analysis>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, app_id, prompt_name, profile, parent_id, review_ids, result_json, model, created_at
         FROM analyses WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], Analysis::from_row)?;
    match rows.next() {
        Some(Ok(a)) => Ok(Some(a)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// List all analyses for an app
pub fn list_analyses_for_app(db: &Database, app_id: &str) -> Result<Vec<Analysis>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, app_id, prompt_name, profile, parent_id, review_ids, result_json, model, created_at
         FROM analyses WHERE app_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([app_id], Analysis::from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Get reviews by a list of review IDs
pub fn get_reviews_by_ids(db: &Database, ids: &[String]) -> Result<Vec<Review>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "SELECT review_id, app_id, platform, rating, title, body, author, date, country, version, scraped_at
         FROM reviews WHERE review_id IN ({}) ORDER BY date DESC",
        placeholders.join(", ")
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();
    let reviews = stmt
        .query_map(params.as_slice(), Review::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(reviews)
}

/// Search apps by name/description matching any of the given terms: name matches first, then by rating_count
pub fn search_apps(db: &Database, terms: &[&str], limit: usize) -> Result<Vec<App>> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let conditions: Vec<String> = terms
        .iter()
        .enumerate()
        .map(|(i, _)| format!("(name LIKE ?{p1} OR description LIKE ?{p1})", p1 = i + 1))
        .collect();
    let where_sql = conditions.join(" OR ");
    // A term in the name beats a passing mention in a description; without
    // this, the biggest apps that mention "habit tracker" anywhere won.
    let name_match: Vec<String> = (1..=terms.len()).map(|i| format!("name LIKE ?{i}")).collect();
    let sql = format!(
        "SELECT * FROM apps WHERE ({}) ORDER BY ({}) DESC, rating_count DESC LIMIT ?{}",
        where_sql,
        name_match.join(" OR "),
        terms.len() + 1
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = terms
        .iter()
        .map(|t| Box::new(format!("%{}%", t)) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    params.push(Box::new(limit as i64));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = db.conn.prepare(&sql)?;
    let apps = stmt
        .query_map(param_refs.as_slice(), App::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(apps)
}

/// Check if a fresh scrape exists for an app (any country)
pub fn last_scrape_date(db: &Database, app_id: &str, source: &str) -> Result<Option<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT MAX(scraped_at) FROM scrape_log WHERE app_id = ?1 AND source = ?2 AND success = 1",
    )?;
    let date: Option<String> = stmt.query_row(rusqlite::params![app_id, source], |row| row.get(0))?;
    Ok(date)
}

// --- Synthesis queries ---

/// Save a cross-app synthesis, returns the new row ID
pub fn save_synthesis(
    db: &Database,
    name: &str,
    profile: &str,
    analysis_ids: &[i64],
    app_ids: &[String],
    result_json: &str,
    model: &str,
) -> Result<i64> {
    let now = Utc::now().to_rfc3339();
    let analysis_ids_json = serde_json::to_string(analysis_ids)?;
    let app_ids_json = serde_json::to_string(app_ids)?;
    db.conn.execute(
        "INSERT INTO syntheses (name, profile, analysis_ids, app_ids, result_json, model, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![name, profile, analysis_ids_json, app_ids_json, result_json, model, now],
    )?;
    Ok(db.conn.last_insert_rowid())
}

/// List all syntheses
pub fn list_syntheses(db: &Database) -> Result<Vec<Synthesis>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, name, profile, analysis_ids, app_ids, result_json, model, created_at
         FROM syntheses ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Synthesis {
            id: row.get("id")?,
            name: row.get("name")?,
            profile: row.get("profile")?,
            analysis_ids: row.get("analysis_ids")?,
            app_ids: row.get("app_ids")?,
            result_json: row.get("result_json")?,
            model: row.get("model")?,
            created_at: row.get("created_at")?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Toggle star on an app, returns new starred state
pub fn toggle_star(db: &Database, app_id: &str) -> Result<bool> {
    db.conn.execute(
        "UPDATE apps SET starred = CASE WHEN starred = 1 THEN 0 ELSE 1 END WHERE app_id = ?1",
        [app_id],
    )?;
    let starred: i32 = db.conn.query_row(
        "SELECT starred FROM apps WHERE app_id = ?1",
        [app_id],
        |row| row.get(0),
    )?;
    Ok(starred != 0)
}
