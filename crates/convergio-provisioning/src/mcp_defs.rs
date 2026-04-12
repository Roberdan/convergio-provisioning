//! MCP tool definitions for the provisioning extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn provisioning_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_provision_peer".into(),
            description: "Provision a new mesh peer.".into(),
            method: "POST".into(),
            path: "/api/provision/peer".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "peer_name": {"type": "string", "description": "Name of the peer node"},
                    "ssh_target": {"type": "string", "description": "SSH target (e.g. user@host)"},
                    "remote_base": {"type": "string", "description": "Remote base path"},
                    "include_binary": {"type": "boolean"},
                    "include_config": {"type": "boolean"},
                    "include_agent_defs": {"type": "boolean"},
                    "include_memory": {"type": "boolean"}
                },
                "required": ["peer_name", "ssh_target"]
            }),
            min_ring: "core".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_provision_runs".into(),
            description: "List provisioning runs.".into(),
            method: "GET".into(),
            path: "/api/provision/runs".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_provision_run".into(),
            description: "Get details of a provisioning run.".into(),
            method: "GET".into(),
            path: "/api/provision/run/{id}".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"id": {"type": "string"}},
                "required": ["id"]
            }),
            min_ring: "community".into(),
            path_params: vec!["id".into()],
        },
    ]
}
