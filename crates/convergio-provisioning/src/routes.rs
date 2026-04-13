//! HTTP routes for node provisioning.
//!
//! - POST /api/provision/peer     — trigger provisioning for a peer
//! - GET  /api/provision/runs     — list provisioning runs
//! - GET  /api/provision/run/:id  — get a specific run with items

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use convergio_db::pool::ConnPool;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::provision::provision_peer;
use crate::types::ProvisionRequest;

/// Characters that could trigger shell interpretation on the remote side.
fn has_shell_metachar(s: &str) -> bool {
    s.contains(|c: char| {
        matches!(
            c,
            ';' | '&' | '|' | '$' | '`' | '\'' | '"' | '\\' | '\n' | '\r'
        )
    })
}

/// Validate provisioning request fields to prevent injection/traversal.
pub fn validate_request(req: &ProvisionRequest) -> Result<(), String> {
    // peer_name: alphanumeric, dash, underscore, dot only
    if req.peer_name.is_empty() || req.peer_name.len() > 128 {
        return Err("peer_name must be 1-128 characters".into());
    }
    if !req
        .peer_name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err("peer_name must contain only alphanumeric, dash, underscore, or dot".into());
    }

    // ssh_target: length + no shell metacharacters
    if req.ssh_target.is_empty() || req.ssh_target.len() > 256 {
        return Err("ssh_target must be 1-256 characters".into());
    }
    if has_shell_metachar(&req.ssh_target) {
        return Err("ssh_target contains invalid characters".into());
    }

    // remote_base: length, no traversal, no shell metacharacters
    if req.remote_base.is_empty() || req.remote_base.len() > 512 {
        return Err("remote_base must be 1-512 characters".into());
    }
    if req.remote_base.contains("..") {
        return Err("remote_base must not contain path traversal".into());
    }
    if has_shell_metachar(&req.remote_base) {
        return Err("remote_base contains invalid characters".into());
    }

    Ok(())
}

pub struct ProvisionState {
    pub pool: ConnPool,
}

pub fn provision_routes(state: Arc<ProvisionState>) -> Router {
    Router::new()
        .route("/api/provision/peer", post(handle_provision))
        .route("/api/provision/runs", get(handle_list_runs))
        .route("/api/provision/run/{id}", get(handle_get_run))
        .with_state(state)
}

async fn handle_provision(
    State(s): State<Arc<ProvisionState>>,
    Json(req): Json<ProvisionRequest>,
) -> Json<Value> {
    if let Err(e) = validate_request(&req) {
        return Json(json!({"error": e}));
    }
    let pool = s.pool.clone();
    let peer = req.peer_name.clone();
    let handle = tokio::spawn(async move {
        match provision_peer(&pool, &req).await {
            Ok(run_id) => tracing::info!(run_id, peer = %peer, "provisioning complete"),
            Err(e) => tracing::warn!(peer = %peer, error = %e, "provisioning failed"),
        }
    });
    // Log if the spawned task panics
    tokio::spawn(async move {
        if let Err(e) = handle.await {
            tracing::error!(error = %e, "provisioning task panicked");
        }
    });
    Json(json!({"ok": true, "message": "provisioning started"}))
}

#[derive(Deserialize, Default)]
struct ListQuery {
    limit: Option<u32>,
}

async fn handle_list_runs(
    State(s): State<Arc<ProvisionState>>,
    Query(q): Query<ListQuery>,
) -> Json<Value> {
    let conn = match s.pool.get() {
        Ok(c) => c,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let limit = q.limit.unwrap_or(20).min(100);
    let mut stmt = match conn.prepare(
        "SELECT id, peer_name, ssh_target, status, items_total, items_done, \
         started_at, completed_at, error_message \
         FROM provision_runs ORDER BY id DESC LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let rows: Vec<Value> = stmt
        .query_map([limit], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "peer_name": r.get::<_, String>(1)?,
                "ssh_target": r.get::<_, String>(2)?,
                "status": r.get::<_, String>(3)?,
                "items_total": r.get::<_, u32>(4)?,
                "items_done": r.get::<_, u32>(5)?,
                "started_at": r.get::<_, String>(6)?,
                "completed_at": r.get::<_, Option<String>>(7)?,
                "error": r.get::<_, Option<String>>(8)?,
            }))
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default();
    Json(json!({"runs": rows}))
}

async fn handle_get_run(State(s): State<Arc<ProvisionState>>, Path(id): Path<i64>) -> Json<Value> {
    let conn = match s.pool.get() {
        Ok(c) => c,
        Err(e) => return Json(json!({"error": e.to_string()})),
    };
    let run = conn.query_row(
        "SELECT id, peer_name, ssh_target, status, items_total, items_done, \
         started_at, completed_at, error_message FROM provision_runs WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "peer_name": r.get::<_, String>(1)?,
                "ssh_target": r.get::<_, String>(2)?,
                "status": r.get::<_, String>(3)?,
                "items_total": r.get::<_, u32>(4)?,
                "items_done": r.get::<_, u32>(5)?,
                "started_at": r.get::<_, String>(6)?,
                "completed_at": r.get::<_, Option<String>>(7)?,
                "error": r.get::<_, Option<String>>(8)?,
            }))
        },
    );
    let items = list_items(&conn, id);
    match run {
        Ok(r) => Json(json!({"run": r, "items": items})),
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

fn list_items(conn: &rusqlite::Connection, run_id: i64) -> Vec<Value> {
    let mut stmt = match conn.prepare(
        "SELECT id, item_type, source_path, dest_path, status, \
         bytes_transferred, duration_ms, error_message \
         FROM provision_items WHERE run_id = ?1 ORDER BY id",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([run_id], |r| {
        Ok(json!({
            "id": r.get::<_, i64>(0)?,
            "item_type": r.get::<_, String>(1)?,
            "source": r.get::<_, String>(2)?,
            "dest": r.get::<_, String>(3)?,
            "status": r.get::<_, String>(4)?,
            "bytes": r.get::<_, u64>(5)?,
            "duration_ms": r.get::<_, u64>(6)?,
            "error": r.get::<_, Option<String>>(7)?,
        }))
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}
