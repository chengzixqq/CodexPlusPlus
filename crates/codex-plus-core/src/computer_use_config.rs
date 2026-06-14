use anyhow::Context;
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{DocumentMut, Item, Table};

const COMPUTER_USE_VERSION: &str = "0.1.0-local";
const PERSISTENT_BUNDLED_PLUGINS: &[&str] = &["browser", "chrome"];

#[derive(Debug, Clone, Default)]
pub struct ComputerUseRepairOptions {
    pub bundled_source: Option<PathBuf>,
    pub skip_user_environment: bool,
    pub verify_helper_transport: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseRepairReport {
    pub status: String,
    pub message: String,
    pub codex_home: String,
    pub marketplace_root: String,
    pub bundled_source: Option<String>,
    pub config_path: String,
    pub config_backup_path: Option<String>,
    pub changed: bool,
    pub steps: Vec<ComputerUseRepairStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseRepairStep {
    pub name: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseStatusReport {
    pub status: String,
    pub message: String,
    pub codex_home: String,
    pub marketplace_root: String,
    pub config_path: String,
    pub marketplace_manifest_exists: bool,
    pub plugin_source_exists: bool,
    pub plugin_cache_exists: bool,
    pub browser_plugin_source_exists: bool,
    pub browser_plugin_cache_exists: bool,
    pub chrome_plugin_source_exists: bool,
    pub chrome_plugin_cache_exists: bool,
    pub helper_transport_exists: bool,
    pub plugin_enabled: bool,
    pub computer_use_feature_enabled: bool,
    pub remote_connections_enabled: bool,
    pub windows_sandbox: Option<String>,
    pub user_environment_enabled: bool,
    pub chrome_native_manifest_path: Option<String>,
    pub chrome_native_manifest_valid: Option<bool>,
}

pub fn repair_default_computer_use() -> anyhow::Result<ComputerUseRepairReport> {
    repair_computer_use_in_home(
        &crate::relay_config::default_codex_home_dir(),
        ComputerUseRepairOptions::default(),
    )
}

pub fn inspect_default_computer_use() -> ComputerUseStatusReport {
    inspect_computer_use_in_home(&crate::relay_config::default_codex_home_dir())
}

pub fn inspect_computer_use_in_home(home: &Path) -> ComputerUseStatusReport {
    let home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    let marketplace_root = home
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    let cache_latest = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("computer-use")
        .join("latest");
    let config_path = home.join("config.toml");
    let config = std::fs::read_to_string(&config_path).unwrap_or_default();
    let parsed_config = toml::from_str::<toml::Value>(&config).ok();
    let plugin_enabled = parsed_config
        .as_ref()
        .and_then(|data| data.get("plugins"))
        .and_then(|plugins| plugins.get("computer-use@openai-bundled"))
        .and_then(|plugin| plugin.get("enabled"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    let computer_use_feature_enabled = parsed_config
        .as_ref()
        .and_then(|data| data.get("features"))
        .and_then(|features| features.get("computer_use"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    let remote_connections_enabled = parsed_config
        .as_ref()
        .and_then(|data| data.get("features"))
        .and_then(|features| features.get("remote_connections"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    let windows_sandbox = parsed_config
        .as_ref()
        .and_then(|data| data.get("windows"))
        .and_then(|windows| windows.get("sandbox"))
        .and_then(toml::Value::as_str)
        .map(ToString::to_string);
    let chrome_native_manifest = chrome_native_messaging_manifest_path();
    let chrome_native_manifest_valid = chrome_native_manifest.as_ref().and_then(|path| {
        if !path.is_file() {
            return Some(false);
        }
        let manifest = read_json_file(path)?;
        let path = manifest.get("path").and_then(Value::as_str)?;
        Some(Path::new(path).is_file())
    });
    let marketplace_manifest_exists = bundled_manifest_path(&marketplace_root).is_file();
    let plugin_source_exists = marketplace_root
        .join("plugins")
        .join("computer-use")
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file();
    let plugin_cache_exists = cache_latest
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file();
    let browser_plugin_source_exists = bundled_plugin_source_exists(&marketplace_root, "browser");
    let browser_plugin_cache_exists = bundled_plugin_cache_exists(&home, "browser");
    let chrome_plugin_source_exists = bundled_plugin_source_exists(&marketplace_root, "chrome");
    let chrome_plugin_cache_exists = bundled_plugin_cache_exists(&home, "chrome");
    let helper_transport_exists = helper_transport_path(&cache_latest).is_file();
    let user_environment_enabled = user_environment_enabled();
    let ready = marketplace_manifest_exists
        && plugin_source_exists
        && plugin_cache_exists
        && (!browser_plugin_source_exists || browser_plugin_cache_exists)
        && (!chrome_plugin_source_exists || chrome_plugin_cache_exists)
        && helper_transport_exists
        && plugin_enabled
        && computer_use_feature_enabled
        && remote_connections_enabled
        && windows_sandbox.as_deref() == Some("unelevated")
        && user_environment_enabled;

    ComputerUseStatusReport {
        status: if ready { "ok" } else { "needs_repair" }.to_string(),
        message: if ready {
            "Computer Use 用户级配置已就绪。"
        } else {
            "Computer Use 用户级配置需要修复。"
        }
        .to_string(),
        codex_home: home.to_string_lossy().to_string(),
        marketplace_root: marketplace_root.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        marketplace_manifest_exists,
        plugin_source_exists,
        plugin_cache_exists,
        browser_plugin_source_exists,
        browser_plugin_cache_exists,
        chrome_plugin_source_exists,
        chrome_plugin_cache_exists,
        helper_transport_exists,
        plugin_enabled,
        computer_use_feature_enabled,
        remote_connections_enabled,
        windows_sandbox,
        user_environment_enabled,
        chrome_native_manifest_path: chrome_native_manifest
            .filter(|path| path.exists())
            .map(|path| path.to_string_lossy().to_string()),
        chrome_native_manifest_valid,
    }
}

pub fn repair_computer_use_in_home(
    home: &Path,
    options: ComputerUseRepairOptions,
) -> anyhow::Result<ComputerUseRepairReport> {
    std::fs::create_dir_all(home)?;
    let home = home
        .canonicalize()
        .with_context(|| format!("failed to resolve Codex home: {}", home.display()))?;
    let marketplace_root = home
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    let mut steps = Vec::new();

    let bundled_source =
        resolve_bundled_marketplace_source(&home, options.bundled_source.as_deref());
    let mut bundled_marketplace_synced = false;
    if let Some(source) = &bundled_source {
        if same_path(source, &marketplace_root) {
            bundled_marketplace_synced = true;
            steps.push(ok_step(
                "syncBundledMarketplace",
                "openai-bundled marketplace 已在本地镜像路径。".to_string(),
            ));
        } else {
            match copy_dir_mirror(source, &marketplace_root) {
                Ok(()) => {
                    bundled_marketplace_synced = true;
                    steps.push(ok_step(
                        "syncBundledMarketplace",
                        format!(
                            "已同步 openai-bundled marketplace：{}",
                            source.to_string_lossy()
                        ),
                    ));
                }
                Err(error) => steps.push(warn_step(
                    "syncBundledMarketplace",
                    format!("openai-bundled marketplace 同步失败，将重建最小本地镜像：{error}"),
                )),
            }
        }
    } else {
        steps.push(warn_step(
            "syncBundledMarketplace",
            "未找到可读 openai-bundled marketplace 源，将重建最小本地镜像。".to_string(),
        ));
    }
    if !bundled_marketplace_synced {
        std::fs::create_dir_all(marketplace_root.join("plugins"))?;
    }

    let plugin_source_root = marketplace_root.join("plugins").join("computer-use");
    let cache_root = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("computer-use");
    let cache_version_root = cache_root.join(COMPUTER_USE_VERSION);
    let latest_path = cache_root.join("latest");

    write_computer_use_plugin_tree(&plugin_source_root)?;
    write_computer_use_plugin_tree(&cache_version_root)?;
    create_latest_link_or_copy(&latest_path, &cache_version_root)?;
    update_bundled_marketplace_manifest(&marketplace_root)?;
    let mut changed = true;
    steps.push(ok_step(
        "computerUsePlugin",
        "已写入 computer-use compatibility plugin 和 cache/latest。".to_string(),
    ));

    let marketplace_report =
        crate::marketplace_config::repair_local_marketplace_config_in_home(&home)?;
    changed |= marketplace_report.changed;
    let marketplace_config_backup_path = marketplace_report.backup_path.clone();
    let configured_marketplaces = marketplace_report
        .marketplaces
        .iter()
        .filter(|marketplace| marketplace.configured)
        .count();
    steps.push(ok_step(
        "pluginMarketplaces",
        format!("已注册 {configured_marketplaces} 个本地插件市场。"),
    ));

    for plugin in ["browser", "chrome"] {
        match sync_bundled_plugin_cache(&home, &marketplace_root, plugin) {
            Ok(Some(version_root)) => {
                changed = true;
                steps.push(ok_step(
                    format!("{plugin}Cache"),
                    format!("已同步 {plugin} 稳定 cache：{}", version_root.display()),
                ));
            }
            Ok(None) => steps.push(skip_step(
                format!("{plugin}Cache"),
                format!("openai-bundled 中未找到 {plugin}，已跳过。"),
            )),
            Err(error) => steps.push(warn_step(
                format!("{plugin}Cache"),
                format!("{plugin} 稳定 cache 同步失败：{error}"),
            )),
        }
    }

    match update_chrome_native_messaging_manifest(&home) {
        Ok(Some(path)) => {
            changed = true;
            steps.push(ok_step(
                "chromeNativeMessaging",
                format!(
                    "已更新 Chrome native messaging manifest：{}",
                    path.display()
                ),
            ));
        }
        Ok(None) => steps.push(skip_step(
            "chromeNativeMessaging",
            "未找到 Chrome native messaging manifest，已跳过。".to_string(),
        )),
        Err(error) => steps.push(warn_step(
            "chromeNativeMessaging",
            format!("Chrome native messaging manifest 更新失败：{error}"),
        )),
    }

    match remove_stale_chrome_native_host_entries(&home) {
        Ok(removed) if removed > 0 => {
            changed = true;
            steps.push(ok_step(
                "chromeNativeHostState",
                format!("已移除 {removed} 个失效 chrome-native-hosts.json 条目。"),
            ));
        }
        Ok(_) => steps.push(ok_step(
            "chromeNativeHostState",
            "chrome-native-hosts.json 无失效条目。".to_string(),
        )),
        Err(error) => steps.push(warn_step(
            "chromeNativeHostState",
            format!("chrome-native-hosts.json 清理失败：{error}"),
        )),
    }

    let config_path = home.join("config.toml");
    let (config_changed, config_backup_path) = update_codex_config(&home)?;
    changed |= config_changed;
    steps.push(ok_step(
        "codexConfig",
        if config_changed {
            "已写入 computer_use、remote_connections 和 windows.sandbox。".to_string()
        } else {
            "Codex config 已包含 Computer Use 所需用户级配置。".to_string()
        },
    ));

    if options.skip_user_environment {
        steps.push(skip_step(
            "userEnvironment",
            "按请求跳过用户环境变量写入。".to_string(),
        ));
    } else {
        set_user_environment_variable()?;
        changed = true;
        steps.push(ok_step(
            "userEnvironment",
            "已设置 CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE=1。".to_string(),
        ));
    }

    verify_computer_use_paths(&home, &marketplace_root)?;
    if options.verify_helper_transport {
        verify_helper_transport(&latest_path)?;
    }
    steps.push(ok_step(
        "verify",
        "Computer Use 用户级文件和配置验证通过。".to_string(),
    ));

    Ok(ComputerUseRepairReport {
        status: "ok".to_string(),
        message: if changed {
            "Computer Use 用户级配置已修复。"
        } else {
            "Computer Use 用户级配置已是最新。"
        }
        .to_string(),
        codex_home: home.to_string_lossy().to_string(),
        marketplace_root: marketplace_root.to_string_lossy().to_string(),
        bundled_source: bundled_source.map(|path| path.to_string_lossy().to_string()),
        config_path: config_path.to_string_lossy().to_string(),
        config_backup_path: config_backup_path.or(marketplace_config_backup_path),
        changed,
        steps,
    })
}

fn resolve_bundled_marketplace_source(
    home: &Path,
    override_source: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(source) = override_source.filter(|source| bundled_manifest_path(source).is_file()) {
        return Some(source.to_path_buf());
    }

    let existing = home
        .join(".tmp")
        .join("bundled-marketplaces")
        .join("openai-bundled");
    if bundled_manifest_path(&existing).is_file() {
        return Some(existing);
    }

    installed_bundled_marketplace_root()
        .ok()
        .filter(|source| bundled_manifest_path(source).is_file())
}

fn installed_bundled_marketplace_root() -> anyhow::Result<PathBuf> {
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-AppxPackage -Name OpenAI.Codex | Sort-Object Version -Descending | Select-Object -First 1 -ExpandProperty InstallLocation",
        ])
        .output()
        .context("failed to query OpenAI.Codex AppX package")?;
    if !output.status.success() {
        anyhow::bail!(
            "Get-AppxPackage failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let install_location = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if install_location.is_empty() {
        anyhow::bail!("OpenAI.Codex package is not installed");
    }
    Ok(PathBuf::from(install_location)
        .join("app")
        .join("resources")
        .join("plugins")
        .join("openai-bundled"))
}

fn bundled_manifest_path(root: &Path) -> PathBuf {
    root.join(".agents")
        .join("plugins")
        .join("marketplace.json")
}

fn bundled_plugin_source_exists(marketplace_root: &Path, plugin: &str) -> bool {
    marketplace_root
        .join("plugins")
        .join(plugin)
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
}

fn bundled_plugin_cache_exists(home: &Path, plugin: &str) -> bool {
    home.join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join(plugin)
        .join("latest")
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
}

fn write_computer_use_plugin_tree(root: &Path) -> anyhow::Result<()> {
    write_json_file(
        &root.join(".codex-plugin").join("plugin.json"),
        &plugin_json(),
    )?;
    write_text_file(
        &root.join("skills").join("computer-use").join("SKILL.md"),
        COMPUTER_USE_SKILL_MD,
    )?;
    write_json_file(
        &root
            .join("node_modules")
            .join("@oai")
            .join("sky")
            .join("package.json"),
        &json!({
            "name": "@oai/sky",
            "version": COMPUTER_USE_VERSION,
            "type": "module",
            "private": true
        }),
    )?;
    write_text_file(
        &root
            .join("node_modules")
            .join("@oai")
            .join("sky")
            .join("bin")
            .join("windows")
            .join("codex-computer-use.exe"),
        "# Placeholder executable path for Codex Desktop Windows Computer Use resolution.\r\n# The local helper transport module implements the actual request handling.\r\n",
    )?;
    write_text_file(
        &root
            .join("node_modules")
            .join("@oai")
            .join("sky")
            .join("dist")
            .join("project")
            .join("cua")
            .join("sky_js")
            .join("src")
            .join("targets")
            .join("windows")
            .join("internal")
            .join("helper_transport.js"),
        HELPER_TRANSPORT_JS,
    )?;
    Ok(())
}

fn plugin_json() -> Value {
    json!({
        "name": "computer-use",
        "version": COMPUTER_USE_VERSION,
        "description": "Local Windows Computer Use compatibility helper for Codex Desktop.",
        "author": {"name": "Local"},
        "homepage": "https://openai.com/",
        "repository": "https://openai.com/",
        "license": "Proprietary",
        "keywords": ["computer-use", "windows", "desktop"],
        "skills": "./skills/",
        "interface": {
            "displayName": "Computer Use",
            "shortDescription": "Control this Windows desktop from Codex",
            "longDescription": "Local compatibility plugin that provides the Windows helper paths expected by Codex Desktop Computer Use.",
            "developerName": "Local",
            "category": "Productivity",
            "capabilities": ["Interactive", "Read", "Write"],
            "websiteURL": "https://openai.com/",
            "privacyPolicyURL": "https://openai.com/policies/row-privacy-policy/",
            "termsOfServiceURL": "https://openai.com/policies/row-terms-of-use/",
            "defaultPrompt": ["Look at my screen and help me navigate"],
            "brandColor": "#10A37F",
            "screenshots": []
        }
    })
}

fn update_bundled_marketplace_manifest(marketplace_root: &Path) -> anyhow::Result<()> {
    let manifest_path = bundled_manifest_path(marketplace_root);
    let mut manifest = read_json_file(&manifest_path).unwrap_or_else(|| {
        json!({
            "name": "openai-bundled",
            "interface": {"displayName": "OpenAI Bundled"},
            "plugins": []
        })
    });
    if manifest.get("name").and_then(Value::as_str).is_none() {
        manifest["name"] = json!("openai-bundled");
    }
    if manifest.get("interface").is_none() {
        manifest["interface"] = json!({"displayName": "OpenAI Bundled"});
    }

    let mut plugins = manifest
        .get("plugins")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| entry.get("name").and_then(Value::as_str) != Some("computer-use"))
        .collect::<Vec<_>>();
    for &plugin in PERSISTENT_BUNDLED_PLUGINS {
        ensure_persistent_bundled_plugin_manifest_entry(marketplace_root, &mut plugins, plugin);
    }
    plugins.insert(
        0,
        json!({
            "name": "computer-use",
            "source": {"source": "local", "path": "./plugins/computer-use"},
            "policy": {"installation": "INSTALLED_BY_DEFAULT", "authentication": "ON_INSTALL"},
            "category": "Productivity"
        }),
    );
    manifest["plugins"] = Value::Array(plugins);
    write_json_file(&manifest_path, &manifest)
}

fn ensure_persistent_bundled_plugin_manifest_entry(
    marketplace_root: &Path,
    plugins: &mut Vec<Value>,
    plugin: &str,
) {
    if !bundled_plugin_source_exists(marketplace_root, plugin) {
        return;
    }
    if let Some(entry) = plugins
        .iter_mut()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some(plugin))
    {
        ensure_bundled_plugin_installed_by_default(entry, plugin);
        return;
    }
    plugins.push(json!({
        "name": plugin,
        "source": {"source": "local", "path": format!("./plugins/{plugin}")},
        "policy": {"installation": "INSTALLED_BY_DEFAULT", "authentication": "ON_INSTALL"},
        "category": "Productivity"
    }));
}

fn ensure_bundled_plugin_installed_by_default(entry: &mut Value, plugin: &str) {
    if entry.get("source").is_none() {
        entry["source"] = json!({"source": "local", "path": format!("./plugins/{plugin}")});
    }
    if !entry.get("policy").is_some_and(Value::is_object) {
        entry["policy"] = json!({});
    }
    entry["policy"]["installation"] = json!("INSTALLED_BY_DEFAULT");
    if entry["policy"].get("authentication").is_none() {
        entry["policy"]["authentication"] = json!("ON_INSTALL");
    }
}

fn sync_bundled_plugin_cache(
    home: &Path,
    marketplace_root: &Path,
    plugin: &str,
) -> anyhow::Result<Option<PathBuf>> {
    let source = marketplace_root.join("plugins").join(plugin);
    if !source.join(".codex-plugin").join("plugin.json").is_file() {
        return Ok(None);
    }
    let version = plugin_version(&source)?;
    let cache_root = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join(plugin);
    let version_root = cache_root.join(version);
    assert_under_path(&version_root, &cache_root)?;
    copy_dir_mirror(&source, &version_root)?;
    create_latest_link_or_copy(&cache_root.join("latest"), &version_root)?;
    Ok(Some(version_root))
}

fn update_chrome_native_messaging_manifest(home: &Path) -> anyhow::Result<Option<PathBuf>> {
    let chrome_cache_root = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("chrome");
    let Some(version_root) = latest_cache_version_root(&chrome_cache_root) else {
        return Ok(None);
    };
    let host_exe = version_root
        .join("extension-host")
        .join("windows")
        .join("x64")
        .join("extension-host.exe");
    if !host_exe.is_file() {
        return Ok(None);
    }
    let manifest_path = local_appdata()
        .unwrap_or_default()
        .join("OpenAI")
        .join("extension")
        .join("com.openai.codexextension.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let mut manifest = read_json_file(&manifest_path)
        .ok_or_else(|| anyhow::anyhow!("failed to parse {}", manifest_path.display()))?;
    if manifest.get("path").and_then(Value::as_str) == Some(host_exe.to_string_lossy().as_ref()) {
        return Ok(None);
    }
    backup_file_with_timestamp(&manifest_path)?;
    manifest["path"] = json!(host_exe.to_string_lossy().to_string());
    write_json_file(&manifest_path, &manifest)?;
    Ok(Some(manifest_path))
}

fn remove_stale_chrome_native_host_entries(home: &Path) -> anyhow::Result<usize> {
    let state_path = home.join("chrome-native-hosts.json");
    if !state_path.is_file() {
        return Ok(0);
    }
    let mut state = read_json_file(&state_path)
        .ok_or_else(|| anyhow::anyhow!("failed to parse {}", state_path.display()))?;
    let entries = state
        .get("chromeNativeHosts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if entries.is_empty() {
        return Ok(0);
    }
    let before = entries.len();
    let kept = entries
        .into_iter()
        .filter(|entry| {
            ["extensionHostPath", "browserClientPath"]
                .iter()
                .filter_map(|key| entry.get(*key).and_then(Value::as_str))
                .all(|path| path.trim().is_empty() || Path::new(path).exists())
        })
        .collect::<Vec<_>>();
    let removed = before.saturating_sub(kept.len());
    if removed == 0 {
        return Ok(0);
    }
    let backup = state_path.with_extension("json.stale.bak");
    if !backup.exists() {
        std::fs::copy(&state_path, backup)?;
    }
    state["chromeNativeHosts"] = Value::Array(kept);
    write_json_file(&state_path, &state)?;
    Ok(removed)
}

fn update_codex_config(home: &Path) -> anyhow::Result<(bool, Option<String>)> {
    let config_path = home.join("config.toml");
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut doc = parse_toml_document(&existing)?;

    let before = doc.to_string();
    {
        let plugin =
            nested_table_mut_or_insert(&mut doc, &["plugins", "computer-use@openai-bundled"])?;
        plugin["enabled"] = toml_edit::value(true);
    }
    {
        let plugin = nested_table_mut_or_insert(&mut doc, &["plugins", "browser@openai-bundled"])?;
        plugin["enabled"] = toml_edit::value(true);
    }
    {
        let features = table_mut_or_insert(&mut doc, "features")?;
        features["computer_use"] = toml_edit::value(true);
        features["remote_connections"] = toml_edit::value(true);
    }
    {
        let windows = table_mut_or_insert(&mut doc, "windows")?;
        windows["sandbox"] = toml_edit::value("unelevated");
    }

    let updated = ensure_trailing_newline(doc.to_string());
    if before == doc.to_string() && normalize_newlines(&existing) == normalize_newlines(&updated) {
        return Ok((false, None));
    }
    let backup = backup_config_if_exists(home, &config_path)?;
    crate::settings::atomic_write(&config_path, updated.as_bytes())?;
    Ok((true, backup))
}

fn verify_computer_use_paths(home: &Path, marketplace_root: &Path) -> anyhow::Result<()> {
    let latest = home
        .join("plugins")
        .join("cache")
        .join("openai-bundled")
        .join("computer-use")
        .join("latest");
    for path in [
        bundled_manifest_path(marketplace_root),
        marketplace_root
            .join("plugins")
            .join("computer-use")
            .join(".codex-plugin")
            .join("plugin.json"),
        latest.join(".codex-plugin").join("plugin.json"),
        helper_transport_path(&latest),
    ] {
        if !path.is_file() {
            anyhow::bail!("missing required Computer Use path: {}", path.display());
        }
    }
    for plugin in ["browser", "chrome"] {
        if bundled_plugin_source_exists(marketplace_root, plugin)
            && !bundled_plugin_cache_exists(home, plugin)
        {
            anyhow::bail!(
                "missing required bundled plugin cache path: {}",
                home.join("plugins")
                    .join("cache")
                    .join("openai-bundled")
                    .join(plugin)
                    .join("latest")
                    .join(".codex-plugin")
                    .join("plugin.json")
                    .display()
            );
        }
    }
    let config = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let data = toml::from_str::<toml::Value>(&config).context("config.toml TOML parse failed")?;
    if data
        .get("plugins")
        .and_then(|plugins| plugins.get("computer-use@openai-bundled"))
        .and_then(|plugin| plugin.get("enabled"))
        .and_then(toml::Value::as_bool)
        != Some(true)
    {
        anyhow::bail!(
            "config.toml is missing plugins.\"computer-use@openai-bundled\".enabled=true"
        );
    }
    Ok(())
}

fn verify_helper_transport(latest: &Path) -> anyhow::Result<()> {
    let helper = helper_transport_path(latest);
    let node = std::process::Command::new("node.exe")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success());
    if node.is_none() {
        return Ok(());
    }
    let script = std::env::temp_dir().join(format!(
        "codex-computer-use-verify-{}.mjs",
        timestamp_millis()
    ));
    write_text_file(
        &script,
        r#"
import { pathToFileURL } from "node:url";
const mod = await import(pathToFileURL(process.argv[2]).href);
if (typeof mod.WindowsHelperTransport !== "function") throw new Error("WindowsHelperTransport export is missing");
const transport = new mod.WindowsHelperTransport();
const info = await transport.request("screenInfo", {});
if (!info || typeof info.width !== "number" || info.width <= 0) throw new Error("invalid screenInfo response");
await transport.close();
"#,
    )?;
    let output = std::process::Command::new("node.exe")
        .arg(&script)
        .arg(&helper)
        .output();
    let _ = std::fs::remove_file(&script);
    let output = output?;
    if !output.status.success() {
        anyhow::bail!(
            "helper transport verification failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn helper_transport_path(root: &Path) -> PathBuf {
    root.join("node_modules")
        .join("@oai")
        .join("sky")
        .join("dist")
        .join("project")
        .join("cua")
        .join("sky_js")
        .join("src")
        .join("targets")
        .join("windows")
        .join("internal")
        .join("helper_transport.js")
}

fn plugin_version(plugin_root: &Path) -> anyhow::Result<String> {
    let plugin = read_json_file(&plugin_root.join(".codex-plugin").join("plugin.json"))
        .ok_or_else(|| {
            anyhow::anyhow!("missing plugin manifest under {}", plugin_root.display())
        })?;
    plugin
        .get("version")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "plugin manifest has no version under {}",
                plugin_root.display()
            )
        })
}

fn latest_cache_version_root(cache_root: &Path) -> Option<PathBuf> {
    let latest = cache_root.join("latest");
    if latest.exists() {
        return latest.canonicalize().ok().or(Some(latest));
    }
    std::fs::read_dir(cache_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join(".codex-plugin").join("plugin.json").is_file())
}

fn copy_dir_mirror(source: &Path, destination: &Path) -> anyhow::Result<()> {
    if same_path(source, destination) {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    remove_path(destination)?;
    copy_dir_recursive(source, destination)
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)
        .with_context(|| format!("failed to read directory {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "failed to copy {} -> {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn create_latest_link_or_copy(latest: &Path, target: &Path) -> anyhow::Result<()> {
    if let Some(parent) = latest.parent() {
        std::fs::create_dir_all(parent)?;
        assert_under_path(latest, parent)?;
    }
    remove_path(latest)?;
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(latest)
            .arg(target)
            .output();
        if let Ok(output) = output {
            if output.status.success() {
                return Ok(());
            }
        }
    }
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(target, latest).is_ok() {
            return Ok(());
        }
    }
    copy_dir_recursive(target, latest)
}

fn remove_path(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return std::fs::remove_dir(path).map_err(Into::into);
            }
        }
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn set_user_environment_variable() -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        crate::windows_integration::set_current_user_string_value(
            "Environment",
            "CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE",
            "1",
        )?;
    }
    // SAFETY: This is called synchronously from the repair command so the current
    // process sees the same value it persists for newly launched Codex processes.
    unsafe {
        std::env::set_var("CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE", "1");
    }
    Ok(())
}

fn user_environment_enabled() -> bool {
    std::env::var("CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn parse_toml_document(contents: &str) -> anyhow::Result<DocumentMut> {
    let normalized = crate::relay_config::normalize_config_text(contents);
    if normalized.trim().is_empty() {
        Ok(DocumentMut::new())
    } else {
        normalized
            .parse::<DocumentMut>()
            .map_err(|error| anyhow::anyhow!("config.toml TOML 解析失败：{error}"))
    }
}

fn table_mut_or_insert<'a>(doc: &'a mut DocumentMut, key: &str) -> anyhow::Result<&'a mut Table> {
    if doc.get(key).and_then(Item::as_table).is_none() {
        doc[key] = toml_edit::table();
    }
    doc.get_mut(key)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| anyhow::anyhow!("{key} 必须是 TOML table"))
}

fn nested_table_mut_or_insert<'a>(
    doc: &'a mut DocumentMut,
    keys: &[&str],
) -> anyhow::Result<&'a mut Table> {
    let Some((first, rest)) = keys.split_first() else {
        anyhow::bail!("nested TOML path cannot be empty");
    };
    let mut table = table_mut_or_insert(doc, first)?;
    for key in rest {
        if table.get(key).and_then(Item::as_table).is_none() {
            table[*key] = toml_edit::table();
        }
        table = table
            .get_mut(key)
            .and_then(Item::as_table_mut)
            .ok_or_else(|| anyhow::anyhow!("{} must be TOML table", keys.join(".")))?;
    }
    Ok(table)
}

