//! Provisioning types — runs, items, configuration.

use serde::{Deserialize, Serialize};

/// A provisioning run — one complete sync to a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionRun {
    pub id: i64,
    pub peer_name: String,
    pub ssh_target: String,
    pub status: ProvisionStatus,
    pub items_total: u32,
    pub items_done: u32,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

/// A single item within a provisioning run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionItem {
    pub id: i64,
    pub run_id: i64,
    pub item_type: ProvisionItemType,
    pub source_path: String,
    pub dest_path: String,
    pub status: ProvisionStatus,
    pub bytes_transferred: u64,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

/// Provisioning status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvisionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

impl std::fmt::Display for ProvisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

/// Type of provisioning item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvisionItemType {
    Config,
    AgentDefs,
    Binary,
    Memory,
    Keys,
}

impl std::fmt::Display for ProvisionItemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Config => write!(f, "config"),
            Self::AgentDefs => write!(f, "agent_defs"),
            Self::Binary => write!(f, "binary"),
            Self::Memory => write!(f, "memory"),
            Self::Keys => write!(f, "keys"),
        }
    }
}

/// Request to provision a peer node.
#[derive(Debug, Clone, Deserialize)]
pub struct ProvisionRequest {
    pub peer_name: String,
    pub ssh_target: String,
    #[serde(default = "default_remote_base")]
    pub remote_base: String,
    #[serde(default)]
    pub include_binary: bool,
    #[serde(default = "default_true")]
    pub include_config: bool,
    #[serde(default = "default_true")]
    pub include_agent_defs: bool,
    #[serde(default)]
    pub include_memory: bool,
}

fn default_remote_base() -> String {
    "~/.convergio".to_string()
}
fn default_true() -> bool {
    true
}
