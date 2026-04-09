use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "report", rename_all = "snake_case")]
pub enum AppReport {
    Discovery(DiscoveryReport),
    Library(LibraryReport),
    Media(MediaReport),
}

#[derive(Debug, Serialize)]
pub struct DiscoveryReport {
    pub source: DiscoverSource,
    pub query: Option<String>,
    pub limit: u32,
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
