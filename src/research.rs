use anyhow::{bail, Result};
use futures::stream::{self, StreamExt};

use crate::cli::ResearchArgs;
use crate::config::{AppConfig, ModelRole};
use crate::db::{queries, Database};
use crate::diagnose::llm::{self, LlmUsage};
use crate::diagnose::reviews;
use crate::http::RateLimitedClient;
use crate::models::{App, CompetitiveResearchResult, Review};
use crate::profile::{self, Profile};
use crate::text::truncate;

// Rough token estimates (empirical, pre-thinking; thinking adds output).
const TOKENS_PER_REVIEW: usize = 150;
const PROMPT_OVERHEAD_TOKENS: usize = 800;
const OUTPUT_TOKENS_PER_BATCH: usize = 4000;
const SYNTHESIS_INPUT_TOKENS: usize = 10000;
const SYNTHESIS_OUTPUT_TOKENS: usize = 4000;
const CROSS_APP_INPUT_PER_APP: usize = 5000;
const CROSS_APP_OUTPUT_TOKENS: usize = 5000;
const SECS_PER_LLM_CALL: f64 = 35.0;
const DEFAULT_TOP: usize = 8;

/// Top-level dispatch for `lemonade research` — routes to the right mode based on flags.
pub async fn dispatch(db: &Database, client: &RateLimitedClient, config: &AppConfig, args: ResearchArgs) -> Result<()> {
    if args.list {
        return match &args.app_id {
            Some(id) => list_analyses(db, id),
            None => list_all_syntheses(db),
        };
    }

    if args.followup.is_some() || args.parent.is_some() {
        let (Some(id), Some(question), Some(parent_id)) = (&args.app_id, &args.followup, args.parent) else {
            bail!("--followup needs --app-id and --parent");
        };
        llm::require_api_key()?;
        let app = queries::get_app(db, id)?.ok_or_else(|| anyhow::anyhow!("App not found: {}", id))?;
        return run_followup(db, client, config, args.profile.as_deref(), &app, question, parent_id).await;
    }

    let profile = Profile::load(config, args.profile.as_deref().unwrap_or(profile::GENERAL))?;
    let apps = resolve_apps(db, &profile, args.app_id.as_deref(), args.search.as_deref(), args.top)?;
    if apps.is_empty() {
        bail!("No apps found. Try different search terms or harvest first (`lemonade harvest --search-only`).");
    }

    let estimate = estimate_cost(db, config, &apps, args.max_reviews, args.batch_size)?;
    print_estimate(&estimate, &profile);
    if args.plan {
        return Ok(());
    }
    llm::require_api_key()?;

    // Each analysis stays paired with its own app: apps that fail drop out,
    // so positions in `apps` no longer line up with the analyses.
    let mut researched: Vec<(i64, &App)> = Vec::new();
    let mut total_usage = LlmUsage::default();

    for app in &apps {
        match run_initial(db, client, config, &profile, app, args.max_reviews, args.batch_size).await {
            Ok((aid, usage)) => {
                researched.push((aid, app));
                total_usage.add(&usage);
            }
            Err(e) => tracing::error!("Failed to research {} ({}): {:#}", app.name, app.app_id, e),
        }
    }
    if researched.is_empty() {
        bail!("Every app failed; nothing saved. See the errors above.");
    }
    if researched.len() < apps.len() {
        println!("Researched {} of {} apps; see errors above.", researched.len(), apps.len());
    }

    if apps.len() > 1 && !researched.is_empty() {
        let synthesis_name = args.name.as_deref().unwrap_or("Untitled synthesis");
        let usage = run_cross_app_synthesis(db, client, config, &profile, &researched, synthesis_name).await?;
        total_usage.add(&usage);
    }

    print_actual_cost(&total_usage);
    Ok(())
}

