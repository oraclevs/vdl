mod download;
mod routes;
mod state;

use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;

use state::AppState;

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/config", get(routes::get_config).put(routes::put_config))
        .route("/api/fetch-metadata", post(routes::fetch_metadata))
        .route("/api/download", post(routes::start_download))
        .route("/api/download/{session_id}", delete(routes::cancel_download))
        .route("/api/downloads", get(routes::list_downloads))
        .with_state(state)
}
