use anyhow::Result;

use crate::models::{App, Review};
use crate::profile::Profile;

pub fn render_review_prompt(profile: &Profile, app: &App, reviews: &[Review]) -> Result<String> {
    let mut context = app_context(app);
    insert_reviews(&mut context, reviews)?;
    profile.render("review_analysis.tera", &context)
}

pub fn render_competitive_prompt(profile: &Profile, app: &App, reviews: &[Review]) -> Result<String> {
    let mut context = app_context(app);
    insert_reviews(&mut context, reviews)?;
    profile.render("competitive_research.tera", &context)
}

pub fn render_cross_app_synthesis_prompt(profile: &Profile, app_summaries: &[serde_json::Value]) -> Result<String> {
    let mut context = tera::Context::new();
    context.insert("apps", app_summaries);
    context.insert("app_count", &app_summaries.len());
    profile.render("cross_app_synthesis.tera", &context)
}

pub fn render_synthesis_prompt(
    profile: &Profile,
    app: &App,
    raw_results_json: &str,
    batch_count: usize,
) -> Result<String> {
    let mut context = app_context(app);
    context.insert("raw_results", raw_results_json);
    context.insert("batch_count", &batch_count);
    profile.render("synthesis.tera", &context)
}

pub fn render_followup_prompt(
    profile: &Profile,
    app: &App,
    reviews: &[Review],
    parent_analysis_json: &str,
    followup_question: &str,
) -> Result<String> {
    let mut context = app_context(app);
    context.insert("parent_analysis", parent_analysis_json);
    context.insert("followup_question", followup_question);
    insert_reviews(&mut context, reviews)?;
    profile.render("followup.tera", &context)
}

fn app_context(app: &App) -> tera::Context {
    let mut context = tera::Context::new();
    context.insert("app_name", &app.name);
    context.insert("app_category", &app.category.as_deref().unwrap_or("Unknown"));
    context.insert("app_rating", &app.rating.unwrap_or(0.0));
    context.insert("app_rating_count", &app.rating_count.unwrap_or(0));
    context
}

fn insert_reviews(context: &mut tera::Context, reviews: &[Review]) -> Result<()> {
    let review_data: Vec<serde_json::Value> = reviews
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.review_id,
                "rating": r.rating,
                "title": r.title.as_deref().unwrap_or(""),
                "body": r.body.as_deref().unwrap_or(""),
                "date": r.date.as_deref().unwrap_or(""),
                "version": r.version.as_deref().unwrap_or(""),
            })
        })
        .collect();
    context.insert("reviews_json", &serde_json::to_string_pretty(&review_data)?);
    context.insert("review_count", &reviews.len());
    Ok(())
}