fn backup_config_if_exists(home: &Path, config_path: &Path) -> anyhow::Result<Option<String>> {
    if !config_path.is_file() {
        return Ok(None);
    }
    let backup_dir = home
        .join("backups")
        .join(format!("codex-plus-computer-use-{}", timestamp_millis()));
    std::fs::create_dir_all(&backup_dir)?;
    std::fs::copy(config_path, backup_dir.join("config.toml"))?;
    Ok(Some(backup_dir.to_string_lossy().to_string()))
}

fn backup_file_with_timestamp(path: &Path) -> anyhow::Result<PathBuf> {
    let backup = path.with_extension(format!(
        "{}.{}.bak",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bak"),
        timestamp_millis()
    ));
    std::fs::copy(path, &backup)?;
    Ok(backup)
}

fn write_json_file(path: &Path, value: &Value) -> anyhow::Result<()> {
    write_text_file(path, &format!("{}\n", serde_json::to_string_pretty(value)?))
}

fn write_text_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::settings::atomic_write(path, contents.as_bytes())
}

fn read_json_file(path: &Path) -> Option<Value> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn assert_under_path(path: &Path, parent: &Path) -> anyhow::Result<()> {
    let full =
        path.parent().unwrap_or(path).canonicalize().or_else(|_| {
            Ok::<PathBuf, std::io::Error>(path.parent().unwrap_or(path).to_path_buf())
        })?;
    let root = parent
        .canonicalize()
        .or_else(|_| Ok::<PathBuf, std::io::Error>(parent.to_path_buf()))?;
    if !full.starts_with(&root) {
        anyhow::bail!(
            "refusing to modify path outside expected root: {}",
            path.display()
        );
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn local_appdata() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn chrome_native_messaging_manifest_path() -> Option<PathBuf> {
    Some(
        local_appdata()?
            .join("OpenAI")
            .join("extension")
            .join("com.openai.codexextension.json"),
    )
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn ensure_trailing_newline(mut contents: String) -> String {
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents
}

fn normalize_newlines(contents: &str) -> String {
    contents.replace("\r\n", "\n")
}

fn ok_step(name: impl Into<String>, message: impl Into<String>) -> ComputerUseRepairStep {
    ComputerUseRepairStep {
        name: name.into(),
        status: "ok".to_string(),
        message: message.into(),
    }
}

fn skip_step(name: impl Into<String>, message: impl Into<String>) -> ComputerUseRepairStep {
    ComputerUseRepairStep {
        name: name.into(),
        status: "skipped".to_string(),
        message: message.into(),
    }
}

fn warn_step(name: impl Into<String>, message: impl Into<String>) -> ComputerUseRepairStep {
    ComputerUseRepairStep {
        name: name.into(),
        status: "warning".to_string(),
        message: message.into(),
    }
}

const COMPUTER_USE_SKILL_MD: &str = r#"---
name: computer-use
description: Local Windows Computer Use compatibility helper for Codex Desktop. Provides the @oai/sky paths that the Desktop app expects when CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE=1.
---

# Computer Use

This local compatibility plugin is installed by Codex++. It supplies the Windows helper transport paths that Codex Desktop resolves for Computer Use.

The Desktop app must be launched with `CODEX_ELECTRON_ENABLE_WINDOWS_COMPUTER_USE=1`. Codex++ writes that as a user environment variable, so restart Codex after installation.
"#;

const HELPER_TRANSPORT_JS: &str = r##"import { execFile } from "node:child_process";
import { appendFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const logPath = join(process.env.LOCALAPPDATA || process.env.TEMP || ".", "OpenAI", "Codex", "computer-use-local-helper.log");

async function log(entry) {
  try {
    await mkdir(dirname(logPath), { recursive: true });
    await appendFile(logPath, `${new Date().toISOString()} ${JSON.stringify(entry)}\n`, "utf8");
  } catch {}
}

function encodePowerShell(script) {
  return Buffer.from(script, "utf16le").toString("base64");
}

async function runPowerShell(script, timeout = 30000) {
  const { stdout } = await execFileAsync("powershell.exe", ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", encodePowerShell(script)], {
    encoding: "utf8",
    env: process.env,
    timeout,
    windowsHide: true,
    maxBuffer: 64 * 1024 * 1024,
  });
  const text = stdout.trim();
  return text.length === 0 ? null : JSON.parse(text);
}

function numberFrom(params, names, fallback = 0) {
  for (const name of names) {
    const value = params?.[name];
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value))) return Number(value);
  }
  return fallback;
}

