# capcut-cli

An open source, agent-first Rust CLI for discovering, importing, and composing short-form social video assets.

## What works now

The main working path is a TikTok trending sound ingestion pipeline built around Apify:

1. discover trending sounds with `alien_force~tiktok-trending-sounds-tracker`
2. fetch posts for each sound with `powerai~tiktok-music-posts-video-scraper`
3. rank candidate posts with a comment-heavy sort
4. fall back to the trend actor's `related_items` when music-post results are empty
5. download the representative video with `dltik~tiktok-video-downloader`
6. extract audio locally with `ffmpeg`
7. persist video, audio, per-step metadata, and manifest entries in-repo

The CLI stays machine-readable throughout, so agents can inspect every step without scraping terminal text.

## Prerequisites

- Rust toolchain
- `ffmpeg` on `PATH`
- an Apify token

## Auth

Check current auth state:

```bash
cargo run -- auth
```

Persist a token directly:

```bash
cargo run -- auth --apify "$APIFY_API_TOKEN"
```

Persist the current `CAPCUT_CLI_APIFY_TOKEN` env var:

```bash
export CAPCUT_CLI_APIFY_TOKEN="$APIFY_API_TOKEN"
cargo run -- auth --from-env
```

The config file lives at `~/.config/capcut-cli/config.json`. At runtime, `CAPCUT_CLI_APIFY_TOKEN` still overrides the file.

## Discover trending TikTok sounds

```bash
cargo run -- discover tiktok-sounds --country "United States" --period 7 --limit 5
```

This returns live sound metadata from Apify, including `song_id`, `clip_id`, and the count of fallback `related_items`.

## Import trending TikTok sounds

```bash
cargo run -- library sound import-tiktok-trending \
  --country "United States" \
  --period 7 \
  --limit 3 \
  --max-posts 30 \
  --download-attempts 5
```

What the importer does for each sound:

1. writes `trend.json` with the trend actor payload
2. writes `posts.json` with the music-post actor payload
3. writes `selection.json` with normalized/ranked candidate posts
4. tries the top ranked candidate URLs with `dltik`
5. stores the winning `video.mp4`
6. extracts `audio.mp3` locally with `ffmpeg`
7. writes `download.json` and a summary `metadata.json`
8. merges the result into `library/sounds/manifest.json`

The ranking strategy is explicit and stable: `comment_count desc`, then `share_count`, then `digg_count`, then `play_count`.

## Output layout

Imported sounds are written under `library/sounds/imported/<slug>/`:

- `trend.json`
- `posts.json`
- `selection.json`
- `download.json`
- `metadata.json`
- `video.mp4`
- `audio.mp3`

The top-level manifest records provenance plus local paths for both media files and the step metadata.

## CLI surface

```bash
cargo run -- auth
cargo run -- auth --apify <token>
cargo run -- auth --from-env
cargo run -- discover tiktok-sounds --limit 10
cargo run -- discover x-clips --query "ai agents" --limit 10
cargo run -- library plan sound
cargo run -- library sound import-tiktok-trending --limit 3 --max-posts 30 --download-attempts 5
cargo run -- compose --sound sound_123 --clip clip_a --clip clip_b --duration-seconds 30
```

## Status of X support

`discover x-clips` is still a planning stub. The TikTok import path is the only live end-to-end ingestion pipeline in the repo today.

## Rights

Downloaded media should be treated as research and prototyping assets unless rights have been separately verified. The importer records provenance and rights notes so the local library does not lose source context.
