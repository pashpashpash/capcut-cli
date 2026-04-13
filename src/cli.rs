use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::time::Instant;

use crate::{config, deps, discover, library, media, output};

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

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage dependencies (yt-dlp, ffmpeg).
    Deps(DepsArgs),
    /// Discover trending sounds and viral clips.
    Discover(DiscoverArgs),
    /// Manage the local asset library.
    Library(LibraryArgs),
    /// Compose clips with a sound into a final video.
    Compose(ComposeArgs),
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Deps(args) => args.run(),
            Command::Discover(args) => args.run(),
            Command::Library(args) => args.run(),
            Command::Compose(args) => args.run(),
        }
    }
}

// ── deps ────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
struct DepsArgs {
    #[command(subcommand)]
    action: DepsAction,
}

#[derive(Debug, Subcommand)]
enum DepsAction {
    /// Check if all dependencies are installed.
    Check,
    /// Download and install all dependencies.
    Install,
}

impl DepsArgs {
    fn run(self) -> Result<()> {
        match self.action {
            DepsAction::Check => {
                let t = Instant::now();
                let result = deps::check_all();
                let all_ok = result
                    .as_object()
                    .map(|m| {
                        m.values()
                            .all(|v| v.get("installed").and_then(|i| i.as_bool()).unwrap_or(false))
                    })
                    .unwrap_or(false);

                if all_ok {
                    output::emit(&output::success("deps check", result, Some(t)));
                } else {
                    let mut env = output::error(
                        "deps check",
                        "MISSING_DEPS",
                        "Some dependencies are not installed.",
                        Some("Run 'capcut-cli deps install' to install them."),
                    );
                    env.data = result;
                    output::emit(&env);
                    std::process::exit(2);
                }
            }
            DepsAction::Install => {
                let t = Instant::now();
                config::ensure_dirs();
                output::log("Installing dependencies...");
                match deps::install_all() {
                    Ok(result) => {
                        output::emit(&output::success("deps install", result, Some(t)));
                    }
                    Err(e) => {
                        output::emit(&output::error(
                            "deps install",
                            "INSTALL_FAILED",
                            &e.to_string(),
                            None,
                        ));
                        std::process::exit(1);
                    }
                }
            }
        }
        Ok(())
    }
}

// ── discover ────────────────────────────────────────────────────────

#[derive(Debug, Args)]
struct DiscoverArgs {
    #[command(subcommand)]
    action: DiscoverAction,
}

#[derive(Debug, Subcommand)]
enum DiscoverAction {
    /// Find currently trending TikTok sounds.
    #[command(name = "tiktok-sounds")]
    TiktokSounds {
        /// Max results to return.
        #[arg(long, default_value_t = 10)]
        limit: u32,
        /// Region code.
        #[arg(long, default_value = "US")]
        region: String,
    },
    /// Find viral video clips on X/Twitter.
    #[command(name = "x-clips")]
    XClips {
        /// Search query for viral clips.
        #[arg(long)]
        query: String,
        /// Max results.
        #[arg(long, default_value_t = 10)]
        limit: u32,
        /// Minimum likes filter.
        #[arg(long, default_value_t = 1000)]
        min_likes: u64,
    },
}

impl DiscoverArgs {
    fn run(self) -> Result<()> {
        match self.action {
            DiscoverAction::TiktokSounds { limit, region } => {
                let t = Instant::now();
                match discover::tiktok::find_trending_sounds(limit, &region) {
                    Ok(data) => {
                        output::emit(&output::success("discover tiktok-sounds", data, Some(t)));
                    }
                    Err(e) => {
                        output::emit(&output::error(
                            "discover tiktok-sounds",
                            "DISCOVERY_FAILED",
                            &e.to_string(),
                            Some(
                                "TikTok endpoints may be rate-limited. Try again later or import \
                                 sounds manually with 'capcut-cli library import <url>'.",
                            ),
                        ));
                        std::process::exit(1);
                    }
                }
            }
            DiscoverAction::XClips {
                query,
                limit,
                min_likes,
            } => {
                let t = Instant::now();
                match discover::twitter::find_viral_clips(&query, limit, min_likes) {
                    Ok(data) => {
                        output::emit(&output::success("discover x-clips", data, Some(t)));
                    }
                    Err(e) => {
                        output::emit(&output::error(
                            "discover x-clips",
                            "DISCOVERY_FAILED",
                            &e.to_string(),
                            None,
                        ));
                        std::process::exit(1);
                    }
                }
            }
        }
        Ok(())
    }
}

