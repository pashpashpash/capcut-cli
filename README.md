# capcut-cli

An open source, agent-first video editing CLI for generating short social clips without touching a timeline.

## What this does

`capcut-cli` lets an agent (or human) discover trending audio, import sounds and clips from any major platform, and compose them into short-form vertical videos — all from the command line, all with structured JSON output.

**This is a working pipeline.** Import a sound, import a clip, compose a video — real MP4 output.

## Quick start

```bash
# Install (puts capcut-cli on your PATH)
cargo install --path .

# 1. Import a sound
capcut-cli library import "https://www.youtube.com/watch?v=oCrwzN6eb4Q" --type sound
# → { "report": "import", "asset": { "id": "snd_XXXXXXXX", ... } }

# 2. Import a video clip
capcut-cli library import "https://www.youtube.com/watch?v=YE7VzlLtp-4" --type clip
# → { "report": "import", "asset": { "id": "clp_XXXXXXXX", ... } }

# 3. Check your library
capcut-cli library list
# → { "report": "library_list", "assets": [...], "total": 2 }

# 4. Compose — use the IDs from steps 1 and 2
capcut-cli compose --sound snd_XXXXXXXX --clip clp_XXXXXXXX --duration-seconds 30
# → { "report": "compose_result", "output_path": "/path/to/render.mp4", ... }
```

Replace `snd_XXXXXXXX` and `clp_XXXXXXXX` with the actual IDs returned by the import commands.

## Prerequisites

- **Rust 1.85+** (build only — uses edition 2024)
- **yt-dlp** — downloads media from YouTube, TikTok, X/Twitter, and [1000+ sites](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md). Install: `brew install yt-dlp`
- **ffmpeg** — runs the video render pipeline. Install: `brew install ffmpeg`

## CLI commands

### `discover` — Find trending content

```bash
# Trending TikTok sounds
capcut-cli discover tiktok-sounds
capcut-cli discover tiktok-sounds --query "hyperpop" --limit 20

# Viral X/Twitter clips
capcut-cli discover x-clips
capcut-cli discover x-clips --query "ai agents" --limit 10
```

Returns a discovery report with notes on API constraints and recommended next steps for each platform.

### `library import` — Download and store assets

```bash
# Import a sound (audio extracted as MP3)
capcut-cli library import "https://www.youtube.com/watch?v=..." --type sound

# Import a video clip (downloaded as MP4)
capcut-cli library import "https://www.youtube.com/watch?v=..." --type clip
```

Downloads the media via yt-dlp, extracts metadata (title, creator, duration, platform), assigns a stable ID (`snd_XXXXXXXX` or `clp_XXXXXXXX`), and persists it to the library manifest.

### `library list` — Browse your library

```bash
capcut-cli library list              # All assets
capcut-cli library list --type sound # Sounds only
capcut-cli library list --type clip  # Clips only
```

### `compose` — Render a video

```bash
capcut-cli compose \
  --sound snd_abc123 \
  --clip clp_def456 \
  --clip clp_ghi789 \
  --duration-seconds 30
```

Runs a four-step ffmpeg pipeline:
1. **Scale and crop** — fits each clip to 720x1280 (9:16 vertical)
2. **Trim/loop clips** — loops short clips or trims long ones to hit the target duration; concatenates multiple clips
3. **Normalize audio** — loudness normalization (loudnorm I=-14, LRA=11, TP=-1) and loop/trim to match video duration
4. **Mux** — combines the video and audio tracks into the final MP4

Output: a real, playable MP4 file. The path is returned in the JSON response.

Optional flags:
- `--duration-seconds <N>` — target duration (default: 30)
- `--output <PATH>` — custom output path (default: library renders directory)

## Agent-first design

Every command outputs structured JSON to stdout:

```json
{
  "report": "import",
  "asset": {
    "id": "snd_41bbac6b",
    "kind": "sound",
    "source_url": "https://www.youtube.com/watch?v=...",
    "platform": "youtube",
    "local_path": "/Users/.../capcut-cli/sounds/snd_41bbac6b.mp3",
    "duration_seconds": 283.0,
    "title": "Song Title",
    "creator": "Artist Name",
    "added_at": 1776039684
  }
}
```