function buttonFrom(params) {
  const raw = String(params?.button || params?.mouseButton || "left").toLowerCase();
  if (raw.includes("right")) return "right";
  if (raw.includes("middle")) return "middle";
  return "left";
}

function keyFrom(params) {
  return String(params?.key || params?.keys || params?.text || params?.value || "");
}

function textFrom(params) {
  return String(params?.text ?? params?.value ?? params?.input ?? "");
}

const user32Script = `
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class CodexUser32 {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, int dwData, UIntPtr dwExtraInfo);
}
"@
`;

function mouseFlags(button, action) {
  if (button === "right") return action === "down" ? "0x0008" : "0x0010";
  if (button === "middle") return action === "down" ? "0x0020" : "0x0040";
  return action === "down" ? "0x0002" : "0x0004";
}

async function screenshot() {
  return await runPowerShell(`
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
$bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
$bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bounds.Size)
$stream = New-Object System.IO.MemoryStream
$bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()
$bytes = $stream.ToArray()
$stream.Dispose()
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::Write((ConvertTo-Json -Compress @{
  mimeType = "image/png"
  data = [Convert]::ToBase64String($bytes)
  width = $bounds.Width
  height = $bounds.Height
  left = $bounds.Left
  top = $bounds.Top
}))
`, 30000);
}