// ── library ─────────────────────────────────────────────────────────

#[derive(Debug, Args)]
struct LibraryArgs {
    #[command(subcommand)]
    action: LibraryAction,
}

#[derive(Debug, Subcommand)]
enum LibraryAction {
    /// Download a sound or clip from a URL into the library.
    Import {
        /// URL to import.
        url: String,
        /// Asset type. Auto-detected from URL if omitted.
        #[arg(long = "type")]
        asset_type: Option<String>,
        /// Comma-separated tags.
        #[arg(long, default_value = "")]
        tags: String,
    },
    /// List all assets in the library.
    List {
        /// Filter by type.
        #[arg(long = "type")]
        asset_type: Option<String>,
    },
    /// Show details of a specific asset.
    Show {
        /// Asset ID.
        asset_id: String,
    },
    /// Remove an asset from the library.
    Delete {
        /// Asset ID.
        asset_id: String,
    },
}

impl LibraryArgs {
    fn run(self) -> Result<()> {
        match self.action {
            LibraryAction::Import {
                url,
                asset_type,
                tags,
            } => {
                let t = Instant::now();
                config::ensure_dirs();
                let tag_list: Vec<String> = tags
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match library::import_asset(&url, asset_type.as_deref(), &tag_list) {
                    Ok(asset) => {
                        let data = serde_json::to_value(&asset)?;
                        output::emit(&output::success("library import", data, Some(t)));
                    }
                    Err(e) => {
                        output::emit(&output::error(
                            "library import",
                            "IMPORT_FAILED",
                            &e.to_string(),
                            Some("Run 'capcut-cli deps check' to verify yt-dlp is installed."),
                        ));
                        std::process::exit(1);
                    }
                }
            }
            LibraryAction::List { asset_type } => {
                let t = Instant::now();
                let assets = library::list_assets(asset_type.as_deref())?;
                let data = serde_json::json!({
                    "count": assets.len(),
                    "assets": assets.iter().map(|a| serde_json::to_value(a).unwrap()).collect::<Vec<_>>(),
                });
                output::emit(&output::success("library list", data, Some(t)));
            }
            LibraryAction::Show { asset_id } => {
                let t = Instant::now();
                match library::get_asset(&asset_id)? {
                    Some(asset) => {
                        let data = serde_json::to_value(&asset)?;
                        output::emit(&output::success("library show", data, Some(t)));
                    }
                    None => {
                        output::emit(&output::error(
                            "library show",
                            "NOT_FOUND",
                            &format!("Asset '{asset_id}' not found."),
                            Some("Run 'capcut-cli library list' to see available assets."),
                        ));
                        std::process::exit(1);
                    }
                }
            }
            LibraryAction::Delete { asset_id } => {
                let t = Instant::now();
                match library::delete_asset(&asset_id) {
                    Ok(()) => {
                        output::emit(&output::success(
                            "library delete",
                            serde_json::json!({"deleted": asset_id}),
                            Some(t),
                        ));
                    }
                    Err(e) => {
                        output::emit(&output::error(
                            "library delete",
                            "DELETE_FAILED",
                            &e.to_string(),
                            None,
                        ));
                        std::process::exit(1);
                    }
                }
            }
        }
        Ok(())
    }
}

// ── compose ─────────────────────────────────────────────────────────

#[derive(Debug, Args)]
struct ComposeArgs {
    /// Sound asset ID from the library.
    #[arg(long)]
    sound: String,

    /// Clip asset ID (repeatable).
    #[arg(long = "clip", required = true)]
    clips: Vec<String>,

    /// Output duration in seconds.
    #[arg(long, default_value_t = 30.0)]
    duration: f64,

    /// Output file path. Auto-generated if omitted.
    #[arg(long)]
    output: Option<String>,

