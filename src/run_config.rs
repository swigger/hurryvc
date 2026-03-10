use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::util;

pub const DEFAULT_SERVER: &str = "127.0.0.1:6600";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunConfig {
    pub server: String,
    pub group_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct StoredRunConfig {
    pub server: Option<String>,
    pub group_key: Option<String>,
}

pub fn load_or_create() -> Result<RunConfig> {
    let path = config_path()?;
    load_or_create_at(&path)
}

fn config_path() -> Result<PathBuf> {
    Ok(util::config_dir()?.join("run.json5"))
}

fn load_or_create_at(path: &Path) -> Result<RunConfig> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let stored = parse_stored_config(path, &content)?;
            let config = normalize_config(stored);
            save_if_needed(path, &config, &content)?;
            Ok(config)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let config = RunConfig {
                server: DEFAULT_SERVER.to_string(),
                group_key: util::generate_key("p"),
            };
            save(path, &config)?;
            Ok(config)
        }
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn parse_stored_config(path: &Path, content: &str) -> Result<StoredRunConfig> {
    json5::from_str(content).with_context(|| format!("failed to parse {}", path.display()))
}

fn normalize_config(stored: StoredRunConfig) -> RunConfig {
    let server = stored
        .server
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_SERVER.to_string());
    let group_key = stored
        .group_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| util::generate_key("p"));
    RunConfig { server, group_key }
}

fn save_if_needed(path: &Path, config: &RunConfig, original_content: &str) -> Result<()> {
    let serialized = serialize(config)?;
    if original_content != serialized {
        save_serialized(path, &serialized)?;
    }
    Ok(())
}

fn save(path: &Path, config: &RunConfig) -> Result<()> {
    let serialized = serialize(config)?;
    save_serialized(path, &serialized)
}

fn serialize(config: &RunConfig) -> Result<String> {
    let stored = StoredRunConfig {
        server: Some(config.server.clone()),
        group_key: Some(config.group_key.clone()),
    };
    let mut serialized = json5::to_string(&stored).context("failed to serialize run config")?;
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    Ok(serialized)
}

fn save_serialized(path: &Path, serialized: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, serialized).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use anyhow::Result;
    use uuid::Uuid;

    use super::{DEFAULT_SERVER, load_or_create_at};

    fn temp_config_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("hurryvc-run-test-{}", Uuid::new_v4()))
            .join(".config")
            .join("hurryvc")
            .join("run.json5")
    }

    #[test]
    fn creates_run_config_when_missing() -> Result<()> {
        let path = temp_config_path();
        let config = load_or_create_at(&path)?;
        let written = fs::read_to_string(&path)?;

        assert_eq!(config.server, DEFAULT_SERVER);
        assert!(config.group_key.starts_with("p-"));
        assert!(written.contains("group_key"));

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(|parent| parent.parent())
                .and_then(|parent| parent.parent())
                .expect("config path has temp root"),
        );
        Ok(())
    }

    #[test]
    fn reads_existing_json5_run_config() -> Result<()> {
        let path = temp_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, "{ server: 'ws://example.com/base', group_key: 'p-existing' }\n")?;

        let config = load_or_create_at(&path)?;

        assert_eq!(config.server, "ws://example.com/base");
        assert_eq!(config.group_key, "p-existing");

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(|parent| parent.parent())
                .and_then(|parent| parent.parent())
                .expect("config path has temp root"),
        );
        Ok(())
    }
}
