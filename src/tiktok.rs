use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    apify::{self, ActorRun},
    models::{DiscoveredSound, FailedSoundImport, ImportedSound, JudgedSound},
};

pub const DEFAULT_IMPORT_OUTPUT_DIR: &str = "library/sounds/imported";
pub const LIBRARY_MANIFEST_PATH: &str = "library/sounds/manifest.json";
pub const TRENDS_ACTOR_ID: &str = "alien_force~tiktok-trending-sounds-tracker";
pub const DEFAULT_SOUND_RESOLVER_REGION: &str = "US";

const DIRECT_DOWNLOAD_METHOD: &str = "direct_http";
const SOUND_RESOLVER_INPUT_TYPE: &str = "MUSIC";
const SOUND_RESOLVER_INPUT_LIMIT: usize = 20;
const STRONG_TREND_RANK_CUTOFF: u32 = 25;

#[derive(Debug, Clone)]
pub struct ImportTrendingSoundsOptions {
    pub country: String,
    pub limit: usize,
    pub period: String,
    pub max_posts: usize,
    pub download_attempts: usize,
    pub resolver_actor_id: String,
    pub output_dir: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug)]
pub struct ImportTrendingSoundsResult {
    pub imported: Vec<ImportedSound>,
    pub failed: Vec<FailedSoundImport>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrendingSoundItem {
    pub rank: u32,
    pub title: String,
    pub author: String,
    pub link: String,
    pub clip_id: String,
    pub song_id: String,
    pub duration: u32,
    pub country_code: String,
    #[serde(default)]
    pub related_items: Vec<RelatedItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelatedItem {
    pub item_id: u64,
    #[serde(default)]
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TrendDiscoveryExecution {
    pub actor_id: &'static str,
    pub actor_run: ActorRun,
    pub items: Vec<TrendingSoundItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum CandidatePostSource {
    SoundResolverActor,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum DownloadStatus {
    Downloaded,
    DownloadedVideoOnly,
    SkippedMissingMediaUrl,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct CandidatePost {
    selection_rank: usize,
    resolver_index: usize,
    source: CandidatePostSource,
    video_id: String,
    aweme_id: Option<String>,
    video_url: String,
    author_unique_id: Option<String>,
    author_nickname: Option<String>,
    title: Option<String>,
    region: Option<String>,
    duration_seconds: Option<u32>,
    play_count: Option<u64>,
    digg_count: Option<u64>,
    comment_count: Option<u64>,
    share_count: Option<u64>,
    download_url: Option<String>,
    public_media_url: Option<String>,
    audio_url: Option<String>,
    cover_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct TrendArtifact {
    actor_id: String,
    actor_run: ActorRun,
    item: TrendingSoundItem,
}

#[derive(Debug, Serialize)]
struct ResolverPostsArtifact {
    actor_id: String,
    actor_run: ActorRun,
    input_profile: Value,
    requested_sound_url: String,
    requested_max_results: usize,
    debug_related_items: Vec<RelatedItem>,
    raw_dataset: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct CandidateSelectionArtifact {
    actor_id: String,
    actor_run: ActorRun,
    requested_sound_url: String,
    requested_max_results: usize,
    raw_dataset_count: usize,
    normalized_candidate_count: usize,
    debug_related_item_count: usize,
    ranking_strategy: String,
    preferred_candidate: Option<CandidatePost>,
    candidates: Vec<CandidatePost>,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateDownloadAttemptArtifact {
    attempt_number: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CandidateDownloadArtifact {
    candidate_rank: usize,
    resolver_index: usize,
    candidate_source: CandidatePostSource,
    candidate_video_id: String,
    candidate_video_url: String,
    resolved_direct_video_url: Option<String>,
    resolved_audio_url: Option<String>,
    local_video_path: Option<String>,
    local_audio_path: Option<String>,
    status: DownloadStatus,
    error: Option<String>,
    attempts: Vec<CandidateDownloadAttemptArtifact>,
}

#[derive(Debug, Serialize)]
struct DownloadArtifact {
    method: String,
    requested_candidate_count: usize,
    successful_video_count: usize,
    extracted_audio_count: usize,
    representative_video_id: Option<String>,
    representative_local_video_path: Option<String>,
    representative_local_audio_path: Option<String>,
    assets: Vec<CandidateDownloadArtifact>,
}

#[derive(Debug, Serialize)]
struct ImportedSoundMetadata {
    id: String,
    rank: u32,
    title: String,
    author: String,
    trend_link: String,
    clip_id: String,
    song_id: String,
    country_code: String,
    duration_seconds: u32,
    downloaded_video_count: usize,
    extracted_audio_count: usize,
    actors: ActorChainMetadata,
    selection: SelectionSummary,
    files: LocalArtifacts,
    assets: Vec<DownloadedAssetMetadata>,
    provenance: String,
    rights_note: String,
}

#[derive(Debug, Serialize)]
struct ActorChainMetadata {
    trends_actor: String,
    sound_resolver_actor: String,
    download_method: String,
}

#[derive(Debug, Serialize)]
struct SelectionSummary {
    ranking_strategy: String,
    candidate_count: usize,
    representative_video_id: String,
    representative_video_url: String,
    representative_direct_video_url: String,
    representative_audio_url: Option<String>,
    representative_comment_count: Option<u64>,
    representative_share_count: Option<u64>,
    representative_like_count: Option<u64>,
    representative_view_count: Option<u64>,
}

#[derive(Debug, Serialize)]
struct LocalArtifacts {
    trend_path: String,
    posts_path: String,
    selection_path: String,
    download_path: String,
    metadata_path: String,
    videos_dir: String,
    audios_dir: String,
    representative_video_path: String,
    representative_audio_path: String,
}

#[derive(Debug, Serialize)]
struct DownloadedAssetMetadata {
    candidate_rank: usize,
    resolver_index: usize,
    source: CandidatePostSource,
    video_id: String,
    video_url: String,
    direct_video_url: String,
    audio_url: Option<String>,
    author_unique_id: Option<String>,
    author_nickname: Option<String>,
    title: Option<String>,
    region: Option<String>,
    duration_seconds: Option<u32>,
    play_count: Option<u64>,
    like_count: Option<u64>,
    comment_count: Option<u64>,
    share_count: Option<u64>,
    local_video_path: String,
    local_audio_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct RepresentativeEngagementMetrics {
    view_count: Option<u64>,
    like_count: Option<u64>,
    comment_count: Option<u64>,
    share_count: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct RepresentativeMusicSignals {
    duration_seconds: Option<f64>,
    can_read: Option<bool>,
    can_reuse: Option<bool>,
    is_original_sound: Option<bool>,
    commercial_right_type: Option<u64>,
    is_batch_take_down_music: Option<bool>,
    reviewed: Option<bool>,
    has_strong_beat_url: Option<bool>,
    music_vid: Option<String>,
}

const REPRESENTATIVE_MUSIC_FIELDS: [&str; 9] = [
    "representative_music_duration_seconds",
    "representative_music_can_read",
    "representative_music_can_reuse",
    "representative_music_is_original_sound",
    "representative_music_commercial_right_type",
    "representative_music_is_batch_take_down_music",
    "representative_music_reviewed",
    "representative_music_has_strong_beat_url",
    "representative_music_vid",
];

#[derive(Debug, Deserialize, Serialize, Default)]
struct Manifest {
    #[serde(default)]
    sounds: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ManifestEntry {
    id: String,
    title: String,
    author: String,
    platform: String,
    trend_rank: Option<u32>,
    source_url: String,
    source_video_url: Option<String>,
    duration_seconds: Option<u32>,
    local_audio_path: String,
    local_metadata_path: String,
    rights_note: String,
    provenance: String,
    #[serde(default)]
    song_id: Option<String>,
    #[serde(default)]
    clip_id: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    local_video_path: Option<String>,
    #[serde(default)]
    local_trend_path: Option<String>,
    #[serde(default)]
    local_posts_path: Option<String>,
    #[serde(default)]
    local_selection_path: Option<String>,
    #[serde(default)]
    local_download_path: Option<String>,
    #[serde(default)]
    local_videos_dir: Option<String>,
    #[serde(default)]
    local_audios_dir: Option<String>,
    #[serde(default)]
    downloaded_video_count: Option<usize>,
    #[serde(default)]
    extracted_audio_count: Option<usize>,
    #[serde(default)]
    representative_video_url: Option<String>,
    #[serde(default)]
    representative_video_id: Option<String>,
    #[serde(default)]
    representative_comment_count: Option<u64>,
    #[serde(default)]
    representative_share_count: Option<u64>,
    #[serde(default)]
    representative_like_count: Option<u64>,
    #[serde(default)]
    representative_view_count: Option<u64>,
    #[serde(default)]
    resolver_actor_id: Option<String>,
    #[serde(default)]
    download_method: Option<String>,
}

struct CandidateSelectionResult {
    resolver_posts_artifact: ResolverPostsArtifact,
    selection_artifact: CandidateSelectionArtifact,
}

struct DownloadResolution {
    downloaded_assets: Vec<DownloadedAsset>,
    asset_artifacts: Vec<CandidateDownloadArtifact>,
}

#[derive(Debug, Clone)]
struct DownloadedAsset {
    candidate: CandidatePost,
    selected_media_url: String,
    video_path: PathBuf,
    audio_path: Option<PathBuf>,
}

struct ImportedSoundRecord {
    report: ImportedSound,
    manifest_entry: ManifestEntry,
}

pub fn ensure_ffmpeg_available() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .with_context(|| {
            "ffmpeg is required to extract audio from downloaded videos; install ffmpeg and retry"
        })?;

    if !output.status.success() {
        bail!(
            "ffmpeg -version failed with status {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        )
    }

    Ok(())
}

pub fn discover_trending_sounds(
    client: &Client,
    token: &str,
    country: &str,
    limit: usize,
    period: &str,
) -> Result<TrendDiscoveryExecution> {
    let actor_run = apify::run_actor(
        client,
        token,
        TRENDS_ACTOR_ID,
        &json!({
            "country": country,
            "limit": limit,
            "period": period,
        }),
    )?;

    let items: Vec<TrendingSoundItem> =
        apify::fetch_dataset_items(client, token, &actor_run.default_dataset_id)?;

    if items.is_empty() {
        bail!("trending sounds actor returned no items")
    }

    Ok(TrendDiscoveryExecution {
        actor_id: TRENDS_ACTOR_ID,
        actor_run,
        items,
    })
}

pub fn summarize_trending_sound(item: &TrendingSoundItem) -> DiscoveredSound {
    DiscoveredSound {
        rank: item.rank,
        title: item.title.clone(),
        author: item.author.clone(),
        song_id: item.song_id.clone(),
        clip_id: item.clip_id.clone(),
        trend_link: item.link.clone(),
        duration_seconds: item.duration,
        country_code: item.country_code.clone(),
        related_item_count: item.related_items.len(),
    }
}

pub fn import_trending_sounds(
    client: &Client,
    token: &str,
    options: &ImportTrendingSoundsOptions,
) -> Result<ImportTrendingSoundsResult> {
    ensure_ffmpeg_available()?;

    let discovery = discover_trending_sounds(
        client,
        token,
        &options.country,
        options.limit,
        &options.period,
    )?;
    let selected = discovery
        .items
        .iter()
        .cloned()
        .take(options.limit)
        .collect::<Vec<_>>();

    if selected.is_empty() {
        bail!("trending sounds actor returned no usable items after limiting")
    }

    fs::create_dir_all(&options.output_dir)
        .with_context(|| format!("failed to create {}", options.output_dir.display()))?;

    let mut imported = Vec::new();
    let mut failed = Vec::new();
    let mut manifest = read_manifest(&options.manifest_path)?;

    for item in selected {
        let failure_context = (
            item.rank,
            item.title.clone(),
            item.song_id.clone(),
            item.clip_id.clone(),
            item.link.clone(),
        );

        match import_trending_sound_item(
            client,
            token,
            options,
            discovery.actor_id,
            &discovery.actor_run,
            item,
        ) {
            Ok(record) => {
                merge_manifest_entry(&mut manifest, record.manifest_entry);
                imported.push(record.report);
            }
            Err(error) => failed.push(FailedSoundImport {
                rank: failure_context.0,
                title: failure_context.1,
                song_id: failure_context.2,
                clip_id: failure_context.3,
                trend_link: failure_context.4,
                error: format!("{error:#}"),
            }),
        }
    }

    write_manifest(&options.manifest_path, &manifest)?;

    Ok(ImportTrendingSoundsResult {
        imported,
        failed,
        manifest_path: options.manifest_path.clone(),
    })
}

pub fn judge_sound_library(manifest_path: &Path) -> Result<Vec<JudgedSound>> {
    let manifest = read_manifest(manifest_path)?;
    let mut sounds = manifest
        .sounds
        .iter()
        .map(|entry| judge_manifest_entry(manifest_path, entry))
        .collect::<Result<Vec<_>>>()?;

    annotate_song_id_country_coverage_counts(&mut sounds);
    annotate_song_id_top_25_country_counts(&mut sounds);
    annotate_song_id_best_trend_ranks(&mut sounds);
    annotate_song_id_best_representative_view_counts(&mut sounds);
    annotate_song_id_best_representative_engagement_counts(&mut sounds);
    annotate_song_id_best_representative_share_counts(&mut sounds);
    annotate_song_id_best_representative_engagement_rates(&mut sounds);
    annotate_song_id_best_representative_share_rates(&mut sounds);
    apply_song_id_country_coverage_signal(&mut sounds);
    sort_and_rank_judged_sounds(&mut sounds);

    Ok(sounds)
}

fn annotate_song_id_country_coverage_counts(sounds: &mut [JudgedSound]) {
    let mut song_countries = BTreeMap::<String, BTreeSet<String>>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(country_code)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound
                .country_code
                .as_deref()
                .map(str::trim)
                .filter(|country_code| !country_code.is_empty()),
        ) {
            song_countries
                .entry(song_id.to_string())
                .or_default()
                .insert(country_code.to_ascii_uppercase());
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_country_coverage_count = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_countries.get(song_id).map(BTreeSet::len));
    }
}

fn annotate_song_id_top_25_country_counts(sounds: &mut [JudgedSound]) {
    let mut song_top_25_countries = BTreeMap::<String, BTreeSet<String>>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(country_code), Some(trend_rank)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound
                .country_code
                .as_deref()
                .map(str::trim)
                .filter(|country_code| !country_code.is_empty()),
            sound.trend_rank,
        ) {
            if trend_rank <= STRONG_TREND_RANK_CUTOFF {
                song_top_25_countries
                    .entry(song_id.to_string())
                    .or_default()
                    .insert(country_code.to_ascii_uppercase());
            }
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_top_25_country_count = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_top_25_countries.get(song_id).map(BTreeSet::len));
    }
}

fn annotate_song_id_best_trend_ranks(sounds: &mut [JudgedSound]) {
    let mut song_best_ranks = BTreeMap::<String, u32>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(trend_rank)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.trend_rank,
        ) {
            song_best_ranks
                .entry(song_id.to_string())
                .and_modify(|best_rank| *best_rank = (*best_rank).min(trend_rank))
                .or_insert(trend_rank);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_trend_rank = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_ranks.get(song_id).copied());
    }
}

fn annotate_song_id_best_representative_view_counts(sounds: &mut [JudgedSound]) {
    let mut song_best_views = BTreeMap::<String, u64>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(view_count)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.representative_view_count,
        ) {
            song_best_views
                .entry(song_id.to_string())
                .and_modify(|best_view_count| *best_view_count = (*best_view_count).max(view_count))
                .or_insert(view_count);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_representative_view_count = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_views.get(song_id).copied());
    }
}

fn annotate_song_id_best_representative_engagement_counts(sounds: &mut [JudgedSound]) {
    let mut song_best_engagements = BTreeMap::<String, u64>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(engagement_count)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.representative_engagement_count,
        ) {
            song_best_engagements
                .entry(song_id.to_string())
                .and_modify(|best_engagement_count| {
                    *best_engagement_count = (*best_engagement_count).max(engagement_count)
                })
                .or_insert(engagement_count);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_representative_engagement_count = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_engagements.get(song_id).copied());
    }
}

fn annotate_song_id_best_representative_share_counts(sounds: &mut [JudgedSound]) {
    let mut song_best_shares = BTreeMap::<String, u64>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(share_count)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.representative_share_count,
        ) {
            song_best_shares
                .entry(song_id.to_string())
                .and_modify(|best_share_count| {
                    *best_share_count = (*best_share_count).max(share_count)
                })
                .or_insert(share_count);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_representative_share_count = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_shares.get(song_id).copied());
    }
}

fn annotate_song_id_best_representative_engagement_rates(sounds: &mut [JudgedSound]) {
    let mut song_best_engagement_rates = BTreeMap::<String, u64>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(engagement_rate_per_1000_views)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.representative_engagement_rate_per_1000_views,
        ) {
            song_best_engagement_rates
                .entry(song_id.to_string())
                .and_modify(|best_engagement_rate_per_1000_views| {
                    *best_engagement_rate_per_1000_views =
                        (*best_engagement_rate_per_1000_views).max(engagement_rate_per_1000_views)
                })
                .or_insert(engagement_rate_per_1000_views);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_representative_engagement_rate_per_1000_views = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_engagement_rates.get(song_id).copied());
    }
}

