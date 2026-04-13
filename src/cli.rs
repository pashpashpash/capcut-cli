use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use crate::models::{
    AppReport, AssetKind, ComposeResultReport, DiscoverSource, DiscoveryReport, ImportReport,
    LibraryListReport,
};
use crate::{downloader, ffmpeg, library};

// ---------------------------------------------------------------------------
// Root CLI
// ---------------------------------------------------------------------------

#[derive(Debug, Parser)]
#[command(
    name = "capcut-cli",
    version,
    about = "Agent-first CLI for discovering and composing short-form social video"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let report = match self.command {
            Command::Discover(a) => a.run(),
            Command::Library(a) => a.run(),
            Command::Compose(a) => a.run(),
        }?;

        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Discover(DiscoverArgs),
    Library(LibraryArgs),
    Compose(ComposeArgs),
}

// ---------------------------------------------------------------------------
// discover
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct DiscoverArgs {
    #[arg(value_enum)]
    source: DiscoverSourceArg,

    #[arg(long)]
    query: Option<String>,

    #[arg(long, default_value_t = 10)]
    limit: u32,
}

impl DiscoverArgs {
    fn run(self) -> Result<AppReport> {
        let (source, notes, next_steps) = match self.source {
            DiscoverSourceArg::TiktokSounds => (
                DiscoverSource::TiktokSounds,
                vec![
                    "Official TikTok APIs are weak for trending sound discovery".to_string(),
                    "MVP should use provider adapters, scraper adapters, or import mode".to_string(),
                    "Keep direct scraping optional because anti-bot measures will change"
                        .to_string(),
                ],
                vec![
                    "Add provider adapters with consistent normalized sound metadata".to_string(),
                    "Support import by sound URL or sound ID for manual seeding".to_string(),
                ],
            ),
            DiscoverSourceArg::XClips => (
                DiscoverSource::XClips,
                vec![
                    "Prototype discovery via X search plus engagement metrics".to_string(),
                    "Require attached video media and rank by likes, reposts, replies, quotes, views, and recency".to_string(),
                    "Media retrieval may still require a separate downloader/import adapter"
                        .to_string(),
                ],
                vec![
                    "Add X API credential support and search adapters".to_string(),
                    "Add downloader abstraction for video asset retrieval".to_string(),
                ],
            ),
        };

        Ok(AppReport::Discovery(DiscoveryReport {
            source,
            query: self.query,
            limit: self.limit,
            notes,
            next_steps,
        }))
    }
}

#[derive(Clone, Debug, ValueEnum)]
enum DiscoverSourceArg {
    #[value(name = "tiktok-sounds")]
    TiktokSounds,
    #[value(name = "x-clips")]
    XClips,
}

// ---------------------------------------------------------------------------
// library
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct LibraryArgs {
    #[command(subcommand)]
    subcommand: LibrarySubcommand,
}

impl LibraryArgs {
    fn run(self) -> Result<AppReport> {
        match self.subcommand {
            LibrarySubcommand::Import(a) => a.run(),
            LibrarySubcommand::List(a) => a.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum LibrarySubcommand {
    Import(ImportArgs),
    List(ListArgs),
}

// library import <url> --type sound|clip

#[derive(Debug, Args)]
struct ImportArgs {
    url: String,

    #[arg(long = "type", value_enum)]
    asset_type: AssetTypeArg,
}

impl ImportArgs {
    fn run(self) -> Result<AppReport> {
        let mut lib = library::Library::open()?;

        let asset = match self.asset_type {
            AssetTypeArg::Sound => downloader::import_sound(&self.url, &lib.sounds_dir())?,
            AssetTypeArg::Clip => downloader::import_clip(&self.url, &lib.clips_dir())?,
        };

        lib.add_asset(asset.clone())?;

        eprintln!("Saved {} to library.", asset.id);
        Ok(AppReport::Import(ImportReport { asset }))
    }
}

// library list [--type sound|clip]

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long = "type", value_enum)]
    asset_type: Option<AssetTypeArg>,
}

impl ListArgs {
    fn run(self) -> Result<AppReport> {
        let lib = library::Library::open()?;
        let kind = self.asset_type.map(|t| match t {
            AssetTypeArg::Sound => AssetKind::Sound,
            AssetTypeArg::Clip => AssetKind::Clip,
        });
        let assets: Vec<_> = lib.list_assets(kind).into_iter().cloned().collect();
        let total = assets.len();
        Ok(AppReport::LibraryList(LibraryListReport { assets, total }))
    }
}

// ---------------------------------------------------------------------------
// compose
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
struct ComposeArgs {
    /// ID of the sound asset to use (from `library import --type sound`).
    #[arg(long)]
    sound: String,

