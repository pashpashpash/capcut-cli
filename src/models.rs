use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "report", rename_all = "snake_case")]
pub enum AppReport {
    Auth(AuthReport),
    SoundImport(SoundImportReport),
    Discovery(DiscoveryReport),
    Library(LibraryReport),
    Media(MediaReport),
    Update(UpdateReport),
}

#[derive(Debug, Serialize)]
pub struct AuthReport {
    pub provider: String,
    pub action: String,
    pub scope: String,
    pub config_path: String,
    pub env_var: String,
    pub token_present: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured_via: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscoveryReport {
    pub source: DiscoverSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub query: Option<String>,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sounds: Vec<DiscoveredSound>,
    pub notes: Vec<String>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverSource {
    TiktokSounds,
    XClips,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredSound {
    pub rank: u32,
    pub title: String,
    pub author: String,
    pub song_id: String,
    pub clip_id: String,
    pub trend_link: String,
    pub duration_seconds: u32,
    pub country_code: String,
    pub related_item_count: usize,
}

#[derive(Debug, Serialize)]
pub struct LibraryReport {
    pub asset_type: String,
    pub source: Option<String>,
    pub id: Option<String>,
    pub required_metadata: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MediaReport {
    pub sound_id: String,
    pub clip_ids: Vec<String>,
    pub duration_seconds: u32,
    pub pipeline: Vec<PipelineStep>,
}

#[derive(Debug, Serialize)]
pub struct PipelineStep {
    pub kind: PipelineStepKind,
    pub description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStepKind {
    NormalizeAudio,
    TrimClips,
    ScaleAndCrop,
    Mux,
}

#[derive(Debug, Serialize)]
pub struct SoundImportReport {
    pub provider: String,
    pub actor_chain: Vec<String>,
    pub attempted_count: usize,
    pub imported_count: usize,
    pub failed_count: usize,
    pub imported: Vec<ImportedSound>,
    pub failed: Vec<FailedSoundImport>,
    pub manifest_path: String,
    pub output_dir: String,
}

#[derive(Debug, Serialize)]
pub struct ImportedSound {
    pub id: String,
    pub rank: u32,
    pub title: String,
    pub author: String,
    pub song_id: String,
    pub clip_id: String,
    pub trend_link: String,
    pub selected_video_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_like_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_comment_count: Option<u64>,
    pub candidate_posts_considered: usize,
    pub downloaded_video_count: usize,
    pub extracted_audio_count: usize,
    pub resolver_actor: String,
    pub download_method: String,
    pub local_videos_dir: String,
    pub local_audios_dir: String,
    pub local_video_path: String,
    pub local_audio_path: String,
    pub local_metadata_path: String,
}

#[derive(Debug, Serialize)]
pub struct FailedSoundImport {
    pub rank: u32,
    pub title: String,
    pub song_id: String,
    pub clip_id: String,
    pub trend_link: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateReport {
    pub action: String,
    pub repository: String,
    pub current_version: String,
    pub target_version: String,
    pub status: String,
    pub asset_name: String,
    pub download_url: String,
    pub install_path: String,
}
