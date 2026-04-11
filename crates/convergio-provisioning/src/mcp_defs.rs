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
                    "hostname": {"type": "string"},
                    "ssh_user": {"type": "string"}
                },
                "required": ["hostname"]
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
            path: "/api/provision/run/:id".into(),
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
