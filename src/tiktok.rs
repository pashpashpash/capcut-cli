use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    apify::{self, ActorRun},
    models::{DiscoveredSound, FailedSoundImport, ImportedSound},
};

pub const DEFAULT_IMPORT_OUTPUT_DIR: &str = "library/sounds/imported";
pub const LIBRARY_MANIFEST_PATH: &str = "library/sounds/manifest.json";
pub const TRENDS_ACTOR_ID: &str = "alien_force~tiktok-trending-sounds-tracker";
pub const MUSIC_POSTS_ACTOR_ID: &str = "powerai~tiktok-music-posts-video-scraper";
pub const VIDEO_DOWNLOADER_ACTOR_ID: &str = "dltik~tiktok-video-downloader";

const DOWNLOAD_OUTPUT_KEY: &str = "OUTPUT";

#[derive(Debug, Clone)]
pub struct ImportTrendingSoundsOptions {
    pub country: String,
    pub limit: usize,
    pub period: String,
    pub max_posts: usize,
    pub download_attempts: usize,
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
    MusicPostsActor,
    TrendRelatedItem,
}

#[derive(Debug, Clone, Serialize)]
struct CandidatePost {
    selection_rank: usize,
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
}

#[derive(Debug, Serialize)]
struct TrendArtifact {
    actor_id: String,
    actor_run: ActorRun,
    item: TrendingSoundItem,
}