fn annotate_song_id_best_representative_share_rates(sounds: &mut [JudgedSound]) {
    let mut song_best_share_rates = BTreeMap::<String, u64>::new();

    for sound in sounds.iter() {
        if let (Some(song_id), Some(share_rate_per_1000_views)) = (
            sound
                .song_id
                .as_deref()
                .map(str::trim)
                .filter(|song_id| !song_id.is_empty()),
            sound.representative_share_rate_per_1000_views,
        ) {
            song_best_share_rates
                .entry(song_id.to_string())
                .and_modify(|best_share_rate_per_1000_views| {
                    *best_share_rate_per_1000_views =
                        (*best_share_rate_per_1000_views).max(share_rate_per_1000_views)
                })
                .or_insert(share_rate_per_1000_views);
        }
    }

    for sound in sounds.iter_mut() {
        sound.song_id_best_representative_share_rate_per_1000_views = sound
            .song_id
            .as_deref()
            .map(str::trim)
            .filter(|song_id| !song_id.is_empty())
            .and_then(|song_id| song_best_share_rates.get(song_id).copied());
    }
}

fn apply_song_id_country_coverage_signal(sounds: &mut [JudgedSound]) {
    for sound in sounds.iter_mut() {
        if let Some(country_count) = sound.song_id_country_coverage_count {
            let score_bonus = match country_count {
                2 => 4,
                3.. => 8,
                _ => 0,
            };
            let reason = format!("song_id persists across {country_count} recorded trend markets");

            if score_bonus > 0 && !sound.reasons.iter().any(|existing| existing == &reason) {
                sound.score = sound.score.saturating_add(score_bonus);
                sound.reasons.push(reason);
            }

            if country_count >= 2 {
                if let Some(top_25_country_count) = sound.song_id_top_25_country_count {
                    let score_bonus = match top_25_country_count {
                        2 => 3,
                        3.. => 6,
                        _ => 0,
                    };
                    let reason = format!(
                        "song_id charted inside the top {STRONG_TREND_RANK_CUTOFF} in {top_25_country_count} recorded markets"
                    );

                    if score_bonus > 0 && !sound.reasons.iter().any(|existing| existing == &reason)
                    {
                        sound.score = sound.score.saturating_add(score_bonus);
                        sound.reasons.push(reason);
                    }
                }

                if let Some(best_rank) = sound.song_id_best_trend_rank {
                    let score_bonus = match best_rank {
                        1..=10 => 4,
                        11..=25 => 2,
                        _ => 0,
                    };
                    let reason = format!(
                        "song_id reached trend rank {best_rank} in at least one recorded market"
                    );

                    if score_bonus > 0 && !sound.reasons.iter().any(|existing| existing == &reason)
                    {
                        sound.score = sound.score.saturating_add(score_bonus);
                        sound.reasons.push(reason);
                    }
                }
            }
        }

        refresh_derived_judgement_fields(sound);
    }
}

fn refresh_derived_judgement_fields(sound: &mut JudgedSound) {
    sound.score = sound.score.min(100);
    sound.reason_count = sound.reasons.len();
    sound.risk_count = sound.risks.len();
    sound.recommended_action = recommended_action(sound.score, &sound.risks).to_string();
}

fn sort_and_rank_judged_sounds(sounds: &mut [JudgedSound]) {
    sounds.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.trend_rank
                    .unwrap_or(u32::MAX)
                    .cmp(&right.trend_rank.unwrap_or(u32::MAX))
            })
            .then_with(|| left.sound_id.cmp(&right.sound_id))
    });

    for (index, sound) in sounds.iter_mut().enumerate() {
        sound.judgement_rank = Some(index + 1);
    }
}

