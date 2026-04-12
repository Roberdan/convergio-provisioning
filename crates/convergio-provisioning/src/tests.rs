//! Tests for convergio-provisioning.

mod ext_tests {
    use convergio_types::extension::Extension;
    use convergio_types::manifest::ModuleKind;

    use crate::ext::ProvisioningExtension;

    #[test]
    fn manifest_is_extension() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = ProvisioningExtension::new(pool);
        let m = ext.manifest();
        assert_eq!(m.id, "convergio-provisioning");
        assert!(matches!(m.kind, ModuleKind::Extension));
        assert!(m.provides.iter().any(|c| c.name == "node-provisioning"));
    }

    #[test]
    fn has_one_migration() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = ProvisioningExtension::new(pool);
        let migs = ext.migrations();
        assert_eq!(migs.len(), 1);
    }

    #[test]
    fn migrations_sql_valid() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = ProvisioningExtension::new(pool.clone());
        for mig in ext.migrations() {
            conn.execute_batch(mig.up).unwrap();
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM provision_runs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM provision_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn health_ok() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = ProvisioningExtension::new(pool);
        assert!(matches!(
            ext.health(),
            convergio_types::extension::Health::Ok
        ));
    }
}

mod types_tests {
    use crate::types::*;

    #[test]
    fn provision_status_display() {
        assert_eq!(ProvisionStatus::Pending.to_string(), "pending");
        assert_eq!(ProvisionStatus::Running.to_string(), "running");
        assert_eq!(ProvisionStatus::Success.to_string(), "success");
        assert_eq!(ProvisionStatus::Failed.to_string(), "failed");
        assert_eq!(ProvisionStatus::Skipped.to_string(), "skipped");
    }

    #[test]
    fn item_type_display() {
        assert_eq!(ProvisionItemType::Config.to_string(), "config");
        assert_eq!(ProvisionItemType::Binary.to_string(), "binary");
        assert_eq!(ProvisionItemType::Keys.to_string(), "keys");
        assert_eq!(ProvisionItemType::Memory.to_string(), "memory");
        assert_eq!(ProvisionItemType::AgentDefs.to_string(), "agent_defs");
    }
}

mod db_tests {
    #[test]
    fn insert_run_and_items() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = crate::ext::ProvisioningExtension::new(pool.clone());
        for mig in convergio_types::extension::Extension::migrations(&ext) {
            conn.execute_batch(mig.up).unwrap();
        }
        conn.execute(
            "INSERT INTO provision_runs (peer_name, ssh_target, status) \
             VALUES ('m5-max', 'rob@192.168.1.50', 'pending')",
            [],
        )
        .unwrap();
        let run_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO provision_items (run_id, item_type, source_path, dest_path, status) \
             VALUES (?1, 'config', '/local/cfg', '/remote/cfg', 'pending')",
            [run_id],
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM provision_items WHERE run_id = ?1",
                [run_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}

mod mcp_tests {
    use crate::mcp_defs::provisioning_tools;

    #[test]
    fn mcp_schema_matches_api() {
        let tools = provisioning_tools();
        let peer_tool = tools
            .iter()
            .find(|t| t.name == "cvg_provision_peer")
            .unwrap();
        let props = peer_tool.input_schema["properties"].as_object().unwrap();
        // Schema must match ProvisionRequest fields
        assert!(props.contains_key("peer_name"));
        assert!(props.contains_key("ssh_target"));
        let required = peer_tool.input_schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "peer_name"));
        assert!(required.iter().any(|v| v == "ssh_target"));
    }

    #[test]
    fn mcp_run_path_uses_braces() {
        let tools = provisioning_tools();
        let run_tool = tools
            .iter()
            .find(|t| t.name == "cvg_provision_run")
            .unwrap();
        assert!(
            run_tool.path.contains("{id}"),
            "path should use {{id}} not :id"
        );
        assert!(!run_tool.path.contains(":id"));
    }
}

mod validation_tests {
    use crate::routes::validate_request;
    use crate::types::ProvisionRequest;

    fn valid_request() -> ProvisionRequest {
        ProvisionRequest {
            peer_name: "m5-max".into(),
            ssh_target: "rob@192.168.1.50".into(),
            remote_base: "~/.convergio".into(),
            include_binary: false,
            include_config: true,
            include_agent_defs: true,
            include_memory: false,
        }
    }

    #[test]
    fn valid_request_passes() {
        assert!(validate_request(&valid_request()).is_ok());
    }

    #[test]
    fn empty_peer_name_rejected() {
        let mut req = valid_request();
        req.peer_name = String::new();
        assert!(validate_request(&req).is_err());
    }

    #[test]
    fn shell_metachar_in_ssh_target_rejected() {
        for bad in &[";", "&", "|", "$", "`", "'", "\"", "\\", "\n"] {
            let mut req = valid_request();
            req.ssh_target = format!("user@host{bad}evil");
            assert!(validate_request(&req).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn path_traversal_in_remote_base_rejected() {
        let mut req = valid_request();
        req.remote_base = "/etc/../../../root".into();
        assert!(validate_request(&req).is_err());
    }
}
