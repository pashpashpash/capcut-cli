# capcut-cli

An open source, agent-first Rust CLI for discovering, importing, and composing short-form social video assets.

## What works now

The main working path is a TikTok trending sound ingestion pipeline built around Apify:

1. discover trending sounds with `alien_force~tiktok-trending-sounds-tracker`
2. resolve posts for each sound URL with a validated Novi actor using the repo's fixed `MUSIC` input profile
3. normalize the resolver dataset and rank candidates by `digg_count` descending
4. keep one representative post for manifest/reporting while retaining the full ranked post set on disk
5. download every usable resolved video directly from the resolver output
6. extract audio locally with `ffmpeg` for every downloaded video when possible
7. persist multi-asset video/audio bags, per-step metadata, and manifest entries in-repo

The CLI stays machine-readable throughout, so agents can inspect every step without scraping terminal text.

## Prerequisites

- Rust toolchain
- `ffmpeg` on `PATH`
- an Apify token

## Local secret handling for agents

For repo-local end-to-end testing, put the raw Apify token in a git-ignored file:

```bash
mkdir -p .secrets
printf '%s' "$APIFY_API_TOKEN" > .secrets/apify_api_token
chmod 600 .secrets/apify_api_token
```

Token lookup order at runtime:

1. `CAPCUT_CLI_APIFY_TOKEN`
2. `.secrets/apify_api_token`
3. `~/.config/capcut-cli/config.json`

This lets coding agents run real imports from inside the repo without ever committing the key.

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
4. attempts every ranked candidate retained by `--max-posts` and keeps every successfully downloaded video asset
5. extracts audio into a matching file under `audios/` for each downloaded video when extraction succeeds
6. keeps a representative asset pair for manifest/report compatibility
7. writes `download.json` and a summary `metadata.json`
8. merges the result into `library/sounds/manifest.json`

The ranking strategy is explicit and simple: `digg_count desc`, then resolver order.

`--max-posts` caps the ranked candidates retained locally and attempted for download after the resolver returns its fixed `limit: 20` dataset.

`--download-attempts` is now the per-candidate retry budget for direct media download, not a cap on how many ranked posts are attempted.

## Judge imported sounds

Run an offline judgement pass over the local sound manifest:

```bash
cargo run -- library sound judge
```

Surface only the strongest candidates:

```bash
cargo run -- library sound judge --platform tiktok --require-reason "downloaded candidate" --min-score 75 --max-trend-rank 25 --min-downloaded-videos 2 --min-extracted-audios 2 --min-representative-engagement-metrics 2 --require-engagement-metric-field representative_view_count --require-engagement-metric-field representative_like_count --min-representative-views 1000000 --min-representative-likes 100000 --recommended-action shortlist_after_rights_review --exclude-risk "Rights still need" --max-risk-count 1 --top 3
```

The report scores each sound using recorded trend rank, downloaded/extracted asset coverage, representative engagement metrics when present, and provenance/rights risks. This is the deterministic "what deserves attention, and why?" pass before importing or composing more assets.

Filtered reports keep `total_count`, `judged_count`, `filtered_out_count`, echo the applied `filters`, and include both full-library `summary` and returned-row `filtered_summary` counts, so agents can see the whole library distribution while only receiving the shortlist rows they asked for. Reports also echo `sort_order` so callers know returned sounds are ordered by score descending, then trend rank ascending, then sound id ascending; each returned sound includes `judgement_rank` from that full-library ordering, `representative_engagement_metric_count`, present/missing representative engagement metric field names, plus `reason_count` and `risk_count` for direct inspection. The summaries count recommendation actions, platforms, score bands, engagement-metric coverage, missing engagement metric fields, reason text distribution, and risk text distribution. Use repeated `--platform` values when a pass should only consider specific providers such as TikTok. Use repeated `--require-reason` substrings when every returned sound must contain specific positive evidence. Use `--max-trend-rank` when a pass should only consider ranked chart positions. Use `--min-downloaded-videos` and `--min-extracted-audios` when a candidate needs enough usable local material for editing, not just a high score. Use `--min-representative-engagement-metrics` when a pass should require coverage across the representative views, likes, comments, and shares fields before trusting virality, and repeated `--require-engagement-metric-field` values when specific fields such as `representative_view_count` and `representative_like_count` must be present. Use `--min-representative-views` and `--min-representative-likes` when the shortlist should require direct engagement evidence. Use repeated `--exclude-risk` substrings to drop candidates with known blockers such as rights-review risk, and `--max-risk-count 0` when a pass should only return risk-free rows.

## Output layout

Imported sounds are written under `library/sounds/imported/<slug>/`:

- `trend.json`
- `posts.json`
- `selection.json`
- `download.json`
- `metadata.json`
- `videos/`
- `audios/`

The top-level manifest records provenance, the resolver actor id, the download method, representative media paths, multi-asset directory paths, and downloaded/extracted asset counts.

## Install and update from GitHub releases

Install the latest release archive for the current machine:

```bash
./scripts/install-from-github-release.sh
```

Override the target install directory if needed:

```bash
./scripts/install-from-github-release.sh --bin-dir "$HOME/.local/bin"
```

Once the CLI is installed, update it in place:

```bash
capcut-cli update
```

When `capcut-cli update` is run from a repo build such as `cargo run`, it defaults to installing `~/.local/bin/capcut-cli`. Override that explicitly if you want a different target path:

```bash
cargo run -- update --bin-path "$HOME/.local/bin/capcut-cli"
```

## Release automation

Release automation is split into three GitHub Actions workflows:

- `.github/workflows/release-please.yml` updates release PRs and tags/releases from pushes to `main`
- `.github/workflows/build-release-binaries.yml` builds release archives on pushes to `main`
- `.github/workflows/publish-release-assets.yml` attaches packaged archives to published GitHub releases

Release archive names match the update/install path exactly: `capcut-cli-<target>.tar.gz`, with the `capcut-cli` binary at archive root.

## CLI surface

```bash
cargo run -- auth
cargo run -- auth --apify <token>
cargo run -- auth --from-env
cargo run -- discover tiktok-sounds --limit 10
cargo run -- discover x-clips --query "ai agents" --limit 10
cargo run -- library plan sound
cargo run -- library sound import-tiktok-trending --resolver-actor-id "<novi actor id>" --limit 3 --max-posts 20 --download-attempts 5
cargo run -- library sound judge --top 3 --platform tiktok --require-reason "downloaded candidate" --min-score 75 --max-trend-rank 25
cargo run -- update --bin-path "$HOME/.local/bin/capcut-cli"
cargo run -- compose --sound sound_123 --clip clip_a --clip clip_b --duration-seconds 30
```

## Status of X support

`discover x-clips` is still a planning stub. The TikTok import path is the only live end-to-end ingestion pipeline in the repo today.

## Rights

Downloaded media should be treated as research and prototyping assets unless rights have been separately verified. The importer records provenance and rights notes so the local library does not lose source context.

## Repo skills

- `skills/capcut-cli/SKILL.md` for operating the CLI
- `skills/apify/SKILL.md` for safe live Apify usage, local secret handling, and end-to-end smoke-test examples
