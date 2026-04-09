# capcut-cli

An open source, agent-first Rust CLI for discovering, importing, and composing short-form social video assets.

## What works now

The main working path is a TikTok trending sound ingestion pipeline built around Apify:

1. discover trending sounds with `alien_force~tiktok-trending-sounds-tracker`
2. resolve posts for each sound URL with a validated Novi actor using the repo's fixed `MUSIC` input profile
3. normalize the resolver dataset and rank candidates by `digg_count` descending
4. select the top-liked post, with ordered fallback only if that row does not expose a usable direct video media URL
5. download the representative video directly from the resolver output when it already provides a public or downloadable media URL
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

This returns live sound metadata from Apify, including `song_id`, `clip_id`, and the count of `related_items` kept only as debug metadata.

## Configure the Novi resolver actor

The resolver input shape is fixed in code, including `limit: 20`, but the exact Novi actor id is supplied at runtime so agents can point the CLI at the validated actor without patching the repo again.

Use a one-off flag:

```bash
cargo run -- library sound import-tiktok-trending \
  --resolver-actor-id "<novi actor id>" \
  --limit 1
```

Or set an env var once:

```bash
export CAPCUT_CLI_TIKTOK_SOUND_RESOLVER_ACTOR_ID="<novi actor id>"
```

## Import trending TikTok sounds

```bash
cargo run -- library sound import-tiktok-trending \
  --resolver-actor-id "<novi actor id>" \
  --country "United States" \
  --period 7 \
  --limit 3 \
  --max-posts 20 \
  --download-attempts 5
```

What the importer does for each sound:

1. writes `trend.json` with the trend actor payload
2. writes `posts.json` with the raw Novi resolver dataset plus the exact resolver input profile
3. writes `selection.json` with normalized candidates ranked by likes
4. tries the top ranked candidates in order only until one exposes a usable direct video media URL
5. stores the winning `video.mp4`
6. extracts `audio.mp3` locally with `ffmpeg`
7. writes `download.json` and a summary `metadata.json`
8. merges the result into `library/sounds/manifest.json`

The ranking strategy is explicit and simple: `digg_count desc`, then resolver order.

`--max-posts` now caps the ranked candidates retained locally after the resolver returns its fixed `limit: 20` dataset.

## Output layout

Imported sounds are written under `library/sounds/imported/<slug>/`:

- `trend.json`
- `posts.json`
- `selection.json`
- `download.json`
- `metadata.json`
- `video.mp4`
- `audio.mp3`

The top-level manifest records provenance, the resolver actor id, the download method, and local paths for both media files and the step metadata.

## CLI surface

```bash
cargo run -- auth
cargo run -- auth --apify <token>
cargo run -- auth --from-env
cargo run -- discover tiktok-sounds --limit 10
cargo run -- discover x-clips --query "ai agents" --limit 10
cargo run -- library plan sound
cargo run -- library sound import-tiktok-trending --resolver-actor-id "<novi actor id>" --limit 3 --max-posts 20 --download-attempts 5
cargo run -- compose --sound sound_123 --clip clip_a --clip clip_b --duration-seconds 30
```

## Status of X support

`discover x-clips` is still a planning stub. The TikTok import path is the only live end-to-end ingestion pipeline in the repo today.

## Rights

Downloaded media should be treated as research and prototyping assets unless rights have been separately verified. The importer records provenance and rights notes so the local library does not lose source context.
