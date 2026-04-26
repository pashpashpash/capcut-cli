# Programmatic TikTok sounds update - 2026-04-22

## Question

How do we get viral TikTok sounds programmatically, repeatably, and with enough provenance that agents can decide what is worth using?

## Current answer

The repo's current provider chain is still the strongest working path:

1. run `alien_force~tiktok-trending-sounds-tracker` for country/period/rank/song identifiers
2. resolve each sound URL with the validated Novi `MUSIC` actor profile
3. rank resolver posts by `digg_count desc, resolver order asc`
4. download every usable candidate video, extract audio locally, and keep all artifacts
5. make a human/agent judgement pass over the retained candidate set before production use

The important refinement is that "viral sound" should not mean "ranked once in one country." Treat it as a scored observation:

- current chart rank
- persistence across runs
- appearance across multiple countries
- top resolver-post engagement
- number of usable downloadable candidates
- whether the sound is commercially usable for the intended posting context

## Source read

- TikTok's public developer surfaces still do not expose a clean organic "popular songs / trending sounds" API for this use case.
- TikTok Commercial Content API is for ad/commercial-content transparency search, not general organic sound discovery.
- TikTok Commercial Music Library is relevant for rights and business-account usability, but it is presented as a product/library workflow rather than a general viral-sound API.
- Apify trend actors remain the practical provider boundary for discovery. They return the fields this repo already needs: rank, title, author, song id, clip id, duration, country, link, and related items.
- Third-party trend writeups are useful as validation signals. A March 2026 cross-country analysis based on TikTok Creative Center data framed the right idea: sounds that chart across several national markets are stronger viral candidates than one-off local spikes.

References:

- https://apify.com/alien_force/tiktok-trending-sounds-tracker
- https://apify.com/burbn/tiktok-trending-sounds
- https://developers.tiktok.com/products/commercial-content-api
- https://ads.tiktok.com/help/article/how-to-use-the-commercial-music-library
- https://www.tiktok.com/legal/page/global/commercial-music-library-user-terms/en
- https://smmnut.com/blog/trending-tiktok-sounds-march-2026/

## Next implementation target

Add a local judgement/report command over imported sounds before adding more network providers:

```bash
capcut-cli library sound judge --manifest library/sounds/manifest.json
capcut-cli library sound judge --manifest library/sounds/manifest.json --platform tiktok --require-reason "downloaded candidate" --min-score 75 --min-reason-count 5 --max-judgement-rank 10 --max-trend-rank 25 --min-downloaded-videos 2 --min-extracted-audios 2 --min-candidate-posts 10 --min-representative-engagement-metrics 2 --require-engagement-metric-field representative_view_count --require-engagement-metric-field representative_like_count --min-representative-views 1000000 --min-representative-likes 100000 --min-representative-engagements 250000 --min-representative-like-rate-per-1000-views 50 --min-representative-engagement-rate-per-1000-views 75 --min-representative-comments 10000 --min-representative-comment-rate-per-1000-views 5 --min-representative-shares 10000 --min-representative-share-rate-per-1000-views 25 --exclude-risk "Rights still need" --max-risk-count 1 --top 3
```

The command should be offline and deterministic. It should read the committed manifest plus per-sound metadata and output JSON with:

- `sound_id`
- `trend_rank`
- `title`
- `author`
- `platform`
- `source_url`
- `source_video_url`
- `song_id`
- `clip_id`
- `country_code`
- `duration_seconds`
- `local_audio_path`
- `local_video_path`
- `local_metadata_path`
- `local_trend_path`
- `local_posts_path`
- `local_selection_path`
- `local_download_path`
- `local_artifact_path_count`
- `local_artifact_path_fields`
- `missing_local_artifact_path_fields`
- `downloaded_video_count`
- `extracted_audio_count`
- `candidate_post_count`
- `representative_view_count`
- `representative_like_count`
- `representative_engagement_count`
- `representative_like_rate_per_1000_views`
- `representative_engagement_rate_per_1000_views`
- `representative_comment_count`
- `representative_comment_rate_per_1000_views`
- `representative_share_count`
- `representative_share_rate_per_1000_views`
- `score`
- `reasons`
- `risks`
- `recommended_action`

This makes the next trust pass concrete: "given 20 imported sounds, surface the best 3 and explain why." That is the judgement layer the Discord discussion keeps orbiting around, and it gives agents a better target than blindly importing more assets.

Implemented refinements:

