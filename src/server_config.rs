use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::util;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub master_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct StoredServerConfig {
    pub master_key: Option<String>,
}

pub fn load_or_create() -> Result<ServerConfig> {
    let path = config_path()?;
    load_or_create_at(&path)
}

pub fn load_existing() -> Result<ServerConfig> {
    let path = config_path()?;
    load_existing_at(&path)
}

fn config_path() -> Result<PathBuf> {
    Ok(util::config_dir()?.join("server.json5"))
}

fn load_or_create_at(path: &Path) -> Result<ServerConfig> {
    match fs::read_to_string(path) {
        Ok(content) => {
            let stored = parse_stored_config(path, &content)?;
            let config = normalize_config(stored);
            save_if_needed(path, &config, &content)?;
            Ok(config)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let config = ServerConfig {
                master_key: util::generate_key("master"),
            };
            save(path, &config)?;
            Ok(config)
        }
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn load_existing_at(path: &Path) -> Result<ServerConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let stored = parse_stored_config(path, &content)?;
    let master_key = stored
        .master_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing master_key in {}", path.display()))?;
    Ok(ServerConfig { master_key })
}

fn parse_stored_config(path: &Path, content: &str) -> Result<StoredServerConfig> {
    json5::from_str(content).with_context(|| format!("failed to parse {}", path.display()))
}

fn normalize_config(stored: StoredServerConfig) -> ServerConfig {
    let master_key = stored
        .master_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| util::generate_key("master"));
    ServerConfig { master_key }
}

fn save_if_needed(path: &Path, config: &ServerConfig, original_content: &str) -> Result<()> {
    let serialized = serialize(config)?;
    if original_content != serialized {
        save_serialized(path, &serialized)?;
    }
    Ok(())
}

fn save(path: &Path, config: &ServerConfig) -> Result<()> {
    let serialized = serialize(config)?;
    save_serialized(path, &serialized)
}

fn serialize(config: &ServerConfig) -> Result<String> {
    let stored = StoredServerConfig {
        master_key: Some(config.master_key.clone()),
    };
    let mut serialized = json5::to_string(&stored).context("failed to serialize server config")?;
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

    use super::{load_existing_at, load_or_create_at};

    fn temp_config_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("hurryvc-test-{}", Uuid::new_v4()))
            .join(".config")
            .join("hurryvc")
            .join("server.json5")
    }

    #[test]
    fn creates_config_when_missing() -> Result<()> {
        let path = temp_config_path();
        let config = load_or_create_at(&path)?;
        let written = fs::read_to_string(&path)?;

        assert!(config.master_key.starts_with("master-"));
        assert!(written.contains("master_key"));

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(|parent| parent.parent())
                .and_then(|parent| parent.parent())
                .expect("config path has temp root"),
        );
        Ok(())
    }

    #[test]
    fn reads_existing_json5_config() -> Result<()> {
        let path = temp_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, "{ master_key: 'master-existing' }\n")?;

        let config = load_or_create_at(&path)?;

        assert_eq!(config.master_key, "master-existing");

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(|parent| parent.parent())
                .and_then(|parent| parent.parent())
                .expect("config path has temp root"),
        );
        Ok(())
    }

    #[test]
    fn load_existing_requires_master_key() -> Result<()> {
        let path = temp_config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, "{ }\n")?;

        let error = load_existing_at(&path).expect_err("missing master_key should fail");
        assert!(error.to_string().contains("missing master_key"));

        let _ = fs::remove_dir_all(
            path.parent()
                .and_then(|parent| parent.parent())
                .and_then(|parent| parent.parent())
                .expect("config path has temp root"),
        );
        Ok(())
    }
}
