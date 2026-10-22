//! Review indexing: per-profile labels in `review_labels`, shared embeddings
//! in `review_embeddings`, and keyword search in `reviews_fts`.

use anyhow::{Context, Result};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};

use crate::config::AppConfig;
use crate::db::queries::{self, REVIEW_COLUMNS};
use crate::db::Database;
use crate::http::RateLimitedClient;
use crate::models::{App, Review};
use crate::openai::{self, OpenaiUsage};
use crate::pricing;
use crate::profile::Profile;
use crate::text::truncate;

const EMBED_BATCH_SIZE: usize = 500;
const CALIBRATION_PER_APP_RATING: usize = 6;
const CALIBRATION_MAX_APPS: usize = 8;

pub struct IndexOptions {
    pub calibrate: bool,
    pub batch_size: usize,
    pub concurrency: usize,
}

pub async fn run(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    apps: &[App],
    options: IndexOptions,
) -> Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .context("OPENAI_API_KEY not set")?;
    let app_ids: Vec<String> = apps.iter().map(|a| a.app_id.clone()).collect();
    let app_names: HashMap<String, String> = apps.iter().map(|a| (a.app_id.clone(), a.name.clone())).collect();

    if options.calibrate {
        // A separate label key, so calibrating never touches real labels.
        let key = format!("{}:calibration", profile.name);
        db.conn.execute("DELETE FROM review_labels WHERE profile = ?1", [&key])?;
        let sample = calibration_sample(db, &app_ids)?;
        print_sample_distribution(&sample, &app_names);
        classify(db, client, config, profile, &key, &api_key, &sample, &app_names, &options).await?;
        return print_label_stats(db, profile, &key);
    }

    // --- Phase 1: Classification ---
    let unlabelled = load_reviews(
        db,
        &app_ids,
        "AND r.review_id NOT IN (SELECT review_id FROM review_labels WHERE profile = ?1)",
        Some(&profile.name),
    )?;
    if unlabelled.is_empty() {
        println!("All {} apps' reviews already labelled for profile '{}'.", apps.len(), profile.name);
    } else {
        classify(db, client, config, profile, &profile.name, &api_key, &unlabelled, &app_names, &options).await?;
    }

    // --- Phase 2: Embeddings ---
    // Judged by the vector actually being there: a timestamp on the review
    // drifted from the vec0 table and stranded reviews unembedded forever.
    let unembedded =
        load_reviews(db, &app_ids, "AND r.review_id NOT IN (SELECT review_id FROM review_embeddings)", None)?;
    if unembedded.is_empty() {
        println!("\nAll reviews already embedded.");
    } else {
        embed(db, client, config, &unembedded).await?;
    }

    // --- Phase 3: keyword index ---
    // insert_review keeps FTS current; this catches reviews stored before it did.
    let added = db.conn.execute(
        "INSERT INTO reviews_fts(review_id, title, body)
         SELECT review_id, COALESCE(title, ''), COALESCE(body, '') FROM reviews
         WHERE review_id NOT IN (SELECT review_id FROM reviews_fts)",
        [],
    )?;
    if added > 0 {
        println!("\nFTS5: added {} reviews", added);
    }

    print_label_stats(db, profile, &profile.name)
}

