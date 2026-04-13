//! ffmpeg subprocess wrappers for the compose pipeline.
//!
//! Pipeline for a single clip:
//!   prepare_clip_single → prepare_audio → mux
//!
//! Pipeline for multiple clips:
//!   scale_and_crop (×N) → concat_videos → loop_and_trim_video
//!   prepare_audio → mux

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// Target social aspect ratio: 9:16 vertical (720 × 1280)
const TARGET_W: u32 = 720;
const TARGET_H: u32 = 1280;

/// Verify ffmpeg is available on PATH before starting a long pipeline.
pub fn check() -> Result<()> {
    Command::new("ffmpeg")
        .args(["-version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("ffmpeg not found — install it: https://ffmpeg.org/download.html")?;
    Ok(())
}

/// Single-clip fast path: scale, crop, loop-to-fill, trim — one encode pass.
pub fn prepare_clip_single(input: &Path, output: &Path, duration_secs: u32) -> Result<()> {
    let vf = format!(
        "scale={TARGET_W}:{TARGET_H}:force_original_aspect_ratio=increase,crop={TARGET_W}:{TARGET_H}"
    );
    ffmpeg(&[
        "-stream_loop",
        "-1",
        "-i",
        path(input),
        "-vf",
        &vf,
        "-t",
        &duration_secs.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "23",
        "-an", // strip source audio; we'll mux our own sound
        "-y",
        path(output),
    ])
}

/// Scale and crop a clip to the target resolution (h264 output, no audio).
/// Used before concat when there are multiple clips.
pub fn scale_and_crop(input: &Path, output: &Path) -> Result<()> {
    let vf = format!(
        "scale={TARGET_W}:{TARGET_H}:force_original_aspect_ratio=increase,crop={TARGET_W}:{TARGET_H}"
    );
    ffmpeg(&[
        "-i",
        path(input),
        "-vf",
        &vf,
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "23",
        "-an",
        "-y",
        path(output),
    ])
}

/// Concatenate multiple same-codec video files (stream copy — no re-encode).
pub fn concat_videos(inputs: &[PathBuf], output: &Path) -> Result<()> {
    // Write a temporary ffmpeg concat list next to the output file.
    let list_path = output
        .parent()
        .unwrap_or(Path::new("."))
        .join("_concat_list.txt");

    let list_content: String = inputs
        .iter()
        .map(|p| format!("file '{}'\n", p.display()))
        .collect();
    fs::write(&list_path, list_content)?;

    let result = ffmpeg(&[
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        path(&list_path),
        "-c",
        "copy",
        "-y",
        path(output),
    ]);

    let _ = fs::remove_file(&list_path);
    result
}

/// Loop a video to fill `duration_secs`, re-encoding (needed for stream_loop).
pub fn loop_and_trim_video(input: &Path, output: &Path, duration_secs: u32) -> Result<()> {
    ffmpeg(&[
        "-stream_loop",
        "-1",
        "-i",
        path(input),
        "-t",
        &duration_secs.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        "fast",
        "-crf",
        "23",
        "-an",
        "-y",
        path(output),
    ])
}

/// Normalize audio loudness (loudnorm), loop to fill duration, encode as AAC.
pub fn prepare_audio(input: &Path, output: &Path, duration_secs: u32) -> Result<()> {
    ffmpeg(&[
        "-stream_loop",
        "-1",
        "-i",
        path(input),
        "-af",
        "loudnorm=I=-14:LRA=11:TP=-1",
        "-t",
        &duration_secs.to_string(),
        "-ar",
        "44100",
        "-c:a",
        "aac",
        "-y",
        path(output),
    ])
}

/// Final mux: combine the prepared video and audio tracks.
pub fn mux(video: &Path, audio: &Path, output: &Path, duration_secs: u32) -> Result<()> {
    ffmpeg(&[
        "-i",
        path(video),
        "-i",
        path(audio),
        "-c:v",
        "copy",
        "-c:a",
        "copy",
        "-map",
        "0:v:0",
        "-map",
        "1:a:0",
        "-t",
        &duration_secs.to_string(),
        "-y",
        path(output),
    ])
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn ffmpeg(args: &[&str]) -> Result<()> {
    let status = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::null()) // ffmpeg writes progress to stderr; keep stdout clean
        .status()
        .context("ffmpeg not found — install it: https://ffmpeg.org/download.html")?;

    if !status.success() {
        bail!("ffmpeg exited with {status}");
    }
    Ok(())
}

/// Convert a Path to &str, panicking loudly if it contains non-UTF-8 bytes.
fn path(p: &Path) -> &str {
    p.to_str().expect("path contains non-UTF-8 characters")
}