/// The apps a command works on: an explicit --app-id, else --search terms,
/// else the profile's own [apps] selection.
pub fn resolve_apps(
    db: &Database,
    profile: &Profile,
    app_id: Option<&str>,
    search: Option<&[String]>,
    top: Option<usize>,
) -> Result<Vec<App>> {
    if let Some(id) = app_id {
        let app = queries::get_app(db, id)?.ok_or_else(|| anyhow::anyhow!("App not found: {}", id))?;
        return Ok(vec![app]);
    }

    let selection = &profile.manifest.apps;
    let terms: Vec<&str> = match search {
        Some(terms) => terms.iter().map(String::as_str).collect(),
        None => selection.search.iter().map(String::as_str).collect(),
    };
    if terms.is_empty() && (search.is_some() || selection.ids.is_empty()) {
        bail!("Provide --app-id or --search, or give profile '{}' an [apps] section", profile.name);
    }

    let mut apps = queries::search_apps(db, &terms, top.or(selection.top).unwrap_or(DEFAULT_TOP))?;
    if search.is_none() {
        for id in &selection.ids {
            if !apps.iter().any(|a| &a.app_id == id) {
                match queries::get_app(db, id)? {
                    Some(app) => apps.push(app),
                    None => tracing::warn!("Profile app {} is not in the DB; harvest it first", id),
                }
            }
        }
    }

    tracing::info!("Found {} apps", apps.len());
    for app in &apps {
        tracing::info!(
            "  {} ({}) — {}/5, {} ratings",
            app.name,
            app.app_id,
            app.rating.map_or("?".to_string(), |r| format!("{:.1}", r)),
            app.rating_count.unwrap_or(0),
        );
    }
    Ok(apps)
}

// --- Cost estimation ---

struct CostEstimate {
    apps: Vec<AppEstimate>,
    total_llm_calls: usize,
    total_input_tokens: usize,
    total_output_tokens: usize,
    total_cost_usd: f64,
    scrape_requests: usize,
    scrape_time_secs: f64,
    llm_time_secs: f64,
}

struct AppEstimate {
    name: String,
    reviews_in_db: i64,
    needs_scrape: bool,
    reviews_to_analyse: usize,
    llm_calls: usize, // batches + synthesis if >1 batch
}

fn estimate_cost(
    db: &Database,
    config: &AppConfig,
    apps: &[App],
    max_reviews: usize,
    batch_size: usize,
) -> Result<CostEstimate> {
    let staleness_days = config.research.review_staleness_days;
    let countries = config.countries.len();
    let pages_per_country: usize = 10;

    let mut app_estimates = Vec::new();
    let mut total_scrape_requests: usize = 0;
    let (mut analysis_in, mut analysis_out) = (0usize, 0usize);

    for app in apps {
        let reviews_in_db = queries::get_review_count(db, &app.app_id)?;
        let needs_scrape = config.countries.iter().any(|c| {
            !queries::has_fresh_scrape(db, &app.app_id, "rss_reviews", c, Some(staleness_days)).unwrap_or(false)
        });
        if needs_scrape {
            // Worst case: all countries, all pages
            total_scrape_requests += countries * pages_per_country;
        }

        let available = if needs_scrape && reviews_in_db == 0 { countries * 500 } else { reviews_in_db as usize };
        let reviews_to_analyse = available.min(max_reviews);
        let batches = reviews_to_analyse.div_ceil(batch_size);

        analysis_in += batches * (PROMPT_OVERHEAD_TOKENS + batch_size.min(reviews_to_analyse) * TOKENS_PER_REVIEW);
        analysis_out += batches * OUTPUT_TOKENS_PER_BATCH;
        if batches > 1 {
            analysis_in += SYNTHESIS_INPUT_TOKENS;
            analysis_out += SYNTHESIS_OUTPUT_TOKENS;
        }

        app_estimates.push(AppEstimate {
            name: app.name.clone(),
            reviews_in_db,
            needs_scrape,
            reviews_to_analyse,
            llm_calls: if batches > 1 { batches + 1 } else { batches },
        });
    }

    let cross_app = apps.len() > 1;
    let (synth_in, synth_out) =
        if cross_app { (apps.len() * CROSS_APP_INPUT_PER_APP, CROSS_APP_OUTPUT_TOKENS) } else { (0, 0) };

    let cost = |role: ModelRole, input: usize, output: usize| {
        let model = config.llm.for_role(role).model;
        llm::price_per_mtok(&model).map_or(0.0, |(i, o)| (input as f64 * i + output as f64 * o) / 1_000_000.0)
    };

    let total_llm_calls = app_estimates.iter().map(|a| a.llm_calls).sum::<usize>() + usize::from(cross_app);
    // Batches for one app run `concurrency` at a time.
    let llm_rounds: usize = app_estimates
        .iter()
        .map(|a| a.llm_calls.div_ceil(config.llm.concurrency.max(1)))
        .sum::<usize>()
        + usize::from(cross_app);

    Ok(CostEstimate {
        apps: app_estimates,
        total_llm_calls,
        total_input_tokens: analysis_in + synth_in,
        total_output_tokens: analysis_out + synth_out,
        total_cost_usd: cost(ModelRole::Analysis, analysis_in, analysis_out)
            + cost(ModelRole::Synthesis, synth_in, synth_out),
        scrape_requests: total_scrape_requests,
        scrape_time_secs: total_scrape_requests as f64 * config.rate_limits.itunes_delay_ms as f64 / 1000.0,
        llm_time_secs: llm_rounds as f64 * SECS_PER_LLM_CALL,
    })
}