#[allow(clippy::too_many_arguments)]
async fn classify(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    label_key: &str,
    api_key: &str,
    reviews: &[Review],
    app_names: &HashMap<String, String>,
    options: &IndexOptions,
) -> Result<()> {
    let schema = batch_schema(profile.classify_schema()?);
    let system = profile.classify_system_prompt()?;
    let model = &config.openai.classification_model;
    let raw_client = client.raw_client();
    let batches: Vec<&[Review]> = reviews.chunks(options.batch_size.max(1)).collect();

    println!("\n--- Classification (profile: {}) ---", profile.name);
    println!(
        "Reviews: {} | Batches: {} of {} | Concurrency: {} | Model: {}",
        reviews.len(),
        batches.len(),
        options.batch_size,
        options.concurrency,
        model
    );
    // Measured per review on gpt-5-mini: ~150 input, ~330 output tokens.
    let n = reviews.len() as u64;
    match pricing::cost(model, 150 * n, 330 * n) {
        Some(usd) => println!("Estimated cost: ${:.2}", usd),
        None => println!("Estimated cost: unknown (no price for {})", model),
    }

    let mut results = stream::iter(batches)
        .map(|batch| {
            let prompt = format_review_batch(batch, app_names);
            let (raw_client, schema, system) = (&raw_client, &schema, &system);
            async move {
                let result = openai::chat_structured(
                    raw_client,
                    model,
                    config.openai.max_tokens,
                    api_key,
                    system,
                    &prompt,
                    schema,
                    "review_classifications",
                    None,
                )
                .await;
                (batch, result)
            }
        })
        .buffer_unordered(options.concurrency.max(1));

    let mut usage = OpenaiUsage::default();
    let (mut labelled, mut failed, mut done_batches) = (0usize, 0usize, 0usize);
    // Each batch is stored as it lands, so an interrupted run keeps what it paid for.
    while let Some((batch, result)) = results.next().await {
        done_batches += 1;
        let stored = result.and_then(|(json, batch_usage)| {
            usage.add(&batch_usage);
            store_labels(db, label_key, model, batch, &json)
        });
        match stored {
            Ok(n) => {
                labelled += n;
                failed += batch.len() - n;
            }
            Err(e) => {
                tracing::error!("Batch failed: {:#}", e);
                failed += batch.len();
            }
        }
        if done_batches % 50 == 0 {
            tracing::info!("Progress: {} labelled, {} failed", labelled, failed);
        }
    }

    println!("Labelled: {} / {} reviews ({} failed; rerun to retry them)", labelled, reviews.len(), failed);
    println!("Tokens: {} input + {} output ({} calls)", usage.input_tokens, usage.output_tokens, usage.calls);
    println!("Cost: ${:.4}", pricing::cost(model, usage.input_tokens, usage.output_tokens).unwrap_or(0.0));
    Ok(())
}

/// Stores the batch's labels, ignoring any the model invented for review IDs
/// not in the batch. Returns how many were stored.
fn store_labels(db: &Database, label_key: &str, model: &str, batch: &[Review], response_json: &str) -> Result<usize> {
    let parsed: serde_json::Value = serde_json::from_str(response_json)
        .with_context(|| format!("Unparseable classification: {}", truncate(response_json, 300)))?;
    let items = parsed["classifications"].as_array().context("No classifications array")?;
    let batch_ids: HashSet<&str> = batch.iter().map(|r| r.review_id.as_str()).collect();
    let now = Utc::now().to_rfc3339();

    let tx = db.conn.unchecked_transaction()?;
    let mut stored = 0;
    for item in items {
        let Some(review_id) = item["review_id"].as_str().filter(|id| batch_ids.contains(id)) else {
            continue;
        };
        let mut labels = item.clone();
        if let Some(obj) = labels.as_object_mut() {
            obj.remove("review_id");
        }
        tx.execute(
            "INSERT OR REPLACE INTO review_labels (review_id, profile, labels_json, model, classified_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![review_id, label_key, labels.to_string(), model, now],
        )?;
        stored += 1;
    }
    tx.commit()?;
    if stored != batch.len() {
        tracing::warn!("Expected {} classifications, stored {}", batch.len(), stored);
    }
    Ok(stored)
}

/// Wraps a profile's per-review label schema into the batch response shape.
fn batch_schema(labels: serde_json::Value) -> serde_json::Value {
    let mut item = labels;
    let props = item["properties"].as_object_mut().expect("validated by Profile::classify_schema");
    let mut with_id = serde_json::Map::new();
    with_id.insert("review_id".into(), serde_json::json!({ "type": "string" }));
    with_id.extend(std::mem::take(props));
    let required: Vec<serde_json::Value> = with_id.keys().cloned().map(Into::into).collect();
    item["properties"] = with_id.into();
    item["required"] = required.into();

    serde_json::json!({
        "type": "object",
        "properties": { "classifications": { "type": "array", "items": item } },
        "required": ["classifications"],
        "additionalProperties": false
    })
}

