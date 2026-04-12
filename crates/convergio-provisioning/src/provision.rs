//! Core provisioning logic — orchestrates rsync operations to remote peers.

use crate::types::{ProvisionItemType, ProvisionRequest, ProvisionStatus};
use convergio_db::pool::ConnPool;
use std::time::Instant;

/// Execute a full provisioning run for a peer.
pub async fn provision_peer(pool: &ConnPool, req: &ProvisionRequest) -> Result<i64, String> {
    let run_id = create_run(pool, &req.peer_name, &req.ssh_target)?;
    update_run_status(pool, run_id, ProvisionStatus::Running);

    let mut items_done = 0u32;
    let mut items_total = 0u32;
    let mut last_error: Option<String> = None;

    let repo_root = std::env::var("CONVERGIO_REPO_ROOT")
        .or_else(|_| std::env::current_dir().map(|p| p.display().to_string()))
        .unwrap_or_else(|_| ".".into());

    // Config files
    if req.include_config {
        items_total += 1;
        let source = convergio_types::platform_paths::convergio_data_dir();
        match rsync_item(
            pool,
            run_id,
            ProvisionItemType::Config,
            &source.to_string_lossy(),
            &req.remote_base,
            &req.ssh_target,
            &["*.db", "*.db-wal", "*.db-shm"],
        )
        .await
        {
            Ok(_) => items_done += 1,
            Err(e) => last_error = Some(e),
        }
    }

    // Agent definitions
    if req.include_agent_defs {
        items_total += 1;
        let source = format!("{repo_root}/claude-config/");
        let dest = format!("{}/agent-defs/", req.remote_base);
        match rsync_item(
            pool,
            run_id,
            ProvisionItemType::AgentDefs,
            &source,
            &dest,
            &req.ssh_target,
            &[],
        )
        .await
        {
            Ok(_) => items_done += 1,
            Err(e) => last_error = Some(e),
        }
    }

    // Binary
    if req.include_binary {
        items_total += 1;
        let source = format!("{repo_root}/daemon/target/release/convergio");
        let dest = format!("{}/bin/convergio", req.remote_base);
        match rsync_item(
            pool,
            run_id,
            ProvisionItemType::Binary,
            &source,
            &dest,
            &req.ssh_target,
            &[],
        )
        .await
        {
            Ok(_) => items_done += 1,
            Err(e) => last_error = Some(e),
        }
    }

    // Memory/context
    if req.include_memory {
        items_total += 1;
        let source = convergio_types::platform_paths::convergio_data_dir().join("memory");
        let dest = format!("{}/memory/", req.remote_base);
        match rsync_item(
            pool,
            run_id,
            ProvisionItemType::Memory,
            &source.to_string_lossy(),
            &dest,
            &req.ssh_target,
            &[],
        )
        .await
        {
            Ok(_) => items_done += 1,
            Err(e) => last_error = Some(e),
        }
    }

    let status = if items_done == items_total {
        ProvisionStatus::Success
    } else {
        ProvisionStatus::Failed
    };
    complete_run(pool, run_id, status, items_total, items_done, last_error);
    Ok(run_id)
}

async fn rsync_item(
    pool: &ConnPool,
    run_id: i64,
    item_type: ProvisionItemType,
    source: &str,
    dest: &str,
    ssh_target: &str,
    exclude: &[&str],
) -> Result<(), String> {
    let item_id = create_item(pool, run_id, item_type, source, dest)?;
    update_item_status(pool, item_id, ProvisionStatus::Running);

    let start = Instant::now();
    let mut cmd = tokio::process::Command::new("rsync");
    cmd.args([
        "-avz",
        "-e",
        "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10",
    ]);
    for pat in exclude {
        cmd.arg(format!("--exclude={pat}"));
    }
    cmd.arg(source);
    cmd.arg(format!("{ssh_target}:{dest}"));

    let output = cmd
        .output()
        .await
        .map_err(|e: std::io::Error| e.to_string())?;
    let duration_ms = start.elapsed().as_millis() as u64;

    if output.status.success() {
        complete_item(
            pool,
            item_id,
            ProvisionStatus::Success,
            0,
            duration_ms,
            None,
        );
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        complete_item(
            pool,
            item_id,
            ProvisionStatus::Failed,
            0,
            duration_ms,
            Some(&err),
        );
        Err(err)
    }
}

// --- DB helpers ---

fn create_run(pool: &ConnPool, peer: &str, ssh: &str) -> Result<i64, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO provision_runs (peer_name, ssh_target, status) VALUES (?1, ?2, 'pending')",
        rusqlite::params![peer, ssh],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn update_run_status(pool: &ConnPool, id: i64, status: ProvisionStatus) {
    if let Ok(conn) = pool.get() {
        if let Err(e) = conn.execute(
            "UPDATE provision_runs SET status = ?1 WHERE id = ?2",
            rusqlite::params![status.to_string(), id],
        ) {
            tracing::warn!(run_id = id, error = %e, "failed to update run status");
        }
    }
}

fn complete_run(
    pool: &ConnPool,
    id: i64,
    status: ProvisionStatus,
    total: u32,
    done: u32,
    error: Option<String>,
) {
    if let Ok(conn) = pool.get() {
        if let Err(e) = conn.execute(
            "UPDATE provision_runs SET status=?1, items_total=?2, items_done=?3, \
             error_message=?4, completed_at=datetime('now') WHERE id=?5",
            rusqlite::params![status.to_string(), total, done, error, id],
        ) {
            tracing::warn!(run_id = id, error = %e, "failed to complete run");
        }
    }
}

fn create_item(
    pool: &ConnPool,
    run_id: i64,
    item_type: ProvisionItemType,
    source: &str,
    dest: &str,
) -> Result<i64, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO provision_items (run_id, item_type, source_path, dest_path, status) \
         VALUES (?1, ?2, ?3, ?4, 'pending')",
        rusqlite::params![run_id, item_type.to_string(), source, dest],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn update_item_status(pool: &ConnPool, id: i64, status: ProvisionStatus) {
    if let Ok(conn) = pool.get() {
        if let Err(e) = conn.execute(
            "UPDATE provision_items SET status = ?1 WHERE id = ?2",
            rusqlite::params![status.to_string(), id],
        ) {
            tracing::warn!(item_id = id, error = %e, "failed to update item status");
        }
    }
}

fn complete_item(
    pool: &ConnPool,
    id: i64,
    status: ProvisionStatus,
    bytes: u64,
    duration_ms: u64,
    error: Option<&str>,
) {
    if let Ok(conn) = pool.get() {
        if let Err(e) = conn.execute(
            "UPDATE provision_items SET status=?1, bytes_transferred=?2, \
             duration_ms=?3, error_message=?4 WHERE id=?5",
            rusqlite::params![status.to_string(), bytes, duration_ms, error, id],
        ) {
            tracing::warn!(item_id = id, error = %e, "failed to complete item");
        }
    }
}
