---
name: apify
description: Use when agents working in this repo need to run live Apify-backed TikTok discovery/import flows safely, without committing secrets. Covers local secret loading, actor usage, and end-to-end smoke tests.
---

# Apify for capcut-cli agents

Use this skill when you need to run live Apify actors from inside `capcut-cli` and keep the API key out of git.

## Secret handling

Preferred repo-local secret path:

```bash
mkdir -p .secrets
printf '%s' "$APIFY_API_TOKEN" > .secrets/apify_api_token
chmod 600 .secrets/apify_api_token
```

This path is git-ignored.

Token lookup order in the CLI:

1. `CAPCUT_CLI_APIFY_TOKEN`
2. `.secrets/apify_api_token`
3. `~/.config/capcut-cli/config.json`

Use `.secrets/apify_api_token` for agent work in this repo so end-to-end tests can run without rewriting user-global config.

## Resolver actor

Set the validated Novi resolver actor id once per shell:

```bash
export CAPCUT_CLI_TIKTOK_SOUND_RESOLVER_ACTOR_ID="nCNiU9QG1e0nMwgWj"
```

Validated resolver input profile used by the repo:

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

Defaults reflect the current product decision:
- US region
- last month
- top-liked bias via `sortType: 1`
- choose the top-liked returned post

## Common examples

### Check auth resolution

```bash
cargo run -- auth
```

### Persist token to user config if explicitly desired

```bash
cargo run -- auth --apify "$APIFY_API_TOKEN"
```

### Discover trending US TikTok sounds

```bash
cargo run -- discover tiktok-sounds --country "United States" --period 7 --limit 5
```

### Run a real end-to-end import

```bash
export CAPCUT_CLI_TIKTOK_SOUND_RESOLVER_ACTOR_ID="nCNiU9QG1e0nMwgWj"
cargo run -- library sound import-tiktok-trending \
  --country "United States" \
  --period 7 \
  --limit 1 \
  --max-posts 20 \
  --download-attempts 5
```

### One-off import with explicit resolver actor id

```bash
cargo run -- library sound import-tiktok-trending \
  --resolver-actor-id "nCNiU9QG1e0nMwgWj" \
  --country "United States" \
  --period 7 \
  --limit 1 \
  --max-posts 20 \
  --download-attempts 5
```

### Manual proof-of-concept flow with raw API calls

1. Discover a trending US sound with `alien_force~tiktok-trending-sounds-tracker`
2. Feed its `link` into the Novi resolver with the fixed `MUSIC` input profile
3. Pick the top-liked post from the returned dataset
4. Verify the returned public post URL and audio/video media URLs

## Expected artifacts after a successful import

Under `library/sounds/imported/<slug>/`:
- `trend.json`
- `posts.json`
- `selection.json`
- `download.json`
- `metadata.json`
- `video.mp4` if direct video retrieval succeeds
- `audio.mp3` or equivalent extracted/normalized audio file

## Rules for agents

- Never commit `.secrets/`
- Never paste the token into tracked files, commits, issues, or PR text
- Prefer running real smoke tests over claiming success from unit tests alone
- If Apify or DNS fails, report the exact blocker instead of faking completion
- Keep raw actor payloads in step artifacts so the pipeline stays inspectable