async fn embed(db: &Database, client: &RateLimitedClient, config: &AppConfig, reviews: &[Review]) -> Result<()> {
    println!("\n--- Embeddings ---");
    println!("Reviews to embed: {}", reviews.len());

    let num_batches = reviews.len().div_ceil(EMBED_BATCH_SIZE);
    let mut usage = OpenaiUsage::default();
    for (batch_idx, chunk) in reviews.chunks(EMBED_BATCH_SIZE).enumerate() {
        let texts: Vec<String> = chunk.iter().map(embedding_text).collect();
        let (embeddings, batch_usage) = openai::embed_batch(client, config, &texts).await?;
        usage.add(&batch_usage);

        let tx = db.conn.unchecked_transaction()?;
        for (review, embedding) in chunk.iter().zip(&embeddings) {
            // vec0 has no upsert; delete first so a re-embed can't collide.
            tx.execute("DELETE FROM review_embeddings WHERE review_id = ?1", [&review.review_id])?;
            tx.execute(
                "INSERT INTO review_embeddings(review_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![review.review_id, openai::f32_to_bytes(embedding)],
            )?;
        }
        tx.commit()?;
        tracing::info!("Embedded batch {}/{}", batch_idx + 1, num_batches);
    }

    println!("Embedded: {} reviews", reviews.len());
    println!("Tokens: {} ({} calls)", usage.input_tokens, usage.calls);
    let model = &config.openai.embedding_model;
    println!("Cost: ${:.4}", pricing::cost(model, usage.input_tokens, 0).unwrap_or(0.0));
    Ok(())
}