#[derive(Debug, Serialize)]
struct MusicPostsArtifact {
    actor_id: String,
    actor_run: Option<ActorRun>,
    actor_error: Option<String>,
    requested_music_id: String,
    requested_max_results: usize,
    raw_dataset: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct CandidateSelectionArtifact {
    actor_id: String,
    actor_run: Option<ActorRun>,
    actor_error: Option<String>,
    requested_music_id: String,
    requested_max_results: usize,
    raw_dataset_count: usize,
    normalized_candidate_count: usize,
    fallback_related_item_count: usize,
    ranking_strategy: String,
    candidates: Vec<CandidatePost>,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadedVideoMetadata {
    normalized_url: Option<String>,
    title: Option<String>,
    author: Option<String>,
    duration_seconds: Option<u32>,
    thumbnail: Option<String>,
    view_count: Option<u64>,
    like_count: Option<u64>,
    without_watermark_available: Option<bool>,
    available_outputs: Vec<String>,
    max_video_height: Option<u32>,
    output_format: Option<String>,
    file_name: Option<String>,
    file_size_bytes: Option<u64>,
    file_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct DownloadAttemptArtifact {
    attempt_number: usize,
    candidate_rank: usize,
    candidate_source: CandidatePostSource,
    candidate_video_id: String,
    candidate_video_url: String,
    actor_id: String,
    actor_run: Option<ActorRun>,
    raw_dataset_count: usize,
    selected_dataset_item: Option<DownloadedVideoMetadata>,
    resolved_file_url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DownloadArtifact {
    actor_id: String,
    request_defaults: Value,
    attempts: Vec<DownloadAttemptArtifact>,
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
    actors: ActorChainMetadata,
    selection: SelectionSummary,
    files: LocalArtifacts,
    provenance: String,
    rights_note: String,
}

#[derive(Debug, Serialize)]
struct ActorChainMetadata {
    trends_actor: String,
    music_posts_actor: String,
    downloader_actor: String,
}

#[derive(Debug, Serialize)]
struct SelectionSummary {
    ranking_strategy: String,
    candidate_count: usize,
    selected_video_id: String,
    selected_video_url: String,
    selected_comment_count: Option<u64>,
    selected_share_count: Option<u64>,
    selected_like_count: Option<u64>,
    selected_view_count: Option<u64>,
}

#[derive(Debug, Serialize)]
struct LocalArtifacts {
    trend_path: String,
    posts_path: String,
    selection_path: String,
    download_path: String,
    metadata_path: String,
    local_video_path: String,
    local_audio_path: String,
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
}

struct CandidateSelectionResult {
    music_posts_artifact: MusicPostsArtifact,
    selection_artifact: CandidateSelectionArtifact,
}

struct DownloadResolution {
    selected_candidate: CandidatePost,
    selected_metadata: DownloadedVideoMetadata,
    attempts: Vec<DownloadAttemptArtifact>,
    video_path: PathBuf,
    audio_path: PathBuf,
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
        .into_iter()
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
    fs::create_dir_all(&sound_dir)
        .with_context(|| format!("failed to create {}", sound_dir.display()))?;

    let trend_path = sound_dir.join("trend.json");
    let posts_path = sound_dir.join("posts.json");
    let selection_path = sound_dir.join("selection.json");
    let download_path = sound_dir.join("download.json");
    let metadata_path = sound_dir.join("metadata.json");

    write_json(
        &trend_path,
        &TrendArtifact {
            actor_id: trend_actor_id.to_string(),
            actor_run: trend_actor_run.clone(),
            item: item.clone(),
        },
    )?;

    let candidates = collect_candidate_posts(client, token, &item, options.max_posts)?;
    write_json(&posts_path, &candidates.music_posts_artifact)?;
    write_json(&selection_path, &candidates.selection_artifact)?;

    let download = download_best_candidate_video(
        client,
        token,
        &sound_dir,
        &candidates.selection_artifact.candidates,
        options.download_attempts,
    )?;
    write_json(
        &download_path,
        &DownloadArtifact {
            actor_id: VIDEO_DOWNLOADER_ACTOR_ID.to_string(),
            request_defaults: json!({
                "output": "mp4",
                "watermarkPolicy": "standard",
                "metadataOnly": false,
            }),
            attempts: download.attempts,
        },
    )?;

    let rights_note =
        "For research and internal prototyping only. Verify rights before redistribution or production use."
            .to_string();
    let provenance =
        "Imported from Apify trending sounds, resolved to music posts, ranked by comments, downloaded with dltik, and audio extracted locally with ffmpeg."
            .to_string();

    let metadata = ImportedSoundMetadata {
        id: sound_id.clone(),
        rank: item.rank,
        title: item.title.clone(),
        author: item.author.clone(),
        trend_link: item.link.clone(),
        clip_id: item.clip_id.clone(),
        song_id: item.song_id.clone(),
        country_code: item.country_code.clone(),
        duration_seconds: item.duration,
        actors: ActorChainMetadata {
            trends_actor: TRENDS_ACTOR_ID.to_string(),
            music_posts_actor: MUSIC_POSTS_ACTOR_ID.to_string(),
            downloader_actor: VIDEO_DOWNLOADER_ACTOR_ID.to_string(),
        },
        selection: SelectionSummary {
            ranking_strategy:
                "comment_count desc, share_count desc, digg_count desc, play_count desc".to_string(),
            candidate_count: candidates.selection_artifact.candidates.len(),
            selected_video_id: download.selected_candidate.video_id.clone(),
            selected_video_url: download.selected_candidate.video_url.clone(),
            selected_comment_count: download.selected_candidate.comment_count,
            selected_share_count: download.selected_candidate.share_count,
            selected_like_count: download.selected_candidate.digg_count,
            selected_view_count: download.selected_candidate.play_count,
        },
        files: LocalArtifacts {
            trend_path: trend_path.display().to_string(),
            posts_path: posts_path.display().to_string(),
            selection_path: selection_path.display().to_string(),
            download_path: download_path.display().to_string(),
            metadata_path: metadata_path.display().to_string(),
            local_video_path: download.video_path.display().to_string(),
            local_audio_path: download.audio_path.display().to_string(),
        },
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
            source_video_url: Some(download.selected_candidate.video_url.clone()),
            duration_seconds: download
                .selected_metadata
                .duration_seconds
                .or(Some(item.duration)),
            local_audio_path: download.audio_path.display().to_string(),
            local_metadata_path: metadata_path.display().to_string(),
            rights_note: rights_note.clone(),
            provenance: provenance.clone(),
            song_id: Some(item.song_id.clone()),
            clip_id: Some(item.clip_id.clone()),
            country_code: Some(item.country_code.clone()),
            local_video_path: Some(download.video_path.display().to_string()),
            local_trend_path: Some(trend_path.display().to_string()),
            local_posts_path: Some(posts_path.display().to_string()),
            local_selection_path: Some(selection_path.display().to_string()),
            representative_video_url: Some(download.selected_candidate.video_url.clone()),
            representative_video_id: Some(download.selected_candidate.video_id.clone()),
            representative_comment_count: download.selected_candidate.comment_count,
            representative_share_count: download.selected_candidate.share_count,
            representative_like_count: download.selected_candidate.digg_count,
            representative_view_count: download.selected_candidate.play_count,
        },
        report: ImportedSound {
            id: sound_id,
            rank: item.rank,
            title: item.title,
            author: item.author,
            song_id: item.song_id,
            clip_id: item.clip_id,
            trend_link: item.link,
            selected_video_url: download.selected_candidate.video_url,
            selected_video_id: Some(download.selected_candidate.video_id),
            selected_comment_count: download.selected_candidate.comment_count,
            candidate_posts_considered: candidates.selection_artifact.candidates.len(),
            downloader_actor: VIDEO_DOWNLOADER_ACTOR_ID.to_string(),
            local_video_path: download.video_path.display().to_string(),
            local_audio_path: download.audio_path.display().to_string(),
            local_metadata_path: metadata_path.display().to_string(),
        },
    })
}

fn collect_candidate_posts(
    client: &Client,
    token: &str,
    item: &TrendingSoundItem,
    max_posts: usize,
) -> Result<CandidateSelectionResult> {
    let mut actor_run = None;
    let mut actor_error = None;
    let mut raw_dataset = Vec::new();

    match apify::run_actor(
        client,
        token,
        MUSIC_POSTS_ACTOR_ID,
        &json!({
            "music_id": item.song_id,
            "maxResults": max_posts,
        }),
    ) {
        Ok(run) => {
            raw_dataset = apify::fetch_dataset_values(client, token, &run.default_dataset_id)
                .with_context(|| {
                    format!(
                        "failed to fetch music posts dataset for sound {}",
                        item.song_id
                    )
                })?;
            actor_run = Some(run);
        }
        Err(error) => {
            actor_error = Some(format!("{error:#}"));
        }
    }

    let mut seen = BTreeSet::new();
    let mut candidates = raw_dataset
        .iter()
        .filter_map(normalize_music_post_item)
        .filter(|candidate| seen.insert(candidate_key(candidate)))
        .collect::<Vec<_>>();
    let normalized_candidate_count = candidates.len();

    let fallback_candidates = fallback_related_item_candidates(item)
        .into_iter()
        .filter(|candidate| seen.insert(candidate_key(candidate)))
        .collect::<Vec<_>>();
    let fallback_related_item_count = fallback_candidates.len();
    candidates.extend(fallback_candidates);

    if candidates.is_empty() {
        bail!(
            "music posts actor returned no usable candidates and trending sound {} had no related_items fallback",
            item.song_id
        )
    }

    rank_candidate_posts(&mut candidates);
    let raw_dataset_count = raw_dataset.len();

    Ok(CandidateSelectionResult {
        music_posts_artifact: MusicPostsArtifact {
            actor_id: MUSIC_POSTS_ACTOR_ID.to_string(),
            actor_run: actor_run.clone(),
            actor_error: actor_error.clone(),
            requested_music_id: item.song_id.clone(),
            requested_max_results: max_posts,
            raw_dataset,
        },
        selection_artifact: CandidateSelectionArtifact {
            actor_id: MUSIC_POSTS_ACTOR_ID.to_string(),
            actor_run,
            actor_error,
            requested_music_id: item.song_id.clone(),
            requested_max_results: max_posts,
            raw_dataset_count,
            normalized_candidate_count,
            fallback_related_item_count,
            ranking_strategy:
                "comment_count desc, share_count desc, digg_count desc, play_count desc".to_string(),
            candidates,
        },
    })
}

fn download_best_candidate_video(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    candidates: &[CandidatePost],
    download_attempts: usize,
) -> Result<DownloadResolution> {
    let max_attempts = download_attempts.max(1).min(candidates.len());
    let mut attempts = Vec::new();

    for (index, candidate) in candidates.iter().take(max_attempts).enumerate() {
        let attempt_number = index + 1;
        match download_candidate_video(client, token, sound_dir, candidate, attempt_number) {
            Ok((selected_metadata, video_path, audio_path, mut artifact)) => {
                artifact.error = None;
                attempts.push(artifact);
                return Ok(DownloadResolution {
                    selected_candidate: candidate.clone(),
                    selected_metadata,
                    attempts,
                    video_path,
                    audio_path,
                });
            }
            Err((artifact, error)) => {
                attempts.push(artifact);
                if attempt_number == max_attempts {
                    return Err(error);
                }
            }
        }
    }

    bail!("no download attempts were made")
}

fn download_candidate_video(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    candidate: &CandidatePost,
    attempt_number: usize,
) -> std::result::Result<
    (
        DownloadedVideoMetadata,
        PathBuf,
        PathBuf,
        DownloadAttemptArtifact,
    ),
    (DownloadAttemptArtifact, anyhow::Error),
> {
    let mut artifact = DownloadAttemptArtifact {
        attempt_number,
        candidate_rank: candidate.selection_rank,
        candidate_source: candidate.source.clone(),
        candidate_video_id: candidate.video_id.clone(),
        candidate_video_url: candidate.video_url.clone(),
        actor_id: VIDEO_DOWNLOADER_ACTOR_ID.to_string(),
        actor_run: None,
        raw_dataset_count: 0,
        selected_dataset_item: None,
        resolved_file_url: None,
        error: None,
    };

    let run = match apify::run_actor(
        client,
        token,
        VIDEO_DOWNLOADER_ACTOR_ID,
        &json!({
            "url": candidate.video_url,
            "output": "mp4",
            "watermarkPolicy": "standard",
            "metadataOnly": false,
        }),
    ) {
        Ok(run) => run,
        Err(error) => {
            artifact.error = Some(format!("{error:#}"));
            return Err((artifact, error));
        }
    };
    artifact.actor_run = Some(run.clone());

    let raw_dataset = match apify::fetch_dataset_values(client, token, &run.default_dataset_id) {
        Ok(items) => items,
        Err(error) => {
            artifact.error = Some(format!("{error:#}"));
            return Err((artifact, error));
        }
    };
    artifact.raw_dataset_count = raw_dataset.len();

    let metadata = raw_dataset
        .iter()
        .filter_map(normalize_downloaded_video_item)
        .next()
        .unwrap_or_else(|| DownloadedVideoMetadata {
            normalized_url: Some(candidate.video_url.clone()),
            title: candidate.title.clone(),
            author: candidate.author_unique_id.clone(),
            duration_seconds: candidate.duration_seconds,
            thumbnail: None,
            view_count: candidate.play_count,
            like_count: candidate.digg_count,
            without_watermark_available: None,
            available_outputs: vec!["mp4".to_string()],
            max_video_height: None,
            output_format: Some("mp4".to_string()),
            file_name: None,
            file_size_bytes: None,
            file_url: None,
        });
    artifact.selected_dataset_item = Some(metadata.clone());

    let file_url = metadata.file_url.clone().or_else(|| {
        run.default_key_value_store_id
            .as_deref()
            .map(|store_id| apify::key_value_store_record_url(store_id, DOWNLOAD_OUTPUT_KEY))
    });

    let Some(file_url) = file_url else {
        let error = anyhow::anyhow!(
            "downloader actor returned no fileUrl and no default key-value store id for {}",
            candidate.video_url
        );
        artifact.error = Some(format!("{error:#}"));
        return Err((artifact, error));
    };
    artifact.resolved_file_url = Some(file_url.clone());

    let temp_video = sound_dir.join(format!("video-attempt-{attempt_number}.mp4"));
    let temp_audio = sound_dir.join(format!("audio-attempt-{attempt_number}.mp3"));
    let final_video = sound_dir.join("video.mp4");
    let final_audio = sound_dir.join("audio.mp3");

    if let Err(error) = apify::download_to_path(client, token, &file_url, &temp_video) {
        let _ = fs::remove_file(&temp_video);
        artifact.error = Some(format!("{error:#}"));
        return Err((artifact, error));
    }

    if let Err(error) = extract_audio_from_video(&temp_video, &temp_audio) {
        let _ = fs::remove_file(&temp_video);
        let _ = fs::remove_file(&temp_audio);
        artifact.error = Some(format!("{error:#}"));
        return Err((artifact, error));
    }

    if let Err(error) = promote_temp_file(&temp_video, &final_video) {
        let _ = fs::remove_file(&temp_video);
        let _ = fs::remove_file(&temp_audio);
        artifact.error = Some(format!("{error:#}"));
        return Err((artifact, error));
    }

    if let Err(error) = promote_temp_file(&temp_audio, &final_audio) {
        let _ = fs::remove_file(&temp_audio);
        artifact.error = Some(format!("{error:#}"));
        return Err((artifact, error));
    }

    Ok((metadata, final_video, final_audio, artifact))
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

fn normalize_music_post_item(item: &Value) -> Option<CandidatePost> {
    let aweme_id = first_non_empty_string(item, &[&["aweme_id"], &["awemeId"]]);
    let author_unique_id = first_non_empty_string(
        item,
        &[
            &["author", "unique_id"],
            &["author", "uniqueId"],
            &["authorUniqueId"],
        ],
    )
    .map(normalize_author_unique_id);
    let video_id = first_non_empty_string(
        item,
        &[&["video_id"], &["videoId"], &["video", "id"], &["id"]],
    )
    .or_else(|| {
        first_non_empty_string(
            item,
            &[
                &["share_url"],
                &["shareUrl"],
                &["url"],
                &["video_url"],
                &["videoUrl"],
            ],
        )
        .and_then(|url| tiktok_video_id(&url).map(ToString::to_string))
    })?;

    let canonical_url = canonical_video_url(author_unique_id.as_deref(), &video_id);
    let video_url = first_non_empty_string(
        item,
        &[
            &["share_url"],
            &["shareUrl"],
            &["share_info", "share_url"],
            &["shareInfo", "shareUrl"],
            &["url"],
        ],
    )
    .filter(|url| is_tiktok_url(url))
    .unwrap_or(canonical_url);

    Some(CandidatePost {
        selection_rank: 0,
        source: CandidatePostSource::MusicPostsActor,
        video_id,
        aweme_id,
        video_url,
        author_unique_id: author_unique_id.clone(),
        author_nickname: first_non_empty_string(
            item,
            &[&["author", "nickname"], &["authorNickname"], &["nickname"]],
        ),
        title: first_non_empty_string(item, &[&["title"], &["desc"]]),
        region: first_non_empty_string(item, &[&["region"]]),
        duration_seconds: first_u32(item, &[&["duration"], &["video", "duration"]]),
        play_count: first_u64(
            item,
            &[&["play_count"], &["playCount"], &["stats", "play_count"]],
        ),
        digg_count: first_u64(
            item,
            &[&["digg_count"], &["diggCount"], &["stats", "digg_count"]],
        ),
        comment_count: first_u64(
            item,
            &[
                &["comment_count"],
                &["commentCount"],
                &["stats", "comment_count"],
            ],
        ),
        share_count: first_u64(
            item,
            &[&["share_count"], &["shareCount"], &["stats", "share_count"]],
        ),
    })
}

fn fallback_related_item_candidates(item: &TrendingSoundItem) -> Vec<CandidatePost> {
    item.related_items
        .iter()
        .map(|related| {
            let video_id = related.item_id.to_string();
            CandidatePost {
                selection_rank: 0,
                source: CandidatePostSource::TrendRelatedItem,
                video_id: video_id.clone(),
                aweme_id: None,
                video_url: canonical_video_url(None, &video_id),
                author_unique_id: None,
                author_nickname: None,
                title: Some(item.title.clone()),
                region: Some(item.country_code.clone()),
                duration_seconds: Some(item.duration),
                play_count: None,
                digg_count: None,
                comment_count: None,
                share_count: None,
            }
        })
        .collect()
}

fn rank_candidate_posts(candidates: &mut [CandidatePost]) {
    candidates.sort_by(|left, right| {
        sort_metric(right.comment_count)
            .cmp(&sort_metric(left.comment_count))
            .then_with(|| {
                sort_metric(right.share_count)
                    .cmp(&sort_metric(left.share_count))
                    .then_with(|| {
                        sort_metric(right.digg_count)
                            .cmp(&sort_metric(left.digg_count))
                            .then_with(|| {
                                sort_metric(right.play_count).cmp(&sort_metric(left.play_count))
                            })
                    })
            })
    });

    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.selection_rank = index + 1;
    }
}

fn normalize_downloaded_video_item(item: &Value) -> Option<DownloadedVideoMetadata> {
    let file_url = first_non_empty_string(
        item,
        &[
            &["fileUrl"],
            &["file_url"],
            &["downloadUrl"],
            &["download_url"],
        ],
    );

    Some(DownloadedVideoMetadata {
        normalized_url: first_non_empty_string(
            item,
            &[&["normalizedUrl"], &["normalized_url"], &["url"]],
        ),
        title: first_non_empty_string(item, &[&["title"]]),
        author: first_non_empty_string(item, &[&["author"]]),
        duration_seconds: first_u32(item, &[&["duration"]]),
        thumbnail: first_non_empty_string(item, &[&["thumbnail"]]),
        view_count: first_u64(item, &[&["viewCount"], &["view_count"]]),
        like_count: first_u64(item, &[&["likeCount"], &["like_count"]]),
        without_watermark_available: first_bool(
            item,
            &[
                &["withoutWatermarkAvailable"],
                &["without_watermark_available"],
            ],
        ),
        available_outputs: first_string_vec(item, &[&["availableOutputs"], &["available_outputs"]])
            .unwrap_or_default(),
        max_video_height: first_u32(item, &[&["maxVideoHeight"], &["max_video_height"]]),
        output_format: first_non_empty_string(item, &[&["outputFormat"], &["output_format"]]),
        file_name: first_non_empty_string(item, &[&["fileName"], &["file_name"]]),
        file_size_bytes: first_u64(item, &[&["fileSizeBytes"], &["file_size_bytes"]]),
        file_url,
    })
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

fn first_u32(value: &Value, paths: &[&[&str]]) -> Option<u32> {
    first_u64(value, paths).and_then(|value| u32::try_from(value).ok())
}

fn first_bool(value: &Value, paths: &[&[&str]]) -> Option<bool> {
    paths.iter().find_map(|path| bool_at_path(value, path))
}

fn first_string_vec(value: &Value, paths: &[&[&str]]) -> Option<Vec<String>> {
    paths
        .iter()
        .find_map(|path| string_vec_at_path(value, path))
}

fn string_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    current
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn unsigned_at_path(value: &Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn bool_at_path(value: &Value, path: &[&str]) -> Option<bool> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    match current {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => match text.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn string_vec_at_path(value: &Value, path: &[&str]) -> Option<Vec<String>> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }

    current.as_array().map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_music_post_builds_canonical_video_url() {
        let item = json!({
            "video_id": "7502551047378832671",
            "comment_count": 5358,
            "digg_count": 7611,
            "share_count": 754,
            "play_count": 1287045,
            "author": {
                "unique_id": "tiktok",
                "nickname": "TikTok"
            },
            "title": "Example"
        });

        let candidate = normalize_music_post_item(&item).expect("candidate");

        assert_eq!(candidate.video_id, "7502551047378832671");
        assert_eq!(
            candidate.video_url,
            "https://www.tiktok.com/@tiktok/video/7502551047378832671"
        );
        assert_eq!(candidate.comment_count, Some(5358));
        assert_eq!(candidate.digg_count, Some(7611));
    }

    #[test]
    fn ranking_prefers_comments_before_other_metrics() {
        let mut candidates = vec![
            CandidatePost {
                selection_rank: 0,
                source: CandidatePostSource::MusicPostsActor,
                video_id: "1".to_string(),
                aweme_id: None,
                video_url: "https://www.tiktok.com/@a/video/1".to_string(),
                author_unique_id: Some("a".to_string()),
                author_nickname: None,
                title: None,
                region: None,
                duration_seconds: None,
                play_count: Some(1_000_000),
                digg_count: Some(100_000),
                comment_count: Some(10),
                share_count: Some(1),
            },
            CandidatePost {
                selection_rank: 0,
                source: CandidatePostSource::MusicPostsActor,
                video_id: "2".to_string(),
                aweme_id: None,
                video_url: "https://www.tiktok.com/@b/video/2".to_string(),
                author_unique_id: Some("b".to_string()),
                author_nickname: None,
                title: None,
                region: None,
                duration_seconds: None,
                play_count: Some(100),
                digg_count: Some(100),
                comment_count: Some(11),
                share_count: Some(0),
            },
        ];

        rank_candidate_posts(&mut candidates);

        assert_eq!(candidates[0].video_id, "2");
        assert_eq!(candidates[0].selection_rank, 1);
        assert_eq!(candidates[1].selection_rank, 2);
    }

    #[test]
    fn normalize_downloaded_video_supports_dltik_shape() {
        let item = json!({
            "normalizedUrl": "https://www.tiktok.com/@username/video/7234071025832989994",
            "title": "Best cooking hack ever",
            "author": "username",
            "duration": 15,
            "viewCount": 1250000,
            "likeCount": 89000,
            "withoutWatermarkAvailable": true,
            "availableOutputs": ["mp4", "mp3"],
            "outputFormat": "mp4",
            "fileName": "best-cooking-hack-ever.mp4",
            "fileSizeBytes": 2456789,
            "fileUrl": "https://api.apify.com/v2/key-value-stores/store/records/OUTPUT"
        });

        let normalized = normalize_downloaded_video_item(&item).expect("normalized");

        assert_eq!(
            normalized.normalized_url.as_deref(),
            Some("https://www.tiktok.com/@username/video/7234071025832989994")
        );
        assert_eq!(normalized.available_outputs, vec!["mp4", "mp3"]);
        assert_eq!(normalized.file_size_bytes, Some(2_456_789));
    }

    #[test]
    fn fallback_related_items_construct_candidate_urls() {
        let item = TrendingSoundItem {
            rank: 1,
            title: "Example".to_string(),
            author: "creator".to_string(),
            link: "https://www.tiktok.com/music/example-sound-123".to_string(),
            clip_id: "123".to_string(),
            song_id: "456".to_string(),
            duration: 12,
            country_code: "US".to_string(),
            related_items: vec![RelatedItem {
                item_id: 42,
                cover_url: None,
            }],
        };

        let candidates = fallback_related_item_candidates(&item);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].video_id, "42");
        assert_eq!(
            candidates[0].video_url,
            "https://www.tiktok.com/@i/video/42"
        );
    }
}
