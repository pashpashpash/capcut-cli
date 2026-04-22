use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    apify,
    config::{self, APIFY_CONFIG_ENV, TIKTOK_SOUND_RESOLVER_ACTOR_ID_ENV},
    models::{
        AppReport, AuthReport, DiscoverSource, DiscoveryReport, JudgedSound, LibraryReport,
        MediaReport, PipelineStep, PipelineStepKind, RecommendedActionCount, ScoreBandCount,
        SoundImportReport, SoundJudgementFilters, SoundJudgementReport, SoundJudgementSummary,
        UpdateReport,
    },
    tiktok::{
        self, DEFAULT_IMPORT_OUTPUT_DIR, ImportTrendingSoundsOptions, LIBRARY_MANIFEST_PATH,
        TRENDS_ACTOR_ID,
    },
    update,
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
            Command::Auth(args) => args.run(),
            Command::Discover(args) => args.run(),
            Command::Library(args) => args.run(),
            Command::Compose(args) => args.run(),
            Command::Update(args) => args.run(),
        }?;

        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    Auth(AuthArgs),
    Discover(DiscoverArgs),
    Library(LibraryArgs),
    Compose(ComposeArgs),
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
struct AuthArgs {
    #[arg(long)]
    apify: Option<String>,

    #[arg(long, default_value_t = false)]
    from_env: bool,
}

impl AuthArgs {
    fn run(self) -> Result<AppReport> {
        if self.apify.is_some() && self.from_env {
            bail!("use either `--apify <token>` or `--from-env`, not both")
        }

        if let Some(token) = self.apify {
            let path = config::write_apify_token(token)?;
            return Ok(AppReport::Auth(AuthReport {
                provider: "apify".to_string(),
                action: "write_config".to_string(),
                scope: "local_user_config".to_string(),
                config_path: path.display().to_string(),
                env_var: APIFY_CONFIG_ENV.to_string(),
                token_present: true,
                configured_via: Some("config_file".to_string()),
            }));
        }

        if self.from_env {
            let token = config::read_env_apify_token()?;
            let path = config::write_apify_token(token)?;
            return Ok(AppReport::Auth(AuthReport {
                provider: "apify".to_string(),
                action: "write_config".to_string(),
                scope: "local_user_config".to_string(),
                config_path: path.display().to_string(),
                env_var: APIFY_CONFIG_ENV.to_string(),
                token_present: true,
                configured_via: Some("env".to_string()),
            }));
        }

        let status = config::apify_auth_status()?;
        Ok(AppReport::Auth(AuthReport {
            provider: "apify".to_string(),
            action: "status".to_string(),
            scope: "env_or_local_user_config".to_string(),
            config_path: status.config_path.display().to_string(),
            env_var: status.env_var.to_string(),
            token_present: status.token_present,
            configured_via: status
                .configured_via
                .map(|source| source.as_str().to_string()),
        }))
    }
}

#[derive(Debug, Args)]
struct DiscoverArgs {
    #[arg(value_enum)]
    source: DiscoverSourceArg,

    #[arg(long)]
    query: Option<String>,

    #[arg(long, default_value_t = 10)]
    limit: u32,

    #[arg(long, default_value = "United States")]
    country: String,

    #[arg(long, default_value = "7")]
    period: String,
}

