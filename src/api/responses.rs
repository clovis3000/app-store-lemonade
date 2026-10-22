use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Serialize)]
pub struct Pagination {
    pub page: u32,
    pub per_page: u32,
    pub total: u32,
    pub total_pages: u32,
}

#[derive(Serialize)]
pub struct AppSummary {
    pub app_id: String,
    pub name: String,
    pub developer: Option<String>,
    pub category: Option<String>,
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub price: Option<f64>,
    pub formatted_price: Option<String>,
    pub monetisation_model: Option<String>,
    pub last_updated: Option<String>,
    pub opportunity_score: Option<f64>,
    pub demand_signal: Option<f64>,
    pub dissatisfaction_signal: Option<f64>,
    pub neglect_signal: Option<f64>,
    pub feasibility_tier: Option<i32>,
    pub recommended_action: Option<String>,
    pub review_count: i64,
    pub starred: bool,
}

impl AppSummary {
    pub fn from_app(app: &crate::models::App, review_count: i64) -> Self {
        Self {
            app_id: app.app_id.clone(),
            name: app.name.clone(),
            developer: app.developer.clone(),
            category: app.category.clone(),
            rating: app.rating,
            rating_count: app.rating_count,
            price: app.price,
            formatted_price: app.formatted_price.clone(),
            monetisation_model: app.monetisation_model.clone(),
            last_updated: app.last_updated.clone(),
            opportunity_score: app.opportunity_score,
            demand_signal: app.demand_signal,
            dissatisfaction_signal: app.dissatisfaction_signal,
            neglect_signal: app.neglect_signal,
            feasibility_tier: app.feasibility_tier,
            recommended_action: app.recommended_action.clone(),
            review_count,
            starred: app.starred,
        }
    }
}

#[derive(Serialize)]
pub struct AppDetail {
    pub app_id: String,
    pub name: String,
    pub developer: Option<String>,
    pub category: Option<String>,
    pub platform: String,
    pub rating: Option<f64>,
    pub rating_count: Option<i64>,
    pub price: Option<f64>,
    pub formatted_price: Option<String>,
    pub monetisation_model: Option<String>,
    pub last_updated: Option<String>,
    pub original_release: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub app_store_url: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub opportunity_score: Option<f64>,
    pub demand_signal: Option<f64>,
    pub dissatisfaction_signal: Option<f64>,
    pub neglect_signal: Option<f64>,
    pub diagnosis: Option<serde_json::Value>,
    pub review_count: i64,
    pub starred: bool,
    pub review_summary: ReviewSummary,
}

#[derive(Serialize)]
pub struct ReviewSummary {
    pub total: i64,
    pub by_rating: HashMap<i32, i64>,
    pub by_country: HashMap<String, i64>,
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub pipeline: PipelineFunnel,
    pub categories: Vec<CategoryStat>,
    pub score_distribution: Vec<ScoreBucket>,
    pub top_opportunities: Vec<AppSummary>,
}

#[derive(Serialize)]
pub struct PipelineFunnel {
    pub total_apps: u32,
    pub scored: u32,
    pub diagnosed: u32,
    pub actionable: u32,
    pub total_reviews: u32,
    pub starred: u32,
}

#[derive(Serialize)]
pub struct CategoryStat {
    pub category: String,
    pub count: u32,
    pub avg_rating: f64,
    pub scored_count: u32,
}

#[derive(Serialize)]
pub struct ScoreBucket {
    pub range: String,
    pub count: u32,
}