async function screenInfo() {
  return await runPowerShell(`
Add-Type -AssemblyName System.Windows.Forms
$bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::Write((ConvertTo-Json -Compress @{
  width = $bounds.Width
  height = $bounds.Height
  left = $bounds.Left
  top = $bounds.Top
}))
`);
}

async function moveMouse(params) {
  const x = Math.round(numberFrom(params, ["x", "X", "left"]));
  const y = Math.round(numberFrom(params, ["y", "Y", "top"]));
  return await runPowerShell(`
${user32Script}
[CodexUser32]::SetCursorPos(${x}, ${y}) | Out-Null
[Console]::Write('{"ok":true}')
`);
}

async function clickMouse(params, count = 1) {
  const x = Math.round(numberFrom(params, ["x", "X", "left"], Number.NaN));
  const y = Math.round(numberFrom(params, ["y", "Y", "top"], Number.NaN));
  const button = buttonFrom(params);
  const down = mouseFlags(button, "down");
  const up = mouseFlags(button, "up");
  const maybeMove = Number.isFinite(x) && Number.isFinite(y) ? `[CodexUser32]::SetCursorPos(${x}, ${y}) | Out-Null` : "";
  return await runPowerShell(`
${user32Script}
${maybeMove}
for ($i = 0; $i -lt ${count}; $i++) {
  [CodexUser32]::mouse_event(${down}, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 35
  [CodexUser32]::mouse_event(${up}, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 70
}
[Console]::Write('{"ok":true}')
`);
}