fn print_estimate(est: &CostEstimate, profile: &Profile) {
    println!("=== Research Plan (profile: {}) ===\n", profile.name);
    println!("{:<30} {:>8} {:>7} {:>8} {:>5}", "App", "Reviews", "Scrape?", "Analyse", "LLM");
    println!("{}", "-".repeat(62));
    for ae in &est.apps {
        println!(
            "{:<30} {:>8} {:>7} {:>8} {:>5}",
            truncate(&ae.name, 30),
            ae.reviews_in_db,
            if ae.needs_scrape { "yes" } else { "no" },
            ae.reviews_to_analyse,
            ae.llm_calls,
        );
    }
    println!("{}", "-".repeat(62));
    println!(
        "\nLLM calls:  {} ({:.0}k input + {:.0}k output tokens)",
        est.total_llm_calls,
        est.total_input_tokens as f64 / 1000.0,
        est.total_output_tokens as f64 / 1000.0,
    );
    println!("Est. cost:  ${:.2}", est.total_cost_usd);
    if est.scrape_requests > 0 {
        println!(
            "Scraping:   up to {} requests (~{:.0}m{:.0}s)",
            est.scrape_requests,
            (est.scrape_time_secs / 60.0).floor(),
            est.scrape_time_secs % 60.0,
        );
    }
    println!(
        "LLM time:   ~{:.0}m{:.0}s",
        (est.llm_time_secs / 60.0).floor(),
        est.llm_time_secs % 60.0,
    );
    println!();
}

fn print_actual_cost(usage: &LlmUsage) {
    println!(
        "\n=== Actual Cost ===\nLLM calls:  {}\nTokens:     {} input + {} output\nCost:       ${:.4}\n",
        usage.calls, usage.input_tokens, usage.output_tokens, usage.cost_usd,
    );
}

// --- Single-app analysis (returns analysis ID) ---

