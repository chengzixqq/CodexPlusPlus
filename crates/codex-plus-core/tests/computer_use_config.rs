use codex_plus_core::computer_use_config::{
    ComputerUseRepairOptions, inspect_computer_use_in_home, repair_computer_use_in_home,
};
use serde_json::json;

#[test]
fn repair_computer_use_writes_plugin_cache_and_config_without_touching_user_env() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join(".codex");
    let source = temp.path().join("source-openai-bundled");
    write_bundled_source(&source);
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        r#"[marketplaces.openai-primary-runtime]
source_type = "local"
source = "C:\\runtime"

[plugins."github@openai-curated"]
enabled = true
"#,
    )
    .unwrap();

    let report = repair_computer_use_in_home(
        &home,
        ComputerUseRepairOptions {
            bundled_source: Some(source.clone()),
            skip_user_environment: true,
            verify_helper_transport: false,
        },
    )
    .unwrap();

    assert_eq!(report.status, "ok");
    assert!(report.changed);
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.name == "pluginMarketplaces" && step.status == "ok")
    );

    let config = std::fs::read_to_string(home.join("config.toml")).unwrap();
    assert!(config.contains("[marketplaces.openai-bundled]"));
    assert!(config.contains("[plugins.\"computer-use@openai-bundled\"]"));
    assert!(config.contains("computer_use = true"));
    assert!(config.contains("remote_connections = true"));
    assert!(config.contains("sandbox = \"unelevated\""));
    assert!(config.contains("[marketplaces.openai-primary-runtime]"));
    assert!(config.contains("[plugins.\"github@openai-curated\"]"));

    let marketplace_root = home
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    assert!(
        marketplace_root
            .join("plugins")
            .join("computer-use")
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file()
    );
    assert!(
        home.join("plugins")
            .join("cache")
            .join("openai-bundled")
            .join("computer-use")
            .join("latest")
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file()
    );
    assert!(
        home.join("plugins")
            .join("cache")
            .join("openai-bundled")
            .join("browser")
            .join("latest")
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file()
    );
    assert!(
        home.join("plugins")
            .join("cache")
            .join("openai-bundled")
            .join("browser")
            .join("1.2.3")
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file()
    );
    assert!(
        home.join("plugins")
            .join("cache")
            .join("openai-bundled")
            .join("chrome")
            .join("latest")
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file()
    );

    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            marketplace_root
                .join(".agents")
                .join("plugins")
                .join("marketplace.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["plugins"][0]["name"], "computer-use");

    let status = inspect_computer_use_in_home(&home);
    assert!(matches!(status.status.as_str(), "ok" | "needs_repair"));
    assert!(status.marketplace_manifest_exists);
    assert!(status.plugin_source_exists);
    assert!(status.plugin_cache_exists);
    assert!(status.browser_plugin_source_exists);
    assert!(status.browser_plugin_cache_exists);
    assert!(status.chrome_plugin_source_exists);
    assert!(status.chrome_plugin_cache_exists);
    assert!(status.helper_transport_exists);
    assert!(status.plugin_enabled);
    assert!(status.computer_use_feature_enabled);
    assert!(status.remote_connections_enabled);
    assert_eq!(status.windows_sandbox.as_deref(), Some("unelevated"));
}

#[test]
fn inspect_computer_use_reports_missing_browser_latest_cache() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join(".codex");
    let source = temp.path().join("source-openai-bundled");
    write_bundled_source(&source);
    std::fs::create_dir_all(&home).unwrap();

    repair_computer_use_in_home(
        &home,
        ComputerUseRepairOptions {
            bundled_source: Some(source),
            skip_user_environment: true,
            verify_helper_transport: false,
        },
    )
    .unwrap();
    let browser_latest = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("browser")
        .join("latest");
    std::fs::remove_dir_all(&browser_latest).unwrap();

    let status = inspect_computer_use_in_home(&home);

    assert_eq!(status.status, "needs_repair");
    assert!(status.browser_plugin_source_exists);
    assert!(!status.browser_plugin_cache_exists);
}

#[test]
fn inspect_computer_use_reports_missing_state_without_writing_files() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join(".codex");
    std::fs::create_dir_all(&home).unwrap();

    let status = inspect_computer_use_in_home(&home);

    assert_eq!(status.status, "needs_repair");
    assert!(!status.marketplace_manifest_exists);
    assert!(!status.plugin_enabled);
    assert!(!home.join("config.toml").exists());
}

fn write_bundled_source(source: &std::path::Path) {
    let manifest_path = source
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    std::fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&json!({
            "name": "openai-bundled",
            "plugins": [
                {
                    "name": "browser",
                    "source": {"source": "local", "path": "./plugins/browser"}
                }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    write_plugin(source, "browser", "1.2.3");
    write_plugin(source, "chrome", "26.609.30741");
}

fn write_plugin(source: &std::path::Path, name: &str, version: &str) {
    let plugin_root = source.join("plugins").join(name);
    let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
    std::fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    std::fs::write(
        manifest_path,
        serde_json::to_string_pretty(&json!({
            "name": name,
            "version": version,
            "skills": "./skills/"
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(plugin_root.join("payload.txt"), name).unwrap();
}
