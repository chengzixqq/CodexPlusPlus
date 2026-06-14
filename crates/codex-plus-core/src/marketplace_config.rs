use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::{DocumentMut, Item, Table};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceRepairReport {
    pub status: String,
    pub message: String,
    pub config_path: String,
    pub backup_path: Option<String>,
    pub changed: bool,
    pub marketplaces: Vec<MarketplaceRepairEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplaceRepairEntry {
    pub name: String,
    pub source: String,
    pub configured: bool,
    pub source_exists: bool,
    pub manifest_exists: bool,
    pub manifest_repaired: bool,
    pub plugin_count: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalMarketplaceSpec {
    name: &'static str,
    source: PathBuf,
}

pub fn repair_default_local_marketplace_config() -> anyhow::Result<MarketplaceRepairReport> {
    repair_local_marketplace_config_in_home(&crate::relay_config::default_codex_home_dir())
}

pub fn repair_local_marketplace_config_in_home(
    home: &Path,
) -> anyhow::Result<MarketplaceRepairReport> {
    std::fs::create_dir_all(home)?;
    let config_path = home.join("config.toml");
    let existing = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut doc = parse_config_for_marketplace_repair(&existing)?;
    let mut entries = Vec::new();
    let mut config_changed = false;

    for spec in local_marketplace_specs(home) {
        let mut entry = marketplace_entry_for_spec(&spec);
        if !entry.source_exists {
            entry.message = "本地 marketplace 目录不存在，已跳过。".to_string();
            entries.push(entry);
            continue;
        }

        let manifest = ensure_supported_manifest_layout(&spec.source)?;
        entry.manifest_exists = manifest.exists;
        entry.manifest_repaired = manifest.repaired;
        entry.plugin_count = manifest.plugin_count;
        if !manifest.exists {
            entry.message = "未找到 marketplace.json，已跳过。".to_string();
            entries.push(entry);
            continue;
        }

        entry.configured = true;
        entry.message = "本地 marketplace 已注册。".to_string();
        config_changed |= upsert_local_marketplace(&mut doc, spec.name, &spec.source)?;
        entries.push(entry);
    }

    if remove_stale_curated_alias(&mut doc, home)? {
        config_changed = true;
        entries.push(MarketplaceRepairEntry {
            name: "openai-curated".to_string(),
            source: "official-remote".to_string(),
            configured: false,
            source_exists: false,
            manifest_exists: false,
            manifest_repaired: false,
            plugin_count: None,
            message: "已移除指向本地快照的旧 openai-curated 配置，恢复官方远程插件市场。"
                .to_string(),
        });
    }

    let updated = ensure_trailing_newline(doc.to_string());
    let changed = config_changed;
    let backup_path = if changed {
        let backup_path = backup_config_if_exists(home, &config_path)?;
        crate::settings::atomic_write(&config_path, updated.as_bytes())?;
        backup_path
    } else {
        None
    };

    Ok(MarketplaceRepairReport {
        status: "ok".to_string(),
        message: if changed {
            "本地插件市场配置已修复。"
        } else {
            "本地插件市场配置已是最新。"
        }
        .to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        backup_path,
        changed,
        marketplaces: entries,
    })
}

fn parse_config_for_marketplace_repair(contents: &str) -> anyhow::Result<DocumentMut> {
    let normalized = crate::relay_config::normalize_config_text(contents);
    if normalized.trim().is_empty() {
        Ok(DocumentMut::new())
    } else {
        normalized
            .parse::<DocumentMut>()
            .map_err(|error| anyhow::anyhow!("config.toml TOML 解析失败：{error}"))
    }
}

fn local_marketplace_specs(home: &Path) -> Vec<LocalMarketplaceSpec> {
    let user_home = home.parent().unwrap_or(home);
    let curated = home.join("marketplaces").join("openai-curated-local");
    let role_specific = home.join("marketplaces").join("openai-role-specific");
    vec![
        LocalMarketplaceSpec {
            name: "openai-primary-runtime",
            source: user_home
                .join(".cache")
                .join("codex-runtimes")
                .join("codex-primary-runtime")
                .join("plugins")
                .join("openai-primary-runtime"),
        },
        LocalMarketplaceSpec {
            name: "openai-bundled",
            source: home
                .join(".tmp")
                .join("bundled-marketplaces")
                .join("openai-bundled"),
        },
        LocalMarketplaceSpec {
            name: "openai-curated-local",
            source: curated.clone(),
        },
        LocalMarketplaceSpec {
            name: "openai-role-specific",
            source: role_specific,
        },
    ]
}

fn marketplace_entry_for_spec(spec: &LocalMarketplaceSpec) -> MarketplaceRepairEntry {
    MarketplaceRepairEntry {
        name: spec.name.to_string(),
        source: spec.source.to_string_lossy().to_string(),
        configured: false,
        source_exists: spec.source.is_dir(),
        manifest_exists: false,
        manifest_repaired: false,
        plugin_count: None,
        message: String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestLayoutStatus {
    exists: bool,
    repaired: bool,
    plugin_count: Option<usize>,
}

fn ensure_supported_manifest_layout(source: &Path) -> anyhow::Result<ManifestLayoutStatus> {
    let nested = source
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    let root = source.join("marketplace.json");
    if nested.is_file() {
        return Ok(ManifestLayoutStatus {
            exists: true,
            repaired: false,
            plugin_count: count_manifest_plugins(&nested),
        });
    }
    if !root.is_file() {
        return Ok(ManifestLayoutStatus {
            exists: false,
            repaired: false,
            plugin_count: None,
        });
    }

    if let Some(parent) = nested.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(&root, &nested)?;
    Ok(ManifestLayoutStatus {
        exists: true,
        repaired: true,
        plugin_count: count_manifest_plugins(&nested),
    })
}

fn count_manifest_plugins(path: &Path) -> Option<usize> {
    let contents = std::fs::read_to_string(path).ok()?;
    let value = serde_json::from_str::<Value>(&contents).ok()?;
    value.get("plugins").and_then(Value::as_array).map(Vec::len)
}

fn upsert_local_marketplace(
    doc: &mut DocumentMut,
    name: &str,
    source: &Path,
) -> anyhow::Result<bool> {
    let marketplaces = table_mut_or_insert(doc, "marketplaces")?;
    if marketplaces.get(name).and_then(Item::as_table).is_none() {
        marketplaces[name] = toml_edit::table();
    }
    let table = marketplaces
        .get_mut(name)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| anyhow::anyhow!("marketplaces.{name} 必须是 TOML table"))?;

    let before = table.to_string();
    table["source_type"] = toml_edit::value("local");
    table["source"] = toml_edit::value(source_path_for_toml(source));
    Ok(before != table.to_string())
}

fn remove_stale_curated_alias(doc: &mut DocumentMut, home: &Path) -> anyhow::Result<bool> {
    let curated = home.join("marketplaces").join("openai-curated-local");
    let Some(marketplaces) = doc.get_mut("marketplaces").and_then(Item::as_table_mut) else {
        return Ok(false);
    };
    let Some(curated_table) = marketplaces.get("openai-curated").and_then(Item::as_table) else {
        return Ok(false);
    };

    let source_type = curated_table
        .get("source_type")
        .and_then(Item::as_str)
        .unwrap_or_default();
    let source = curated_table
        .get("source")
        .and_then(Item::as_str)
        .unwrap_or_default();
    if source_type != "local" || !same_config_path(source, &curated) {
        return Ok(false);
    }

    marketplaces.remove("openai-curated");
    Ok(true)
}

fn table_mut_or_insert<'a>(doc: &'a mut DocumentMut, key: &str) -> anyhow::Result<&'a mut Table> {
    if doc.get(key).and_then(Item::as_table).is_none() {
        doc[key] = toml_edit::table();
    }
    doc.get_mut(key)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| anyhow::anyhow!("{key} 必须是 TOML table"))
}