async fn run_initial(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    app: &App,
    max_reviews: usize,
    batch_size: usize,
) -> Result<(i64, LlmUsage)> {
    tracing::info!("Research: {} ({})", app.name, app.app_id);

    let staleness = Some(config.research.review_staleness_days);
    let review_count =
        reviews::scrape_reviews_with_staleness(db, client, &app.app_id, &config.countries, staleness).await?;
    tracing::info!("  {} reviews stored", review_count);
    if review_count == 0 {
        bail!("No reviews found for {}", app.app_id);
    }

    let mut all_reviews = queries::get_reviews(db, &app.app_id)?;
    all_reviews.truncate(max_reviews);

    let batches: Vec<&[Review]> = all_reviews.chunks(batch_size.max(1)).collect();
    let num_batches = batches.len();
    tracing::info!(
        "  Analysing {} reviews in {} batch(es), {} at a time",
        all_reviews.len(),
        num_batches,
        config.llm.concurrency
    );

    let outcomes: Vec<_> = stream::iter(batches)
        .map(|batch| async move {
            let outcome = llm::competitive_research(client, config, profile, app, batch).await;
            (batch, outcome)
        })
        .buffered(config.llm.concurrency.max(1))
        .collect()
        .await;

    let mut batch_results = Vec::new();
    // Only reviews the model actually read; a follow-up loads exactly these.
    let mut review_ids: Vec<String> = Vec::new();
    let mut usage = LlmUsage::default();
    for (batch, outcome) in outcomes {
        let (result, batch_usage) = outcome?;
        usage.add(&batch_usage);
        if let Some(r) = result {
            batch_results.push(r);
            review_ids.extend(batch.iter().map(|r| r.review_id.clone()));
        }
    }

    if batch_results.is_empty() {
        bail!("All LLM batches failed for {}", app.app_id);
    }
    if batch_results.len() < num_batches {
        tracing::warn!(
            "  {} of {} batches failed; analysis covers {} reviews",
            num_batches - batch_results.len(),
            num_batches,
            review_ids.len()
        );
    }

    let result = if batch_results.len() == 1 {
        batch_results.into_iter().next().unwrap()
    } else {
        let nb = batch_results.len();
        let raw_merged = CompetitiveResearchResult::merge(batch_results);
        let raw_json = serde_json::to_string(&raw_merged)?;

        tracing::info!("  Synthesizing {} batches...", nb);
        let (synth_result, synth_usage) =
            llm::synthesize_batches(client, config, profile, app, &raw_json, nb).await?;
        usage.add(&synth_usage);
        synth_result.unwrap_or_else(|| {
            tracing::warn!("  Synthesis failed, falling back to mechanical merge");
            raw_merged
        })
    };

    let json = serde_json::to_string(&result)?;
    let analysis_id = queries::save_analysis(
        db,
        &app.app_id,
        "research",
        &profile.name,
        None,
        &review_ids,
        &json,
        &config.llm.analysis_model,
    )?;

    tracing::info!(
        "  Analysis #{} saved: {} praised, {} hated, {} wishlist, {} underserved (${:.4})",
        analysis_id,
        result.praised_features.len(),
        result.hated_features.len(),
        result.wishlist.len(),
        result.underserved_segments.len(),
        usage.cost_usd,
    );
    println!("Analysis saved: id={} app={}", analysis_id, app.name);
    Ok((analysis_id, usage))
}

// --- Cross-app synthesis ---

async fn run_cross_app_synthesis(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    profile: &Profile,
    researched: &[(i64, &App)],
    name: &str,
) -> Result<LlmUsage> {
    tracing::info!("Running cross-app synthesis: \"{}\" on {}", name, config.llm.synthesis_model);
    let analysis_ids: Vec<i64> = researched.iter().map(|(aid, _)| *aid).collect();
    let app_ids: Vec<String> = researched.iter().map(|(_, app)| app.app_id.clone()).collect();

    let mut app_summaries = Vec::new();
    for (aid, app) in researched {
        if let Some(a) = queries::get_analysis(db, *aid)? {
            app_summaries.push(serde_json::json!({
                "name": app.name,
                "rating": app.rating.unwrap_or(0.0),
                "rating_count": app.rating_count.unwrap_or(0),
                "analysis": a.result_json.unwrap_or_default(),
            }));
        }
    }
    if app_summaries.is_empty() {
        bail!("No analyses to synthesize");
    }

    let (result, usage) = llm::cross_app_synthesis(client, config, profile, &app_summaries).await?;
    match result {
        Some(synthesis) => {
            let json = serde_json::to_string(&synthesis)?;
            let synthesis_id = queries::save_synthesis(
                db,
                name,
                &profile.name,
                &analysis_ids,
                &app_ids,
                &json,
                &config.llm.synthesis_model,
            )?;
            println!("\nSynthesis saved: id={} name=\"{}\"", synthesis_id, name);
            println!("\n{}", synthesis.market_overview);
            println!(
                "\n{} universal complaints, {} feature gaps, {} underserved opportunities",
                synthesis.universal_complaints.len(),
                synthesis.feature_gaps.len(),
                synthesis.underserved_opportunities.len(),
            );
        }
        None => tracing::error!("Cross-app synthesis LLM call failed"),
    }
    Ok(usage)
}

// --- Followup ---

