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
cargo run -- library sound judge --platform tiktok --country-code US --min-song-id-country-coverage 3 --min-song-id-best-representative-views 1000000 --min-song-id-best-representative-engagements 250000 --min-song-id-best-representative-engagement-rate-per-1000-views 100 --min-song-id-best-representative-share-rate-per-1000-views 25 --distinct-song-id --require-reason "downloaded candidate" --min-score 75 --min-reason-count 5 --max-judgement-rank 10 --max-trend-rank 25 --min-downloaded-videos 2 --min-extracted-audios 2 --min-usable-asset-pairs 2 --min-candidate-posts 10 --min-source-identifiers 6 --require-source-identifier-field source_video_url --require-source-identifier-field song_id --require-resolver-actor-id --require-download-method direct_http --require-provenance "downloaded directly" --exclude-provenance "manual fallback" --require-rights-note "Verify rights" --min-local-artifact-paths 6 --require-local-artifact-path-field local_posts_path --require-local-artifact-path-field local_selection_path --min-representative-engagement-metrics 2 --require-engagement-metric-field representative_view_count --require-engagement-metric-field representative_like_count --min-representative-views 1000000 --min-representative-likes 100000 --min-representative-engagements 250000 --min-representative-like-rate-per-1000-views 50 --min-representative-engagement-rate-per-1000-views 75 --min-representative-comments 10000 --min-representative-comment-rate-per-1000-views 5 --min-representative-shares 10000 --min-representative-share-rate-per-1000-views 25 --min-representative-music-duration-seconds 30 --representative-music-is-original-sound false --representative-music-is-batch-take-down-music false --representative-music-reviewed true --min-representative-music-fields 8 --require-representative-music-field representative_music_reviewed --require-representative-music-can-read --require-representative-music-can-reuse --require-representative-music-has-strong-beat-url --require-representative-music-vid --recommended-action shortlist_after_rights_review --exclude-risk "Rights still need" --max-risk-count 1 --top 3
```

The report scores each sound using recorded trend rank, downloaded/extracted asset coverage, representative engagement metrics when present, full-library `song_id` persistence across recorded trend markets, how many recorded markets that `song_id` held a top-25 chart position in, song-level best recorded chart rank across the library when a sound travels across markets, representative TikTok music metadata when present, and provenance/rights risks. High-scoring sounds that still carry rights-review blockers such as an explicit manual-rights note or TikTok's batch-takedown music flag stay on the `shortlist_after_rights_review` path instead of being marked `use_first`. This is the deterministic "what deserves attention, and why?" pass before importing or composing more assets.

Filtered reports keep `total_count`, `judged_count`, `filtered_out_count`, echo the applied `filters`, and include both full-library `summary` and returned-row `filtered_summary` counts, so agents can see the whole library distribution while only receiving the shortlist rows they asked for. Reports also echo `sort_order` so callers know returned sounds are ordered by score descending, then trend rank ascending, then sound id ascending; each returned sound includes `judgement_rank` from that full-library ordering, manifest repeatability/rights context (`provenance`, `rights_note`, `resolver_actor_id`, and `download_method`), source identifiers (`source_url`, `source_video_url`, `song_id`, `clip_id`, `country_code`, and `duration_seconds`) plus source identifier coverage (`source_identifier_count`, `source_identifier_fields`, and `missing_source_identifier_fields`), full-library song persistence via `song_id_country_coverage_count`, strong cross-market charting via `song_id_top_25_country_count`, song-level best library chart strength via `song_id_best_trend_rank`, song-level best observed scale via `song_id_best_representative_view_count`, song-level best observed depth via `song_id_best_representative_engagement_count`, song-level best observed engagement density via `song_id_best_representative_engagement_rate_per_1000_views`, song-level best observed spread density via `song_id_best_representative_share_rate_per_1000_views`, local artifact paths plus local artifact path coverage (`local_artifact_path_count`, `local_artifact_path_fields`, and `missing_local_artifact_path_fields`), `candidate_post_count`, `usable_asset_pair_count`, representative engagement metrics, representative music fields (`representative_music_duration_seconds`, `representative_music_can_read`, `representative_music_can_reuse`, `representative_music_is_original_sound`, `representative_music_commercial_right_type`, `representative_music_is_batch_take_down_music`, `representative_music_reviewed`, `representative_music_has_strong_beat_url`, and `representative_music_vid`) plus representative music field coverage (`representative_music_field_count`, `representative_music_fields`, and `missing_representative_music_fields`), `representative_engagement_metric_count`, present/missing representative engagement metric field names, plus `reason_count` and `risk_count` for direct inspection. When `--distinct-song-id` is set, repeated rows with the same non-empty `song_id` are collapsed after all other filters and before `--top`, so a shortlist can return one best-ranked representative row per song instead of several country-specific copies. The summaries count recommendation actions, platforms, country-code distribution, full-library `song_id` country-span counts, full-library `song_id` top-25-country counts, full-library `song_id` best-trend-rank bands, song-level best-representative-view bands, song-level best-representative-engagement bands, song-level best-representative-engagement-rate bands, song-level best-representative-share-rate bands, score bands, trend-rank bands, judgement-rank bands, duration-second bands, source-identifier coverage, present source identifier fields, resolver actor coverage, download-method distribution, provenance coverage, rights-note distribution, reason-count coverage, risk-count coverage, downloaded-video coverage, extracted-audio coverage, usable asset-pair coverage, candidate-post coverage, local artifact-path coverage, present local artifact path fields, engagement-metric coverage, present engagement metric fields, representative view-count bands, representative engagement-count bands, representative like-count bands, representative comment-count bands, representative share-count bands, representative like-rate bands, representative engagement-rate bands, representative comment-rate bands, representative share-rate bands, representative music duration bands, representative music field coverage, present representative music fields, representative music read/reuse/original-sound counts, representative music commercial-right-type counts, representative music batch-takedown counts, representative music reviewed counts, representative music strong-beat counts, representative music `music_vid` coverage, missing source identifier fields, missing local artifact path fields, missing engagement metric fields, missing representative music fields, reason text distribution, and risk text distribution. Use repeated `--platform` values when a pass should only consider specific providers such as TikTok, and repeated `--country-code` values when it should focus on one or more trend markets such as `US`. Use `--min-song-id-country-coverage` when the shortlist should only keep sounds whose `song_id` already appears across several recorded trend markets in the full judged library, even if the current pass is narrowed to one country. That same cross-country persistence now contributes a direct judge-score bonus and reason, so sounds that travel across markets rise in the full-library ranking before filters are applied. Use `--distinct-song-id` when the shortlist should collapse those repeated cross-market rows back down to one representative row per non-empty `song_id` after filtering, while still leaving rows with missing `song_id` visible as separate artifacts. Use `--min-song-id-best-representative-views` when a deduped shortlist should still require that a song reached a minimum observed view scale somewhere in the judged library, even if the kept representative row comes from a different market sample. Use `--min-song-id-best-representative-engagements` when a deduped shortlist should still require that a song reached a minimum observed engagement depth somewhere in the judged library, even if the kept representative row comes from a different market sample. Use `--min-song-id-best-representative-engagement-rate-per-1000-views` when a deduped shortlist should still require that a song reached a minimum observed engagement density somewhere in the judged library, even if the kept representative row comes from a weaker market sample. Use `--min-song-id-best-representative-share-rate-per-1000-views` when a deduped shortlist should still require that a song reached a minimum observed spread density somewhere in the judged library, even if the kept representative row comes from a weaker market sample. Use `--min-song-id-top-25-country-count` when the shortlist should require that a `song_id` held a top-25 chart position in several recorded markets, and that strong cross-market charting now contributes its own virality reason and score bonus before filters run. Use `--max-song-id-best-trend-rank` when the shortlist should keep only songs whose strongest recorded chart position anywhere in the judged library still lands inside a chosen rank cutoff, and that multi-market best-rank signal now contributes an extra virality reason and modest score bonus when a `song_id` spans multiple markets. Use repeated `--require-reason` substrings when every returned sound must contain specific positive evidence, and use `--min-reason-count` when the shortlist should require enough positive evidence before returning a sound. Use `--max-judgement-rank` when a filtered pass should stay inside the full-library top N by score, trend rank, and sound id; use `--max-trend-rank` when a pass should only consider ranked chart positions. Use `--min-duration-seconds` and `--max-duration-seconds` when a pass needs sounds long or short enough for the target edit. Use `--min-source-identifiers` and repeated `--require-source-identifier-field` values when a pass needs traceable TikTok identifiers such as `source_video_url`, `song_id`, or `clip_id`. Use `--require-resolver-actor-id`, repeated `--require-download-method` values, repeated `--require-provenance` substrings, and repeated `--exclude-provenance` substrings when a pass needs repeatable resolver provenance and a specific asset acquisition path such as `direct_http`. Use repeated `--require-rights-note` and `--exclude-rights-note` substrings to keep or drop rows with manifest-level rights caveats before opening them. Use `--min-downloaded-videos`, `--min-extracted-audios`, `--min-usable-asset-pairs`, and `--min-candidate-posts` when a candidate needs enough usable local material, complete video/audio pairs, and sampled resolver posts for editing, not just a high score. Use `--min-local-artifact-paths` and repeated `--require-local-artifact-path-field` values when a pass needs retained audit/editing files such as `local_posts_path` or `local_selection_path` before opening a sound. Use `--min-representative-engagement-metrics` when a pass should require coverage across the representative views, likes, comments, and shares fields before trusting virality, and repeated `--require-engagement-metric-field` values when specific fields such as `representative_view_count` and `representative_like_count` must be present. Use `--min-representative-views`, `--min-representative-likes`, `--min-representative-engagements`, `--min-representative-like-rate-per-1000-views`, `--min-representative-engagement-rate-per-1000-views`, `--min-representative-comments`, `--min-representative-comment-rate-per-1000-views`, `--min-representative-shares`, and `--min-representative-share-rate-per-1000-views` when the shortlist should require direct reach, raw like volume, raw engagement volume, like density, total engagement density, raw discussion, discussion density, raw spread, and spread density evidence. Use `--min-representative-music-duration-seconds` and `--max-representative-music-duration-seconds` when the shortlist should keep sounds within a usable full-song length band, use `--representative-music-is-original-sound true|false` when the shortlist should explicitly keep creator-original audio or explicitly favor reusable catalog sounds, use `--representative-music-commercial-right-type <n>` when the shortlist should keep only rows carrying a specific raw TikTok `music.commercial_right_type` value from resolver data, use `--representative-music-is-batch-take-down-music true|false` when the shortlist should explicitly keep or drop rows carrying TikTok's raw batch-takedown flag from resolver data, use `--representative-music-reviewed true|false` when the shortlist should keep only rows carrying TikTok's raw `music.extra.reviewed` flag recovered from resolver artifacts, use `--min-representative-music-fields` when the shortlist should require enough representative music context to trust downstream operational judgments, use repeated `--require-representative-music-field` values when specific representative music fields such as `representative_music_reviewed` or `representative_music_vid` must be present, and use `--require-representative-music-can-read`, `--require-representative-music-can-reuse`, `--require-representative-music-has-strong-beat-url`, and `--require-representative-music-vid` when the shortlist should insist on reusable/readable music metadata, beat-analysis availability, and a stable TikTok music identifier before trusting a candidate as operationally usable. Use repeated `--exclude-risk` substrings to drop candidates with known blockers such as rights-review risk, and `--max-risk-count 0` when a pass should only return risk-free rows.

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
cargo run -- library sound judge --top 3 --distinct-song-id --platform tiktok --require-reason "downloaded candidate" --min-score 75 --min-reason-count 5 --max-judgement-rank 10 --max-trend-rank 25
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