fn backup_config_if_exists(home: &Path, config_path: &Path) -> anyhow::Result<Option<String>> {
    if !config_path.is_file() {
        return Ok(None);
    }
    let backup_dir = home
        .join("backups")
        .join(format!("codex-plus-marketplaces-{}", timestamp_millis()));
    std::fs::create_dir_all(&backup_dir)?;
    std::fs::copy(config_path, backup_dir.join("config.toml"))?;
    Ok(Some(backup_dir.to_string_lossy().to_string()))
}

fn timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn source_path_for_toml(path: &Path) -> String {
    let source = path.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        let source = source.replace('/', "\\");
        if source.starts_with(r"\\?\") {
            source
        } else if path.is_absolute() {
            format!(r"\\?\{source}")
        } else {
            source
        }
    }
    #[cfg(not(windows))]
    {
        source
    }
}

fn same_config_path(configured: &str, expected: &Path) -> bool {
    normalize_config_path(configured) == normalize_config_path(&source_path_for_toml(expected))
}

fn normalize_config_path(path: &str) -> String {
    let mut normalized = path.replace('/', "\\");
    if let Some(stripped) = normalized.strip_prefix(r"\\?\") {
        normalized = stripped.to_string();
    }
    while normalized.ends_with('\\') {
        normalized.pop();
    }
    normalized.to_ascii_lowercase()
}

fn ensure_trailing_newline(mut contents: String) -> String {
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents
}