async function dragMouse(params) {
  const fromX = Math.round(numberFrom(params, ["fromX", "startX", "x1", "x"]));
  const fromY = Math.round(numberFrom(params, ["fromY", "startY", "y1", "y"]));
  const toX = Math.round(numberFrom(params, ["toX", "endX", "x2"]));
  const toY = Math.round(numberFrom(params, ["toY", "endY", "y2"]));
  return await runPowerShell(`
${user32Script}
[CodexUser32]::SetCursorPos(${fromX}, ${fromY}) | Out-Null
Start-Sleep -Milliseconds 80
[CodexUser32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 120
[CodexUser32]::SetCursorPos(${toX}, ${toY}) | Out-Null
Start-Sleep -Milliseconds 120
[CodexUser32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
[Console]::Write('{"ok":true}')
`);
}

async function scrollMouse(params) {
  const delta = Math.round(numberFrom(params, ["delta", "wheelDelta"], 0) || -120 * numberFrom(params, ["amount", "clicks"], 1));
  return await runPowerShell(`
${user32Script}
[CodexUser32]::mouse_event(0x0800, 0, 0, ${delta}, [UIntPtr]::Zero)
[Console]::Write('{"ok":true}')
`);
}

function sendKeysLiteral(text) {
  return text
    .replaceAll("{", "{{}")
    .replaceAll("}", "{}}")
    .replaceAll("+", "{+}")
    .replaceAll("^", "{^}")
    .replaceAll("%", "{%}")
    .replaceAll("~", "{~}")
    .replaceAll("(", "{(}")
    .replaceAll(")", "{)}")
    .replaceAll("[", "{[}")
    .replaceAll("]", "{]}")
    .replaceAll("\n", "{ENTER}");
}