- the judgement command now supports `--min-score`, repeated `--recommended-action`, and `--top`, so an agent can ask for a shortlist directly without post-processing the JSON dump
- filtered judgement reports keep an overall summary with recommendation-action and score-band counts, so the shortlist does not hide the shape of the full local library
- `--min-downloaded-videos` and `--min-extracted-audios` let the shortlist require enough usable local material for editing, which keeps "viral but unusable" sounds out of production-oriented passes
- returned sounds include `candidate_post_count`, and `--min-candidate-posts` lets shortlists require enough resolver sample depth before trusting a representative post
- judgement summaries now include `candidate_post_coverage_counts`, so agents can see resolver sample-depth distribution before trusting a filtered viral shortlist
- judgement summaries now include `downloaded_video_coverage_counts` and `extracted_audio_coverage_counts`, so agents can see local asset readiness distribution before opening individual rows
- returned sounds include `usable_asset_pair_count`, and judgement summaries include `usable_asset_pair_coverage_counts`, so agents can see how many complete video/audio pairs are actually ready
- `--min-usable-asset-pairs` lets production shortlists require complete downloaded-video/extracted-audio pairs instead of checking both raw asset counts manually
- judgement summaries now include `reason_count_coverage_counts` and `risk_count_coverage_counts`, so agents can compare signal and blocker density before opening individual rows
- judgement summaries now include `trend_rank_band_counts`, so agents can compare top-10, mid-chart, long-tail, and unranked sound distribution before opening individual rows
- judgement summaries now include `judgement_rank_band_counts`, so agents can compare full-library top-N, mid-rank, long-tail, and unranked distribution before opening individual rows
- `--min-duration-seconds` and `--max-duration-seconds` let production shortlists require sounds that fit the target edit length before opening individual rows
- judgement summaries now include `duration_seconds_band_counts`, so agents can see sound-length distribution before choosing duration filters
- judgement summaries now include source identifier coverage and missing source identifier field counts, so agents can see whether shortlisted sounds are traceable before opening individual rows
- `--min-source-identifiers` and repeated `--require-source-identifier-field` filters let shortlists require traceable TikTok identifiers such as `source_video_url`, `song_id`, or `clip_id`
- `--min-reason-count` lets shortlists require enough positive evidence before returning a sound, complementing `--max-risk-count` for blocker density
- `--max-judgement-rank` keeps filtered passes inside the full-library top N by score, trend rank, and sound id, so narrow filters do not accidentally surface low-ranked candidates
- returned sounds include source URLs and TikTok identifiers (`source_url`, `source_video_url`, `song_id`, `clip_id`, `country_code`, and `duration_seconds`), so shortlisted rows remain traceable without reopening the manifest
- returned sounds include source identifier coverage (`source_identifier_count`, `source_identifier_fields`, and `missing_source_identifier_fields`), so agents can see traceability completeness without comparing every nullable identifier by hand
- judgement summaries now include `country_code_counts`, and repeated `--country-code` filters let shortlists focus on one or more trend markets before comparing cross-country viral persistence
- returned sounds include manifest repeatability and rights context (`provenance`, `rights_note`, `resolver_actor_id`, and `download_method`), so agents can inspect source chain and production caveats without reopening the manifest
- `--require-resolver-actor-id` and repeated `--require-download-method` filters let shortlists require repeatable resolver provenance and specific asset acquisition methods such as `direct_http`
- repeated `--require-provenance`, `--exclude-provenance`, `--require-rights-note`, and `--exclude-rights-note` filters let shortlists require or drop specific source-chain and rights-note text before opening rows
- judgement summaries now include resolver actor coverage and download method counts, so agents can see whether the library is repeatable/direct-download backed before opening individual rows
- judgement summaries now include provenance coverage and rights-note counts, so agents can compare source-chain completeness and production caveats before opening individual rows
- returned sounds include local artifact paths (`local_audio_path`, `local_video_path`, `local_metadata_path`, `local_trend_path`, `local_posts_path`, `local_selection_path`, and `local_download_path`), so follow-up editing and audit steps can jump straight to retained assets
- returned sounds include local artifact path coverage (`local_artifact_path_count`, `local_artifact_path_fields`, and `missing_local_artifact_path_fields`), so agents can tell which retained audit/editing files are recorded without comparing every nullable path
- judgement summaries now include `local_artifact_path_coverage_counts` and `missing_local_artifact_path_field_counts`, so agents can compare retained audit/editing file coverage before opening individual rows
- judgement summaries now include `local_artifact_path_field_counts`, so agents can see which retained audit/editing path fields are present across a library or filtered shortlist
- `--min-local-artifact-paths` and repeated `--require-local-artifact-path-field` filters let shortlists require retained audit/editing files such as `local_posts_path` and `local_selection_path`
- `--min-representative-views`, `--min-representative-likes`, `--min-representative-comments`, and `--min-representative-shares` let the shortlist require direct engagement, discussion, and spread evidence instead of trusting chart rank alone
- returned sounds include `representative_engagement_count`, and `--min-representative-engagements` lets shortlists require absolute likes, comments, and shares volume before trusting dense but tiny samples
- judgement summaries now include `representative_engagement_count_band_counts`, so agents can compare absolute engagement-volume distribution before opening individual rows
- judgement summaries now include `representative_like_count_band_counts`, so agents can compare absolute like-volume distribution before opening individual rows
- returned sounds include `representative_like_rate_per_1000_views`, and `--min-representative-like-rate-per-1000-views` lets viral shortlists require like density relative to reach instead of raw like counts alone
- judgement summaries now include `representative_like_rate_band_counts`, so agents can compare like-density distribution before opening individual rows
- returned sounds include `representative_engagement_rate_per_1000_views`, and `--min-representative-engagement-rate-per-1000-views` lets shortlists require total likes, comments, and shares density relative to reach
- judgement summaries now include `representative_engagement_rate_band_counts`, so agents can spot missing, weak, or high-density virality evidence before opening individual rows
- judgement summaries now include `representative_view_count_band_counts`, so agents can see reach distribution before opening individual rows
- returned sounds include `representative_comment_rate_per_1000_views`, and `--min-representative-comment-rate-per-1000-views` lets shortlists require discussion density relative to reach instead of raw comment counts alone
- judgement summaries now include `representative_comment_count_band_counts`, so agents can compare absolute discussion-volume distribution before opening individual rows
- judgement summaries now include `representative_comment_rate_band_counts`, so agents can compare discussion-density distribution before opening individual rows
- returned sounds include `representative_share_rate_per_1000_views`, and `--min-representative-share-rate-per-1000-views` lets shortlists require spread density relative to reach instead of raw share counts alone
- judgement summaries now include `representative_share_count_band_counts`, so agents can compare absolute spread-volume distribution before opening individual rows
- judgement summaries now include `representative_share_rate_band_counts`, so agents can compare spread-density distribution before opening individual rows
- judgement reports echo the applied `filters` next to `total_count`, `judged_count`, and `summary`, so zero-result shortlists remain explainable without reconstructing CLI flags from shell history
- `filtered_out_count` makes zero-result or narrow-result reports explicit about how many sounds were removed by the filters
- `filtered_summary` summarizes the returned shortlist separately from the full-library `summary`, which makes narrow passes easier to inspect without re-counting rows
- `sort_order` is echoed in judgement reports so agents know returned rows are ordered by score descending, trend rank ascending, then sound id ascending
- returned sounds include `judgement_rank` from the full sorted library, so filtered shortlists can still show where each candidate ranked before filters were applied
- returned sounds include `representative_engagement_metric_count` so agents can quickly tell whether viral-confidence metrics are present before reading each metric field
- returned sounds include present and missing representative engagement metric field names, so partial metric coverage is inspectable without comparing every nullable count by hand
- judgement summaries now include `representative_engagement_metric_field_counts`, so agents can see which viral-confidence fields are present across a library or filtered shortlist
- `--min-representative-engagement-metrics` filters returned sounds by how many representative engagement fields are present, letting agents require broad metric coverage before trusting a viral shortlist
- repeated `--require-engagement-metric-field` filters keep only sounds with specific representative engagement metric fields such as `representative_view_count` and `representative_like_count`
- returned sounds include representative music operational fields (`representative_music_duration_seconds`, `representative_music_can_read`, `representative_music_can_reuse`, `representative_music_is_original_sound`, `representative_music_commercial_right_type`, `representative_music_is_batch_take_down_music`, `representative_music_reviewed`, `representative_music_has_strong_beat_url`, and `representative_music_vid`), so shortlist passes can inspect whether a viral sound also looks reusable and machine-actionable
- `--min-representative-music-duration-seconds` and `--max-representative-music-duration-seconds` let shortlists require full-song duration bands that are practical for downstream editing instead of accepting any charted sound length
- `--representative-music-commercial-right-type <n>` lets shortlists keep only rows carrying a specific raw TikTok `music.commercial_right_type` value from resolver data, which is useful for rights-layer exploration without pretending the enum has been fully interpreted
- `--representative-music-is-batch-take-down-music true|false` lets shortlists explicitly keep or drop rows carrying TikTok's raw batch-takedown flag from resolver data, which is useful when a viral sound already looks like it may be in a takedown pipeline
- `--representative-music-reviewed true|false` lets shortlists keep only rows carrying TikTok's raw `music.extra.reviewed` flag, now recovered from saved resolver artifacts even when TikTok stores it as `0` or `1` instead of a JSON boolean
- returned sounds now include representative music field coverage (`representative_music_field_count`, `representative_music_fields`, and `missing_representative_music_fields`), so agents can see how complete the operational music context is without reopening raw resolver artifacts
- `--min-representative-music-fields` and repeated `--require-representative-music-field` filters let shortlists require enough representative music context, or insist on specific recovered fields such as `representative_music_reviewed` or `representative_music_vid`, before trusting a viral candidate as operationally usable
- returned sounds now include `song_id_country_coverage_count`, computed from the full judged library before filters, so each shortlisted row can show how many distinct recorded trend markets that `song_id` already spans
- `--min-song-id-country-coverage <n>` lets shortlists require cross-country persistence directly on each returned sound instead of treating the song-level country coverage summary as read-only context
- that same cross-country persistence now feeds the judge score directly, adding an explicit virality reason and score bonus when a `song_id` survives across multiple recorded trend markets before any shortlist filters run
- high-scoring rows that still carry that batch-takedown risk now stay on the `shortlist_after_rights_review` path even if the manifest itself does not already contain a manual rights-warning note
- `--require-representative-music-can-read`, `--require-representative-music-can-reuse`, `--require-representative-music-has-strong-beat-url`, and `--require-representative-music-vid` let shortlists insist on readable/reusable music metadata, beat-analysis availability, and a stable TikTok music identifier before trusting a viral candidate as operationally usable
- judgement summaries now include representative music duration bands, representative music field coverage, present/missing representative music field counts, representative music read/reuse/original-sound/commercial-right-type/batch-takedown/reviewed/strong-beat counts, and `music_vid` coverage, so agents can compare operational music metadata coverage across a library before opening individual rows
- the resolver normalizer now reads Novi-style `statistics.*` engagement metrics, so future imports preserve representative views, likes, comments, and shares for judgement
- the judgement pass can recover representative engagement metrics from the saved `local_posts_path` resolver artifact, so older imports with sparse manifest metadata can still be scored from retained raw data
- returned sounds include `reason_count` so agents can inspect positive-signal density directly instead of counting the `reasons` array
- returned sounds include `risk_count` so agents can inspect risk density directly instead of counting the `risks` array after using risk filters
- repeated `--exclude-risk` filters remove sounds whose risk text contains a matching substring, allowing production-oriented passes to drop known blockers such as unresolved rights review
- `--max-risk-count` caps how many remaining risk notes a returned sound may carry, with `--max-risk-count 0` acting as a strict risk-free shortlist mode
- judgement summaries now include risk text counts, so risk-filtered runs can show both the returned shortlist risks and the full-library blocker distribution
- judgement summaries now include reason text counts, so agents can see which positive signals are actually driving a shortlist instead of reading every returned row
- judgement summaries now include engagement-metric coverage counts, so agents can see whether the library has broad representative views/likes/comments/shares coverage before trusting a viral shortlist
- judgement summaries now include missing engagement metric field counts, so agents can see which representative engagement metrics the resolver/library most often lacks
- `--max-trend-rank` filters the judgement report to sounds with recorded chart positions at or above a rank cutoff, making viral-rank passes explicit instead of relying only on score side effects
- repeated `--platform` filters restrict judgement reports to specific providers such as TikTok, and summary counts now expose the full platform distribution
- repeated `--require-reason` filters keep only sounds whose reason text contains every requested positive-evidence substring, so agents can require specific support such as downloaded assets or platform provenance

## Provider ladder

Keep the acquisition ladder explicit:

1. **Current default:** Apify trend actor plus Novi resolver.
2. **Validation layer:** repeated runs across countries/periods, then compare rank persistence.
3. **Rights layer:** mark whether a candidate appears in CML or otherwise needs manual rights review.
4. **Future provider adapter:** only add another provider if it improves coverage, reliability, cost, or rights metadata.

Do not add browser scraping until the local judgement/report command exists. More raw sounds are less useful than a smaller set with stable scoring and provenance.
