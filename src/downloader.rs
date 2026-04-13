use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::models::{Asset, AssetKind};

// ---------------------------------------------------------------------------
// yt-dlp metadata shape (we only care about a small subset)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct YtDlpInfo {
    title: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    extractor: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Download a URL as an MP3 audio file into `dest_dir` and return an Asset.
pub fn import_sound(url: &str, dest_dir: &Path) -> Result<Asset> {
    check_ytdlp()?;

    let id = new_id("snd");
    let template = dest_dir.join(format!("{id}.%(ext)s"));

    eprintln!("Probing {url}...");
    let info = probe(url)?;

    eprintln!("Downloading audio...");
    run_ytdlp(&[
        "-x",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "0",
        "--no-playlist",
        "-o",
        template.to_str().unwrap(),
        url,
    ])?;

    let local_path = find_file(dest_dir, &id)?;

    Ok(Asset {
        id,
        kind: AssetKind::Sound,
        source_url: url.to_string(),
        platform: info.extractor.unwrap_or_else(|| "unknown".to_string()),
        local_path: local_path.to_string_lossy().into_owned(),
        duration_seconds: info.duration.unwrap_or(0.0),
        title: info.title,
        creator: info.uploader,
        added_at: now_unix(),
    })
}

/// Download a URL as an MP4 video file into `dest_dir` and return an Asset.
pub fn import_clip(url: &str, dest_dir: &Path) -> Result<Asset> {
    check_ytdlp()?;

    let id = new_id("clp");
    let template = dest_dir.join(format!("{id}.%(ext)s"));

    eprintln!("Probing {url}...");
    let info = probe(url)?;

    eprintln!("Downloading video...");
    run_ytdlp(&[
        "-f",
        "bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best",
        "--merge-output-format",
        "mp4",
        "--no-playlist",
        "-o",
        template.to_str().unwrap(),
        url,
    ])?;

    let local_path = find_file(dest_dir, &id)?;

    Ok(Asset {
        id,
        kind: AssetKind::Clip,
        source_url: url.to_string(),
        platform: info.extractor.unwrap_or_else(|| "unknown".to_string()),
        local_path: local_path.to_string_lossy().into_owned(),
        duration_seconds: info.duration.unwrap_or(0.0),
        title: info.title,
        creator: info.uploader,
        added_at: now_unix(),
    })
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn check_ytdlp() -> Result<()> {
    Command::new("yt-dlp")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("yt-dlp not found — install it: https://github.com/yt-dlp/yt-dlp#installation")?;
    Ok(())
}

fn probe(url: &str) -> Result<YtDlpInfo> {
    // --dump-json writes the info JSON to stdout; capture it.
    // stderr inherits so the user sees yt-dlp progress.
    let out = Command::new("yt-dlp")
        .args(["--dump-json", "--no-playlist", url])
        .output()
        .context("yt-dlp not found")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("yt-dlp failed to fetch info: {stderr}");
    }

    serde_json::from_slice(&out.stdout).context("failed to parse yt-dlp info JSON")
}

fn run_ytdlp(args: &[&str]) -> Result<()> {
    // Discard yt-dlp stdout so it doesn't contaminate our JSON output.
    // stderr inherits so the user sees download progress.
    let status = Command::new("yt-dlp")
        .args(args)
        .stdout(Stdio::null())
        .status()
        .context("yt-dlp not found")?;

    if !status.success() {
        bail!("yt-dlp exited with status {status}");
    }
    Ok(())
}

/// Find the file in `dir` whose name starts with `stem` (any extension).
fn find_file(dir: &Path, stem: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(dir).context("failed to read download directory")? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(stem) {
            return Ok(entry.path());
        }
    }
    bail!("download finished but output file not found (stem: {stem})")
}

fn new_id(prefix: &str) -> String {
    let hex = Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &hex[..8])
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