```json
{
  "report": "compose_result",
  "output_path": "/Users/.../capcut-cli/renders/render_cae43596.mp4",
  "sound_id": "snd_41bbac6b",
  "clip_ids": ["clp_ac095b3c"],
  "duration_seconds": 30,
  "pipeline_steps_run": ["scale_and_crop", "trim_clips", "normalize_audio", "mux"]
}
```

- **stdout** = structured JSON only (for agents to parse)
- **stderr** = human-readable progress logs (download progress, ffmpeg output)
- **exit codes**: 0 = success, 1 = error
- **error messages include hints**: e.g. "sound 'snd_xxx' not found — import it first: `capcut-cli library import <url> --type sound`"

### Agent scripting example

```bash
# Import, extract the ID, compose — fully automated
SOUND_JSON=$(capcut-cli library import "$SOUND_URL" --type sound 2>/dev/null)
SOUND_ID=$(echo "$SOUND_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['asset']['id'])")

CLIP_JSON=$(capcut-cli library import "$CLIP_URL" --type clip 2>/dev/null)
CLIP_ID=$(echo "$CLIP_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['asset']['id'])")

RESULT=$(capcut-cli compose --sound "$SOUND_ID" --clip "$CLIP_ID" --duration-seconds 30 2>/dev/null)
OUTPUT_PATH=$(echo "$RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin)['output_path'])")
echo "Video ready at: $OUTPUT_PATH"
```

## Architecture

```
src/
  main.rs          # Entry point — parses CLI, runs command, prints JSON
  cli.rs           # Clap command tree: Discover, Library (import/list), Compose
  models.rs        # AppReport, Asset, Manifest, and all report types
  library.rs       # On-disk library: manifest read/write, asset CRUD
  downloader.rs    # yt-dlp integration: probe metadata + download audio/video
  ffmpeg.rs        # ffmpeg pipeline: scale, crop, loop, trim, normalize, mux

library/
  sounds/          # Committed sound metadata and seed audio samples
    manifest.json
    samples/

notes/             # Research, planning docs, and inspiration
```

**Runtime library location**: `~/Library/Application Support/capcut-cli/` (macOS) containing:
- `manifest.json` — asset index
- `sounds/` — downloaded MP3 files
- `clips/` — downloaded MP4 files
- `renders/` — composed output videos

## Rust dependencies

- [`clap`](https://docs.rs/clap) — CLI framework with derive macros
- [`serde`](https://docs.rs/serde) + [`serde_json`](https://docs.rs/serde_json) — serialization for all JSON output
- [`anyhow`](https://docs.rs/anyhow) — ergonomic error handling
- [`uuid`](https://docs.rs/uuid) — stable asset ID generation
- [`dirs`](https://docs.rs/dirs) — platform-appropriate data directory

## Running tests

```bash
cargo test
```

31 tests (+ 2 ignored integration tests) covering:
- Discovery command logic for both sources (TikTok sounds, X clips)
- CLI argument parsing for all commands, including error cases
- JSON serialization for every report variant and asset type
- Clap validation (required fields, unknown enum values, missing args)

Run the ignored integration tests (requires filesystem access):
```bash
cargo test -- --ignored
```

## Supported platforms for import

Any URL supported by [yt-dlp](https://github.com/yt-dlp/yt-dlp/blob/master/supportedsites.md), including:

| Platform | Sound | Clip |
|----------|-------|------|
| YouTube  | Yes   | Yes  |
| TikTok   | Yes   | Yes  |
| X/Twitter| Yes   | Yes  |
| Instagram| Yes   | Yes  |

## Status

**Fully functional pipeline.**

- `discover` — returns structured acquisition strategy for TikTok sounds and X/Twitter clips
- `library import` — downloads audio/video via yt-dlp, persists metadata to manifest
- `library list` — lists all stored assets with full metadata
- `compose` — executes a four-step ffmpeg pipeline and produces a real MP4

**Next up:**
- Live TikTok trending sound scraping (provider adapters)
- X API credential support for real-time clip discovery
- Additional output formats and aspect ratios
