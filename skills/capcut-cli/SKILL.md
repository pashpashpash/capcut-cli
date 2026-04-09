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

This uses `alien_force~tiktok-trending-sounds-tracker` and returns JSON with `song_id`, `clip_id`, and `related_items` counts kept only as debug metadata.

## Resolver actor

The importer expects the exact validated Novi actor id at runtime. Supply it either with a flag:

```bash
cargo run -- library sound import-tiktok-trending \
  --resolver-actor-id "<novi actor id>" \
  --limit 1
```

Or through the env var:

```bash
export CAPCUT_CLI_TIKTOK_SOUND_RESOLVER_ACTOR_ID="<novi actor id>"
```

The resolver input profile is fixed in the repo:

```json
{
  "type": "MUSIC",
  "url": "<sound url>",
  "region": "US",
  "limit": 20,
  "isUnlimited": false,
  "publishTime": "MONTH",
  "sortType": 1,
  "isDownloadVideo": false,
  "isDownloadVideoCover": false
}
```

## End-to-end import

```bash
cargo run -- library sound import-tiktok-trending \
  --resolver-actor-id "<novi actor id>" \
  --country "United States" \
  --period 7 \
  --limit 1 \
  --max-posts 20 \
  --download-attempts 5
```

The importer uses this actor chain:

1. `alien_force~tiktok-trending-sounds-tracker`
2. the validated Novi sound/music resolver actor passed to `--resolver-actor-id`

Selection behavior:

- rank candidates by `digg_count` descending
- use resolver order as the only tie-breaker
- keep `related_items` only as debug metadata
- try the next ranked candidate only if the preferred row does not expose a usable direct video media URL

`--max-posts` caps the ranked candidates retained locally after the resolver returns its fixed `limit: 20` dataset.

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