function normalizeKey(key) {
  const value = String(key).trim();
  const upper = value.toUpperCase();
  const aliases = {
    ENTER: "{ENTER}", RETURN: "{ENTER}", ESC: "{ESC}", ESCAPE: "{ESC}", TAB: "{TAB}",
    BACKSPACE: "{BACKSPACE}", DELETE: "{DELETE}", DEL: "{DELETE}", SPACE: " ",
    UP: "{UP}", DOWN: "{DOWN}", LEFT: "{LEFT}", RIGHT: "{RIGHT}", HOME: "{HOME}",
    END: "{END}", PAGEUP: "{PGUP}", PAGEDOWN: "{PGDN}",
  };
  if (aliases[upper]) return aliases[upper];
  if (/^F([1-9]|1[0-2])$/.test(upper)) return `{${upper}}`;
  return sendKeysLiteral(value);
}

async function sendKeys(keys) {
  const encoded = Buffer.from(keys, "utf8").toString("base64");
  return await runPowerShell(`
Add-Type -AssemblyName System.Windows.Forms
$keys = [System.Text.Encoding]::UTF8.GetString([Convert]::FromBase64String("${encoded}"))
[System.Windows.Forms.SendKeys]::SendWait($keys)
[Console]::Write('{"ok":true}')
`);
}

