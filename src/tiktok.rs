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
    models::{DiscoveredSound, FailedSoundImport, ImportedSound},
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

#[derive(Debug, Serialize)]
struct DownloadAttemptArtifact {
    attempt_number: usize,
    candidate_rank: usize,
    candidate_source: CandidatePostSource,
    candidate_video_id: String,
    candidate_video_url: String,
    resolved_direct_video_url: Option<String>,
    resolved_audio_url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DownloadArtifact {
    method: String,
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
    sound_resolver_actor: String,
    download_method: String,
}

#[derive(Debug, Serialize)]
struct SelectionSummary {
    ranking_strategy: String,
    candidate_count: usize,
    selected_video_id: String,
    selected_video_url: String,
    selected_direct_video_url: String,
    selected_audio_url: Option<String>,
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
    selected_candidate: CandidatePost,
    selected_media_url: String,
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

    let candidates = collect_candidate_posts(
        client,
        token,
        &options.resolver_actor_id,
        &item,
        options.max_posts,
    )?;
    write_json(&posts_path, &candidates.resolver_posts_artifact)?;
    write_json(&selection_path, &candidates.selection_artifact)?;

    let download = download_best_candidate_media(
        client,
        token,
        &sound_dir,
        &candidates.selection_artifact.candidates,
        options.download_attempts,
    )?;
    write_json(
        &download_path,
        &DownloadArtifact {
            method: DIRECT_DOWNLOAD_METHOD.to_string(),
            attempts: download.attempts,
        },
    )?;

    let rights_note =
        "For research and internal prototyping only. Verify rights before redistribution or production use."
            .to_string();
    let provenance =
        "Imported from Apify trending sounds, resolved from the sound URL with a Novi actor, selected by top like count, downloaded directly from resolver media output, and audio extracted locally with ffmpeg."
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
        duration_seconds: download
            .selected_candidate
            .duration_seconds
            .unwrap_or(item.duration),
        actors: ActorChainMetadata {
            trends_actor: TRENDS_ACTOR_ID.to_string(),
            sound_resolver_actor: options.resolver_actor_id.clone(),
            download_method: DIRECT_DOWNLOAD_METHOD.to_string(),
        },
        selection: SelectionSummary {
            ranking_strategy: candidates.selection_artifact.ranking_strategy.clone(),
            candidate_count: candidates.selection_artifact.candidates.len(),
            selected_video_id: download.selected_candidate.video_id.clone(),
            selected_video_url: download.selected_candidate.video_url.clone(),
            selected_direct_video_url: download.selected_media_url.clone(),
            selected_audio_url: download.selected_candidate.audio_url.clone(),
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
                .selected_candidate
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
            selected_video_url: download.selected_candidate.video_url,
            selected_video_id: Some(download.selected_candidate.video_id),
            selected_like_count: download.selected_candidate.digg_count,
            selected_comment_count: download.selected_candidate.comment_count,
            candidate_posts_considered: candidates.selection_artifact.candidates.len(),
            resolver_actor: options.resolver_actor_id.clone(),
            download_method: DIRECT_DOWNLOAD_METHOD.to_string(),
            local_video_path: download.video_path.display().to_string(),
            local_audio_path: download.audio_path.display().to_string(),
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

fn download_best_candidate_media(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    candidates: &[CandidatePost],
    download_attempts: usize,
) -> Result<DownloadResolution> {
    let max_attempts = download_attempts.max(1).min(candidates.len());
    let mut attempts = Vec::new();
    let mut last_error = None;

    for (index, candidate) in candidates.iter().take(max_attempts).enumerate() {
        let attempt_number = index + 1;

        match download_candidate_media(client, token, sound_dir, candidate, attempt_number) {
            Ok((selected_media_url, video_path, audio_path, artifact)) => {
                attempts.push(artifact);
                return Ok(DownloadResolution {
                    selected_candidate: candidate.clone(),
                    selected_media_url,
                    attempts,
                    video_path,
                    audio_path,
                });
            }
            Err((artifact, error)) => {
                attempts.push(artifact);
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("no download attempts were made")))
}

fn download_candidate_media(
    client: &Client,
    token: &str,
    sound_dir: &Path,
    candidate: &CandidatePost,
    attempt_number: usize,
) -> std::result::Result<
    (String, PathBuf, PathBuf, DownloadAttemptArtifact),
    (DownloadAttemptArtifact, anyhow::Error),
> {
    let mut artifact = DownloadAttemptArtifact {
        attempt_number,
        candidate_rank: candidate.selection_rank,
        candidate_source: candidate.source.clone(),
        candidate_video_id: candidate.video_id.clone(),
        candidate_video_url: candidate.video_url.clone(),
        resolved_direct_video_url: None,
        resolved_audio_url: candidate.audio_url.clone(),
        error: None,
    };

    let Some(media_url) = candidate
        .download_url
        .clone()
        .or_else(|| candidate.public_media_url.clone())
    else {
        let error = anyhow!(
            "candidate {} did not expose a downloadable or public media URL",
            candidate.video_url
        );
        artifact.error = Some(error.to_string());
        return Err((artifact, error));
    };
    artifact.resolved_direct_video_url = Some(media_url.clone());

    let temp_video = sound_dir.join(format!("video-attempt-{attempt_number}.mp4"));
    let temp_audio = sound_dir.join(format!("audio-attempt-{attempt_number}.mp3"));
    let final_video = sound_dir.join("video.mp4");
    let final_audio = sound_dir.join("audio.mp3");

    if let Err(error) = apify::download_to_path(client, token, &media_url, &temp_video) {
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

    Ok((media_url, final_video, final_audio, artifact))
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
            ],
        ),
        comment_count: first_u64(
            item,
            &[
                &["comment_count"],
                &["commentCount"],
                &["stats", "comment_count"],
                &["stats", "commentCount"],
            ],
        ),
        share_count: first_u64(
            item,
            &[
                &["share_count"],
                &["shareCount"],
                &["stats", "share_count"],
                &["stats", "shareCount"],
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
