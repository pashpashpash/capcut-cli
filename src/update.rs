use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::Deserialize;

const GITHUB_OWNER: &str = "pashpashpash";
const GITHUB_REPO: &str = "capcut-cli";
const UPDATE_API_BASE_ENV: &str = "CAPCUT_CLI_UPDATE_API_BASE_URL";

#[derive(Debug)]
pub struct UpdateOptions {
    pub bin_path: Option<PathBuf>,
    pub force: bool,
}

#[derive(Debug)]
pub struct UpdateResult {
    pub action: String,
    pub repository: String,
    pub current_version: String,
    pub target_version: String,
    pub status: String,
    pub asset_name: String,
    pub download_url: String,
    pub install_path: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub fn update_cli(options: UpdateOptions) -> Result<UpdateResult> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .context("failed to build HTTP client for update")?;

    let repository = format!("{GITHUB_OWNER}/{GITHUB_REPO}");
    let asset_name = release_asset_name_for_current_target()?;
    let release = fetch_latest_release(&client)?;
    let target_version = normalize_tag(&release.tag_name);
    let download_url = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .map(|asset| asset.browser_download_url.clone())
        .ok_or_else(|| {
            anyhow!(
                "latest release {} does not include asset {}",
                release.tag_name,
                asset_name
            )
        })?;

    let install_path = resolve_install_path(options.bin_path)?;
    let install_exists = install_path.exists();
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    if !options.force && install_exists && target_version == current_version {
        return Ok(UpdateResult {
            action: if install_exists {
                "update".to_string()
            } else {
                "install".to_string()
            },
            repository,
            current_version,
            target_version,
            status: "already_current".to_string(),
            asset_name,
            download_url,
            install_path: install_path.display().to_string(),
        });
    }

    let temp_dir = make_temp_dir()?;
    let archive_path = temp_dir.join(&asset_name);
    let extracted_path = temp_dir.join("capcut-cli");

    let install_action = if install_exists { "update" } else { "install" }.to_string();

    let install_result = (|| -> Result<()> {
        download_release_asset(&client, &download_url, &archive_path)?;
        extract_archive(&archive_path, &temp_dir)?;

        if !extracted_path.exists() {
            bail!(
                "release archive {} did not contain capcut-cli at its root",
                archive_path.display()
            )
        }

        install_binary(&extracted_path, &install_path)
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    install_result?;

    Ok(UpdateResult {
        action: install_action,
        repository,
        current_version,
        target_version,
        status: "updated".to_string(),
        asset_name,
        download_url,
        install_path: install_path.display().to_string(),
    })
}

fn fetch_latest_release(client: &Client) -> Result<GitHubRelease> {
    let api_base =
        env::var(UPDATE_API_BASE_ENV).unwrap_or_else(|_| "https://api.github.com".to_string());

    client
        .get(format!(
            "{}/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest",
            api_base.trim_end_matches('/')
        ))
        .header("Accept", "application/vnd.github+json")
        .header(
            "User-Agent",
            format!("capcut-cli/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .context("failed to request latest GitHub release metadata")?
        .error_for_status()
        .context("latest GitHub release request failed")?
        .json::<GitHubRelease>()
        .context("failed to parse latest GitHub release metadata")
}

fn release_asset_name_for_current_target() -> Result<String> {
    Ok(format!("capcut-cli-{}.tar.gz", rust_target_triple()?))
}

fn rust_target_triple() -> Result<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        (os, arch) => bail!("self-update is not yet supported for target {arch}-{os}"),
    }
}

fn resolve_install_path(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }

    let current_exe =
        env::current_exe().context("failed to determine current executable path for update")?;
    if !is_cargo_target_binary(&current_exe) {
        return Ok(current_exe);
    }

    default_install_path()
}

fn is_cargo_target_binary(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == "target")
}

fn default_install_path() -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| anyhow!("missing HOME for install path"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("bin")
        .join("capcut-cli"))
}

fn make_temp_dir() -> Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis();
    let path = env::temp_dir().join(format!("capcut-cli-update-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(path)
}

fn download_release_asset(client: &Client, url: &str, archive_path: &Path) -> Result<()> {
    let bytes = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .header(
            "User-Agent",
            format!("capcut-cli/{}", env!("CARGO_PKG_VERSION")),
        )
        .send()
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("release download failed for {url}"))?
        .bytes()
        .with_context(|| format!("failed to read bytes from {url}"))?;

    fs::write(archive_path, &bytes)
        .with_context(|| format!("failed to write {}", archive_path.display()))
}

fn extract_archive(archive_path: &Path, destination_dir: &Path) -> Result<()> {
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(archive_path)
        .arg("-C")
        .arg(destination_dir)
        .output()
        .with_context(|| format!("failed to invoke tar for {}", archive_path.display()))?;

    if !output.status.success() {
        bail!(
            "tar failed while extracting {}: {}",
            archive_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }

    Ok(())
}

fn install_binary(extracted_binary: &Path, install_path: &Path) -> Result<()> {
    let parent = install_path.parent().ok_or_else(|| {
        anyhow!(
            "install path {} has no parent directory",
            install_path.display()
        )
    })?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let temp_install = parent.join(format!(".capcut-cli-install-{}.tmp", std::process::id()));
    fs::copy(extracted_binary, &temp_install).with_context(|| {
        format!(
            "failed to stage {} into {}",
            extracted_binary.display(),
            temp_install.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&temp_install)
            .with_context(|| format!("failed to stat {}", temp_install.display()))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&temp_install, permissions)
            .with_context(|| format!("failed to chmod {}", temp_install.display()))?;
    }

    if install_path.exists() {
        fs::remove_file(install_path)
            .with_context(|| format!("failed to remove {}", install_path.display()))?;
    }

    fs::rename(&temp_install, install_path).with_context(|| {
        format!(
            "failed to move {} to {}",
            temp_install.display(),
            install_path.display()
        )
    })
}

fn normalize_tag(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_leading_v_from_tags() {
        assert_eq!(normalize_tag("v1.2.3"), "1.2.3");
        assert_eq!(normalize_tag("1.2.3"), "1.2.3");
    }

    #[test]
    fn current_target_has_expected_asset_name() {
        let asset_name = release_asset_name_for_current_target().expect("asset name");
        assert!(asset_name.starts_with("capcut-cli-"));
        assert!(asset_name.ends_with(".tar.gz"));
    }
}
