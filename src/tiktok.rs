use std::{
    collections::BTreeSet,
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

    sort_and_rank_judged_sounds(&mut sounds);

    Ok(sounds)
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
        source_url: entry.source_url.clone(),
        source_video_url: entry.source_video_url.clone(),
        song_id: entry.song_id.clone(),
        clip_id: entry.clip_id.clone(),
        country_code: entry.country_code.clone(),
        duration_seconds: entry.duration_seconds,
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

fn metadata_u64(metadata: &Option<Value>, paths: &[&[&str]]) -> Option<u64> {
    metadata.as_ref().and_then(|value| first_u64(value, paths))
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

fn recommended_action(score: u32, risks: &[String]) -> &'static str {
    let rights_review_needed = risks
        .iter()
        .any(|risk| risk.contains("Rights still need manual verification"));

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
            source_url: format!("https://www.tiktok.com/music/{id}"),
            source_video_url: Some(format!("https://www.tiktok.com/@creator/video/{id}")),
            song_id: Some(id.to_string()),
            clip_id: Some(format!("{id}_clip")),
            country_code: Some("US".to_string()),
            duration_seconds: Some(12),
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
        assert_eq!(
            judged.source_video_url,
            Some("https://www.tiktok.com/@creator/video/123".to_string())
        );
        assert_eq!(judged.song_id, Some("123".to_string()));
        assert_eq!(judged.clip_id, Some("456".to_string()));
        assert_eq!(judged.country_code, Some("US".to_string()));
        assert_eq!(judged.duration_seconds, Some(12));
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
        assert_eq!(judged.representative_engagement_metric_count, 4);
        assert_eq!(judged.score, 100);

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
}