    /// ID(s) of clip asset(s) to use (repeatable: --clip id1 --clip id2).
    #[arg(long = "clip", required = true)]
    clips: Vec<String>,

    /// Target duration of the output video in seconds.
    #[arg(long, default_value_t = 30)]
    duration_seconds: u32,

    /// Where to write the output MP4. Defaults to the library renders directory.
    #[arg(long)]
    output: Option<PathBuf>,
}

impl ComposeArgs {
    fn run(self) -> Result<AppReport> {
        // --- Pre-flight ---
        ffmpeg::check()?;

        let lib = library::Library::open()?;

        let sound = lib.get_asset(&self.sound).ok_or_else(|| {
            anyhow::anyhow!(
                "sound '{}' not found in library — import it first:\n  capcut-cli library import <url> --type sound",
                self.sound
            )
        })?;

        let clips: Vec<_> = self
            .clips
            .iter()
            .map(|id| {
                lib.get_asset(id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "clip '{}' not found in library — import it first:\n  capcut-cli library import <url> --type clip",
                        id
                    )
                })
            })
            .collect::<Result<_>>()?;

        // Verify local files still exist.
        if !Path::new(&sound.local_path).exists() {
            bail!(
                "sound file missing at '{}' — was it moved or deleted?",
                sound.local_path
            );
        }
        for clip in &clips {
            if !Path::new(&clip.local_path).exists() {
                bail!(
                    "clip file missing at '{}' — was it moved or deleted?",
                    clip.local_path
                );
            }
        }

        // --- Paths ---
        let tmp = std::env::temp_dir().join(format!("capcut_{}", std::process::id()));
        std::fs::create_dir_all(&tmp)?;

        let output_path = match self.output {
            Some(p) => p,
            None => {
                let hex = Uuid::new_v4().simple().to_string();
                let renders = lib.renders_dir();
                std::fs::create_dir_all(&renders)?;
                renders.join(format!("render_{}.mp4", &hex[..8]))
            }
        };

        // --- Pipeline ---

        // Step 1: prepare video track
        eprintln!("[1/3] Preparing video track...");
        let video_raw = tmp.join("video_raw.mp4");

        if clips.len() == 1 {
            // Single clip: scale + crop + loop/trim in one pass.
            ffmpeg::prepare_clip_single(
                Path::new(&clips[0].local_path),
                &video_raw,
                self.duration_seconds,
            )?;
        } else {
            // Multiple clips: scale+crop each → concat → loop/trim.
            let scaled: Vec<PathBuf> = clips
                .iter()
                .enumerate()
                .map(|(i, clip)| {
                    let p = tmp.join(format!("scaled_{i}.mp4"));
                    ffmpeg::scale_and_crop(Path::new(&clip.local_path), &p)?;
                    Ok(p)
                })
                .collect::<Result<_>>()?;

            let concat = tmp.join("concat.mp4");
            ffmpeg::concat_videos(&scaled, &concat)?;
            ffmpeg::loop_and_trim_video(&concat, &video_raw, self.duration_seconds)?;
        }

        // Step 2: prepare audio track (normalize + loop/trim → AAC).
        eprintln!("[2/3] Preparing audio track...");
        let audio_ready = tmp.join("audio_ready.aac");
        ffmpeg::prepare_audio(
            Path::new(&sound.local_path),
            &audio_ready,
            self.duration_seconds,
        )?;

        // Step 3: mux video + audio.
        eprintln!("[3/3] Muxing...");
        ffmpeg::mux(&video_raw, &audio_ready, &output_path, self.duration_seconds)?;

        // Clean up temp files.
        let _ = std::fs::remove_dir_all(&tmp);

        Ok(AppReport::ComposeResult(ComposeResultReport {
            output_path: output_path.to_string_lossy().into_owned(),
            sound_id: self.sound,
            clip_ids: self.clips,
            duration_seconds: self.duration_seconds,
            pipeline_steps_run: vec![
                "scale_and_crop".to_string(),
                "trim_clips".to_string(),
                "normalize_audio".to_string(),
                "mux".to_string(),
            ],
        }))
    }
}

