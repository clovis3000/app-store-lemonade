mod handlers;
pub mod responses;

use anyhow::Result;
use axum::Router;
use axum::routing::{get, post};
use std::sync::{Arc, Mutex};
use tower_http::services::ServeDir;

use crate::db::Database;

pub struct AppState {
    pub db: Arc<Mutex<Database>>,
}

pub async fn serve(database: Database, root: &std::path::Path, host: String, port: u16) -> Result<()> {
    let state = Arc::new(AppState {
        db: Arc::new(Mutex::new(database)),
    });

    let web_dist = root.join("web/dist");
    let index_html = web_dist.join("index.html");
    tracing::info!("Serving static files from {}", web_dist.display());

    let app_detail_routes = Router::new()
        .route("/", get(handlers::get_app))
        .route("/reviews", get(handlers::get_reviews))
        .route("/star", post(handlers::toggle_star));

    let api_routes = Router::new()
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/apps", get(handlers::list_apps))
        .nest("/api/apps/{app_id}", app_detail_routes);

    let app = api_routes
        .with_state(state)
        .fallback_service(ServeDir::new(&web_dist).fallback(
            tower_http::services::ServeFile::new(&index_html),
        ));

    let addr = format!("{host}:{port}");
    tracing::info!("Starting web server at http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