    /// Output resolution WxH (default: vertical 1080x1920).
    #[arg(long, default_value = "1080x1920")]
    resolution: String,

    /// Loudness preset or LUFS value. Presets: viral (-8, default),
    /// social (-10), podcast (-14), broadcast (-23). Or pass a number like -12.
    #[arg(long)]
    loudness: Option<String>,
}

// Make ComposeArgs fields accessible for testing
#[cfg(test)]
impl ComposeArgs {
    fn resolution(&self) -> &str { &self.resolution }
    fn duration(&self) -> f64 { self.duration }
}

impl ComposeArgs {
    fn run(self) -> Result<()> {
        let t = Instant::now();
        config::ensure_dirs();
        match media::compose::run_compose(
            &self.sound,
            &self.clips,
            self.duration,
            self.output.as_deref(),
            &self.resolution,
            self.loudness.as_deref(),
        ) {
            Ok(result) => {
                let data = serde_json::to_value(&result)?;
                output::emit(&output::success("compose", data, Some(t)));
            }
            Err(e) => {
                output::emit(&output::error(
                    "compose",
                    "COMPOSE_FAILED",
                    &e.to_string(),
                    Some(
                        "Ensure assets exist with 'capcut-cli library list' and deps are \
                         installed with 'capcut-cli deps check'.",
                    ),
                ));
                std::process::exit(1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    // ── deps ───────────────────────────────────────────────────────

    #[test]
    fn parse_deps_check() {
        let cli = parse(&["capcut-cli", "deps", "check"]).unwrap();
        assert!(matches!(cli.command, Command::Deps(_)));
    }

    #[test]
    fn parse_deps_install() {
        let cli = parse(&["capcut-cli", "deps", "install"]).unwrap();
        assert!(matches!(cli.command, Command::Deps(_)));
    }

    // ── discover ───────────────────────────────────────────────────

    #[test]
    fn parse_discover_tiktok_sounds_defaults() {
        let cli = parse(&["capcut-cli", "discover", "tiktok-sounds"]).unwrap();
        match cli.command {
            Command::Discover(DiscoverArgs {
                action: DiscoverAction::TiktokSounds { limit, region },
            }) => {
                assert_eq!(limit, 10);
                assert_eq!(region, "US");
            }
            _ => panic!("expected TiktokSounds"),
        }
    }

    #[test]
    fn parse_discover_tiktok_sounds_custom_args() {
        let cli = parse(&[
            "capcut-cli", "discover", "tiktok-sounds", "--limit", "20", "--region", "UK",
        ])
        .unwrap();
        match cli.command {
            Command::Discover(DiscoverArgs {
                action: DiscoverAction::TiktokSounds { limit, region },
            }) => {
                assert_eq!(limit, 20);
                assert_eq!(region, "UK");
            }
            _ => panic!("expected TiktokSounds"),
        }
    }

    #[test]
    fn parse_discover_x_clips() {
        let cli = parse(&[
            "capcut-cli", "discover", "x-clips", "--query", "ai agents",
        ])
        .unwrap();
        match cli.command {
            Command::Discover(DiscoverArgs {
                action:
                    DiscoverAction::XClips {
                        query,
                        limit,
                        min_likes,
                    },
            }) => {
                assert_eq!(query, "ai agents");
                assert_eq!(limit, 10);
                assert_eq!(min_likes, 1000);
            }
            _ => panic!("expected XClips"),
        }
    }

    #[test]
    fn parse_discover_x_clips_requires_query() {
        let result = parse(&["capcut-cli", "discover", "x-clips"]);
        assert!(result.is_err());
    }

    // ── library ────────────────────────────────────────────────────

    #[test]
    fn parse_library_import() {
        let cli = parse(&[
            "capcut-cli",
            "library",
            "import",
            "https://youtube.com/watch?v=test",
            "--type",
            "sound",
        ])
        .unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action:
                    LibraryAction::Import {
                        url,
                        asset_type,
                        tags,
                    },
            }) => {
                assert_eq!(url, "https://youtube.com/watch?v=test");
                assert_eq!(asset_type.as_deref(), Some("sound"));
                assert_eq!(tags, "");
            }
            _ => panic!("expected Library Import"),
        }
    }

    #[test]
    fn parse_library_import_with_tags() {
        let cli = parse(&[
            "capcut-cli",
            "library",
            "import",
            "https://example.com/vid",
            "--tags",
            "trending,viral",
        ])
        .unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action: LibraryAction::Import { tags, .. },
            }) => {
                assert_eq!(tags, "trending,viral");
            }
            _ => panic!("expected Library Import"),
        }
    }

    #[test]
    fn parse_library_list_no_filter() {
        let cli = parse(&["capcut-cli", "library", "list"]).unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action: LibraryAction::List { asset_type },
            }) => {
                assert!(asset_type.is_none());
            }
            _ => panic!("expected Library List"),
        }
    }

    #[test]
    fn parse_library_list_with_filter() {
        let cli = parse(&["capcut-cli", "library", "list", "--type", "clip"]).unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action: LibraryAction::List { asset_type },
            }) => {
                assert_eq!(asset_type.as_deref(), Some("clip"));
            }
            _ => panic!("expected Library List"),
        }
    }

    #[test]
    fn parse_library_show() {
        let cli = parse(&["capcut-cli", "library", "show", "snd_abc123"]).unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action: LibraryAction::Show { asset_id },
            }) => {
                assert_eq!(asset_id, "snd_abc123");
            }
            _ => panic!("expected Library Show"),
        }
    }

    #[test]
    fn parse_library_delete() {
        let cli = parse(&["capcut-cli", "library", "delete", "clp_xyz789"]).unwrap();
        match cli.command {
            Command::Library(LibraryArgs {
                action: LibraryAction::Delete { asset_id },
            }) => {
                assert_eq!(asset_id, "clp_xyz789");
            }
            _ => panic!("expected Library Delete"),
        }
    }

    // ── compose ────────────────────────────────────────────────────

    #[test]
    fn parse_compose_minimal() {
        let cli = parse(&[
            "capcut-cli", "compose", "--sound", "snd_abc", "--clip", "clp_def",
        ])
        .unwrap();
        match cli.command {
            Command::Compose(args) => {
                assert_eq!(args.sound, "snd_abc");
                assert_eq!(args.clips, vec!["clp_def"]);
                assert_eq!(args.duration(), 30.0);
                assert_eq!(args.resolution(), "1080x1920");
                assert!(args.output.is_none());
                assert!(args.loudness.is_none());
            }
            _ => panic!("expected Compose"),
        }
    }

    #[test]
    fn parse_compose_multiple_clips() {
        let cli = parse(&[
            "capcut-cli", "compose", "--sound", "snd_abc", "--clip", "clp_1", "--clip", "clp_2",
        ])
        .unwrap();
        match cli.command {
            Command::Compose(args) => {
                assert_eq!(args.clips, vec!["clp_1", "clp_2"]);
            }
            _ => panic!("expected Compose"),
        }
    }

    #[test]
    fn parse_compose_all_options() {
        let cli = parse(&[
            "capcut-cli",
            "compose",
            "--sound",
            "snd_abc",
            "--clip",
            "clp_def",
            "--duration",
            "60",
            "--output",
            "/tmp/out.mp4",
            "--resolution",
            "720x1280",
            "--loudness",
            "podcast",
        ])
        .unwrap();
        match cli.command {
            Command::Compose(args) => {
                assert_eq!(args.duration(), 60.0);
                assert_eq!(args.output.as_deref(), Some("/tmp/out.mp4"));
                assert_eq!(args.resolution(), "720x1280");
                assert_eq!(args.loudness.as_deref(), Some("podcast"));
            }
            _ => panic!("expected Compose"),
        }
    }

    #[test]
    fn parse_compose_requires_sound() {
        let result = parse(&["capcut-cli", "compose", "--clip", "clp_def"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_compose_requires_clip() {
        let result = parse(&["capcut-cli", "compose", "--sound", "snd_abc"]);
        assert!(result.is_err());
    }

    // ── error cases ────────────────────────────────────────────────

    #[test]
    fn parse_unknown_command_errors() {
        let result = parse(&["capcut-cli", "nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_no_args_errors() {
        let result = parse(&["capcut-cli"]);
        assert!(result.is_err());
    }
}
