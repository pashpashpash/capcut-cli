use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::models::{Asset, AssetKind, Manifest};

/// Persistent asset library stored under the platform data directory.
///
/// Layout:
///   <data_dir>/capcut-cli/
///     manifest.json
///     sounds/          ← downloaded audio files
///     clips/           ← downloaded video files
///     renders/         ← composed MP4 output
pub struct Library {
    root: PathBuf,
    manifest: Manifest,
}

impl Library {
    /// Open (or create) the library at the platform data directory.
    pub fn open() -> Result<Self> {
        let root = data_dir()?;
        fs::create_dir_all(root.join("sounds"))?;
        fs::create_dir_all(root.join("clips"))?;
        fs::create_dir_all(root.join("renders"))?;

        let manifest_path = root.join("manifest.json");
        let manifest = if manifest_path.exists() {
            let raw = fs::read_to_string(&manifest_path)
                .context("failed to read library manifest")?;
            serde_json::from_str(&raw).context("failed to parse library manifest")?
        } else {
            Manifest::default()
        };

        Ok(Self { root, manifest })
    }

    pub fn sounds_dir(&self) -> PathBuf {
        self.root.join("sounds")
    }

    pub fn clips_dir(&self) -> PathBuf {
        self.root.join("clips")
    }

    pub fn renders_dir(&self) -> PathBuf {
        self.root.join("renders")
    }

    /// Add an asset and persist the manifest.
    pub fn add_asset(&mut self, asset: Asset) -> Result<()> {
        self.manifest.assets.push(asset);
        self.save()
    }

    /// Look up an asset by ID.
    pub fn get_asset(&self, id: &str) -> Option<&Asset> {
        self.manifest.assets.iter().find(|a| a.id == id)
    }

    /// Return all assets, optionally filtered by kind.
    pub fn list_assets(&self, kind: Option<AssetKind>) -> Vec<&Asset> {
        self.manifest
            .assets
            .iter()
            .filter(|a| kind.as_ref().is_none_or(|k| &a.kind == k))
            .collect()
    }

    fn save(&self) -> Result<()> {
        let path = self.root.join("manifest.json");
        let json = serde_json::to_string_pretty(&self.manifest)?;
        fs::write(path, json)?;
        Ok(())
    }
}

fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context(
        "could not determine user data directory — set HOME or XDG_DATA_HOME",
    )?;
    Ok(base.join("capcut-cli"))
}