fn embedding_text(r: &Review) -> String {
    match (r.title.as_deref().unwrap_or(""), r.body.as_deref().unwrap_or("")) {
        ("", body) => body.to_string(),
        (title, body) => format!("{}\n{}", title, body),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn query_semantic(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    query: &str,
    limit: usize,
    app_id: Option<&str>,
    profile: &str,
    json_output: bool,
) -> Result<()> {
    let (embeddings, _usage) = openai::embed_batch(client, config, &[query.to_string()]).await?;
    let query_embedding = embeddings.into_iter().next().context("No embedding returned")?;
    let query_bytes = openai::f32_to_bytes(&query_embedding);

    let neighbors: Vec<(String, f64)> = match app_id {
        // Scan just this app's vectors: filtering a global top-k afterwards
        // returned nothing for small apps whose reviews ranked below it.
        Some(app) => {
            let mut stmt = db.conn.prepare(
                "SELECT e.review_id, vec_distance_cosine(e.embedding, ?1) AS distance
                 FROM review_embeddings e JOIN reviews r ON r.review_id = e.review_id
                 WHERE r.app_id = ?2 ORDER BY distance LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![query_bytes, app, limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            rows
        }
        None => {
            let mut stmt = db
                .conn
                .prepare("SELECT review_id, distance FROM review_embeddings WHERE embedding MATCH ?1 AND k = ?2")?;
            let rows = stmt
                .query_map(rusqlite::params![query_bytes, limit as i64], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<Result<_, _>>()?;
            rows
        }
    };

    let review_ids: Vec<String> = neighbors.iter().map(|(id, _)| id.clone()).collect();
    let reviews = queries::get_reviews_by_ids(db, &review_ids)?;
    let review_map: HashMap<&str, &Review> = reviews.iter().map(|r| (r.review_id.as_str(), r)).collect();

    let mut results: Vec<serde_json::Value> = Vec::new();
    for (rid, dist) in &neighbors {
        let Some(review) = review_map.get(rid.as_str()) else { continue };
        let app_name: String = db
            .conn
            .query_row("SELECT COALESCE(name, app_id) FROM apps WHERE app_id = ?1", [&review.app_id], |row| row.get(0))
            .unwrap_or_else(|_| review.app_id.clone());
        let labels: Option<serde_json::Value> = db
            .conn
            .query_row(
                "SELECT labels_json FROM review_labels WHERE review_id = ?1 AND profile = ?2",
                [rid.as_str(), profile],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        results.push(serde_json::json!({
            "review_id": review.review_id,
            "app_id": review.app_id,
            "app_name": app_name,
            "rating": review.rating,
            "title": review.title,
            "body": review.body,
            "date": review.date,
            "distance": dist,
            "labels": labels,
        }));
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    println!("Semantic search: \"{}\" ({} results)\n", query, results.len());
    for r in &results {
        println!(
            "d={:.4} [{}/5] {} — \"{}\"",
            r["distance"].as_f64().unwrap_or(0.0),
            r["rating"].as_i64().unwrap_or(0),
            r["app_name"].as_str().unwrap_or("?"),
            r["title"].as_str().unwrap_or("(no title)"),
        );
        println!("  {}", truncate(r["body"].as_str().unwrap_or(""), 120));
        if let Some(labels) = r["labels"].as_object() {
            let short: Vec<String> = ["themes", "sentiment", "review_intent"]
                .iter()
                .filter_map(|k| labels.get(*k).filter(|v| !v.is_null()).map(|v| format!("{}={}", k, v)))
                .collect();
            if !short.is_empty() {
                println!("  {}", short.join(" | "));
            }
        }
        println!();
    }
    Ok(())
}

/// Reviews with a body for `app_ids`, narrowed by `extra` (which may use ?1 = `param`).
fn load_reviews(db: &Database, app_ids: &[String], extra: &str, param: Option<&str>) -> Result<Vec<Review>> {
    let columns = REVIEW_COLUMNS.split(", ").map(|c| format!("r.{c}")).collect::<Vec<_>>().join(", ");
    let app_param = if param.is_some() { 2 } else { 1 };
    let sql = format!(
        "SELECT {columns} FROM reviews r
         WHERE r.app_id = ?{app_param} AND r.body IS NOT NULL AND r.body != '' {extra}
         ORDER BY r.rating"
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let mut reviews = Vec::new();
    for app_id in app_ids {
        let rows = match param {
            Some(p) => stmt.query_map([p, app_id.as_str()], Review::from_row)?.collect::<Result<Vec<_>, _>>()?,
            None => stmt.query_map([app_id.as_str()], Review::from_row)?.collect::<Result<Vec<_>, _>>()?,
        };
        reviews.extend(rows);
    }
    Ok(reviews)
}

/// Up to `CALIBRATION_PER_APP_RATING` random reviews per star rating from the
/// profile's best-reviewed apps, so the taxonomy is tried across the range.
fn calibration_sample(db: &Database, app_ids: &[String]) -> Result<Vec<Review>> {
    let mut by_count: Vec<(String, i64)> = app_ids
        .iter()
        .map(|id| queries::get_review_count(db, id).map(|n| (id.clone(), n)))
        .collect::<Result<_>>()?;
    by_count.sort_by(|a, b| b.1.cmp(&a.1));

    let sql = format!(
        "SELECT {REVIEW_COLUMNS} FROM (
            SELECT *, ROW_NUMBER() OVER (PARTITION BY rating ORDER BY RANDOM()) AS rn
            FROM reviews WHERE app_id = ?1 AND body IS NOT NULL AND body != ''
         ) WHERE rn <= ?2 ORDER BY rating"
    );
    let mut stmt = db.conn.prepare(&sql)?;
    let mut sample = Vec::new();
    for (app_id, _) in by_count.iter().take(CALIBRATION_MAX_APPS) {
        let rows = stmt.query_map(rusqlite::params![app_id, CALIBRATION_PER_APP_RATING as i64], Review::from_row)?;
        sample.extend(rows.collect::<Result<Vec<_>, _>>()?);
    }
    Ok(sample)
}

fn print_sample_distribution(reviews: &[Review], app_names: &HashMap<String, String>) {
    let mut by_app: HashMap<&str, [usize; 5]> = HashMap::new();
    for r in reviews {
        by_app.entry(&r.app_id).or_insert([0; 5])[(r.rating.clamp(1, 5) - 1) as usize] += 1;
    }
    println!("\nCalibration sample:");
    println!("{:<35} {:>3} {:>3} {:>3} {:>3} {:>3} {:>5}", "App", "1*", "2*", "3*", "4*", "5*", "Tot");
    println!("{}", "-".repeat(60));
    for (app_id, c) in &by_app {
        let name = app_names.get(*app_id).map(String::as_str).unwrap_or(app_id);
        let total: usize = c.iter().sum();
        println!("{:<35} {:>3} {:>3} {:>3} {:>3} {:>3} {:>5}", truncate(name, 33), c[0], c[1], c[2], c[3], c[4], total);
    }
    println!("{}", "-".repeat(60));
    println!("Total: {} reviews\n", reviews.len());
}

fn format_review_batch(reviews: &[Review], app_names: &HashMap<String, String>) -> String {
    let mut buf = String::with_capacity(reviews.len() * 300);
    buf.push_str(&format!(
        "Classify exactly the {} reviews below. Return exactly {} classifications, one per review, in the same order. Do not add, skip, or split any reviews.\n\n",
        reviews.len(),
        reviews.len()
    ));
    for (i, r) in reviews.iter().enumerate() {
        let app = app_names.get(&r.app_id).map(String::as_str).unwrap_or(&r.app_id);
        buf.push_str(&format!(
            "[{}] review_id={} | app={} | rating={}/5\nTitle: {}\n{}\n\n",
            i + 1,
            r.review_id,
            app,
            r.rating,
            r.title.as_deref().unwrap_or(""),
            r.body.as_deref().unwrap_or("")
        ));
    }
    buf
}

/// Distribution of every label in the profile's schema, read from the stored
/// JSON. Free-text labels have a long tail, so only their head is shown.
fn print_label_stats(db: &Database, profile: &Profile, label_key: &str) -> Result<()> {
    let n: i64 =
        db.conn.query_row("SELECT COUNT(*) FROM review_labels WHERE profile = ?1", [label_key], |row| row.get(0))?;
    println!("\n=== Labels for '{}' ({} reviews) ===", label_key, n);
    if n == 0 {
        return Ok(());
    }

    let schema = profile.classify_schema()?;
    for (field, spec) in schema["properties"].as_object().into_iter().flatten() {
        let is_array = spec["type"] == "array";
        let values = if is_array { &spec["items"] } else { spec };
        let has_enum = values.get("enum").is_some()
            || values["anyOf"].as_array().is_some_and(|a| a.iter().any(|v| v.get("enum").is_some()));
        let is_text = values["type"] == "string" || values.get("anyOf").is_some();
        let cap = if is_text && !has_enum { 10 } else { 50 };

        let sql = if is_array {
            "SELECT j.value, COUNT(*) FROM review_labels l, json_each(l.labels_json, '$.' || ?2) j
             WHERE l.profile = ?1 GROUP BY j.value ORDER BY COUNT(*) DESC LIMIT ?3"
        } else {
            "SELECT json_extract(l.labels_json, '$.' || ?2), COUNT(*) FROM review_labels l
             WHERE l.profile = ?1 GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT ?3"
        };
        let mut stmt = db.conn.prepare(sql)?;
        let rows: Vec<(rusqlite::types::Value, i64)> = stmt
            .query_map(rusqlite::params![label_key, field, cap], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;

        println!("\n{}{}:", field.to_uppercase(), if cap == 10 { " (top 10)" } else { "" });
        for (value, count) in rows {
            let label = match value {
                rusqlite::types::Value::Null => "null".to_string(),
                rusqlite::types::Value::Integer(i) => i.to_string(),
                rusqlite::types::Value::Real(f) => f.to_string(),
                rusqlite::types::Value::Text(t) => truncate(&t, 40),
                rusqlite::types::Value::Blob(_) => "?".to_string(),
            };
            println!("  {:<42} {:>5} ({:>5.1}%)", label, count, count as f64 / n as f64 * 100.0);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::batch_schema;

    #[test]
    fn batch_schema_adds_required_review_id() {
        let labels = serde_json::json!({
            "type": "object",
            "properties": { "sentiment": { "type": "string", "enum": ["positive", "negative"] } },
            "required": ["sentiment"],
            "additionalProperties": false
        });
        let schema = batch_schema(labels);
        let item = &schema["properties"]["classifications"]["items"];
        assert_eq!(item["properties"]["review_id"]["type"], "string");
        assert_eq!(item["required"], serde_json::json!(["review_id", "sentiment"]));
        assert_eq!(item["additionalProperties"], false);
    }
}
