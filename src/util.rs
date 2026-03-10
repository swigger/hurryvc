use std::{env, path::PathBuf};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;

pub fn generate_key(prefix: &str) -> String {
    let mut bytes = [0u8; 32]; // 256 bits
    rand::rng().fill_bytes(&mut bytes);
    format!("{prefix}-{}", URL_SAFE_NO_PAD.encode(bytes))
}

pub fn mask_secret(secret: &str) -> String {
    let preview = secret.chars().take(10).collect::<String>();
    format!("{preview}...")
}

pub fn derive_ws_url(base: &str, path_suffix: &str) -> Result<String> {
    let parsed = url::Url::parse(base)
        .or_else(|_| url::Url::parse(&format!("http://{base}")))
        .with_context(|| format!("invalid server url: {base}"))?;
    let mut parsed = parsed;
    match parsed.scheme() {
        "http" => parsed.set_scheme("ws").expect("http -> ws"),
        "https" => parsed.set_scheme("wss").expect("https -> wss"),
        "ws" | "wss" => {}
        other => anyhow::bail!("unsupported url scheme: {other}"),
    }

    let current_path = parsed.path().trim_end_matches('/');
    let suffix = path_suffix.trim_start_matches('/');
    let next_path = if current_path.is_empty() || current_path == "/" {
        format!("/{suffix}")
    } else if current_path.ends_with(suffix) {
        current_path.to_string()
    } else {
        format!("{current_path}/{suffix}")
    };
    parsed.set_path(&next_path);
    Ok(parsed.to_string())
}

pub fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME/USERPROFILE is not set"))
}

pub fn config_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".config").join("hurryvc"))
}

#[cfg(debug_assertions)]
pub fn debug_web_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("hurryvc-ui")
        .join("dist")
}
