use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::models::{
    AppReport, DiscoverSource, DiscoveryReport, LibraryReport, MediaReport, PipelineStep,
    PipelineStepKind,
};

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
            Command::Discover(args) => args.run(),
            Command::Library(args) => args.run(),
            Command::Compose(args) => args.run(),
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
        let (mode, notes, next_steps) = match self.source {
            DiscoverSourceArg::TiktokSounds => (
                DiscoverSource::TiktokSounds,
                vec![
                    "Official TikTok APIs are weak for trending sound discovery".to_string(),
                    "MVP should use provider adapters, scraper adapters, or import mode".to_string(),
                    "Keep direct scraping optional because anti-bot measures will change".to_string(),
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
                    "Media retrieval may still require a separate downloader/import adapter".to_string(),
                ],
                vec![
                    "Add X API credential support and search adapters".to_string(),
                    "Add downloader abstraction for video asset retrieval".to_string(),
                ],
            ),
        };

        Ok(AppReport::Discovery(DiscoveryReport {
            source: mode,
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

#[derive(Debug, Args)]
struct LibraryArgs {
    #[arg(value_enum)]
    asset_type: AssetTypeArg,

    #[arg(long)]
    from: Option<String>,

    #[arg(long)]
    id: Option<String>,
}

impl LibraryArgs {
    fn run(self) -> Result<AppReport> {
        Ok(AppReport::Library(LibraryReport {
            asset_type: self.asset_type.as_str().to_string(),
            source: self.from,
            id: self.id,
            required_metadata: match self.asset_type {
                AssetTypeArg::Sound => vec![
                    "source_url".to_string(),
                    "platform".to_string(),
                    "duration_seconds".to_string(),
                    "creator".to_string(),
                    "license_or_rights_note".to_string(),
                    "local_audio_path".to_string(),
                ],
                AssetTypeArg::Clip => vec![
                    "source_url".to_string(),
                    "platform".to_string(),
                    "duration_seconds".to_string(),
                    "topic_tags".to_string(),
                    "engagement_metrics".to_string(),
                    "local_video_path".to_string(),
                ],
            },
        }))
    }
}

#[derive(Clone, Debug, ValueEnum)]
enum AssetTypeArg {
    Sound,
    Clip,
}

impl AssetTypeArg {
    fn as_str(&self) -> &'static str {
        match self {
            AssetTypeArg::Sound => "sound",
            AssetTypeArg::Clip => "clip",
        }
    }
}

#[derive(Debug, Args)]
struct ComposeArgs {
    #[arg(long)]
    sound: String,

    #[arg(long = "clip", required = true)]
    clips: Vec<String>,

    #[arg(long, default_value_t = 30)]
    duration_seconds: u32,
}

impl ComposeArgs {
    fn run(self) -> Result<AppReport> {
        Ok(AppReport::Media(MediaReport {
            sound_id: self.sound,
            clip_ids: self.clips,
            duration_seconds: self.duration_seconds,
            pipeline: vec![
                PipelineStep {
                    kind: PipelineStepKind::NormalizeAudio,
                    description: "Normalize imported sound to a consistent loudness target"
                        .to_string(),
                },
                PipelineStep {
                    kind: PipelineStepKind::TrimClips,
                    description: "Trim or subclip candidate visuals to fit target duration"
                        .to_string(),
                },
                PipelineStep {
                    kind: PipelineStepKind::ScaleAndCrop,
                    description: "Scale and crop footage into target social aspect ratio"
                        .to_string(),
                },
                PipelineStep {
                    kind: PipelineStepKind::Mux,
                    description:
                        "Mux selected visuals with normalized audio into the final short clip"
                            .to_string(),
                },
            ],
        }))
    }
}