fn judge_manifest_entry(manifest_path: &Path, entry: &ManifestEntry) -> Result<JudgedSound> {
    let metadata = read_optional_metadata(manifest_path, &entry.local_metadata_path)?;
    let representative_post_engagement =
        representative_engagement_from_posts_artifact(manifest_path, entry)?;
    let representative_music_signals =
        representative_music_signals_from_posts_artifact(manifest_path, entry)?;
    let downloaded_video_count = entry
        .downloaded_video_count
        .or_else(|| metadata_usize(&metadata, &[&["downloaded_video_count"]]))
        .or_else(|| {
            has_entry_or_metadata_path(
                entry.local_video_path.as_deref(),
                &metadata,
                &[
                    &["files", "representative_video_path"],
                    &["files", "local_video_path"],
                ],
            )
            .then_some(1)
        });
    let extracted_audio_count = entry
        .extracted_audio_count
        .or_else(|| metadata_usize(&metadata, &[&["extracted_audio_count"]]))
        .or_else(|| {
            has_entry_or_metadata_path(
                Some(&entry.local_audio_path),
                &metadata,
                &[
                    &["files", "representative_audio_path"],
                    &["files", "local_audio_path"],
                ],
            )
            .then_some(1)
        });
    let candidate_post_count = match metadata_usize(&metadata, &[&["selection", "candidate_count"]])
    {
        Some(count) => Some(count),
        None => candidate_post_count_from_selection_artifact(manifest_path, entry)?,
    };
    let representative_view_count = entry.representative_view_count.or_else(|| {
        metadata_u64(
            &metadata,
            &[
                &["selection", "representative_view_count"],
                &["selection", "selected_view_count"],
            ],
        )
        .or(representative_post_engagement.view_count)
    });
    let representative_like_count = entry.representative_like_count.or_else(|| {
        metadata_u64(
            &metadata,
            &[
                &["selection", "representative_like_count"],
                &["selection", "selected_like_count"],
            ],
        )
        .or(representative_post_engagement.like_count)
    });
    let representative_comment_count = entry.representative_comment_count.or_else(|| {
        metadata_u64(
            &metadata,
            &[
                &["selection", "representative_comment_count"],
                &["selection", "selected_comment_count"],
            ],
        )
        .or(representative_post_engagement.comment_count)
    });
    let representative_share_count = entry.representative_share_count.or_else(|| {
        metadata_u64(
            &metadata,
            &[
                &["selection", "representative_share_count"],
                &["selection", "selected_share_count"],
            ],
        )
        .or(representative_post_engagement.share_count)
    });
    let representative_engagement_count = sum_counts(&[
        representative_like_count,
        representative_comment_count,
        representative_share_count,
    ]);
    let representative_like_rate_per_1000_views =
        rate_per_1000(representative_like_count, representative_view_count);
    let representative_engagement_rate_per_1000_views = rate_sum_per_1000(
        &[
            representative_like_count,
            representative_comment_count,
            representative_share_count,
        ],
        representative_view_count,
    );
    let representative_share_rate_per_1000_views =
        rate_per_1000(representative_share_count, representative_view_count);
    let representative_comment_rate_per_1000_views =
        rate_per_1000(representative_comment_count, representative_view_count);
    let representative_engagement_metrics = [
        (
            "representative_view_count",
            representative_view_count.is_some(),
        ),
        (
            "representative_like_count",
            representative_like_count.is_some(),
        ),
        (
            "representative_comment_count",
            representative_comment_count.is_some(),
        ),
        (
            "representative_share_count",
            representative_share_count.is_some(),
        ),
    ];
    let representative_engagement_metric_fields = representative_engagement_metrics
        .iter()
        .filter_map(|(field, is_present)| is_present.then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let missing_representative_engagement_metric_fields = representative_engagement_metrics
        .iter()
        .filter_map(|(field, is_present)| (!is_present).then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let representative_engagement_metric_count = representative_engagement_metric_fields.len();
    let representative_music_fields = [
        (
            "representative_music_duration_seconds",
            representative_music_signals.duration_seconds.is_some(),
        ),
        (
            "representative_music_can_read",
            representative_music_signals.can_read.is_some(),
        ),
        (
            "representative_music_can_reuse",
            representative_music_signals.can_reuse.is_some(),
        ),
        (
            "representative_music_is_original_sound",
            representative_music_signals.is_original_sound.is_some(),
        ),
        (
            "representative_music_commercial_right_type",
            representative_music_signals.commercial_right_type.is_some(),
        ),
        (
            "representative_music_is_batch_take_down_music",
            representative_music_signals
                .is_batch_take_down_music
                .is_some(),
        ),
        (
            "representative_music_reviewed",
            representative_music_signals.reviewed.is_some(),
        ),
        (
            "representative_music_has_strong_beat_url",
            representative_music_signals.has_strong_beat_url.is_some(),
        ),
        (
            "representative_music_vid",
            representative_music_signals.music_vid.is_some(),
        ),
    ];
    let representative_music_present_fields = representative_music_fields
        .iter()
        .filter_map(|(field, is_present)| is_present.then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let missing_representative_music_fields = representative_music_fields
        .iter()
        .filter_map(|(field, is_present)| (!is_present).then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let representative_music_field_count = representative_music_present_fields.len();
    let source_identifiers = [
        ("source_url", !entry.source_url.trim().is_empty()),
        (
            "source_video_url",
            entry
                .source_video_url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        (
            "song_id",
            entry
                .song_id
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        (
            "clip_id",
            entry
                .clip_id
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        (
            "country_code",
            entry
                .country_code
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        ("duration_seconds", entry.duration_seconds.is_some()),
    ];
    let source_identifier_fields = source_identifiers
        .iter()
        .filter_map(|(field, is_present)| is_present.then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let missing_source_identifier_fields = source_identifiers
        .iter()
        .filter_map(|(field, is_present)| (!is_present).then(|| (*field).to_string()))
        .collect::<Vec<_>>();
    let source_identifier_count = source_identifier_fields.len();
    let local_artifact_paths = [
        ("local_audio_path", Some(entry.local_audio_path.as_str())),
        ("local_video_path", entry.local_video_path.as_deref()),
        (
            "local_metadata_path",
            Some(entry.local_metadata_path.as_str()),
        ),
        ("local_trend_path", entry.local_trend_path.as_deref()),
        ("local_posts_path", entry.local_posts_path.as_deref()),
        (
            "local_selection_path",
            entry.local_selection_path.as_deref(),
        ),
        ("local_download_path", entry.local_download_path.as_deref()),
    ];
    let local_artifact_path_fields = local_artifact_paths
        .iter()
        .filter_map(|(field, path)| {
            path.is_some_and(|path| !path.trim().is_empty())
                .then(|| (*field).to_string())
        })
        .collect::<Vec<_>>();
    let missing_local_artifact_path_fields = local_artifact_paths
        .iter()
        .filter_map(|(field, path)| {
            (!path.is_some_and(|path| !path.trim().is_empty())).then(|| (*field).to_string())
        })
        .collect::<Vec<_>>();
    let local_artifact_path_count = local_artifact_path_fields.len();

    let mut score = 0;
    let mut reasons = Vec::new();
    let mut risks = Vec::new();

    if entry.platform == "tiktok" {
        score += 10;
        reasons.push("TikTok-sourced sound with platform provenance".to_string());
    } else {
        risks.push(format!(
            "Platform `{}` is not a live TikTok sound source",
            entry.platform
        ));
    }

    if let Some(rank) = entry.trend_rank {
        let rank_points = trend_rank_score(rank);
        score += rank_points;
        reasons.push(format!(
            "Trend rank {rank} contributes {rank_points} points"
        ));
    } else {
        risks.push("No trend rank is recorded".to_string());
    }

    match downloaded_video_count {
        Some(count) if count > 1 => {
            score += 15;
            reasons.push(format!("{count} downloaded candidate videos are available"));
        }
        Some(1) => {
            score += 10;
            reasons.push("One downloaded candidate video is available".to_string());
        }
        _ => risks.push("No downloaded candidate video is recorded".to_string()),
    }

    match extracted_audio_count {
        Some(count) if count > 1 => {
            score += 20;
            reasons.push(format!("{count} extracted audio assets are available"));
        }
        Some(1) => {
            score += 15;
            reasons.push("One extracted audio asset is available".to_string());
        }
        _ => risks.push("No extracted audio asset is recorded".to_string()),
    }

    add_engagement_signal(
        representative_view_count,
        representative_like_count,
        representative_comment_count,
        representative_share_count,
        &mut score,
        &mut reasons,
        &mut risks,
    );

    if representative_music_signals.can_reuse == Some(true) {
        score += 5;
        reasons.push("Representative music metadata marks the sound reusable".to_string());
    } else if representative_music_signals.can_reuse == Some(false) {
        risks.push("Representative music metadata does not mark the sound reusable".to_string());
    }

    if representative_music_signals.can_read == Some(false) {
        risks.push("Representative music metadata does not mark the sound readable".to_string());
    }

    if representative_music_signals.is_batch_take_down_music == Some(true) {
        risks.push(
            "Representative music metadata marks the sound as batch-takedown music".to_string(),
        );
    }

    if representative_music_signals.has_strong_beat_url == Some(true) {
        score += 3;
        reasons.push("Representative music metadata includes a strong beat track".to_string());
    }

    if representative_music_signals.music_vid.is_some() {
        score += 2;
        reasons.push("Representative music metadata includes a stable music_vid".to_string());
    }

    if entry.resolver_actor_id.is_some() {
        score += 5;
        reasons.push("Resolver actor id is recorded for repeatability".to_string());
    } else if entry.platform == "tiktok" {
        risks.push("Resolver actor id is missing".to_string());
    }

    if entry
        .rights_note
        .to_ascii_lowercase()
        .contains("verify rights")
    {
        risks.push("Rights still need manual verification before production use".to_string());
    }

    let score = score.min(100);
    let recommended_action = recommended_action(score, &risks).to_string();
    let usable_asset_pair_count = match (downloaded_video_count, extracted_audio_count) {
        (Some(downloaded), Some(extracted)) => Some(downloaded.min(extracted)),
        _ => None,
    };

    Ok(JudgedSound {
        sound_id: entry.id.clone(),
        judgement_rank: None,
        trend_rank: entry.trend_rank,
        title: entry.title.clone(),
        author: entry.author.clone(),
        platform: entry.platform.clone(),
        provenance: entry.provenance.clone(),
        rights_note: entry.rights_note.clone(),
        resolver_actor_id: entry.resolver_actor_id.clone(),
        download_method: entry.download_method.clone(),
        source_url: entry.source_url.clone(),
        source_video_url: entry.source_video_url.clone(),
        song_id: entry.song_id.clone(),
        song_id_country_coverage_count: None,
        song_id_top_25_country_count: None,
        song_id_best_trend_rank: None,
        song_id_best_representative_view_count: None,
        song_id_best_representative_engagement_count: None,
        song_id_best_representative_share_count: None,
        song_id_best_representative_engagement_rate_per_1000_views: None,
        song_id_best_representative_share_rate_per_1000_views: None,
        clip_id: entry.clip_id.clone(),
        country_code: entry.country_code.clone(),
        duration_seconds: entry.duration_seconds,
        source_identifier_count,
        source_identifier_fields,
        missing_source_identifier_fields,
        local_audio_path: entry.local_audio_path.clone(),
        local_video_path: entry.local_video_path.clone(),
        local_metadata_path: entry.local_metadata_path.clone(),
        local_trend_path: entry.local_trend_path.clone(),
        local_posts_path: entry.local_posts_path.clone(),
        local_selection_path: entry.local_selection_path.clone(),
        local_download_path: entry.local_download_path.clone(),
        local_artifact_path_count,
        local_artifact_path_fields,
        missing_local_artifact_path_fields,
        downloaded_video_count,
        extracted_audio_count,
        usable_asset_pair_count,
        candidate_post_count,
        representative_view_count,
        representative_like_count,
        representative_engagement_count,
        representative_like_rate_per_1000_views,
        representative_engagement_rate_per_1000_views,
        representative_comment_count,
        representative_comment_rate_per_1000_views,
        representative_share_count,
        representative_share_rate_per_1000_views,
        representative_music_duration_seconds: representative_music_signals.duration_seconds,
        representative_music_can_read: representative_music_signals.can_read,
        representative_music_can_reuse: representative_music_signals.can_reuse,
        representative_music_is_original_sound: representative_music_signals.is_original_sound,
        representative_music_commercial_right_type: representative_music_signals
            .commercial_right_type,
        representative_music_is_batch_take_down_music: representative_music_signals
            .is_batch_take_down_music,
        representative_music_reviewed: representative_music_signals.reviewed,
        representative_music_has_strong_beat_url: representative_music_signals.has_strong_beat_url,
        representative_music_vid: representative_music_signals.music_vid,
        representative_music_field_count,
        representative_music_fields: representative_music_present_fields,
        missing_representative_music_fields,
        representative_engagement_metric_count,
        representative_engagement_metric_fields,
        missing_representative_engagement_metric_fields,
        score,
        reason_count: reasons.len(),
        reasons,
        risk_count: risks.len(),
        risks,
        recommended_action,
    })
}

fn import_trending_sound_item(
    client: &Client,
    token: &str,
    options: &ImportTrendingSoundsOptions,
    trend_actor_id: &str,
    trend_actor_run: &ActorRun,
    item: TrendingSoundItem,
) -> Result<ImportedSoundRecord> {
    let slug = slugify(&format!("{}-{}-{}", item.rank, item.title, item.song_id));
    let sound_id = format!("tiktok_sound_{}", item.song_id);
    let sound_dir = options.output_dir.join(&slug);
    prepare_sound_dir(&sound_dir)?;

    let trend_path = sound_dir.join("trend.json");
    let posts_path = sound_dir.join("posts.json");
    let selection_path = sound_dir.join("selection.json");
    let download_path = sound_dir.join("download.json");
    let metadata_path = sound_dir.join("metadata.json");
    let videos_dir = sound_dir.join("videos");
    let audios_dir = sound_dir.join("audios");

    write_json(
        &trend_path,
        &TrendArtifact {
            actor_id: trend_actor_id.to_string(),
            actor_run: trend_actor_run.clone(),
            item: item.clone(),
        },
    )?;

    let candidates = collect_candidate_posts(
        client,
        token,
        &options.resolver_actor_id,
        &item,
        options.max_posts,
    )?;
    write_json(&posts_path, &candidates.resolver_posts_artifact)?;
    write_json(&selection_path, &candidates.selection_artifact)?;

    let download = download_candidate_media_assets(
        client,
        token,
        &sound_dir,
        &videos_dir,
        &audios_dir,
        &candidates.selection_artifact.candidates,
        options.download_attempts,
    )?;
    write_json(
        &download_path,
        &DownloadArtifact {
            method: DIRECT_DOWNLOAD_METHOD.to_string(),
            requested_candidate_count: candidates.selection_artifact.candidates.len(),
            successful_video_count: download.downloaded_video_count(),
            extracted_audio_count: download.extracted_audio_count(),
            representative_video_id: download
                .representative_asset()
                .map(|asset| asset.candidate.video_id.clone()),
            representative_local_video_path: download
                .representative_asset()
                .map(|asset| asset.video_path.display().to_string()),
            representative_local_audio_path: download
                .representative_asset()
                .and_then(|asset| asset.audio_path.as_ref())
                .map(|path| path.display().to_string()),
            assets: download.asset_artifacts.clone(),
        },
    )?;

    if download.downloaded_video_count() == 0 {
        bail!(
            "resolver actor {} returned candidates for sound {} but none exposed a usable media URL that downloaded successfully",
            options.resolver_actor_id,
            item.link
        )
    }

    let representative_asset = download.representative_asset().ok_or_else(|| {
        anyhow!(
            "downloaded {} videos for sound {} but could not extract audio from any of them",
            download.downloaded_video_count(),
            item.link
        )
    })?;
    let representative_audio_path = representative_asset
        .audio_path
        .as_ref()
        .context("representative asset is missing extracted audio")?;

    let rights_note =
        "For research and internal prototyping only. Verify rights before redistribution or production use."
            .to_string();
    let provenance =
        "Imported from Apify trending sounds, resolved from the sound URL with a Novi actor, ranked by like count, downloaded for every usable resolver post media URL, and audio extracted locally with ffmpeg for each downloaded video when possible."
            .to_string();

    let downloaded_assets = download
        .downloaded_assets
        .iter()
        .map(|asset| DownloadedAssetMetadata {
            candidate_rank: asset.candidate.selection_rank,
            resolver_index: asset.candidate.resolver_index,
            source: asset.candidate.source.clone(),
            video_id: asset.candidate.video_id.clone(),
            video_url: asset.candidate.video_url.clone(),
            direct_video_url: asset.selected_media_url.clone(),
            audio_url: asset.candidate.audio_url.clone(),
            author_unique_id: asset.candidate.author_unique_id.clone(),
            author_nickname: asset.candidate.author_nickname.clone(),
            title: asset.candidate.title.clone(),
            region: asset.candidate.region.clone(),
            duration_seconds: asset.candidate.duration_seconds,
            play_count: asset.candidate.play_count,
            like_count: asset.candidate.digg_count,
            comment_count: asset.candidate.comment_count,
            share_count: asset.candidate.share_count,
            local_video_path: asset.video_path.display().to_string(),
            local_audio_path: asset
                .audio_path
                .as_ref()
                .map(|path| path.display().to_string()),
        })
        .collect::<Vec<_>>();

    let metadata = ImportedSoundMetadata {
        id: sound_id.clone(),
        rank: item.rank,
        title: item.title.clone(),
        author: item.author.clone(),
        trend_link: item.link.clone(),
        clip_id: item.clip_id.clone(),
        song_id: item.song_id.clone(),
        country_code: item.country_code.clone(),
        duration_seconds: representative_asset
            .candidate
            .duration_seconds
            .unwrap_or(item.duration),
        downloaded_video_count: download.downloaded_video_count(),
        extracted_audio_count: download.extracted_audio_count(),
        actors: ActorChainMetadata {
            trends_actor: TRENDS_ACTOR_ID.to_string(),
            sound_resolver_actor: options.resolver_actor_id.clone(),
            download_method: DIRECT_DOWNLOAD_METHOD.to_string(),
        },
        selection: SelectionSummary {
            ranking_strategy: candidates.selection_artifact.ranking_strategy.clone(),
            candidate_count: candidates.selection_artifact.candidates.len(),
            representative_video_id: representative_asset.candidate.video_id.clone(),
            representative_video_url: representative_asset.candidate.video_url.clone(),
            representative_direct_video_url: representative_asset.selected_media_url.clone(),
            representative_audio_url: representative_asset.candidate.audio_url.clone(),
            representative_comment_count: representative_asset.candidate.comment_count,
            representative_share_count: representative_asset.candidate.share_count,
            representative_like_count: representative_asset.candidate.digg_count,
            representative_view_count: representative_asset.candidate.play_count,
        },
        files: LocalArtifacts {
            trend_path: trend_path.display().to_string(),
            posts_path: posts_path.display().to_string(),
            selection_path: selection_path.display().to_string(),
            download_path: download_path.display().to_string(),
            metadata_path: metadata_path.display().to_string(),
            videos_dir: videos_dir.display().to_string(),
            audios_dir: audios_dir.display().to_string(),
            representative_video_path: representative_asset.video_path.display().to_string(),
            representative_audio_path: representative_audio_path.display().to_string(),
        },
        assets: downloaded_assets,
        provenance: provenance.clone(),
        rights_note: rights_note.clone(),
    };
    write_json(&metadata_path, &metadata)?;

    Ok(ImportedSoundRecord {
        manifest_entry: ManifestEntry {
            id: sound_id.clone(),
            title: item.title.clone(),
            author: item.author.clone(),
            platform: "tiktok".to_string(),
            trend_rank: Some(item.rank),
            source_url: item.link.clone(),
            source_video_url: Some(representative_asset.candidate.video_url.clone()),
            duration_seconds: representative_asset
                .candidate
                .duration_seconds
                .or(Some(item.duration)),
            local_audio_path: representative_audio_path.display().to_string(),
            local_metadata_path: metadata_path.display().to_string(),
            rights_note: rights_note.clone(),
            provenance: provenance.clone(),
            song_id: Some(item.song_id.clone()),
            clip_id: Some(item.clip_id.clone()),
            country_code: Some(item.country_code.clone()),
            local_video_path: Some(representative_asset.video_path.display().to_string()),
            local_trend_path: Some(trend_path.display().to_string()),
            local_posts_path: Some(posts_path.display().to_string()),
            local_selection_path: Some(selection_path.display().to_string()),
            local_download_path: Some(download_path.display().to_string()),
            local_videos_dir: Some(videos_dir.display().to_string()),
            local_audios_dir: Some(audios_dir.display().to_string()),
            downloaded_video_count: Some(download.downloaded_video_count()),
            extracted_audio_count: Some(download.extracted_audio_count()),
            representative_video_url: Some(representative_asset.candidate.video_url.clone()),
            representative_video_id: Some(representative_asset.candidate.video_id.clone()),
            representative_comment_count: representative_asset.candidate.comment_count,
            representative_share_count: representative_asset.candidate.share_count,
            representative_like_count: representative_asset.candidate.digg_count,
            representative_view_count: representative_asset.candidate.play_count,
            resolver_actor_id: Some(options.resolver_actor_id.clone()),
            download_method: Some(DIRECT_DOWNLOAD_METHOD.to_string()),
        },
        report: ImportedSound {
            id: sound_id,
            rank: item.rank,
            title: item.title,
            author: item.author,
            song_id: item.song_id,
            clip_id: item.clip_id,
            trend_link: item.link,
            selected_video_url: representative_asset.candidate.video_url.clone(),
            selected_video_id: Some(representative_asset.candidate.video_id.clone()),
            selected_like_count: representative_asset.candidate.digg_count,
            selected_comment_count: representative_asset.candidate.comment_count,
            candidate_posts_considered: candidates.selection_artifact.candidates.len(),
            downloaded_video_count: download.downloaded_video_count(),
            extracted_audio_count: download.extracted_audio_count(),
            resolver_actor: options.resolver_actor_id.clone(),
            download_method: DIRECT_DOWNLOAD_METHOD.to_string(),
            local_videos_dir: videos_dir.display().to_string(),
            local_audios_dir: audios_dir.display().to_string(),
            local_video_path: representative_asset.video_path.display().to_string(),
            local_audio_path: representative_audio_path.display().to_string(),
            local_metadata_path: metadata_path.display().to_string(),
        },
    })
}

fn collect_candidate_posts(
    client: &Client,
    token: &str,
    resolver_actor_id: &str,
    item: &TrendingSoundItem,
    max_posts: usize,
) -> Result<CandidateSelectionResult> {
    let input_profile = json!({
        "type": SOUND_RESOLVER_INPUT_TYPE,
        "url": item.link,
        "region": DEFAULT_SOUND_RESOLVER_REGION,
        "limit": SOUND_RESOLVER_INPUT_LIMIT,
        "isUnlimited": false,
        "publishTime": "MONTH",
        "sortType": 1,
        "isDownloadVideo": false,
        "isDownloadVideoCover": false
    });

    let actor_run = apify::run_actor(client, token, resolver_actor_id, &input_profile)?;
    let raw_dataset = apify::fetch_dataset_values(client, token, &actor_run.default_dataset_id)
        .with_context(|| {
            format!(
                "failed to fetch sound resolver dataset for sound URL {}",
                item.link
            )
        })?;

    let mut seen = BTreeSet::new();
    let mut candidates = raw_dataset
        .iter()
        .enumerate()
        .filter_map(|(index, row)| normalize_resolver_post_item(row, index))
        .filter(|candidate| seen.insert(candidate_key(candidate)))
        .collect::<Vec<_>>();
    let raw_dataset_count = raw_dataset.len();
    let normalized_candidate_count = candidates.len();

    if candidates.is_empty() {
        bail!(
            "resolver actor {} returned no usable candidates for sound {}",
            resolver_actor_id,
            item.link
        )
    }

    rank_candidate_posts(&mut candidates);
    candidates.truncate(max_posts.min(SOUND_RESOLVER_INPUT_LIMIT));
    let preferred_candidate = candidates.first().cloned();

    Ok(CandidateSelectionResult {
        resolver_posts_artifact: ResolverPostsArtifact {
            actor_id: resolver_actor_id.to_string(),
            actor_run: actor_run.clone(),
            input_profile,
            requested_sound_url: item.link.clone(),
            requested_max_results: SOUND_RESOLVER_INPUT_LIMIT,
            debug_related_items: item.related_items.clone(),
            raw_dataset,
        },
        selection_artifact: CandidateSelectionArtifact {
            actor_id: resolver_actor_id.to_string(),
            actor_run,
            requested_sound_url: item.link.clone(),
            requested_max_results: max_posts.min(SOUND_RESOLVER_INPUT_LIMIT),
            raw_dataset_count,
            normalized_candidate_count,
            debug_related_item_count: item.related_items.len(),
            ranking_strategy: "digg_count desc, resolver order asc".to_string(),
            preferred_candidate,
            candidates,
        },
    })
}

impl DownloadResolution {
    fn representative_asset(&self) -> Option<&DownloadedAsset> {
        self.downloaded_assets
            .iter()
            .find(|asset| asset.audio_path.is_some())
    }

    fn downloaded_video_count(&self) -> usize {
        self.downloaded_assets.len()
    }

    fn extracted_audio_count(&self) -> usize {
        self.downloaded_assets
            .iter()
            .filter(|asset| asset.audio_path.is_some())
            .count()
    }
}

fn prepare_sound_dir(sound_dir: &Path) -> Result<()> {
    fs::create_dir_all(sound_dir)
        .with_context(|| format!("failed to create {}", sound_dir.display()))?;

    for path in [
        sound_dir.join("videos"),
        sound_dir.join("audios"),
        sound_dir.join("video.mp4"),
        sound_dir.join("audio.mp3"),
    ] {
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }
    }

    fs::create_dir_all(sound_dir.join("videos"))
        .with_context(|| format!("failed to create {}", sound_dir.join("videos").display()))?;
    fs::create_dir_all(sound_dir.join("audios"))
        .with_context(|| format!("failed to create {}", sound_dir.join("audios").display()))?;

    Ok(())
}

fn download_candidate_media_assets(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    videos_dir: &Path,
    audios_dir: &Path,
    candidates: &[CandidatePost],
    download_attempts: usize,
) -> Result<DownloadResolution> {
    let mut downloaded_assets = Vec::new();
    let mut asset_artifacts = Vec::new();

    for candidate in candidates {
        let (downloaded_asset, artifact) = download_candidate_media(
            client,
            token,
            sound_dir,
            videos_dir,
            audios_dir,
            candidate,
            download_attempts,
        );

        if let Some(asset) = downloaded_asset {
            downloaded_assets.push(asset);
        }
        asset_artifacts.push(artifact);
    }

    Ok(DownloadResolution {
        downloaded_assets,
        asset_artifacts,
    })
}

fn download_candidate_media(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    videos_dir: &Path,
    audios_dir: &Path,
    candidate: &CandidatePost,
    download_attempts: usize,
) -> (Option<DownloadedAsset>, CandidateDownloadArtifact) {
    let mut artifact = CandidateDownloadArtifact {
        candidate_rank: candidate.selection_rank,
        resolver_index: candidate.resolver_index,
        candidate_source: candidate.source.clone(),
        candidate_video_id: candidate.video_id.clone(),
        candidate_video_url: candidate.video_url.clone(),
        resolved_direct_video_url: None,
        resolved_audio_url: candidate.audio_url.clone(),
        local_video_path: None,
        local_audio_path: None,
        status: DownloadStatus::Failed,
        error: None,
        attempts: Vec::new(),
    };

    let Some(media_url) = candidate
        .download_url
        .clone()
        .or_else(|| candidate.public_media_url.clone())
    else {
        artifact.status = DownloadStatus::SkippedMissingMediaUrl;
        artifact.error = Some(format!(
            "candidate {} did not expose a downloadable or public media URL",
            candidate.video_url
        ));
        return (None, artifact);
    };
    artifact.resolved_direct_video_url = Some(media_url.clone());

    let base_name = asset_file_stem(candidate);
    let final_video = videos_dir.join(format!("{base_name}.mp4"));
    let final_audio = audios_dir.join(format!("{base_name}.mp3"));
    let max_attempts = download_attempts.max(1);

    for attempt_number in 1..=max_attempts {
        let temp_video = sound_dir.join(format!(".{base_name}.download.{attempt_number}.mp4"));
        let temp_audio = sound_dir.join(format!(".{base_name}.audio.{attempt_number}.mp3"));
        let mut attempt = CandidateDownloadAttemptArtifact {
            attempt_number,
            error: None,
        };

        if let Err(error) = apify::download_to_path(client, token, &media_url, &temp_video) {
            let _ = fs::remove_file(&temp_video);
            attempt.error = Some(format!("{error:#}"));
            artifact.attempts.push(attempt);
            artifact.error = Some(format!("{error:#}"));
            continue;
        }

        if let Err(error) = promote_temp_file(&temp_video, &final_video) {
            let _ = fs::remove_file(&temp_video);
            attempt.error = Some(format!("{error:#}"));
            artifact.attempts.push(attempt);
            artifact.error = Some(format!("{error:#}"));
            continue;
        }

        artifact.local_video_path = Some(final_video.display().to_string());

        match extract_audio_from_video(&final_video, &temp_audio) {
            Ok(()) => match promote_temp_file(&temp_audio, &final_audio) {
                Ok(()) => {
                    artifact.local_audio_path = Some(final_audio.display().to_string());
                    artifact.status = DownloadStatus::Downloaded;
                    artifact.error = None;
                    artifact.attempts.push(attempt);
                    return (
                        Some(DownloadedAsset {
                            candidate: candidate.clone(),
                            selected_media_url: media_url.clone(),
                            video_path: final_video,
                            audio_path: Some(final_audio),
                        }),
                        artifact,
                    );
                }
                Err(error) => {
                    let _ = fs::remove_file(&temp_audio);
                    attempt.error = Some(format!("{error:#}"));
                    artifact.status = DownloadStatus::DownloadedVideoOnly;
                    artifact.error = Some(format!("{error:#}"));
                    artifact.attempts.push(attempt);
                    return (
                        Some(DownloadedAsset {
                            candidate: candidate.clone(),
                            selected_media_url: media_url.clone(),
                            video_path: final_video,
                            audio_path: None,
                        }),
                        artifact,
                    );
                }
            },
            Err(error) => {
                let _ = fs::remove_file(&temp_audio);
                attempt.error = Some(format!("{error:#}"));
                artifact.status = DownloadStatus::DownloadedVideoOnly;
                artifact.error = Some(format!("{error:#}"));
                artifact.attempts.push(attempt);
                return (
                    Some(DownloadedAsset {
                        candidate: candidate.clone(),
                        selected_media_url: media_url.clone(),
                        video_path: final_video,
                        audio_path: None,
                    }),
                    artifact,
                );
            }
        }
    }

    (None, artifact)
}

fn extract_audio_from_video(video_path: &Path, audio_path: &Path) -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video_path)
        .arg("-vn")
        .arg("-acodec")
        .arg("libmp3lame")
        .arg("-q:a")
        .arg("2")
        .arg(audio_path)
        .output()
        .with_context(|| {
            format!(
                "failed to invoke ffmpeg while extracting audio from {}",
                video_path.display()
            )
        })?;

    if !output.status.success() {
        bail!(
            "ffmpeg failed while extracting audio from {}: {}",
            video_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }

    Ok(())
}

fn promote_temp_file(from: &Path, to: &Path) -> Result<()> {
    if to.exists() {
        fs::remove_file(to).with_context(|| format!("failed to remove {}", to.display()))?;
    }

    fs::rename(from, to)
        .with_context(|| format!("failed to rename {} to {}", from.display(), to.display()))
}

fn normalize_resolver_post_item(item: &Value, resolver_index: usize) -> Option<CandidatePost> {
    let aweme_id = first_non_empty_string(
        item,
        &[
            &["aweme_id"],
            &["awemeId"],
            &["video_id"],
            &["videoId"],
            &["item_id"],
            &["itemId"],
            &["id"],
        ],
    );
    let author_unique_id = first_non_empty_string(
        item,
        &[
            &["author", "unique_id"],
            &["author", "uniqueId"],
            &["authorUniqueId"],
        ],
    )
    .map(normalize_author_unique_id);
    let share_url = first_non_empty_string(
        item,
        &[
            &["share_url"],
            &["shareUrl"],
            &["share_info", "share_url"],
            &["shareInfo", "shareUrl"],
            &["web_video_url"],
            &["webVideoUrl"],
            &["post_url"],
            &["postUrl"],
            &["url"],
        ],
    )
    .filter(|url| is_tiktok_url(url));
    let video_id = aweme_id.clone().or_else(|| {
        share_url
            .as_deref()
            .and_then(tiktok_video_id)
            .map(ToString::to_string)
    })?;
    let canonical_url = canonical_video_url(author_unique_id.as_deref(), &video_id);
    let video_url = share_url.unwrap_or(canonical_url);

    Some(CandidatePost {
        selection_rank: 0,
        resolver_index,
        source: CandidatePostSource::SoundResolverActor,
        video_id,
        aweme_id,
        video_url,
        author_unique_id,
        author_nickname: first_non_empty_string(
            item,
            &[&["author", "nickname"], &["authorNickname"], &["nickname"]],
        ),
        title: first_non_empty_string(item, &[&["title"], &["desc"]]),
        region: first_non_empty_string(item, &[&["region"]]),
        duration_seconds: first_duration_seconds(
            item,
            &[
                &["duration"],
                &["video", "duration"],
                &["video", "durationMs"],
                &["video", "duration_ms"],
            ],
        ),
        play_count: first_u64(
            item,
            &[
                &["play_count"],
                &["playCount"],
                &["stats", "play_count"],
                &["stats", "playCount"],
                &["statistics", "play_count"],
                &["statistics", "playCount"],
            ],
        ),
        digg_count: first_u64(
            item,
            &[
                &["digg_count"],
                &["diggCount"],
                &["like_count"],
                &["likeCount"],
                &["stats", "digg_count"],
                &["stats", "diggCount"],
                &["stats", "likeCount"],
                &["statistics", "digg_count"],
                &["statistics", "diggCount"],
                &["statistics", "like_count"],
                &["statistics", "likeCount"],
            ],
        ),
        comment_count: first_u64(
            item,
            &[
                &["comment_count"],
                &["commentCount"],
                &["stats", "comment_count"],
                &["stats", "commentCount"],
                &["statistics", "comment_count"],
                &["statistics", "commentCount"],
            ],
        ),
        share_count: first_u64(
            item,
            &[
                &["share_count"],
                &["shareCount"],
                &["stats", "share_count"],
                &["stats", "shareCount"],
                &["statistics", "share_count"],
                &["statistics", "shareCount"],
            ],
        ),
        download_url: first_non_empty_string(
            item,
            &[
                &["video", "downloadAddr"],
                &["video", "download_addr"],
                &["video", "downloadUrl"],
                &["video", "download_url"],
                &["video", "downloadAddr", "urlList", "*"],
                &["video", "downloadAddr", "url_list", "*"],
                &["video", "download_addr", "urlList", "*"],
                &["video", "download_addr", "url_list", "*"],
                &["downloadUrl"],
                &["download_url"],
                &["videoDownloadUrl"],
                &["video_download_url"],
            ],
        ),
        public_media_url: first_non_empty_string(
            item,
            &[
                &["video", "playAddr"],
                &["video", "play_addr"],
                &["video", "playUrl"],
                &["video", "play_url"],
                &["video", "playAddr", "urlList", "*"],
                &["video", "playAddr", "url_list", "*"],
                &["video", "play_addr", "urlList", "*"],
                &["video", "play_addr", "url_list", "*"],
                &["video", "bitrateInfo", "*", "playAddr", "urlList", "*"],
                &["video", "bitrate_info", "*", "play_addr", "url_list", "*"],
                &["playUrl"],
                &["play_url"],
                &["videoUrl"],
                &["video_url"],
            ],
        ),
        audio_url: first_non_empty_string(
            item,
            &[
                &["music", "playUrl"],
                &["music", "play_url"],
                &["music", "playUrl", "urlList", "*"],
                &["music", "play_url", "url_list", "*"],
                &["music", "audioUrl"],
                &["music", "audio_url"],
                &["audioUrl"],
                &["audio_url"],
            ],
        ),
        cover_url: first_non_empty_string(
            item,
            &[
                &["video", "cover"],
                &["video", "cover", "urlList", "*"],
                &["video", "cover", "url_list", "*"],
                &["cover"],
                &["coverUrl"],
                &["cover_url"],
            ],
        ),
    })
}

fn rank_candidate_posts(candidates: &mut [CandidatePost]) {
    candidates.sort_by(|left, right| {
        sort_metric(right.digg_count)
            .cmp(&sort_metric(left.digg_count))
            .then_with(|| left.resolver_index.cmp(&right.resolver_index))
    });

    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.selection_rank = index + 1;
    }
}

fn read_optional_metadata(manifest_path: &Path, metadata_path: &str) -> Result<Option<Value>> {
    let path = resolve_library_path(manifest_path, metadata_path);
    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))
        .map(Some)
}

fn candidate_post_count_from_selection_artifact(
    manifest_path: &Path,
    entry: &ManifestEntry,
) -> Result<Option<usize>> {
    let Some(selection_path) = entry
        .local_selection_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(None);
    };

    let path = resolve_library_path(manifest_path, selection_path);
    if !path.exists() {
        return Ok(None);
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let metadata = Some(value);
    Ok(metadata_usize(
        &metadata,
        &[
            &["candidate_count"],
            &["normalized_candidate_count"],
            &["raw_dataset_count"],
        ],
    )
    .or_else(|| {
        metadata
            .as_ref()
            .and_then(|value| value.get("candidates"))
            .and_then(Value::as_array)
            .map(Vec::len)
    }))
}

fn representative_engagement_from_posts_artifact(
    manifest_path: &Path,
    entry: &ManifestEntry,
) -> Result<RepresentativeEngagementMetrics> {
    let Some(posts_path) = entry
        .local_posts_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(RepresentativeEngagementMetrics::default());
    };
    let Some(video_id) = representative_video_id(entry) else {
        return Ok(RepresentativeEngagementMetrics::default());
    };

    let path = resolve_library_path(manifest_path, posts_path);
    if !path.exists() {
        return Ok(RepresentativeEngagementMetrics::default());
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(raw_dataset) = resolver_raw_dataset(&value) else {
        return Ok(RepresentativeEngagementMetrics::default());
    };

    for (index, row) in raw_dataset.iter().enumerate() {
        if let Some(candidate) = normalize_resolver_post_item(row, index) {
            let is_representative = candidate.video_id == video_id
                || candidate.aweme_id.as_deref() == Some(video_id.as_str());
            if is_representative {
                return Ok(RepresentativeEngagementMetrics {
                    view_count: candidate.play_count,
                    like_count: candidate.digg_count,
                    comment_count: candidate.comment_count,
                    share_count: candidate.share_count,
                });
            }
        }
    }

    Ok(RepresentativeEngagementMetrics::default())
}

fn representative_music_signals_from_posts_artifact(
    manifest_path: &Path,
    entry: &ManifestEntry,
) -> Result<RepresentativeMusicSignals> {
    let Some(posts_path) = entry
        .local_posts_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(RepresentativeMusicSignals::default());
    };
    let Some(video_id) = representative_video_id(entry) else {
        return Ok(RepresentativeMusicSignals::default());
    };

    let path = resolve_library_path(manifest_path, posts_path);
    if !path.exists() {
        return Ok(RepresentativeMusicSignals::default());
    }

    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let Some(raw_dataset) = resolver_raw_dataset(&value) else {
        return Ok(RepresentativeMusicSignals::default());
    };

    for (index, row) in raw_dataset.iter().enumerate() {
        if let Some(candidate) = normalize_resolver_post_item(row, index) {
            let is_representative = candidate.video_id == video_id
                || candidate.aweme_id.as_deref() == Some(video_id.as_str());
            if is_representative {
                return Ok(representative_music_signals_from_row(row));
            }
        }
    }

    Ok(RepresentativeMusicSignals::default())
}

fn representative_video_id(entry: &ManifestEntry) -> Option<String> {
    entry
        .representative_video_id
        .clone()
        .or_else(|| {
            entry
                .representative_video_url
                .as_deref()
                .and_then(tiktok_video_id)
                .map(ToString::to_string)
        })
        .or_else(|| {
            entry
                .source_video_url
                .as_deref()
                .and_then(tiktok_video_id)
                .map(ToString::to_string)
        })
}

fn resolver_raw_dataset(value: &Value) -> Option<&[Value]> {
    value
        .get("raw_dataset")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .map(Vec::as_slice)
}

fn resolve_library_path(manifest_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() || path.exists() {
        return path;
    }

    manifest_path
        .parent()
        .map(|parent| parent.join(&path))
        .filter(|candidate| candidate.exists())
        .unwrap_or(path)
}

fn representative_music_signals_from_row(row: &Value) -> RepresentativeMusicSignals {
    let parsed_extra = row
        .get("music")
        .and_then(|music| music.get("extra"))
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok());

    RepresentativeMusicSignals {
        duration_seconds: parsed_extra
            .as_ref()
            .and_then(|extra| first_f64(extra, &[&["aed_music_dur"]])),
        can_read: parsed_extra
            .as_ref()
            .and_then(|extra| first_bool(extra, &[&["can_read"]])),
        can_reuse: parsed_extra
            .as_ref()
            .and_then(|extra| first_bool(extra, &[&["can_reuse"]])),
        is_original_sound: first_bool(
            row,
            &[&["music", "is_original_sound"], &["music", "is_original"]],
        ),
        commercial_right_type: first_u64(
            row,
            &[
                &["music", "commercial_right_type"],
                &["music", "commercialRightType"],
            ],
        )
        .or_else(|| {
            parsed_extra.as_ref().and_then(|extra| {
                first_u64(
                    extra,
                    &[&["commercial_right_type"], &["commercialRightType"]],
                )
            })
        }),
        is_batch_take_down_music: first_bool(
            row,
            &[
                &["music", "is_batch_take_down_music"],
                &["music", "isBatchTakeDownMusic"],
            ],
        )
        .or_else(|| {
            parsed_extra.as_ref().and_then(|extra| {
                first_bool(
                    extra,
                    &[&["is_batch_take_down_music"], &["isBatchTakeDownMusic"]],
                )
            })
        }),
        reviewed: parsed_extra
            .as_ref()
            .and_then(|extra| first_bool(extra, &[&["reviewed"]])),
        has_strong_beat_url: first_non_empty_string(
            row,
            &[
                &["music", "strong_beat_url", "url_list", "*"],
                &["music", "strong_beat_url", "uri"],
                &["music", "strongBeatUrl", "urlList", "*"],
            ],
        )
        .map(|_| true),
        music_vid: parsed_extra
            .as_ref()
            .and_then(|extra| first_non_empty_string(extra, &[&["music_vid"]])),
    }
}

fn metadata_u64(metadata: &Option<Value>, paths: &[&[&str]]) -> Option<u64> {
    metadata.as_ref().and_then(|value| first_u64(value, paths))
}

fn first_f64(value: &Value, paths: &[&[&str]]) -> Option<f64> {
    paths.iter().find_map(|path| float_at_path(value, path))
}

fn first_bool(value: &Value, paths: &[&[&str]]) -> Option<bool> {
    paths.iter().find_map(|path| bool_at_path(value, path))
}

fn metadata_usize(metadata: &Option<Value>, paths: &[&[&str]]) -> Option<usize> {
    metadata_u64(metadata, paths).and_then(|value| usize::try_from(value).ok())
}

fn has_entry_or_metadata_path(
    entry_path: Option<&str>,
    metadata: &Option<Value>,
    metadata_paths: &[&[&str]],
) -> bool {
    entry_path.is_some_and(|path| !path.trim().is_empty())
        || metadata
            .as_ref()
            .and_then(|value| first_non_empty_string(value, metadata_paths))
            .is_some()
}

fn trend_rank_score(rank: u32) -> u32 {
    match rank {
        1..=3 => 35,
        4..=10 => 25,
        11..=50 => 15,
        _ => 5,
    }
}

fn add_engagement_signal(
    views: Option<u64>,
    likes: Option<u64>,
    comments: Option<u64>,
    shares: Option<u64>,
    score: &mut u32,
    reasons: &mut Vec<String>,
    risks: &mut Vec<String>,
) {
    let mut engagement_points = 0;

    if let Some(value) = views {
        let points = threshold_score(value, &[(1_000_000, 15), (100_000, 8), (1, 3)]);
        engagement_points += points;
        if points > 0 {
            reasons.push(format!("{value} representative views are recorded"));
        }
    }

    if let Some(value) = likes {
        let points = threshold_score(value, &[(100_000, 20), (10_000, 12), (1, 5)]);
        engagement_points += points;
        if points > 0 {
            reasons.push(format!("{value} representative likes are recorded"));
        }
    }

    if let Some(value) = comments {
        let points = threshold_score(value, &[(10_000, 8), (1_000, 4), (1, 2)]);
        engagement_points += points;
        if points > 0 {
            reasons.push(format!("{value} representative comments are recorded"));
        }
    }

    if let Some(value) = shares {
        let points = threshold_score(value, &[(10_000, 8), (1_000, 4), (1, 2)]);
        engagement_points += points;
        if points > 0 {
            reasons.push(format!("{value} representative shares are recorded"));
        }
    }

    if engagement_points == 0 {
        risks.push("No representative engagement metrics are recorded".to_string());
    } else {
        *score += engagement_points.min(25);
    }
}

fn threshold_score(value: u64, thresholds: &[(u64, u32)]) -> u32 {
    thresholds
        .iter()
        .find_map(|(minimum, points)| (value >= *minimum).then_some(*points))
        .unwrap_or_default()
}

fn rate_per_1000(numerator: Option<u64>, denominator: Option<u64>) -> Option<u64> {
    rate_sum_per_1000(&[numerator], denominator)
}

fn sum_counts(values: &[Option<u64>]) -> Option<u64> {
    let mut total = 0_u128;
    let mut has_value = false;
    for value in values.iter().flatten() {
        total += u128::from(*value);
        has_value = true;
    }

    has_value.then(|| u64::try_from(total).ok())?
}

fn rate_sum_per_1000(numerators: &[Option<u64>], denominator: Option<u64>) -> Option<u64> {
    let denominator = u128::from(denominator?);
    if denominator == 0 {
        return None;
    }

    let mut total = 0_u128;
    let mut has_numerator = false;
    for numerator in numerators.iter().flatten() {
        total += u128::from(*numerator);
        has_numerator = true;
    }

    has_numerator.then(|| u64::try_from((total * 1_000) / denominator).ok())?
}

fn risk_requires_rights_review(risk: &str) -> bool {
    risk.contains("Rights still need manual verification") || risk.contains("batch-takedown music")
}

fn recommended_action(score: u32, risks: &[String]) -> &'static str {
    let rights_review_needed = risks.iter().any(|risk| risk_requires_rights_review(risk));

    match (score, rights_review_needed) {
        (75..=100, false) => "use_first",
        (75..=100, true) => "shortlist_after_rights_review",
        (50..=74, _) => "shortlist",
        (30..=49, _) => "needs_review",
        _ => "skip_for_now",
    }
}

fn read_manifest(path: &Path) -> Result<Manifest> {
    if !path.exists() {
        return Ok(Manifest::default());
    }

    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    if value.get("sounds").is_some() {
        serde_json::from_value(value).context("failed to parse structured manifest")
    } else if value.is_array() {
        let sounds = serde_json::from_value::<Vec<ManifestEntry>>(value)
            .context("failed to parse legacy array manifest")?;
        Ok(Manifest { sounds })
    } else {
        bail!("unsupported manifest shape in {}", path.display())
    }
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    fs::write(path, serde_json::to_vec_pretty(manifest)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn merge_manifest_entry(manifest: &mut Manifest, entry: ManifestEntry) {
    if let Some(existing) = manifest
        .sounds
        .iter_mut()
        .find(|sound| sound.id == entry.id)
    {
        *existing = entry;
    } else {
        manifest.sounds.push(entry);
        manifest
            .sounds
            .sort_by_key(|sound| sound.trend_rank.unwrap_or(u32::MAX));
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;

    for ch in input.chars() {
        let lowered = ch.to_ascii_lowercase();
        if lowered.is_ascii_alphanumeric() {
            out.push(lowered);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }

    out.trim_matches('-').to_string()
}

fn candidate_key(candidate: &CandidatePost) -> String {
    format!("{}|{}", candidate.video_id, candidate.video_url)
}

fn asset_file_stem(candidate: &CandidatePost) -> String {
    format!("{:02}-{}", candidate.selection_rank, candidate.video_id)
}

fn sort_metric(value: Option<u64>) -> u64 {
    value.unwrap_or_default()
}

fn canonical_video_url(author_unique_id: Option<&str>, video_id: &str) -> String {
    format!(
        "https://www.tiktok.com/@{}/video/{}",
        author_unique_id.unwrap_or("i"),
        video_id
    )
}

fn normalize_author_unique_id(author_unique_id: String) -> String {
    author_unique_id.trim_start_matches('@').to_string()
}

fn is_tiktok_url(value: &str) -> bool {
    value.contains("tiktok.com")
}

fn tiktok_video_id(url: &str) -> Option<&str> {
    extract_numeric_suffix(url, "/video/")
        .or_else(|| extract_numeric_suffix(url, "/v/"))
        .or_else(|| extract_numeric_suffix(url, "/videoId/"))
}

fn extract_numeric_suffix<'a>(value: &'a str, marker: &str) -> Option<&'a str> {
    let start = value.find(marker)? + marker.len();
    let end = value[start..]
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .map(|(index, _)| index)
        .last()?;

    Some(&value[start..=start + end])
}

fn first_non_empty_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| string_at_path(value, path))
        .map(ToString::to_string)
}

fn first_u64(value: &Value, paths: &[&[&str]]) -> Option<u64> {
    paths.iter().find_map(|path| unsigned_at_path(value, path))
}

fn first_duration_seconds(value: &Value, paths: &[&[&str]]) -> Option<u32> {
    first_u64(value, paths).and_then(|raw| {
        let seconds = if raw > 1_000 { raw / 1_000 } else { raw };
        u32::try_from(seconds).ok()
    })
}

fn string_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    values_at_path(value, path)
        .into_iter()
        .find_map(|candidate| candidate.as_str().map(str::trim))
        .filter(|candidate| !candidate.is_empty())
}

