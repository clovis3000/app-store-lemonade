use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use super::responses::*;
use super::AppState;
use crate::db::queries;

#[derive(Deserialize)]
pub struct ListAppsParams {
    pub filter: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub sort: Option<String>,
    pub order: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct ListReviewsParams {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub country: Option<String>,
    pub rating: Option<i32>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

pub async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<StatsResponse>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let pipeline = queries::get_pipeline_stats(&db)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let categories = queries::get_category_stats(&db)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let score_distribution = queries::get_score_distribution(&db)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let top_opportunities = queries::get_top_opportunities(&db, 10)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok::<_, StatusCode>(StatsResponse {
            pipeline,
            categories,
            score_distribution,
            top_opportunities,
        })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(result))
}

pub async fn list_apps(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListAppsParams>,
) -> Result<Json<PaginatedResponse<AppSummary>>, StatusCode> {
    let db = state.db.clone();
    let filter = params.filter.unwrap_or_else(|| "all".into());
    let sort = params.sort.unwrap_or_else(|| "score".into());
    let order = params.order.unwrap_or_else(|| "desc".into());
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);
    let category = params.category;
    let search = params.search;

    let result = tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let (apps, total) = queries::list_apps_paginated(
            &db, &filter, &sort, &order,
            category.as_deref(), search.as_deref(),
            page, per_page,
        ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok::<_, StatusCode>(PaginatedResponse {
            data: apps,
            pagination: Pagination {
                page,
                per_page,
                total,
                total_pages: total.div_ceil(per_page),
            },
        })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(result))
}

pub async fn get_app(
    State(state): State<Arc<AppState>>,
    Path(app_id): Path<String>,
) -> Result<Json<AppDetail>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let app = queries::get_app(&db, &app_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        let review_summary = queries::get_review_summary(&db, &app.app_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let review_count = review_summary.total;

        let diagnosis: Option<serde_json::Value> = app.diagnosis_json.as_ref().and_then(|json| {
            serde_json::from_str(json).ok()
        });

        Ok::<_, StatusCode>(AppDetail {
            app_id: app.app_id,
            name: app.name,
            developer: app.developer,
            category: app.category,
            platform: app.platform,
            rating: app.rating,
            rating_count: app.rating_count,
            price: app.price,
            formatted_price: app.formatted_price,
            monetisation_model: app.monetisation_model,
            last_updated: app.last_updated,
            original_release: app.original_release,
            version: app.version,
            description: app.description,
            app_store_url: app.app_store_url,
            file_size_bytes: app.file_size_bytes,
            opportunity_score: app.opportunity_score,
            demand_signal: app.demand_signal,
            dissatisfaction_signal: app.dissatisfaction_signal,
            neglect_signal: app.neglect_signal,
            diagnosis,
            review_count,
            starred: app.starred,
            review_summary,
        })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(result))
}

pub async fn get_reviews(
    State(state): State<Arc<AppState>>,
    Path(app_id): Path<String>,
    Query(params): Query<ListReviewsParams>,
) -> Result<Json<PaginatedResponse<crate::models::Review>>, StatusCode> {
    let db = state.db.clone();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).clamp(1, 200);
    let sort = params.sort.unwrap_or_else(|| "date".into());
    let order = params.order.unwrap_or_else(|| "desc".into());
    let country = params.country;
    let rating = params.rating;

    let result = tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let (reviews, total) = queries::get_reviews_paginated(
            &db, &app_id,
            country.as_deref(), rating,
            &sort, &order,
            page, per_page,
        ).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok::<_, StatusCode>(PaginatedResponse {
            data: reviews,
            pagination: Pagination {
                page,
                per_page,
                total,
                total_pages: if total == 0 { 1 } else { total.div_ceil(per_page) },
            },
        })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(result))
}

pub async fn toggle_star(
    State(state): State<Arc<AppState>>,
    Path(app_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = db.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let starred = queries::toggle_star(&db, &app_id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok::<_, StatusCode>(serde_json::json!({ "starred": starred }))
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)??;
    Ok(Json(result))
}
