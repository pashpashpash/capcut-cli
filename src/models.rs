use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Top-level report envelope — every command returns one of these as JSON.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(tag = "report", rename_all = "snake_case")]
pub enum AppReport {
    Discovery(DiscoveryReport),
    Import(ImportReport),
    LibraryList(LibraryListReport),
    ComposeResult(ComposeResultReport),
}

// ---------------------------------------------------------------------------
// discover
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Assets — stored in the library manifest.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Sound,
    Clip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    pub source_url: String,
    pub platform: String,
    /// Absolute path to the downloaded file.
    pub local_path: String,
    pub duration_seconds: f64,
    pub title: Option<String>,
    pub creator: Option<String>,
    /// Unix timestamp (seconds since epoch).
    pub added_at: u64,
}

/// The on-disk library manifest (`manifest.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub assets: Vec<Asset>,
}

// ---------------------------------------------------------------------------
// library import
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub asset: Asset,
}

// ---------------------------------------------------------------------------
// library list
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct LibraryListReport {
    pub assets: Vec<Asset>,
    pub total: usize,
}

// ---------------------------------------------------------------------------
// compose
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ComposeResultReport {
    pub output_path: String,
    pub sound_id: String,
    pub clip_ids: Vec<String>,
    pub duration_seconds: u32,
    pub pipeline_steps_run: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn discovery_report() -> AppReport {
        AppReport::Discovery(DiscoveryReport {
            source: DiscoverSource::TiktokSounds,
            query: None,
            limit: 10,
            notes: vec![],
            next_steps: vec![],
        })
    }

    fn sample_asset() -> Asset {
        Asset {
            id: "snd_abc123".to_string(),
            kind: AssetKind::Sound,
            source_url: "https://example.com/sound".to_string(),
            platform: "tiktok".to_string(),
            local_path: "/tmp/snd_abc123.mp3".to_string(),
            duration_seconds: 30.0,
            title: Some("Test Sound".to_string()),
            creator: Some("artist".to_string()),
            added_at: 1_700_000_000,
        }
    }

    // --- AppReport tag ---

    #[test]
    fn discovery_report_has_correct_tag() {
        let v: Value = serde_json::to_value(discovery_report()).unwrap();
        assert_eq!(v["report"], "discovery");
    }

    #[test]
    fn import_report_has_correct_tag() {
        let report = AppReport::Import(ImportReport { asset: sample_asset() });
        let v: Value = serde_json::to_value(report).unwrap();
        assert_eq!(v["report"], "import");
    }

    #[test]
    fn library_list_report_has_correct_tag() {
        let report = AppReport::LibraryList(LibraryListReport {
            assets: vec![],
            total: 0,
        });
        let v: Value = serde_json::to_value(report).unwrap();
        assert_eq!(v["report"], "library_list");
    }

    #[test]
    fn compose_result_report_has_correct_tag() {
        let report = AppReport::ComposeResult(ComposeResultReport {
            output_path: "/tmp/out.mp4".to_string(),
            sound_id: "snd_abc".to_string(),
            clip_ids: vec!["clp_def".to_string()],
            duration_seconds: 30,
            pipeline_steps_run: vec![],
        });
        let v: Value = serde_json::to_value(report).unwrap();
        assert_eq!(v["report"], "compose_result");
    }

    // --- DiscoverSource serialization ---

    #[test]
    fn discover_source_tiktok_sounds_serializes() {
        let v: Value = serde_json::to_value(DiscoverSource::TiktokSounds).unwrap();
        assert_eq!(v, "tiktok_sounds");
    }

    #[test]
    fn discover_source_x_clips_serializes() {
        let v: Value = serde_json::to_value(DiscoverSource::XClips).unwrap();
        assert_eq!(v, "x_clips");
    }

    // --- AssetKind serialization ---

    #[test]
    fn asset_kind_sound_serializes() {
        let v: Value = serde_json::to_value(AssetKind::Sound).unwrap();
        assert_eq!(v, "sound");
    }

    #[test]
    fn asset_kind_clip_serializes() {
        let v: Value = serde_json::to_value(AssetKind::Clip).unwrap();
        assert_eq!(v, "clip");
    }

    // --- Asset serialization ---

    #[test]
    fn asset_serializes_all_fields() {
        let v: Value = serde_json::to_value(sample_asset()).unwrap();
        assert_eq!(v["id"], "snd_abc123");
        assert_eq!(v["kind"], "sound");
        assert_eq!(v["source_url"], "https://example.com/sound");
        assert_eq!(v["platform"], "tiktok");
        assert_eq!(v["local_path"], "/tmp/snd_abc123.mp3");
        assert_eq!(v["duration_seconds"], 30.0);
        assert_eq!(v["title"], "Test Sound");
        assert_eq!(v["creator"], "artist");
        assert_eq!(v["added_at"], 1_700_000_000u64);
    }

    #[test]
    fn asset_null_fields_when_none() {
        let mut a = sample_asset();
        a.title = None;
        a.creator = None;
        let v: Value = serde_json::to_value(a).unwrap();
        assert!(v["title"].is_null());
        assert!(v["creator"].is_null());
    }

    // --- DiscoveryReport fields ---

    #[test]
    fn discovery_report_preserves_query_and_limit() {
        let report = AppReport::Discovery(DiscoveryReport {
            source: DiscoverSource::XClips,
            query: Some("cats".to_string()),
            limit: 25,
            notes: vec!["a note".to_string()],
            next_steps: vec!["a step".to_string()],
        });
        let v: Value = serde_json::to_value(report).unwrap();
        assert_eq!(v["query"], "cats");
        assert_eq!(v["limit"], 25);
        assert_eq!(v["notes"][0], "a note");
        assert_eq!(v["next_steps"][0], "a step");
    }

    #[test]
    fn discovery_report_null_query_when_none() {
        let v: Value = serde_json::to_value(discovery_report()).unwrap();
        assert!(v["query"].is_null());
    }

    // --- ComposeResultReport fields ---

    #[test]
    fn compose_result_preserves_fields() {
        let report = AppReport::ComposeResult(ComposeResultReport {
            output_path: "/renders/out.mp4".to_string(),
            sound_id: "snd_1".to_string(),
            clip_ids: vec!["clp_a".to_string(), "clp_b".to_string()],
            duration_seconds: 30,
            pipeline_steps_run: vec!["scale_and_crop".to_string(), "mux".to_string()],
        });
        let v: Value = serde_json::to_value(report).unwrap();
        assert_eq!(v["output_path"], "/renders/out.mp4");
        assert_eq!(v["sound_id"], "snd_1");
        assert_eq!(v["clip_ids"][1], "clp_b");
        assert_eq!(v["duration_seconds"], 30);
        assert_eq!(v["pipeline_steps_run"][0], "scale_and_crop");
    }
}
