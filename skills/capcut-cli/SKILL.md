---
name: capcut-cli
description: Use when working in this repo to authenticate Apify, discover trending TikTok sounds, import them through the repo's Apify pipeline, inspect the sound manifest, or debug per-step TikTok ingestion artifacts.
---

# capcut-cli

Use this skill when an agent needs to operate the local Rust CLI instead of hand-rolling Apify requests.

## Quick start

Check auth first:

```bash
cargo run -- auth
```

Persist a token directly:

```bash
cargo run -- auth --apify "$APIFY_API_TOKEN"
```

Or persist the current env token:

```bash
export CAPCUT_CLI_APIFY_TOKEN="..."
cargo run -- auth --from-env
```

## Live discovery

Use the live discovery command when you only need trending sound ids and metadata:

```bash
cargo run -- discover tiktok-sounds --country "United States" --period 7 --limit 5
```

This uses `alien_force~tiktok-trending-sounds-tracker` and returns JSON with `song_id`, `clip_id`, and fallback `related_items` counts.

## End-to-end import

```bash
cargo run -- library sound import-tiktok-trending \
  --country "United States" \
  --period 7 \
  --limit 1 \
  --max-posts 30 \
  --download-attempts 5
```

The importer uses this actor chain:

1. `alien_force~tiktok-trending-sounds-tracker`
2. `powerai~tiktok-music-posts-video-scraper`
3. `dltik~tiktok-video-downloader`

Selection behavior:

- prefer the candidate with the highest `comment_count`
- break ties with `share_count`, then `digg_count`, then `play_count`
- fall back to the trend actor's `related_items` when the music-post actor returns no usable rows
- try multiple ranked candidates before failing the sound

## Expected artifacts

Each imported sound directory under `library/sounds/imported/` should contain:

- `trend.json`
- `posts.json`
- `selection.json`
- `download.json`
- `metadata.json`
- `video.mp4`
- `audio.mp3`

`library/sounds/manifest.json` should also be updated with provenance, media paths, and representative post metrics.

## Operating rules

- Do not commit API keys or copied config files.
- Prefer machine-readable JSON command output over prose when chaining commands.
- Verify both `video.mp4` and `audio.mp3` exist before claiming an import succeeded.
- Treat DNS/connectivity failures to `api.apify.com` as environment blockers, not actor-shape regressions.
- Treat imported media as research/prototyping assets unless rights are separately verified.
