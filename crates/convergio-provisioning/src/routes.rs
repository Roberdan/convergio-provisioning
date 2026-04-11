//! HTTP API routes for convergio-provisioning.

use axum::Router;

/// Returns the router for this crate's API endpoints.
pub fn routes() -> Router {
    Router::new()
    // .route("/api/provisioning/health", get(health))
}
