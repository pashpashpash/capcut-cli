use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "report", rename_all = "snake_case")]
pub enum AppReport {
    Auth(AuthReport),
    SoundImport(SoundImportReport),
    SoundJudgement(SoundJudgementReport),
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
pub struct SoundJudgementReport {
    pub manifest_path: String,
    pub total_count: usize,
    pub judged_count: usize,
    pub filtered_out_count: usize,
    pub sort_order: String,
    pub filters: SoundJudgementFilters,
    pub summary: SoundJudgementSummary,
    pub filtered_summary: SoundJudgementSummary,
    pub sounds: Vec<JudgedSound>,
}

#[derive(Debug, Serialize)]
pub struct SoundJudgementFilters {
    pub top: Option<usize>,
    pub distinct_song_id: bool,
    pub min_score: Option<u32>,
    pub max_trend_rank: Option<u32>,
    pub max_judgement_rank: Option<usize>,
    pub platforms: Vec<String>,
    pub country_codes: Vec<String>,
    pub min_song_id_country_coverage: Option<usize>,
    pub min_song_id_top_25_country_count: Option<usize>,
    pub max_song_id_best_trend_rank: Option<u32>,
    pub min_song_id_best_representative_views: Option<u64>,
    pub min_song_id_best_representative_engagements: Option<u64>,
    pub min_song_id_best_representative_comments: Option<u64>,
    pub min_song_id_best_representative_shares: Option<u64>,
    pub min_song_id_best_representative_engagement_rate_per_1000_views: Option<u64>,
    pub min_song_id_best_representative_share_rate_per_1000_views: Option<u64>,
    pub required_reasons: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub excluded_risks: Vec<String>,
    pub min_reason_count: Option<usize>,
    pub max_risk_count: Option<usize>,
    pub min_downloaded_videos: Option<usize>,
    pub min_extracted_audios: Option<usize>,
    pub min_usable_asset_pairs: Option<usize>,
    pub min_candidate_posts: Option<usize>,
    pub min_duration_seconds: Option<u32>,
    pub max_duration_seconds: Option<u32>,
    pub min_source_identifiers: Option<usize>,
    pub required_source_identifier_fields: Vec<String>,
    pub require_resolver_actor_id: bool,
    pub required_download_methods: Vec<String>,
    pub required_provenance_terms: Vec<String>,
    pub excluded_provenance_terms: Vec<String>,
    pub required_rights_notes: Vec<String>,
    pub excluded_rights_notes: Vec<String>,
    pub min_local_artifact_paths: Option<usize>,
    pub required_local_artifact_path_fields: Vec<String>,
    pub min_representative_views: Option<u64>,
    pub min_representative_likes: Option<u64>,
    pub min_representative_engagements: Option<u64>,
    pub min_representative_like_rate_per_1000_views: Option<u64>,
    pub min_representative_engagement_rate_per_1000_views: Option<u64>,
    pub min_representative_comments: Option<u64>,
    pub min_representative_comment_rate_per_1000_views: Option<u64>,
    pub min_representative_shares: Option<u64>,
    pub min_representative_share_rate_per_1000_views: Option<u64>,
    pub min_representative_music_duration_seconds: Option<f64>,
    pub max_representative_music_duration_seconds: Option<f64>,
    pub representative_music_is_original_sound: Option<bool>,
    pub representative_music_commercial_right_type: Option<u64>,
    pub representative_music_is_batch_take_down_music: Option<bool>,
    pub representative_music_reviewed: Option<bool>,
    pub min_representative_music_fields: Option<usize>,
    pub required_representative_music_fields: Vec<String>,
    pub require_representative_music_can_read: bool,
    pub require_representative_music_can_reuse: bool,
    pub require_representative_music_has_strong_beat_url: bool,
    pub require_representative_music_vid: bool,
    pub min_representative_engagement_metrics: Option<usize>,
    pub required_engagement_metric_fields: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SoundJudgementSummary {
    pub recommended_action_counts: Vec<RecommendedActionCount>,
    pub platform_counts: Vec<PlatformCount>,
    pub country_code_counts: Vec<CountryCodeCount>,
    pub song_id_country_coverage_counts: Vec<SongIdCountryCoverageCount>,
    pub song_id_top_25_country_count_counts: Vec<SongIdTop25CountryCount>,
    pub song_id_best_trend_rank_band_counts: Vec<TrendRankBandCount>,
    pub song_id_best_representative_view_count_band_counts: Vec<RepresentativeViewCountBandCount>,
    pub song_id_best_representative_engagement_count_band_counts:
        Vec<RepresentativeEngagementCountBandCount>,
    pub song_id_best_representative_comment_count_band_counts:
        Vec<RepresentativeCommentCountBandCount>,
    pub song_id_best_representative_share_count_band_counts: Vec<RepresentativeShareCountBandCount>,
    pub song_id_best_representative_engagement_rate_band_counts:
        Vec<RepresentativeEngagementRateBandCount>,
    pub song_id_best_representative_share_rate_band_counts: Vec<RepresentativeShareRateBandCount>,
    pub score_band_counts: Vec<ScoreBandCount>,
    pub trend_rank_band_counts: Vec<TrendRankBandCount>,
    pub judgement_rank_band_counts: Vec<JudgementRankBandCount>,
    pub duration_seconds_band_counts: Vec<DurationSecondsBandCount>,
    pub source_identifier_coverage_counts: Vec<SourceIdentifierCoverageCount>,
    pub source_identifier_field_counts: Vec<SourceIdentifierFieldCount>,
    pub resolver_actor_id_coverage_counts: Vec<ResolverActorIdCoverageCount>,
    pub download_method_counts: Vec<DownloadMethodCount>,
    pub provenance_coverage_counts: Vec<ProvenanceCoverageCount>,
    pub rights_note_counts: Vec<RightsNoteCount>,
    pub reason_count_coverage_counts: Vec<ReasonCountCoverageCount>,
    pub risk_count_coverage_counts: Vec<RiskCountCoverageCount>,
    pub downloaded_video_coverage_counts: Vec<DownloadedVideoCoverageCount>,
    pub extracted_audio_coverage_counts: Vec<ExtractedAudioCoverageCount>,
    pub usable_asset_pair_coverage_counts: Vec<UsableAssetPairCoverageCount>,
    pub candidate_post_coverage_counts: Vec<CandidatePostCoverageCount>,
    pub local_artifact_path_coverage_counts: Vec<LocalArtifactPathCoverageCount>,
    pub local_artifact_path_field_counts: Vec<LocalArtifactPathFieldCount>,
    pub engagement_metric_coverage_counts: Vec<EngagementMetricCoverageCount>,
    pub representative_engagement_metric_field_counts:
        Vec<RepresentativeEngagementMetricFieldCount>,
    pub representative_view_count_band_counts: Vec<RepresentativeViewCountBandCount>,
    pub representative_engagement_count_band_counts: Vec<RepresentativeEngagementCountBandCount>,
    pub representative_like_count_band_counts: Vec<RepresentativeLikeCountBandCount>,
    pub representative_comment_count_band_counts: Vec<RepresentativeCommentCountBandCount>,
    pub representative_share_count_band_counts: Vec<RepresentativeShareCountBandCount>,
    pub representative_like_rate_band_counts: Vec<RepresentativeLikeRateBandCount>,
    pub representative_engagement_rate_band_counts: Vec<RepresentativeEngagementRateBandCount>,
    pub representative_comment_rate_band_counts: Vec<RepresentativeCommentRateBandCount>,
    pub representative_share_rate_band_counts: Vec<RepresentativeShareRateBandCount>,
    pub representative_music_duration_band_counts: Vec<RepresentativeMusicDurationBandCount>,
    pub representative_music_field_coverage_counts: Vec<RepresentativeMusicFieldCoverageCount>,
    pub representative_music_field_counts: Vec<RepresentativeMusicFieldCount>,
    pub representative_music_can_read_counts: Vec<RepresentativeMusicCanReadCount>,
    pub representative_music_can_reuse_counts: Vec<RepresentativeMusicCanReuseCount>,
    pub representative_music_is_original_sound_counts: Vec<RepresentativeMusicIsOriginalSoundCount>,
    pub representative_music_commercial_right_type_counts:
        Vec<RepresentativeMusicCommercialRightTypeCount>,
    pub representative_music_is_batch_take_down_music_counts:
        Vec<RepresentativeMusicIsBatchTakeDownMusicCount>,
    pub representative_music_reviewed_counts: Vec<RepresentativeMusicReviewedCount>,
    pub representative_music_has_strong_beat_url_counts:
        Vec<RepresentativeMusicHasStrongBeatUrlCount>,
    pub representative_music_vid_coverage_counts: Vec<RepresentativeMusicVidCoverageCount>,
    pub missing_source_identifier_field_counts: Vec<MissingSourceIdentifierFieldCount>,
    pub missing_local_artifact_path_field_counts: Vec<MissingLocalArtifactPathFieldCount>,
    pub missing_engagement_metric_field_counts: Vec<MissingEngagementMetricFieldCount>,
    pub missing_representative_music_field_counts: Vec<MissingRepresentativeMusicFieldCount>,
    pub reason_counts: Vec<ReasonCount>,
    pub risk_counts: Vec<RiskCount>,
}

#[derive(Debug, Serialize)]
pub struct RecommendedActionCount {
    pub recommended_action: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct PlatformCount {
    pub platform: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct CountryCodeCount {
    pub country_code: Option<String>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct SongIdCountryCoverageCount {
    pub country_code_count: usize,
    pub song_id_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SongIdTop25CountryCount {
    pub top_25_country_count: usize,
    pub song_id_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ScoreBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct TrendRankBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct JudgementRankBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct DurationSecondsBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct SourceIdentifierCoverageCount {
    pub source_identifier_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct SourceIdentifierFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ResolverActorIdCoverageCount {
    pub resolver_actor_id_present: bool,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct DownloadMethodCount {
    pub download_method: Option<String>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ProvenanceCoverageCount {
    pub provenance_present: bool,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RightsNoteCount {
    pub rights_note: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ReasonCountCoverageCount {
    pub reason_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RiskCountCoverageCount {
    pub risk_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct DownloadedVideoCoverageCount {
    pub downloaded_video_count: Option<usize>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ExtractedAudioCoverageCount {
    pub extracted_audio_count: Option<usize>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct UsableAssetPairCoverageCount {
    pub usable_asset_pair_count: Option<usize>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct CandidatePostCoverageCount {
    pub candidate_post_count: Option<usize>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct EngagementMetricCoverageCount {
    pub representative_engagement_metric_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct LocalArtifactPathCoverageCount {
    pub local_artifact_path_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct LocalArtifactPathFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeEngagementMetricFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeViewCountBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeEngagementCountBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeLikeCountBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeCommentCountBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeShareCountBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeLikeRateBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeEngagementRateBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeCommentRateBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeShareRateBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicDurationBandCount {
    pub band: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicFieldCoverageCount {
    pub representative_music_field_count: usize,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicCanReadCount {
    pub can_read: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicCanReuseCount {
    pub can_reuse: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicIsOriginalSoundCount {
    pub is_original_sound: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicCommercialRightTypeCount {
    pub commercial_right_type: Option<u64>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicIsBatchTakeDownMusicCount {
    pub is_batch_take_down_music: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicReviewedCount {
    pub reviewed: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicHasStrongBeatUrlCount {
    pub has_strong_beat_url: Option<bool>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepresentativeMusicVidCoverageCount {
    pub music_vid_present: bool,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct MissingRepresentativeMusicFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct MissingSourceIdentifierFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct MissingEngagementMetricFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct MissingLocalArtifactPathFieldCount {
    pub field: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ReasonCount {
    pub reason: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct RiskCount {
    pub risk: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct JudgedSound {
    pub sound_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub judgement_rank: Option<usize>,
    pub trend_rank: Option<u32>,
    pub title: String,
    pub author: String,
    pub platform: String,
    pub provenance: String,
    pub rights_note: String,
    pub resolver_actor_id: Option<String>,
    pub download_method: Option<String>,
    pub source_url: String,
    pub source_video_url: Option<String>,
    pub song_id: Option<String>,
    pub song_id_country_coverage_count: Option<usize>,
    pub song_id_top_25_country_count: Option<usize>,
    pub song_id_best_trend_rank: Option<u32>,
    pub song_id_best_representative_view_count: Option<u64>,
    pub song_id_best_representative_engagement_count: Option<u64>,
    pub song_id_best_representative_comment_count: Option<u64>,
    pub song_id_best_representative_share_count: Option<u64>,
    pub song_id_best_representative_engagement_rate_per_1000_views: Option<u64>,
    pub song_id_best_representative_share_rate_per_1000_views: Option<u64>,
    pub clip_id: Option<String>,
    pub country_code: Option<String>,
    pub duration_seconds: Option<u32>,
    pub source_identifier_count: usize,
    pub source_identifier_fields: Vec<String>,
    pub missing_source_identifier_fields: Vec<String>,
    pub local_audio_path: String,
    pub local_video_path: Option<String>,
    pub local_metadata_path: String,
    pub local_trend_path: Option<String>,
    pub local_posts_path: Option<String>,
    pub local_selection_path: Option<String>,
    pub local_download_path: Option<String>,
    pub local_artifact_path_count: usize,
    pub local_artifact_path_fields: Vec<String>,
    pub missing_local_artifact_path_fields: Vec<String>,
    pub downloaded_video_count: Option<usize>,
    pub extracted_audio_count: Option<usize>,
    pub usable_asset_pair_count: Option<usize>,
    pub candidate_post_count: Option<usize>,
    pub representative_view_count: Option<u64>,
    pub representative_like_count: Option<u64>,
    pub representative_engagement_count: Option<u64>,
    pub representative_like_rate_per_1000_views: Option<u64>,
    pub representative_engagement_rate_per_1000_views: Option<u64>,
    pub representative_comment_count: Option<u64>,
    pub representative_comment_rate_per_1000_views: Option<u64>,
    pub representative_share_count: Option<u64>,
    pub representative_share_rate_per_1000_views: Option<u64>,
    pub representative_music_duration_seconds: Option<f64>,
    pub representative_music_can_read: Option<bool>,
    pub representative_music_can_reuse: Option<bool>,
    pub representative_music_is_original_sound: Option<bool>,
    pub representative_music_commercial_right_type: Option<u64>,
    pub representative_music_is_batch_take_down_music: Option<bool>,
    pub representative_music_reviewed: Option<bool>,
    pub representative_music_has_strong_beat_url: Option<bool>,
    pub representative_music_vid: Option<String>,
    pub representative_music_field_count: usize,
    pub representative_music_fields: Vec<String>,
    pub missing_representative_music_fields: Vec<String>,
    pub representative_engagement_metric_count: usize,
    pub representative_engagement_metric_fields: Vec<String>,
    pub missing_representative_engagement_metric_fields: Vec<String>,
    pub score: u32,
    pub reason_count: usize,
    pub reasons: Vec<String>,
    pub risk_count: usize,
    pub risks: Vec<String>,
    pub recommended_action: String,
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