async function typeText(params) {
  return await sendKeys(sendKeysLiteral(textFrom(params)));
}

async function keypress(params) {
  return await sendKeys(normalizeKey(keyFrom(params)));
}

export class WindowsHelperTransport {
  constructor({ helperArgs = [], helperCommand = null } = {}) {
    this.helperArgs = helperArgs;
    this.helperCommand = helperCommand;
    log({ event: "transport-created", helperCommand, helperArgs }).catch(() => {});
  }

  async request(method, params = {}, options = {}) {
    await log({ event: "request", method, params, hasTurnMetadata: !!options?.codexTurnMetadata });
    const name = String(method || "").replace(/[-_]/g, "").toLowerCase();
    if (name === "ping") return "pong";
    if (["screenshot", "takescreenshot", "capture", "captureimage", "capturescreen", "screencapture"].includes(name)) return await screenshot(params);
    if (["screeninfo", "getscreeninfo", "displays", "getdisplays", "screenstate"].includes(name)) return await screenInfo(params);
    if (["movemouse", "mousemove", "move"].includes(name)) return await moveMouse(params);
    if (["click", "mouseclick", "clickmouse"].includes(name)) return await clickMouse(params, 1);
    if (["doubleclick", "mousedoubleclick"].includes(name)) return await clickMouse(params, 2);
    if (["drag", "mousedrag", "dragmouse"].includes(name)) return await dragMouse(params);
    if (["scroll", "mousescroll", "scrollmouse"].includes(name)) return await scrollMouse(params);
    if (["type", "typetext", "text"].includes(name)) return await typeText(params);
    if (["keypress", "presskey", "key", "sendkey"].includes(name)) return await keypress(params);
    if (["close", "shutdown"].includes(name)) return { ok: true };
    await log({ event: "unknown-method", method, params });
    throw new Error(`Unsupported local Computer Use helper method: ${method}`);
  }

  async close() {
    await log({ event: "transport-closed" });
  }
}
"##;
