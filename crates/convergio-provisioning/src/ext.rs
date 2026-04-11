//! Extension trait implementation for provisioning.

use std::sync::Arc;

use convergio_db::pool::ConnPool;
use convergio_types::extension::{AppContext, Extension, Health, McpToolDef, Metric, Migration};
use convergio_types::manifest::{Capability, Manifest, ModuleKind};

use crate::routes::{provision_routes, ProvisionState};

pub struct ProvisioningExtension {
    pool: ConnPool,
}

impl ProvisioningExtension {
    pub fn new(pool: ConnPool) -> Self {
        Self { pool }
    }
}

impl Extension for ProvisioningExtension {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "convergio-provisioning".to_string(),
            description: "Node provisioning — sync config, keys, binary to remote peers"
                .to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            kind: ModuleKind::Extension,
            provides: vec![Capability {
                name: "node-provisioning".to_string(),
                version: "1.0.0".to_string(),
                description: "Sync config/keys/binary to peers via rsync/SSH".to_string(),
            }],
            requires: vec![],
            agent_tools: vec![],
            required_roles: vec!["orchestrator".into(), "all".into()],
        }
    }

    fn routes(&self, _ctx: &AppContext) -> Option<axum::Router> {
        let state = Arc::new(ProvisionState {
            pool: self.pool.clone(),
        });
        Some(provision_routes(state))
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "provisioning tables",
            up: "CREATE TABLE IF NOT EXISTS provision_runs (\
                    id INTEGER PRIMARY KEY,\
                    peer_name TEXT NOT NULL,\
                    ssh_target TEXT NOT NULL,\
                    status TEXT DEFAULT 'pending',\
                    items_total INTEGER DEFAULT 0,\
                    items_done INTEGER DEFAULT 0,\
                    started_at TEXT DEFAULT (datetime('now')),\
                    completed_at TEXT,\
                    error_message TEXT\
                );\
                CREATE INDEX IF NOT EXISTS idx_pr_peer \
                    ON provision_runs(peer_name);\
                CREATE INDEX IF NOT EXISTS idx_pr_status \
                    ON provision_runs(status);\
                CREATE TABLE IF NOT EXISTS provision_items (\
                    id INTEGER PRIMARY KEY,\
                    run_id INTEGER NOT NULL REFERENCES provision_runs(id),\
                    item_type TEXT NOT NULL,\
                    source_path TEXT,\
                    dest_path TEXT,\
                    status TEXT DEFAULT 'pending',\
                    bytes_transferred INTEGER DEFAULT 0,\
                    duration_ms INTEGER DEFAULT 0,\
                    error_message TEXT\
                );\
                CREATE INDEX IF NOT EXISTS idx_pi_run \
                    ON provision_items(run_id);",
        }]
    }

    fn health(&self) -> Health {
        match self.pool.get() {
            Ok(_) => Health::Ok,
            Err(e) => Health::Degraded {
                reason: format!("db: {e}"),
            },
        }
    }

    fn metrics(&self) -> Vec<Metric> {
        let run_count: f64 = self
            .pool
            .get()
            .ok()
            .and_then(|c| {
                c.query_row("SELECT COUNT(*) FROM provision_runs", [], |r| {
                    r.get::<_, i64>(0)
                })
                .ok()
            })
            .unwrap_or(0) as f64;
        vec![Metric {
            name: "provision_runs_total".to_string(),
            value: run_count,
            labels: vec![],
        }]
    }

    fn mcp_tools(&self) -> Vec<McpToolDef> {
        crate::mcp_defs::provisioning_tools()
    }
}