async fn run_followup(
    db: &Database,
    client: &RateLimitedClient,
    config: &AppConfig,
    profile_name: Option<&str>,
    app: &App,
    question: &str,
    parent_id: i64,
) -> Result<()> {
    let parent = queries::get_analysis(db, parent_id)?
        .ok_or_else(|| anyhow::anyhow!("Analysis not found: #{}", parent_id))?;
    if parent.app_id != app.app_id {
        bail!("Parent analysis #{} is for app {}, not {}", parent_id, parent.app_id, app.app_id);
    }
    // A follow-up keeps asking in the parent's frame unless told otherwise.
    let profile = Profile::load(config, profile_name.unwrap_or(&parent.profile))?;

    let parent_json = parent
        .result_json
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Parent analysis #{} has no result", parent_id))?;

    let review_ids: Vec<String> = match &parent.review_ids {
        Some(ids_json) => serde_json::from_str(ids_json)?,
        None => Vec::new(),
    };
    let reviews_for_followup = if review_ids.is_empty() {
        tracing::warn!("  Parent analysis has no review IDs, loading all reviews");
        queries::get_reviews(db, &app.app_id)?
    } else {
        queries::get_reviews_by_ids(db, &review_ids)?
    };

    tracing::info!(
        "  Followup on analysis #{} with {} reviews on {}: \"{}\"",
        parent_id,
        reviews_for_followup.len(),
        config.llm.synthesis_model,
        question
    );

    let (result, usage) =
        llm::followup_analysis(client, config, &profile, app, &reviews_for_followup, parent_json, question).await?;
    let Some(followup) = result else {
        bail!("LLM followup call failed");
    };

    let json = serde_json::to_string(&followup)?;
    let used_ids: Vec<String> = followup.evidence.iter().map(|e| e.review_id.clone()).collect();
    let analysis_id = queries::save_analysis(
        db,
        &app.app_id,
        "followup",
        &profile.name,
        Some(parent_id),
        &used_ids,
        &json,
        &config.llm.synthesis_model,
    )?;

    println!("\n{}", followup.answer);
    if !followup.further_questions.is_empty() {
        println!("\nSuggested follow-ups:");
        for q in &followup.further_questions {
            println!("  - {}", q);
        }
    }
    print_actual_cost(&usage);
    println!("Analysis saved: id={}", analysis_id);
    Ok(())
}

// --- List modes ---

pub fn list_analyses(db: &Database, app_id: &str) -> Result<()> {
    let analyses = queries::list_analyses_for_app(db, app_id)?;
    if analyses.is_empty() {
        println!("No analyses found for {}", app_id);
        return Ok(());
    }
    for a in &analyses {
        let kind = a.prompt_name.as_deref().unwrap_or("?");
        let parent = a.parent_id.map(|p| format!(" (followup on #{})", p)).unwrap_or_default();
        let created = a.created_at.as_deref().unwrap_or("?");
        let preview = json_preview(a.result_json.as_deref(), &["overall_summary", "answer"]);
        println!("#{:>4}  {:<10} [{}] {}{}", a.id, kind, a.profile, created, parent);
        if !preview.is_empty() {
            println!("       {}", preview);
        }
    }
    Ok(())
}

fn list_all_syntheses(db: &Database) -> Result<()> {
    let syntheses = queries::list_syntheses(db)?;
    if syntheses.is_empty() {
        println!("No syntheses found");
        return Ok(());
    }
    for s in &syntheses {
        let created = s.created_at.as_deref().unwrap_or("?");
        let app_count = s
            .app_ids
            .as_deref()
            .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
            .map_or(0, |v| v.len());
        let preview = json_preview(s.result_json.as_deref(), &["market_overview"]);
        println!("#{:>4}  \"{}\" [{}]  {} apps  {}", s.id, s.name, s.profile, app_count, created);
        if !preview.is_empty() {
            println!("       {}", preview);
        }
    }
    Ok(())
}

/// The first of `keys` found in a stored result, cut to one line.
fn json_preview(json: Option<&str>, keys: &[&str]) -> String {
    json.and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
        .and_then(|v| keys.iter().find_map(|k| v.get(*k).and_then(|s| s.as_str()).map(|s| truncate(s, 80))))
        .unwrap_or_default()
}
