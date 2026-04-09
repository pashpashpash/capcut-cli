use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

pub const APIFY_CONFIG_ENV: &str = "CAPCUT_CLI_APIFY_TOKEN";

const APP_DIR_NAME: &str = "capcut-cli";
const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    apify_api_token: String,
}

#[derive(Debug, Clone, Copy)]
pub enum AuthSource {
    Env,
    ConfigFile,
}

impl AuthSource {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthSource::Env => "env",
            AuthSource::ConfigFile => "config_file",
        }
    }
}

pub struct AuthStatus {
    pub config_path: PathBuf,
    pub env_var: &'static str,
    pub token_present: bool,
    pub configured_via: Option<AuthSource>,
}

pub fn write_apify_token(token: String) -> Result<PathBuf> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(
        &path,
        serde_json::to_vec_pretty(&ConfigFile {
            apify_api_token: token,
        })?,
    )
    .with_context(|| format!("failed to write {}", path.display()))?;

    Ok(path)
}

pub fn load_apify_token() -> Result<String> {
    if let Ok(value) = env::var(APIFY_CONFIG_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let config = read_config()?;
    if config.apify_api_token.trim().is_empty() {
        bail!("Apify token is empty. Run `capcut-cli auth --apify <token>` first.")
    }

    Ok(config.apify_api_token)
}

pub fn read_env_apify_token() -> Result<String> {
    let value = env::var(APIFY_CONFIG_ENV)
        .with_context(|| format!("missing {APIFY_CONFIG_ENV} in the current environment"))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{APIFY_CONFIG_ENV} is set but empty")
    }

    Ok(trimmed.to_string())
}

pub fn apify_auth_status() -> Result<AuthStatus> {
    let config_path = config_path()?;

    if let Ok(value) = env::var(APIFY_CONFIG_ENV) {
        if !value.trim().is_empty() {
            return Ok(AuthStatus {
                config_path,
                env_var: APIFY_CONFIG_ENV,
                token_present: true,
                configured_via: Some(AuthSource::Env),
            });
        }
    }

    let token_present = if config_path.exists() {
        !read_config()?.apify_api_token.trim().is_empty()
    } else {
        false
    };

    Ok(AuthStatus {
        config_path,
        env_var: APIFY_CONFIG_ENV,
        token_present,
        configured_via: token_present.then_some(AuthSource::ConfigFile),
    })
}

fn read_config() -> Result<ConfigFile> {
    let path = config_path()?;
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "missing config at {}. Run `capcut-cli auth --apify <token>` first.",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn config_path() -> Result<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| anyhow!("could not determine config directory"))?;

    Ok(base.join(APP_DIR_NAME).join(CONFIG_FILE_NAME))
}