fn unsigned_at_path(value: &Value, path: &[&str]) -> Option<u64> {
    values_at_path(value, path)
        .into_iter()
        .find_map(|candidate| match candidate {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        })
}

fn float_at_path(value: &Value, path: &[&str]) -> Option<f64> {
    values_at_path(value, path)
        .into_iter()
        .find_map(|candidate| match candidate {
            Value::Number(number) => number.as_f64(),
            Value::String(text) => text.trim().parse().ok(),
            _ => None,
        })
}

fn bool_at_path(value: &Value, path: &[&str]) -> Option<bool> {
    values_at_path(value, path)
        .into_iter()
        .find_map(|candidate| match candidate {
            Value::Bool(boolean) => Some(*boolean),
            Value::Number(number) => match number.as_i64() {
                Some(1) => Some(true),
                Some(0) => Some(false),
                _ => None,
            },
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                "1" => Some(true),
                "0" => Some(false),
                _ => None,
            },
            _ => None,
        })
}

fn values_at_path<'a>(value: &'a Value, path: &[&str]) -> Vec<&'a Value> {
    if path.is_empty() {
        return vec![value];
    }

    let (segment, rest) = path.split_first().expect("non-empty path");

    if *segment == "*" {
        match value {
            Value::Array(items) => items
                .iter()
                .flat_map(|item| values_at_path(item, rest))
                .collect(),
            _ => Vec::new(),
        }
    } else {
        value
            .get(*segment)
            .map(|next| values_at_path(next, rest))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judged_sound(id: &str, score: u32, trend_rank: Option<u32>) -> JudgedSound {
        JudgedSound {
            sound_id: id.to_string(),
            judgement_rank: None,
            trend_rank,
            title: id.to_string(),
            author: "creator".to_string(),
            platform: "tiktok".to_string(),
            provenance: "test fixture".to_string(),
            rights_note: "For research only. Verify rights before production use.".to_string(),
            resolver_actor_id: Some("resolver".to_string()),
            download_method: Some(DIRECT_DOWNLOAD_METHOD.to_string()),
            source_url: format!("https://www.tiktok.com/music/{id}"),
            source_video_url: Some(format!("https://www.tiktok.com/@creator/video/{id}")),
            song_id: Some(id.to_string()),
            song_id_country_coverage_count: None,
            song_id_top_25_country_count: None,
            song_id_best_trend_rank: None,
            song_id_best_representative_view_count: None,
            song_id_best_representative_engagement_count: None,
            song_id_best_representative_share_count: None,
            song_id_best_representative_engagement_rate_per_1000_views: None,
            song_id_best_representative_share_rate_per_1000_views: None,
            clip_id: Some(format!("{id}_clip")),
            country_code: Some("US".to_string()),
            duration_seconds: Some(12),
            source_identifier_count: 6,
            source_identifier_fields: vec![
                "source_url".to_string(),
                "source_video_url".to_string(),
                "song_id".to_string(),
                "clip_id".to_string(),
                "country_code".to_string(),
                "duration_seconds".to_string(),
            ],
            missing_source_identifier_fields: Vec::new(),
            local_audio_path: format!("library/sounds/imported/{id}/audio.mp3"),
            local_video_path: Some(format!("library/sounds/imported/{id}/video.mp4")),
            local_metadata_path: format!("library/sounds/imported/{id}/metadata.json"),
            local_trend_path: Some(format!("library/sounds/imported/{id}/trend.json")),
            local_posts_path: Some(format!("library/sounds/imported/{id}/posts.json")),
            local_selection_path: Some(format!("library/sounds/imported/{id}/selection.json")),
            local_download_path: Some(format!("library/sounds/imported/{id}/download.json")),
            local_artifact_path_count: 7,
            local_artifact_path_fields: vec![
                "local_audio_path".to_string(),
                "local_video_path".to_string(),
                "local_metadata_path".to_string(),
                "local_trend_path".to_string(),
                "local_posts_path".to_string(),
                "local_selection_path".to_string(),
                "local_download_path".to_string(),
            ],
            missing_local_artifact_path_fields: Vec::new(),
            downloaded_video_count: Some(1),
            extracted_audio_count: Some(1),
            usable_asset_pair_count: Some(1),
            candidate_post_count: None,
            representative_view_count: None,
            representative_like_count: None,
            representative_engagement_count: None,
            representative_like_rate_per_1000_views: None,
            representative_engagement_rate_per_1000_views: None,
            representative_comment_count: None,
            representative_comment_rate_per_1000_views: None,
            representative_share_count: None,
            representative_share_rate_per_1000_views: None,
            representative_music_duration_seconds: None,
            representative_music_can_read: None,
            representative_music_can_reuse: None,
            representative_music_is_original_sound: None,
            representative_music_commercial_right_type: None,
            representative_music_is_batch_take_down_music: None,
            representative_music_reviewed: None,
            representative_music_has_strong_beat_url: None,
            representative_music_vid: None,
            representative_music_field_count: 0,
            representative_music_fields: Vec::new(),
            missing_representative_music_fields: REPRESENTATIVE_MUSIC_FIELDS
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
            representative_engagement_metric_count: 0,
            representative_engagement_metric_fields: Vec::new(),
            missing_representative_engagement_metric_fields: Vec::new(),
            score,
            reason_count: 0,
            reasons: Vec::new(),
            risk_count: 0,
            risks: Vec::new(),
            recommended_action: "shortlist_after_rights_review".to_string(),
        }
    }

    #[test]
    fn sort_and_rank_judged_sounds_assigns_stable_library_rank() {
        let mut sounds = vec![
            judged_sound("sound_c", 90, Some(2)),
            judged_sound("sound_b", 90, Some(1)),
            judged_sound("sound_a", 95, None),
            judged_sound("sound_d", 90, Some(1)),
        ];

        sort_and_rank_judged_sounds(&mut sounds);

        assert_eq!(
            sounds
                .iter()
                .map(|sound| sound.sound_id.as_str())
                .collect::<Vec<_>>(),
            vec!["sound_a", "sound_b", "sound_d", "sound_c"]
        );
        assert_eq!(
            sounds
                .iter()
                .map(|sound| sound.judgement_rank)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
    }

    #[test]
    fn annotate_song_id_country_coverage_counts_distinct_countries_per_song() {
        let mut us = judged_sound("sound_us", 95, Some(1));
        us.song_id = Some("shared_song".to_string());
        us.country_code = Some("US".to_string());

        let mut gb = judged_sound("sound_gb", 90, Some(2));
        gb.song_id = Some("shared_song".to_string());
        gb.country_code = Some("GB".to_string());

        let mut duplicate_us = judged_sound("sound_us_2", 85, Some(3));
        duplicate_us.song_id = Some("shared_song".to_string());
        duplicate_us.country_code = Some("us".to_string());

        let mut regional = judged_sound("sound_ca", 80, Some(4));
        regional.song_id = Some("regional_song".to_string());
        regional.country_code = Some("CA".to_string());

        let mut no_country = judged_sound("sound_unknown_country", 75, Some(5));
        no_country.song_id = Some("unknown_song".to_string());
        no_country.country_code = None;

        let mut no_song = judged_sound("sound_missing_song", 70, Some(6));
        no_song.song_id = None;
        no_song.country_code = Some("US".to_string());

        let mut sounds = vec![us, gb, duplicate_us, regional, no_country, no_song];
        annotate_song_id_country_coverage_counts(&mut sounds);

        let coverage = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_country_coverage_count,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(coverage.get("sound_us"), Some(&Some(2)));
        assert_eq!(coverage.get("sound_gb"), Some(&Some(2)));
        assert_eq!(coverage.get("sound_us_2"), Some(&Some(2)));
        assert_eq!(coverage.get("sound_ca"), Some(&Some(1)));
        assert_eq!(coverage.get("sound_unknown_country"), Some(&None));
        assert_eq!(coverage.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_trend_ranks_uses_lowest_rank_per_song() {
        let mut best = judged_sound("sound_best", 95, Some(4));
        best.song_id = Some("shared_song".to_string());

        let mut later_market = judged_sound("sound_later_market", 90, Some(19));
        later_market.song_id = Some("shared_song".to_string());

        let mut local = judged_sound("sound_local", 80, Some(33));
        local.song_id = Some("local_song".to_string());

        let mut missing_rank = judged_sound("sound_missing_rank", 75, None);
        missing_rank.song_id = Some("missing_rank_song".to_string());

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;

        let mut sounds = vec![best, later_market, local, missing_rank, missing_song];
        annotate_song_id_best_trend_ranks(&mut sounds);

        let best_ranks = sounds
            .iter()
            .map(|sound| (sound.sound_id.as_str(), sound.song_id_best_trend_rank))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_ranks.get("sound_best"), Some(&Some(4)));
        assert_eq!(best_ranks.get("sound_later_market"), Some(&Some(4)));
        assert_eq!(best_ranks.get("sound_local"), Some(&Some(33)));
        assert_eq!(best_ranks.get("sound_missing_rank"), Some(&None));
        assert_eq!(best_ranks.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_top_25_country_counts_tracks_strong_cross_market_charting() {
        let mut us = judged_sound("sound_us", 95, Some(4));
        us.song_id = Some("shared_song".to_string());
        us.country_code = Some("US".to_string());

        let mut gb = judged_sound("sound_gb", 90, Some(18));
        gb.song_id = Some("shared_song".to_string());
        gb.country_code = Some("GB".to_string());

        let mut weak_ca = judged_sound("sound_ca", 85, Some(33));
        weak_ca.song_id = Some("shared_song".to_string());
        weak_ca.country_code = Some("CA".to_string());

        let mut duplicate_us = judged_sound("sound_us_2", 80, Some(12));
        duplicate_us.song_id = Some("shared_song".to_string());
        duplicate_us.country_code = Some("us".to_string());

        let mut regional = judged_sound("sound_regional", 75, Some(7));
        regional.song_id = Some("regional_song".to_string());
        regional.country_code = Some("AU".to_string());

        let mut no_country = judged_sound("sound_unknown_country", 70, Some(5));
        no_country.song_id = Some("unknown_song".to_string());
        no_country.country_code = None;

        let mut sounds = vec![us, gb, weak_ca, duplicate_us, regional, no_country];
        annotate_song_id_top_25_country_counts(&mut sounds);

        let counts = sounds
            .iter()
            .map(|sound| (sound.sound_id.as_str(), sound.song_id_top_25_country_count))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(counts.get("sound_us"), Some(&Some(2)));
        assert_eq!(counts.get("sound_gb"), Some(&Some(2)));
        assert_eq!(counts.get("sound_ca"), Some(&Some(2)));
        assert_eq!(counts.get("sound_us_2"), Some(&Some(2)));
        assert_eq!(counts.get("sound_regional"), Some(&Some(1)));
        assert_eq!(counts.get("sound_unknown_country"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_representative_view_counts_uses_highest_views_per_song() {
        let mut biggest = judged_sound("sound_biggest", 95, Some(4));
        biggest.song_id = Some("shared_song".to_string());
        biggest.representative_view_count = Some(3_500_000);

        let mut smaller = judged_sound("sound_smaller", 90, Some(19));
        smaller.song_id = Some("shared_song".to_string());
        smaller.representative_view_count = Some(850_000);

        let mut local = judged_sound("sound_local", 80, Some(33));
        local.song_id = Some("local_song".to_string());
        local.representative_view_count = Some(120_000);

        let mut missing_views = judged_sound("sound_missing_views", 75, None);
        missing_views.song_id = Some("missing_views_song".to_string());
        missing_views.representative_view_count = None;

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;
        missing_song.representative_view_count = Some(9_000_000);

        let mut sounds = vec![biggest, smaller, local, missing_views, missing_song];
        annotate_song_id_best_representative_view_counts(&mut sounds);

        let best_views = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_best_representative_view_count,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_views.get("sound_biggest"), Some(&Some(3_500_000)));
        assert_eq!(best_views.get("sound_smaller"), Some(&Some(3_500_000)));
        assert_eq!(best_views.get("sound_local"), Some(&Some(120_000)));
        assert_eq!(best_views.get("sound_missing_views"), Some(&None));
        assert_eq!(best_views.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_representative_engagement_counts_uses_highest_engagements_per_song() {
        let mut biggest = judged_sound("sound_biggest", 95, Some(4));
        biggest.song_id = Some("shared_song".to_string());
        biggest.representative_engagement_count = Some(825_000);

        let mut smaller = judged_sound("sound_smaller", 90, Some(19));
        smaller.song_id = Some("shared_song".to_string());
        smaller.representative_engagement_count = Some(240_000);

        let mut local = judged_sound("sound_local", 80, Some(33));
        local.song_id = Some("local_song".to_string());
        local.representative_engagement_count = Some(125_000);

        let mut missing_engagement = judged_sound("sound_missing_engagement", 75, None);
        missing_engagement.song_id = Some("missing_engagement_song".to_string());
        missing_engagement.representative_engagement_count = None;

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;
        missing_song.representative_engagement_count = Some(1_250_000);

        let mut sounds = vec![biggest, smaller, local, missing_engagement, missing_song];
        annotate_song_id_best_representative_engagement_counts(&mut sounds);

        let best_engagements = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_best_representative_engagement_count,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_engagements.get("sound_biggest"), Some(&Some(825_000)));
        assert_eq!(best_engagements.get("sound_smaller"), Some(&Some(825_000)));
        assert_eq!(best_engagements.get("sound_local"), Some(&Some(125_000)));
        assert_eq!(
            best_engagements.get("sound_missing_engagement"),
            Some(&None)
        );
        assert_eq!(best_engagements.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_representative_share_counts_uses_highest_shares_per_song() {
        let mut biggest = judged_sound("sound_biggest", 95, Some(4));
        biggest.song_id = Some("shared_song".to_string());
        biggest.representative_share_count = Some(1_375_712);

        let mut smaller = judged_sound("sound_smaller", 83, Some(12));
        smaller.song_id = Some("shared_song".to_string());
        smaller.representative_share_count = Some(50_000);

        let mut local = judged_sound("sound_local", 77, Some(24));
        local.song_id = Some("local_song".to_string());
        local.representative_share_count = Some(8_000);

        let mut missing_share_count = judged_sound("sound_missing_share_count", 68, Some(30));
        missing_share_count.song_id = Some("missing_share_song".to_string());
        missing_share_count.representative_share_count = None;

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;
        missing_song.representative_share_count = Some(250_000);

        let mut sounds = vec![biggest, smaller, local, missing_share_count, missing_song];
        annotate_song_id_best_representative_share_counts(&mut sounds);

        let best_shares = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_best_representative_share_count,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_shares.get("sound_biggest"), Some(&Some(1_375_712)));
        assert_eq!(best_shares.get("sound_smaller"), Some(&Some(1_375_712)));
        assert_eq!(best_shares.get("sound_local"), Some(&Some(8_000)));
        assert_eq!(best_shares.get("sound_missing_share_count"), Some(&None));
        assert_eq!(best_shares.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_representative_engagement_rates_uses_highest_density_per_song() {
        let mut biggest = judged_sound("sound_biggest", 95, Some(4));
        biggest.song_id = Some("shared_song".to_string());
        biggest.representative_engagement_rate_per_1000_views = Some(235);

        let mut smaller = judged_sound("sound_smaller", 83, Some(12));
        smaller.song_id = Some("shared_song".to_string());
        smaller.representative_engagement_rate_per_1000_views = Some(85);

        let mut local = judged_sound("sound_local", 77, Some(24));
        local.song_id = Some("local_song".to_string());
        local.representative_engagement_rate_per_1000_views = Some(65);

        let mut missing_density = judged_sound("sound_missing_density", 68, Some(30));
        missing_density.song_id = Some("missing_density_song".to_string());
        missing_density.representative_engagement_rate_per_1000_views = None;

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;
        missing_song.representative_engagement_rate_per_1000_views = Some(110);

        let mut sounds = vec![biggest, smaller, local, missing_density, missing_song];
        annotate_song_id_best_representative_engagement_rates(&mut sounds);

        let best_rates = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_best_representative_engagement_rate_per_1000_views,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_rates.get("sound_biggest"), Some(&Some(235)));
        assert_eq!(best_rates.get("sound_smaller"), Some(&Some(235)));
        assert_eq!(best_rates.get("sound_local"), Some(&Some(65)));
        assert_eq!(best_rates.get("sound_missing_density"), Some(&None));
        assert_eq!(best_rates.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn annotate_song_id_best_representative_share_rates_uses_highest_spread_density_per_song() {
        let mut biggest = judged_sound("sound_biggest", 95, Some(4));
        biggest.song_id = Some("shared_song".to_string());
        biggest.representative_share_rate_per_1000_views = Some(36);

        let mut smaller = judged_sound("sound_smaller", 83, Some(12));
        smaller.song_id = Some("shared_song".to_string());
        smaller.representative_share_rate_per_1000_views = Some(12);

        let mut local = judged_sound("sound_local", 77, Some(24));
        local.song_id = Some("local_song".to_string());
        local.representative_share_rate_per_1000_views = Some(8);

        let mut missing_density = judged_sound("sound_missing_density", 68, Some(30));
        missing_density.song_id = Some("missing_density_song".to_string());
        missing_density.representative_share_rate_per_1000_views = None;

        let mut missing_song = judged_sound("sound_missing_song", 70, Some(12));
        missing_song.song_id = None;
        missing_song.representative_share_rate_per_1000_views = Some(25);

        let mut sounds = vec![biggest, smaller, local, missing_density, missing_song];
        annotate_song_id_best_representative_share_rates(&mut sounds);

        let best_rates = sounds
            .iter()
            .map(|sound| {
                (
                    sound.sound_id.as_str(),
                    sound.song_id_best_representative_share_rate_per_1000_views,
                )
            })
            .collect::<BTreeMap<_, _>>();

        assert_eq!(best_rates.get("sound_biggest"), Some(&Some(36)));
        assert_eq!(best_rates.get("sound_smaller"), Some(&Some(36)));
        assert_eq!(best_rates.get("sound_local"), Some(&Some(8)));
        assert_eq!(best_rates.get("sound_missing_density"), Some(&None));
        assert_eq!(best_rates.get("sound_missing_song"), Some(&None));
    }

    #[test]
    fn apply_song_id_country_coverage_signal_rewards_cross_country_persistence() {
        let mut global = judged_sound("sound_global", 70, Some(1));
        global.song_id_country_coverage_count = Some(3);
        global.song_id_top_25_country_count = Some(3);
        global.song_id_best_trend_rank = Some(5);

        let mut cross_market = judged_sound("sound_cross_market", 70, Some(2));
        cross_market.song_id_country_coverage_count = Some(2);
        cross_market.song_id_top_25_country_count = Some(2);
        cross_market.song_id_best_trend_rank = Some(18);

        let mut local_only = judged_sound("sound_local_only", 70, Some(3));
        local_only.song_id_country_coverage_count = Some(1);
        local_only.song_id_top_25_country_count = Some(1);
        local_only.song_id_best_trend_rank = Some(8);

        let mut missing = judged_sound("sound_missing", 70, Some(4));
        missing.song_id_country_coverage_count = None;
        missing.song_id_top_25_country_count = None;
        missing.song_id_best_trend_rank = None;

        let mut sounds = vec![global, cross_market, local_only, missing];
        apply_song_id_country_coverage_signal(&mut sounds);

        let by_id = sounds
            .into_iter()
            .map(|sound| (sound.sound_id.clone(), sound))
            .collect::<BTreeMap<_, _>>();

        let global = by_id.get("sound_global").expect("global sound");
        assert_eq!(global.score, 88);
        assert_eq!(global.reason_count, 3);
        assert_eq!(
            global.reasons,
            vec![
                "song_id persists across 3 recorded trend markets".to_string(),
                "song_id charted inside the top 25 in 3 recorded markets".to_string(),
                "song_id reached trend rank 5 in at least one recorded market".to_string(),
            ]
        );
        assert_eq!(global.recommended_action, "use_first");

        let cross_market = by_id.get("sound_cross_market").expect("cross-market sound");
        assert_eq!(cross_market.score, 79);
        assert_eq!(cross_market.reason_count, 3);
        assert_eq!(
            cross_market.reasons,
            vec![
                "song_id persists across 2 recorded trend markets".to_string(),
                "song_id charted inside the top 25 in 2 recorded markets".to_string(),
                "song_id reached trend rank 18 in at least one recorded market".to_string(),
            ]
        );
        assert_eq!(cross_market.recommended_action, "use_first");

        let local_only = by_id.get("sound_local_only").expect("local-only sound");
        assert_eq!(local_only.score, 70);
        assert!(local_only.reasons.is_empty());
        assert_eq!(local_only.reason_count, 0);
        assert_eq!(local_only.recommended_action, "shortlist");

        let missing = by_id.get("sound_missing").expect("missing sound");
        assert_eq!(missing.score, 70);
        assert!(missing.reasons.is_empty());
        assert_eq!(missing.reason_count, 0);
        assert_eq!(missing.recommended_action, "shortlist");
    }

    #[test]
    fn judging_scores_imported_tiktok_sounds_and_flags_rights() {
        let entry = ManifestEntry {
            id: "tiktok_sound_123".to_string(),
            title: "Example Sound".to_string(),
            author: "Example Creator".to_string(),
            platform: "tiktok".to_string(),
            trend_rank: Some(1),
            source_url: "https://www.tiktok.com/music/example-123".to_string(),
            source_video_url: Some("https://www.tiktok.com/@creator/video/123".to_string()),
            duration_seconds: Some(12),
            local_audio_path: "library/sounds/imported/example/audio.mp3".to_string(),
            local_metadata_path: "missing-metadata.json".to_string(),
            rights_note: "For research only. Verify rights before production use.".to_string(),
            provenance: "Imported from Apify trending sounds".to_string(),
            song_id: Some("123".to_string()),
            clip_id: Some("456".to_string()),
            country_code: Some("US".to_string()),
            local_video_path: Some("library/sounds/imported/example/video.mp4".to_string()),
            local_trend_path: None,
            local_posts_path: None,
            local_selection_path: None,
            local_download_path: None,
            local_videos_dir: Some("library/sounds/imported/example/videos".to_string()),
            local_audios_dir: Some("library/sounds/imported/example/audios".to_string()),
            downloaded_video_count: Some(3),
            extracted_audio_count: Some(2),
            representative_video_url: Some("https://www.tiktok.com/@creator/video/123".to_string()),
            representative_video_id: Some("123".to_string()),
            representative_comment_count: Some(2_500),
            representative_share_count: Some(1_500),
            representative_like_count: Some(125_000),
            representative_view_count: Some(1_500_000),
            resolver_actor_id: Some("resolver".to_string()),
            download_method: Some(DIRECT_DOWNLOAD_METHOD.to_string()),
        };

        let judged = judge_manifest_entry(Path::new("library/sounds/manifest.json"), &entry)
            .expect("judged sound");

        assert_eq!(judged.score, 100);
        assert_eq!(judged.recommended_action, "shortlist_after_rights_review");
        assert_eq!(
            judged.source_url,
            "https://www.tiktok.com/music/example-123"
        );
        assert_eq!(judged.provenance, "Imported from Apify trending sounds");
        assert_eq!(
            judged.rights_note,
            "For research only. Verify rights before production use."
        );
        assert_eq!(judged.resolver_actor_id, Some("resolver".to_string()));
        assert_eq!(
            judged.download_method,
            Some(DIRECT_DOWNLOAD_METHOD.to_string())
        );
        assert_eq!(
            judged.source_video_url,
            Some("https://www.tiktok.com/@creator/video/123".to_string())
        );
        assert_eq!(judged.song_id, Some("123".to_string()));
        assert_eq!(judged.clip_id, Some("456".to_string()));
        assert_eq!(judged.country_code, Some("US".to_string()));
        assert_eq!(judged.duration_seconds, Some(12));
        assert_eq!(judged.source_identifier_count, 6);
        assert_eq!(
            judged.source_identifier_fields,
            vec![
                "source_url".to_string(),
                "source_video_url".to_string(),
                "song_id".to_string(),
                "clip_id".to_string(),
                "country_code".to_string(),
                "duration_seconds".to_string()
            ]
        );
        assert!(judged.missing_source_identifier_fields.is_empty());
        assert_eq!(
            judged.local_audio_path,
            "library/sounds/imported/example/audio.mp3"
        );
        assert_eq!(
            judged.local_video_path,
            Some("library/sounds/imported/example/video.mp4".to_string())
        );
        assert_eq!(judged.local_metadata_path, "missing-metadata.json");
        assert_eq!(judged.local_artifact_path_count, 3);
        assert_eq!(
            judged.local_artifact_path_fields,
            vec![
                "local_audio_path".to_string(),
                "local_video_path".to_string(),
                "local_metadata_path".to_string()
            ]
        );
        assert_eq!(
            judged.missing_local_artifact_path_fields,
            vec![
                "local_trend_path".to_string(),
                "local_posts_path".to_string(),
                "local_selection_path".to_string(),
                "local_download_path".to_string()
            ]
        );
        assert_eq!(judged.candidate_post_count, None);
        assert_eq!(judged.usable_asset_pair_count, Some(2));
        assert_eq!(judged.representative_engagement_count, Some(129_000));
        assert_eq!(judged.representative_like_rate_per_1000_views, Some(83));
        assert_eq!(
            judged.representative_engagement_rate_per_1000_views,
            Some(86)
        );
        assert_eq!(judged.representative_comment_rate_per_1000_views, Some(1));
        assert_eq!(judged.representative_share_rate_per_1000_views, Some(1));
        assert_eq!(judged.representative_music_duration_seconds, None);
        assert_eq!(judged.representative_music_can_read, None);
        assert_eq!(judged.representative_music_can_reuse, None);
        assert_eq!(judged.representative_music_is_original_sound, None);
        assert_eq!(judged.representative_music_commercial_right_type, None);
        assert_eq!(judged.representative_music_is_batch_take_down_music, None);
        assert_eq!(judged.representative_music_reviewed, None);
        assert_eq!(judged.representative_music_has_strong_beat_url, None);
        assert_eq!(judged.representative_music_vid, None);
        assert_eq!(judged.representative_music_field_count, 0);
        assert!(judged.representative_music_fields.is_empty());
        assert_eq!(
            judged.missing_representative_music_fields,
            REPRESENTATIVE_MUSIC_FIELDS
                .iter()
                .map(|field| (*field).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(judged.representative_engagement_metric_count, 4);
        assert_eq!(
            judged.representative_engagement_metric_fields,
            vec![
                "representative_view_count",
                "representative_like_count",
                "representative_comment_count",
                "representative_share_count"
            ]
        );
        assert!(
            judged
                .missing_representative_engagement_metric_fields
                .is_empty()
        );
        assert_eq!(judged.reason_count, judged.reasons.len());
        assert!(
            judged.risks.contains(
                &"Rights still need manual verification before production use".to_string()
            )
        );
        assert_eq!(judged.risk_count, judged.risks.len());
    }

    #[test]
    fn judging_recovers_representative_engagement_from_posts_artifact() {
        let temp_dir = std::env::temp_dir().join(format!(
            "capcut-cli-posts-artifact-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let posts_path = temp_dir.join("posts.json");
        let selection_path = temp_dir.join("selection.json");
        std::fs::write(
            &posts_path,
            serde_json::to_vec_pretty(&json!({
                "raw_dataset": [
                    {
                        "aweme_id": "7564571947263069454",
                        "author": {
                            "unique_id": "creator"
                        },
                        "statistics": {
                            "play_count": 37_548_076,
                            "digg_count": 7_427_697,
                            "comment_count": 51_294,
                            "share_count": 1_375_712
                        },
                        "music": {
                            "is_original_sound": false,
                            "commercial_right_type": 2,
                            "strong_beat_url": {
                                "url_list": ["https://cdn.example.com/beat-track"]
                            },
                            "extra": "{\"aed_music_dur\":212.28,\"can_read\":true,\"can_reuse\":true,\"is_batch_take_down_music\":true,\"reviewed\":1,\"music_vid\":\"v10ad6g50000cds030jc77u5bevbglsg\"}"
                        }
                    }
                ]
            }))
            .expect("serialize posts"),
        )
        .expect("write posts");
        std::fs::write(
            &selection_path,
            serde_json::to_vec_pretty(&json!({
                "normalized_candidate_count": 20
            }))
            .expect("serialize selection"),
        )
        .expect("write selection");

        let entry = ManifestEntry {
            id: "tiktok_sound_123".to_string(),
            title: "Example Sound".to_string(),
            author: "Example Creator".to_string(),
            platform: "tiktok".to_string(),
            trend_rank: Some(1),
            source_url: "https://www.tiktok.com/music/example-123".to_string(),
            source_video_url: Some(
                "https://www.tiktok.com/@creator/video/7564571947263069454".to_string(),
            ),
            duration_seconds: Some(12),
            local_audio_path: "library/sounds/imported/example/audio.mp3".to_string(),
            local_metadata_path: "missing-metadata.json".to_string(),
            rights_note: "For research only. Verify rights before production use.".to_string(),
            provenance: "Imported from Apify trending sounds".to_string(),
            song_id: Some("123".to_string()),
            clip_id: Some("456".to_string()),
            country_code: Some("US".to_string()),
            local_video_path: Some("library/sounds/imported/example/video.mp4".to_string()),
            local_trend_path: None,
            local_posts_path: Some(posts_path.display().to_string()),
            local_selection_path: Some(selection_path.display().to_string()),
            local_download_path: None,
            local_videos_dir: Some("library/sounds/imported/example/videos".to_string()),
            local_audios_dir: Some("library/sounds/imported/example/audios".to_string()),
            downloaded_video_count: Some(1),
            extracted_audio_count: Some(1),
            representative_video_url: None,
            representative_video_id: Some("7564571947263069454".to_string()),
            representative_comment_count: None,
            representative_share_count: None,
            representative_like_count: None,
            representative_view_count: None,
            resolver_actor_id: Some("resolver".to_string()),
            download_method: Some(DIRECT_DOWNLOAD_METHOD.to_string()),
        };

        let judged = judge_manifest_entry(Path::new("library/sounds/manifest.json"), &entry)
            .expect("judged sound");

        assert_eq!(judged.candidate_post_count, Some(20));
        assert_eq!(judged.usable_asset_pair_count, Some(1));
        assert_eq!(judged.representative_view_count, Some(37_548_076));
        assert_eq!(judged.representative_like_count, Some(7_427_697));
        assert_eq!(judged.representative_engagement_count, Some(8_854_703));
        assert_eq!(judged.representative_like_rate_per_1000_views, Some(197));
        assert_eq!(
            judged.representative_engagement_rate_per_1000_views,
            Some(235)
        );
        assert_eq!(judged.representative_comment_count, Some(51_294));
        assert_eq!(judged.representative_comment_rate_per_1000_views, Some(1));
        assert_eq!(judged.representative_share_count, Some(1_375_712));
        assert_eq!(judged.representative_share_rate_per_1000_views, Some(36));
        assert_eq!(judged.representative_music_duration_seconds, Some(212.28));
        assert_eq!(judged.representative_music_can_read, Some(true));
        assert_eq!(judged.representative_music_can_reuse, Some(true));
        assert_eq!(judged.representative_music_is_original_sound, Some(false));
        assert_eq!(judged.representative_music_commercial_right_type, Some(2));
        assert_eq!(
            judged.representative_music_is_batch_take_down_music,
            Some(true)
        );
        assert_eq!(judged.representative_music_reviewed, Some(true));
        assert_eq!(judged.representative_music_has_strong_beat_url, Some(true));
        assert_eq!(
            judged.representative_music_vid,
            Some("v10ad6g50000cds030jc77u5bevbglsg".to_string())
        );
        assert_eq!(judged.representative_music_field_count, 9);
        assert!(
            judged
                .representative_music_fields
                .iter()
                .any(|field| field == "representative_music_reviewed")
        );
        assert!(judged.missing_representative_music_fields.is_empty());
        assert_eq!(judged.representative_engagement_metric_count, 4);
        assert!(judged.risks.contains(
            &"Representative music metadata marks the sound as batch-takedown music".to_string()
        ));
        assert_eq!(judged.score, 100);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn judging_batch_takedown_music_requires_rights_review_even_without_rights_note() {
        let temp_dir = std::env::temp_dir().join(format!(
            "capcut-cli-batch-takedown-rights-review-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).expect("create temp dir");
        let posts_path = temp_dir.join("posts.json");
        let selection_path = temp_dir.join("selection.json");
        std::fs::write(
            &posts_path,
            serde_json::to_vec_pretty(&json!({
                "raw_dataset": [
                    {
                        "aweme_id": "7564571947263069454",
                        "author": {
                            "unique_id": "creator"
                        },
                        "statistics": {
                            "play_count": 37_548_076,
                            "digg_count": 7_427_697,
                            "comment_count": 51_294,
                            "share_count": 1_375_712
                        },
                        "music": {
                            "is_original_sound": false,
                            "commercial_right_type": 2,
                            "strong_beat_url": {
                                "url_list": ["https://cdn.example.com/beat-track"]
                            },
                            "extra": "{\"aed_music_dur\":212.28,\"can_read\":true,\"can_reuse\":true,\"is_batch_take_down_music\":true,\"reviewed\":1,\"music_vid\":\"v10ad6g50000cds030jc77u5bevbglsg\"}"
                        }
                    }
                ]
            }))
            .expect("serialize posts"),
        )
        .expect("write posts");
        std::fs::write(
            &selection_path,
            serde_json::to_vec_pretty(&json!({
                "normalized_candidate_count": 20
            }))
            .expect("serialize selection"),
        )
        .expect("write selection");

        let entry = ManifestEntry {
            id: "tiktok_sound_123".to_string(),
            title: "Example Sound".to_string(),
            author: "Example Creator".to_string(),
            platform: "tiktok".to_string(),
            trend_rank: Some(1),
            source_url: "https://www.tiktok.com/music/example-123".to_string(),
            source_video_url: Some(
                "https://www.tiktok.com/@creator/video/7564571947263069454".to_string(),
            ),
            duration_seconds: Some(12),
            local_audio_path: "library/sounds/imported/example/audio.mp3".to_string(),
            local_metadata_path: "missing-metadata.json".to_string(),
            rights_note: "Catalog metadata is present.".to_string(),
            provenance: "Imported from Apify trending sounds".to_string(),
            song_id: Some("123".to_string()),
            clip_id: Some("456".to_string()),
            country_code: Some("US".to_string()),
            local_video_path: Some("library/sounds/imported/example/video.mp4".to_string()),
            local_trend_path: None,
            local_posts_path: Some(posts_path.display().to_string()),
            local_selection_path: Some(selection_path.display().to_string()),
            local_download_path: None,
            local_videos_dir: Some("library/sounds/imported/example/videos".to_string()),
            local_audios_dir: Some("library/sounds/imported/example/audios".to_string()),
            downloaded_video_count: Some(1),
            extracted_audio_count: Some(1),
            representative_video_url: None,
            representative_video_id: Some("7564571947263069454".to_string()),
            representative_comment_count: None,
            representative_share_count: None,
            representative_like_count: None,
            representative_view_count: None,
            resolver_actor_id: Some("resolver".to_string()),
            download_method: Some(DIRECT_DOWNLOAD_METHOD.to_string()),
        };

        let judged = judge_manifest_entry(Path::new("library/sounds/manifest.json"), &entry)
            .expect("judged sound");

        assert_eq!(judged.score, 100);
        assert_eq!(judged.recommended_action, "shortlist_after_rights_review");
        assert!(judged.risks.contains(
            &"Representative music metadata marks the sound as batch-takedown music".to_string()
        ));
        assert!(
            judged
                .risks
                .iter()
                .all(|risk| !risk.contains("Rights still need manual verification"))
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn normalize_resolver_post_builds_canonical_video_url_and_media_urls() {
        let item = json!({
            "aweme_id": "7502551047378832671",
            "diggCount": 7611,
            "commentCount": 5358,
            "author": {
                "uniqueId": "tiktok",
                "nickname": "TikTok"
            },
            "video": {
                "downloadAddr": {
                    "urlList": ["https://cdn.example.com/video-download.mp4"]
                },
                "playAddr": {
                    "urlList": ["https://cdn.example.com/video-play.mp4"]
                },
                "duration": 15
            },
            "music": {
                "playUrl": {
                    "urlList": ["https://cdn.example.com/audio.mp3"]
                }
            },
            "title": "Example"
        });

        let candidate = normalize_resolver_post_item(&item, 0).expect("candidate");

        assert_eq!(candidate.video_id, "7502551047378832671");
        assert_eq!(
            candidate.video_url,
            "https://www.tiktok.com/@tiktok/video/7502551047378832671"
        );
        assert_eq!(
            candidate.download_url.as_deref(),
            Some("https://cdn.example.com/video-download.mp4")
        );
        assert_eq!(
            candidate.public_media_url.as_deref(),
            Some("https://cdn.example.com/video-play.mp4")
        );
        assert_eq!(
            candidate.audio_url.as_deref(),
            Some("https://cdn.example.com/audio.mp3")
        );
        assert_eq!(candidate.digg_count, Some(7611));
        assert_eq!(candidate.comment_count, Some(5358));
    }

    #[test]
    fn normalize_resolver_post_reads_statistics_engagement_metrics() {
        let item = json!({
            "aweme_id": "7564571947263069454",
            "author": {
                "unique_id": "creator"
            },
            "statistics": {
                "play_count": 37_548_076,
                "digg_count": 7_427_697,
                "comment_count": 51_294,
                "share_count": 1_375_712
            }
        });

        let candidate = normalize_resolver_post_item(&item, 0).expect("candidate");

        assert_eq!(candidate.play_count, Some(37_548_076));
        assert_eq!(candidate.digg_count, Some(7_427_697));
        assert_eq!(candidate.comment_count, Some(51_294));
        assert_eq!(candidate.share_count, Some(1_375_712));
    }

    #[test]
    fn ranking_prefers_like_count_before_resolver_order() {
        let mut candidates = vec![
            CandidatePost {
                selection_rank: 0,
                resolver_index: 1,
                source: CandidatePostSource::SoundResolverActor,
                video_id: "1".to_string(),
                aweme_id: None,
                video_url: "https://www.tiktok.com/@a/video/1".to_string(),
                author_unique_id: Some("a".to_string()),
                author_nickname: None,
                title: None,
                region: None,
                duration_seconds: None,
                play_count: Some(1_000_000),
                digg_count: Some(100),
                comment_count: Some(1_000),
                share_count: Some(50),
                download_url: Some("https://cdn.example.com/1.mp4".to_string()),
                public_media_url: None,
                audio_url: None,
                cover_url: None,
            },
            CandidatePost {
                selection_rank: 0,
                resolver_index: 0,
                source: CandidatePostSource::SoundResolverActor,
                video_id: "2".to_string(),
                aweme_id: None,
                video_url: "https://www.tiktok.com/@b/video/2".to_string(),
                author_unique_id: Some("b".to_string()),
                author_nickname: None,
                title: None,
                region: None,
                duration_seconds: None,
                play_count: Some(100),
                digg_count: Some(101),
                comment_count: Some(0),
                share_count: Some(0),
                download_url: Some("https://cdn.example.com/2.mp4".to_string()),
                public_media_url: None,
                audio_url: None,
                cover_url: None,
            },
        ];

        rank_candidate_posts(&mut candidates);

        assert_eq!(candidates[0].video_id, "2");
        assert_eq!(candidates[0].selection_rank, 1);
        assert_eq!(candidates[1].selection_rank, 2);
    }

    #[test]
    fn normalize_resolver_post_supports_wildcard_url_lists() {
        let item = json!({
            "id": "7234071025832989994",
            "shareUrl": "https://www.tiktok.com/@username/video/7234071025832989994",
            "stats": {
                "diggCount": "89000"
            },
            "video": {
                "bitrateInfo": [
                    {
                        "playAddr": {
                            "urlList": ["https://cdn.example.com/bitrate.mp4"]
                        }
                    }
                ]
            }
        });

        let candidate = normalize_resolver_post_item(&item, 0).expect("normalized");

        assert_eq!(candidate.video_id, "7234071025832989994");
        assert_eq!(candidate.digg_count, Some(89_000));
        assert_eq!(
            candidate.public_media_url.as_deref(),
            Some("https://cdn.example.com/bitrate.mp4")
        );
    }

    #[test]
    fn bool_at_path_accepts_numeric_and_string_boolean_forms() {
        let value = json!({
            "music": {
                "extra": {
                    "reviewed_numeric_true": 1,
                    "reviewed_numeric_false": 0,
                    "reviewed_string_true": "1",
                    "reviewed_string_false": "0",
                    "reviewed_bool_true": true,
                    "reviewed_bool_false": false
                }
            }
        });

        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_numeric_true"]),
            Some(true)
        );
        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_numeric_false"]),
            Some(false)
        );
        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_string_true"]),
            Some(true)
        );
        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_string_false"]),
            Some(false)
        );
        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_bool_true"]),
            Some(true)
        );
        assert_eq!(
            bool_at_path(&value, &["music", "extra", "reviewed_bool_false"]),
            Some(false)
        );
    }
}
