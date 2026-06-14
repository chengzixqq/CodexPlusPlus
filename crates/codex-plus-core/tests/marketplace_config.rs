use codex_plus_core::marketplace_config::repair_local_marketplace_config_in_home;

#[test]
fn repairs_local_marketplaces_and_legacy_manifest_layout() {
    let temp = tempfile::tempdir().unwrap();
    let codex_home = temp.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("config.toml"),
        r#"model = "gpt-5"

[plugins."computer-use@openai-bundled"]
enabled = true
"#,
    )
    .unwrap();

    let primary = temp
        .path()
        .join(".cache")
        .join("codex-runtimes")
        .join("codex-primary-runtime")
        .join("plugins")
        .join("openai-primary-runtime");
    write_nested_manifest(
        &primary,
        "openai-primary-runtime",
        &["documents", "spreadsheets"],
    );

    let bundled = codex_home
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    write_nested_manifest(&bundled, "openai-bundled", &["computer-use", "browser"]);

    let curated = codex_home.join("marketplaces").join("openai-curated-local");
    std::fs::create_dir_all(&curated).unwrap();
    std::fs::write(
        curated.join("marketplace.json"),
        manifest_json("openai-curated", &["github", "gmail", "figma"]),
    )
    .unwrap();

    let report = repair_local_marketplace_config_in_home(&codex_home).unwrap();

    assert_eq!(report.status, "ok");
    assert!(report.changed);
    assert!(report.backup_path.is_some());
    assert!(curated.join(".agents/plugins/marketplace.json").is_file());
    assert_eq!(report.marketplaces.len(), 4);
    assert!(
        report
            .marketplaces
            .iter()
            .any(|entry| entry.name == "openai-curated-local"
                && entry.configured
                && entry.manifest_repaired
                && entry.plugin_count == Some(3))
    );

    let config = std::fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("model = \"gpt-5\""));
    assert!(config.contains("[plugins.\"computer-use@openai-bundled\"]"));
    for name in [
        "openai-primary-runtime",
        "openai-bundled",
        "openai-curated-local",
    ] {
        assert!(config.contains(&format!("[marketplaces.{name}]")));
    }
    assert!(!config.contains("[marketplaces.openai-curated]"));
    assert_eq!(config.matches("source_type = \"local\"").count(), 3);
}

#[test]
fn registers_openai_role_specific_marketplace_when_present() {
    let temp = tempfile::tempdir().unwrap();
    let codex_home = temp.path().join(".codex");
    let role_specific = codex_home.join("marketplaces").join("openai-role-specific");
    write_nested_manifest(
        &role_specific,
        "role-specific-plugins",
        &[
            "sales",
            "data-analytics",
            "product-design",
            "financial-markets",
        ],
    );
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let report = repair_local_marketplace_config_in_home(&codex_home).unwrap();

    assert!(report.changed);
    assert!(
        report
            .marketplaces
            .iter()
            .any(|entry| entry.name == "openai-role-specific"
                && entry.configured
                && entry.plugin_count == Some(4))
    );
    let config = std::fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("[marketplaces.openai-role-specific]"));
    assert!(config.contains("openai-role-specific"));
}

#[test]
fn removes_stale_openai_curated_alias_to_restore_official_remote() {
    let temp = tempfile::tempdir().unwrap();
    let codex_home = temp.path().join(".codex");
    let curated = codex_home.join("marketplaces").join("openai-curated-local");
    std::fs::create_dir_all(&curated).unwrap();
    std::fs::write(
        curated.join("marketplace.json"),
        manifest_json("openai-curated", &["github"]),
    )
    .unwrap();
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"[marketplaces.openai-curated]
source_type = "local"
source = "{}"

[plugins."github@openai-curated"]
enabled = true
"#,
            curated.to_string_lossy().replace('\\', "\\\\")
        ),
    )
    .unwrap();

    let report = repair_local_marketplace_config_in_home(&codex_home).unwrap();

    assert!(report.changed);
    assert!(
        report
            .marketplaces
            .iter()
            .any(|entry| entry.name == "openai-curated"
                && entry.message.contains("恢复官方远程插件市场"))
    );
    let config = std::fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(!config.contains("[marketplaces.openai-curated]"));
    assert!(config.contains("[marketplaces.openai-curated-local]"));
    assert!(config.contains("[plugins.\"github@openai-curated\"]"));
}

#[test]
fn preserves_non_local_openai_curated_marketplace_override() {
    let temp = tempfile::tempdir().unwrap();
    let codex_home = temp.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(
        codex_home.join("config.toml"),
        r#"[marketplaces.openai-curated]
source_type = "remote"
source = "https://example.invalid/marketplace.json"
"#,
    )
    .unwrap();

    let report = repair_local_marketplace_config_in_home(&codex_home).unwrap();

    assert!(!report.changed);
    let config = std::fs::read_to_string(codex_home.join("config.toml")).unwrap();
    assert!(config.contains("[marketplaces.openai-curated]"));
    assert!(config.contains("source_type = \"remote\""));
}

#[test]
fn skips_missing_marketplace_sources_without_writing_config() {
    let temp = tempfile::tempdir().unwrap();
    let codex_home = temp.path().join(".codex");
    std::fs::create_dir_all(&codex_home).unwrap();
    std::fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();

    let report = repair_local_marketplace_config_in_home(&codex_home).unwrap();

    assert_eq!(report.status, "ok");
    assert!(!report.changed);
    assert!(report.backup_path.is_none());
    assert!(report.marketplaces.iter().all(|entry| !entry.configured));
    assert_eq!(
        std::fs::read_to_string(codex_home.join("config.toml")).unwrap(),
        "model = \"gpt-5\"\n"
    );
}

fn write_nested_manifest(root: &std::path::Path, name: &str, plugins: &[&str]) {
    let manifest = root
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
    std::fs::write(manifest, manifest_json(name, plugins)).unwrap();
}

fn manifest_json(name: &str, plugins: &[&str]) -> String {
    let plugins = plugins
        .iter()
        .map(|plugin| {
            format!(
                r#"{{"name":"{plugin}","source":{{"source":"local","path":"./plugins/{plugin}"}}}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"name":"{name}","plugins":[{plugins}]}}"#)
}