impl DiscoverArgs {
    fn run(self) -> Result<AppReport> {
        if self.limit == 0 {
            bail!("--limit must be greater than 0")
        }

        let report = match self.source {
            DiscoverSourceArg::TiktokSounds => {
                let token = config::load_apify_token()?;
                let client = apify::build_client()?;
                let discovery = tiktok::discover_trending_sounds(
                    &client,
                    &token,
                    &self.country,
                    self.limit as usize,
                    &self.period,
                )?;

                DiscoveryReport {
                    source: DiscoverSource::TiktokSounds,
                    provider: Some("apify".to_string()),
                    query: self.query,
                    limit: self.limit,
                    country: Some(self.country),
                    period: Some(self.period),
                    sounds: discovery
                        .items
                        .iter()
                        .map(tiktok::summarize_trending_sound)
                        .collect(),
                    notes: vec![
                        format!("Uses Apify actor `{TRENDS_ACTOR_ID}` for live trend discovery"),
                        "Each result includes sound identifiers plus trend-related item counts kept only as debug metadata".to_string(),
                    ],
                    next_steps: vec![
                        format!(
                            "Use `capcut-cli library sound import-tiktok-trending --resolver-actor-id <novi-actor> --limit <n>` to ingest video and audio assets"
                        ),
                        format!(
                            "Or set `{TIKTOK_SOUND_RESOLVER_ACTOR_ID_ENV}` once for agent-friendly imports"
                        ),
                    ],
                }
            }
            DiscoverSourceArg::XClips => DiscoveryReport {
                source: DiscoverSource::XClips,
                provider: None,
                query: self.query,
                limit: self.limit,
                country: None,
                period: None,
                sounds: Vec::new(),
                notes: vec![
                    "Prototype discovery via X search plus engagement metrics".to_string(),
                    "Require attached video media and rank by likes, reposts, replies, quotes, views, and recency".to_string(),
                    "Media retrieval may still require a separate downloader/import adapter".to_string(),
                ],
                next_steps: vec![
                    "Add X API credential support and search adapters".to_string(),
                    "Add downloader abstraction for video asset retrieval".to_string(),
                ],
            },
        };

        Ok(AppReport::Discovery(report))
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
    #[command(subcommand)]
    command: LibraryCommand,
}

impl LibraryArgs {
    fn run(self) -> Result<AppReport> {
        match self.command {
            LibraryCommand::Plan(args) => args.run(),
            LibraryCommand::Sound(sound_args) => sound_args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum LibraryCommand {
    Plan(LibraryPlanArgs),
    Sound(SoundArgs),
}

#[derive(Debug, Args)]
struct LibraryPlanArgs {
    #[arg(value_enum)]
    asset_type: AssetTypeArg,

    #[arg(long)]
    from: Option<String>,

    #[arg(long)]
    id: Option<String>,
}

impl LibraryPlanArgs {
    fn run(self) -> Result<AppReport> {
        Ok(AppReport::Library(LibraryReport {
            asset_type: self.asset_type.as_str().to_string(),
            source: self.from,
            id: self.id,
            required_metadata: match self.asset_type {
                AssetTypeArg::Sound => vec![
                    "source_url".to_string(),
                    "source_video_url".to_string(),
                    "platform".to_string(),
                    "duration_seconds".to_string(),
                    "creator".to_string(),
                    "local_video_path".to_string(),
                    "local_audio_path".to_string(),
                    "local_videos_dir".to_string(),
                    "local_audios_dir".to_string(),
                    "local_metadata_path".to_string(),
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
struct SoundArgs {
    #[command(subcommand)]
    command: SoundCommand,
}

impl SoundArgs {
    fn run(self) -> Result<AppReport> {
        match self.command {
            SoundCommand::ImportTiktokTrending(args) => args.run(),
            SoundCommand::Judge(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum SoundCommand {
    ImportTiktokTrending(ImportTiktokTrendingArgs),
    Judge(JudgeSoundArgs),
}

#[derive(Debug, Args)]
struct ImportTiktokTrendingArgs {
    #[arg(long, default_value = "United States")]
    country: String,

    #[arg(long, default_value_t = 3)]
    limit: usize,

    #[arg(long, default_value = "7")]
    period: String,

    #[arg(long, default_value_t = 20)]
    max_posts: usize,

    #[arg(long, default_value_t = 5)]
    download_attempts: usize,

    #[arg(long)]
    resolver_actor_id: Option<String>,

    #[arg(long)]
    output_dir: Option<PathBuf>,
}

impl ImportTiktokTrendingArgs {
    fn run(self) -> Result<AppReport> {
        if self.limit == 0 {
            bail!("--limit must be greater than 0")
        }
        if self.max_posts == 0 {
            bail!("--max-posts must be greater than 0")
        }
        if self.download_attempts == 0 {
            bail!("--download-attempts must be greater than 0")
        }

        let token = config::load_apify_token()?;
        let resolver_actor_id =
            config::load_tiktok_sound_resolver_actor_id(self.resolver_actor_id)?;
        let client = apify::build_client()?;
        let output_dir = self
            .output_dir
            .unwrap_or_else(|| PathBuf::from(DEFAULT_IMPORT_OUTPUT_DIR));
        let result = tiktok::import_trending_sounds(
            &client,
            &token,
            &ImportTrendingSoundsOptions {
                country: self.country,
                limit: self.limit,
                period: self.period,
                max_posts: self.max_posts,
                download_attempts: self.download_attempts,
                resolver_actor_id: resolver_actor_id.clone(),
                output_dir: output_dir.clone(),
                manifest_path: PathBuf::from(LIBRARY_MANIFEST_PATH),
            },
        )?;

        Ok(AppReport::SoundImport(SoundImportReport {
            provider: "apify".to_string(),
            actor_chain: vec![TRENDS_ACTOR_ID.to_string(), resolver_actor_id],
            attempted_count: result.imported.len() + result.failed.len(),
            imported_count: result.imported.len(),
            failed_count: result.failed.len(),
            imported: result.imported,
            failed: result.failed,
            manifest_path: result.manifest_path.display().to_string(),
            output_dir: output_dir.display().to_string(),
        }))
    }
}

#[derive(Debug, Args)]
struct JudgeSoundArgs {
    #[arg(long, default_value = LIBRARY_MANIFEST_PATH)]
    manifest: PathBuf,

    #[arg(long)]
    top: Option<usize>,

    #[arg(long)]
    min_score: Option<u32>,

    #[arg(long = "recommended-action")]
    recommended_actions: Vec<String>,

    #[arg(long)]
    min_downloaded_videos: Option<usize>,

    #[arg(long)]
    min_extracted_audios: Option<usize>,

    #[arg(long)]
    min_representative_views: Option<u64>,

    #[arg(long)]
    min_representative_likes: Option<u64>,
}

impl JudgeSoundArgs {
    fn run(self) -> Result<AppReport> {
        if self.top == Some(0) {
            bail!("--top must be greater than 0")
        }
        if self.min_score.is_some_and(|score| score > 100) {
            bail!("--min-score must be between 0 and 100")
        }
        if self
            .recommended_actions
            .iter()
            .any(|action| action.trim().is_empty())
        {
            bail!("--recommended-action values must not be empty")
        }

        let sounds = tiktok::judge_sound_library(&self.manifest)?;
        let total_count = sounds.len();
        let summary = summarize_judged_sounds(&sounds);
        let filters = SoundJudgementFilters {
            top: self.top,
            min_score: self.min_score,
            recommended_actions: self.recommended_actions.clone(),
            min_downloaded_videos: self.min_downloaded_videos,
            min_extracted_audios: self.min_extracted_audios,
            min_representative_views: self.min_representative_views,
            min_representative_likes: self.min_representative_likes,
        };
        let sounds = filter_judged_sounds(
            sounds,
            self.min_score,
            &self.recommended_actions,
            self.min_downloaded_videos,
            self.min_extracted_audios,
            self.min_representative_views,
            self.min_representative_likes,
            self.top,
        );
        let filtered_out_count = total_count - sounds.len();
        let filtered_summary = summarize_judged_sounds(&sounds);

        Ok(AppReport::SoundJudgement(SoundJudgementReport {
            manifest_path: self.manifest.display().to_string(),
            total_count,
            judged_count: sounds.len(),
            filtered_out_count,
            filters,
            summary,
            filtered_summary,
            sounds,
        }))
    }
}

fn summarize_judged_sounds(sounds: &[JudgedSound]) -> SoundJudgementSummary {
    let mut recommended_action_counts = BTreeMap::new();
    let mut score_band_counts = BTreeMap::new();

    for sound in sounds {
        *recommended_action_counts
            .entry(sound.recommended_action.clone())
            .or_insert(0) += 1;
        *score_band_counts
            .entry(score_band(sound.score).to_string())
            .or_insert(0) += 1;
    }

    SoundJudgementSummary {
        recommended_action_counts: recommended_action_counts
            .into_iter()
            .map(|(recommended_action, count)| RecommendedActionCount {
                recommended_action,
                count,
            })
            .collect(),
        score_band_counts: score_band_counts
            .into_iter()
            .map(|(band, count)| ScoreBandCount { band, count })
            .collect(),
    }
}

fn score_band(score: u32) -> &'static str {
    match score {
        75..=100 => "75_100",
        50..=74 => "50_74",
        30..=49 => "30_49",
        _ => "0_29",
    }
}

fn filter_judged_sounds(
    mut sounds: Vec<JudgedSound>,
    min_score: Option<u32>,
    recommended_actions: &[String],
    min_downloaded_videos: Option<usize>,
    min_extracted_audios: Option<usize>,
    min_representative_views: Option<u64>,
    min_representative_likes: Option<u64>,
    top: Option<usize>,
) -> Vec<JudgedSound> {
    if let Some(min_score) = min_score {
        sounds.retain(|sound| sound.score >= min_score);
    }

    if !recommended_actions.is_empty() {
        sounds.retain(|sound| {
            recommended_actions
                .iter()
                .any(|action| sound.recommended_action.eq_ignore_ascii_case(action.trim()))
        });
    }

    if let Some(min_downloaded_videos) = min_downloaded_videos {
        sounds.retain(|sound| {
            sound.downloaded_video_count.unwrap_or_default() >= min_downloaded_videos
        });
    }

    if let Some(min_extracted_audios) = min_extracted_audios {
        sounds.retain(|sound| {
            sound.extracted_audio_count.unwrap_or_default() >= min_extracted_audios
        });
    }

    if let Some(min_representative_views) = min_representative_views {
        sounds.retain(|sound| {
            sound.representative_view_count.unwrap_or_default() >= min_representative_views
        });
    }

    if let Some(min_representative_likes) = min_representative_likes {
        sounds.retain(|sound| {
            sound.representative_like_count.unwrap_or_default() >= min_representative_likes
        });
    }

    if let Some(top) = top {
        sounds.truncate(top);
    }

    sounds
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

#[derive(Debug, Args)]
struct UpdateArgs {
    #[arg(long)]
    bin_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    force: bool,
}

impl UpdateArgs {
    fn run(self) -> Result<AppReport> {
        let report = update::update_cli(update::UpdateOptions {
            bin_path: self.bin_path,
            force: self.force,
        })?;

        Ok(AppReport::Update(UpdateReport {
            action: report.action,
            repository: report.repository,
            current_version: report.current_version,
            target_version: report.target_version,
            status: report.status,
            asset_name: report.asset_name,
            download_url: report.download_url,
            install_path: report.install_path,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judged_sound(id: &str, score: u32, recommended_action: &str) -> JudgedSound {
        JudgedSound {
            sound_id: id.to_string(),
            trend_rank: None,
            title: id.to_string(),
            author: "creator".to_string(),
            platform: "tiktok".to_string(),
            downloaded_video_count: Some(1),
            extracted_audio_count: Some(1),
            representative_view_count: None,
            representative_like_count: None,
            representative_comment_count: None,
            representative_share_count: None,
            score,
            reasons: Vec::new(),
            risks: Vec::new(),
            recommended_action: recommended_action.to_string(),
        }
    }

    #[test]
    fn filter_judged_sounds_applies_score_action_and_top_limit() {
        let sounds = vec![
            judged_sound("sound_a", 95, "shortlist_after_rights_review"),
            judged_sound("sound_b", 82, "use_first"),
            judged_sound("sound_c", 65, "shortlist"),
            judged_sound("sound_d", 40, "needs_review"),
        ];

        let filtered = filter_judged_sounds(
            sounds,
            Some(50),
            &["USE_FIRST".to_string(), "shortlist".to_string()],
            None,
            None,
            None,
            None,
            Some(1),
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].sound_id, "sound_b");
    }

    #[test]
    fn filter_judged_sounds_applies_asset_coverage_thresholds() {
        let mut strong_asset = judged_sound("sound_a", 95, "shortlist_after_rights_review");
        strong_asset.downloaded_video_count = Some(3);
        strong_asset.extracted_audio_count = Some(2);
        let mut weak_asset = judged_sound("sound_b", 95, "shortlist_after_rights_review");
        weak_asset.downloaded_video_count = Some(3);
        weak_asset.extracted_audio_count = Some(1);
        let missing_counts = judged_sound("sound_c", 95, "shortlist_after_rights_review");

        let filtered = filter_judged_sounds(
            vec![strong_asset, weak_asset, missing_counts],
            None,
            &[],
            Some(2),
            Some(2),
            None,
            None,
            None,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].sound_id, "sound_a");
    }

    #[test]
    fn filter_judged_sounds_applies_engagement_thresholds() {
        let mut high_engagement = judged_sound("sound_a", 95, "shortlist_after_rights_review");
        high_engagement.representative_view_count = Some(2_000_000);
        high_engagement.representative_like_count = Some(150_000);
        let mut low_likes = judged_sound("sound_b", 95, "shortlist_after_rights_review");
        low_likes.representative_view_count = Some(2_000_000);
        low_likes.representative_like_count = Some(25_000);
        let missing_metrics = judged_sound("sound_c", 95, "shortlist_after_rights_review");

        let filtered = filter_judged_sounds(
            vec![high_engagement, low_likes, missing_metrics],
            None,
            &[],
            None,
            None,
            Some(1_000_000),
            Some(100_000),
            None,
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].sound_id, "sound_a");
    }

    #[test]
    fn summarize_judged_sounds_counts_actions_and_score_bands() {
        let sounds = vec![
            judged_sound("sound_a", 95, "shortlist_after_rights_review"),
            judged_sound("sound_b", 82, "shortlist_after_rights_review"),
            judged_sound("sound_c", 65, "shortlist"),
            judged_sound("sound_d", 40, "needs_review"),
            judged_sound("sound_e", 20, "skip_for_now"),
        ];

        let summary = summarize_judged_sounds(&sounds);

        assert_eq!(summary.recommended_action_counts.len(), 4);
        assert!(summary.recommended_action_counts.iter().any(|count| {
            count.recommended_action == "shortlist_after_rights_review" && count.count == 2
        }));
        assert!(
            summary
                .score_band_counts
                .iter()
                .any(|count| { count.band == "75_100" && count.count == 2 })
        );
        assert!(
            summary
                .score_band_counts
                .iter()
                .any(|count| { count.band == "0_29" && count.count == 1 })
        );
    }
}