#[derive(Clone, Debug, ValueEnum)]
enum AssetTypeArg {
    Sound,
    Clip,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppReport, DiscoverSource};

    // --- discover (pure logic, no external deps) ---

    #[test]
    fn discover_tiktok_sounds_default_limit() {
        let report = DiscoverArgs {
            source: DiscoverSourceArg::TiktokSounds,
            query: None,
            limit: 10,
        }
        .run()
        .unwrap();
        let AppReport::Discovery(d) = report else {
            panic!("wrong variant")
        };
        assert!(matches!(d.source, DiscoverSource::TiktokSounds));
        assert_eq!(d.limit, 10);
        assert!(d.query.is_none());
        assert!(!d.notes.is_empty());
        assert!(!d.next_steps.is_empty());
    }

    #[test]
    fn discover_tiktok_sounds_with_query_and_limit() {
        let report = DiscoverArgs {
            source: DiscoverSourceArg::TiktokSounds,
            query: Some("viral".to_string()),
            limit: 20,
        }
        .run()
        .unwrap();
        let AppReport::Discovery(d) = report else {
            panic!("wrong variant")
        };
        assert_eq!(d.query.as_deref(), Some("viral"));
        assert_eq!(d.limit, 20);
    }

    #[test]
    fn discover_x_clips_returns_discovery_report() {
        let report = DiscoverArgs {
            source: DiscoverSourceArg::XClips,
            query: None,
            limit: 10,
        }
        .run()
        .unwrap();
        let AppReport::Discovery(d) = report else {
            panic!("wrong variant")
        };
        assert!(matches!(d.source, DiscoverSource::XClips));
        assert!(!d.notes.is_empty());
        assert!(!d.next_steps.is_empty());
    }

    // --- clap parse tests (no execution) ---

    #[test]
    fn cli_parses_discover_tiktok_sounds() {
        assert!(Cli::try_parse_from(["capcut-cli", "discover", "tiktok-sounds"]).is_ok());
    }

    #[test]
    fn cli_parses_discover_x_clips_with_query() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "discover",
                "x-clips",
                "--query",
                "trending"
            ])
            .is_ok()
        );
    }

    #[test]
    fn discover_unknown_source_fails_to_parse() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "discover", "instagram"]).is_err()
        );
    }

    #[test]
    fn cli_parses_library_import_sound() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "library",
                "import",
                "https://example.com/sound",
                "--type",
                "sound"
            ])
            .is_ok()
        );
    }

    #[test]
    fn cli_parses_library_import_clip() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "library",
                "import",
                "https://example.com/clip",
                "--type",
                "clip"
            ])
            .is_ok()
        );
    }

    #[test]
    fn library_import_missing_type_fails() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "library",
                "import",
                "https://example.com"
            ])
            .is_err()
        );
    }

    #[test]
    fn library_import_missing_url_fails() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "library", "import", "--type", "sound"]).is_err()
        );
    }

    #[test]
    fn cli_parses_library_list() {
        assert!(Cli::try_parse_from(["capcut-cli", "library", "list"]).is_ok());
    }

    #[test]
    fn cli_parses_library_list_filtered() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "library", "list", "--type", "clip"]).is_ok()
        );
    }

    #[test]
    fn library_list_unknown_type_fails() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "library", "list", "--type", "video"]).is_err()
        );
    }

    #[test]
    fn cli_parses_compose_single_clip() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "compose",
                "--sound",
                "snd_abc",
                "--clip",
                "clp_def"
            ])
            .is_ok()
        );
    }

    #[test]
    fn cli_parses_compose_multi_clip_with_duration() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "compose",
                "--sound",
                "snd_abc",
                "--clip",
                "clp_1",
                "--clip",
                "clp_2",
                "--duration-seconds",
                "15"
            ])
            .is_ok()
        );
    }

    #[test]
    fn cli_parses_compose_with_output_path() {
        assert!(
            Cli::try_parse_from([
                "capcut-cli",
                "compose",
                "--sound",
                "snd_abc",
                "--clip",
                "clp_def",
                "--output",
                "/tmp/out.mp4"
            ])
            .is_ok()
        );
    }

    #[test]
    fn compose_missing_clip_fails_to_parse() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "compose", "--sound", "snd_abc"]).is_err()
        );
    }

    #[test]
    fn compose_missing_sound_fails_to_parse() {
        assert!(
            Cli::try_parse_from(["capcut-cli", "compose", "--clip", "clp_def"]).is_err()
        );
    }

    /// Compose returns a clear error when the sound is not in the library.
    /// Requires filesystem access to open the library — run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn compose_unknown_sound_returns_helpful_error() {
        let err = ComposeArgs {
            sound: "snd_doesnotexist".to_string(),
            clips: vec!["clp_doesnotexist".to_string()],
            duration_seconds: 30,
            output: None,
        }
        .run()
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("snd_doesnotexist"));
        assert!(msg.contains("library import"));
    }

    /// End-to-end library list against the real on-disk library (may be empty).
    /// Run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn library_list_runs_on_real_library() {
        let report = ListArgs { asset_type: None }.run().unwrap();
        let AppReport::LibraryList(l) = report else {
            panic!("wrong variant")
        };
        assert_eq!(l.assets.len(), l.total);
    }
}
