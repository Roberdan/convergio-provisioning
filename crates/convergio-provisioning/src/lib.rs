//! convergio-provisioning — sync config/keys/binary to remote nodes.
//!
//! Orchestrates rsync operations to provision peer nodes with:
//! - Config files (~/.convergio/config.toml, env)
//! - Agent definitions (claude-config/ TOML files)
//! - Daemon binary (target/release/convergio)
//! - Memory/context data
//!
//! DB tables: provision_runs, provision_items.

pub mod ext;
pub mod provision;
pub mod routes;
pub mod types;

pub use ext::ProvisioningExtension;
pub use types::{ProvisionItem, ProvisionRun, ProvisionStatus};

pub mod mcp_defs;
#[cfg(test)]
mod tests;
