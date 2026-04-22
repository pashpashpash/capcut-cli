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
capcut-cli library sound judge --manifest library/sounds/manifest.json --platform tiktok --require-reason "downloaded candidate" --min-score 75 --max-trend-rank 25 --min-downloaded-videos 2 --min-extracted-audios 2 --min-representative-engagement-metrics 2 --min-representative-views 1000000 --min-representative-likes 100000 --exclude-risk "Rights still need" --max-risk-count 1 --top 3
```

The command should be offline and deterministic. It should read the committed manifest plus per-sound metadata and output JSON with:

- `sound_id`
- `trend_rank`
- `title`
- `author`
- `platform`
- `downloaded_video_count`
- `extracted_audio_count`
- `representative_view_count`
- `representative_like_count`
- `representative_comment_count`
- `representative_share_count`
- `score`
- `reasons`
- `risks`
- `recommended_action`

This makes the next trust pass concrete: "given 20 imported sounds, surface the best 3 and explain why." That is the judgement layer the Discord discussion keeps orbiting around, and it gives agents a better target than blindly importing more assets.

Implemented refinements:

- the judgement command now supports `--min-score`, repeated `--recommended-action`, and `--top`, so an agent can ask for a shortlist directly without post-processing the JSON dump
- filtered judgement reports keep an overall summary with recommendation-action and score-band counts, so the shortlist does not hide the shape of the full local library
- `--min-downloaded-videos` and `--min-extracted-audios` let the shortlist require enough usable local material for editing, which keeps "viral but unusable" sounds out of production-oriented passes
- `--min-representative-views` and `--min-representative-likes` let the shortlist require direct engagement evidence instead of trusting chart rank alone
- judgement reports echo the applied `filters` next to `total_count`, `judged_count`, and `summary`, so zero-result shortlists remain explainable without reconstructing CLI flags from shell history
- `filtered_out_count` makes zero-result or narrow-result reports explicit about how many sounds were removed by the filters
- `filtered_summary` summarizes the returned shortlist separately from the full-library `summary`, which makes narrow passes easier to inspect without re-counting rows
- `sort_order` is echoed in judgement reports so agents know returned rows are ordered by score descending, trend rank ascending, then sound id ascending
- returned sounds include `judgement_rank` from the full sorted library, so filtered shortlists can still show where each candidate ranked before filters were applied
- returned sounds include `representative_engagement_metric_count` so agents can quickly tell whether viral-confidence metrics are present before reading each metric field
- `--min-representative-engagement-metrics` filters returned sounds by how many representative engagement fields are present, letting agents require broad metric coverage before trusting a viral shortlist
- returned sounds include `reason_count` so agents can inspect positive-signal density directly instead of counting the `reasons` array
- returned sounds include `risk_count` so agents can inspect risk density directly instead of counting the `risks` array after using risk filters
- repeated `--exclude-risk` filters remove sounds whose risk text contains a matching substring, allowing production-oriented passes to drop known blockers such as unresolved rights review
- `--max-risk-count` caps how many remaining risk notes a returned sound may carry, with `--max-risk-count 0` acting as a strict risk-free shortlist mode
- judgement summaries now include risk text counts, so risk-filtered runs can show both the returned shortlist risks and the full-library blocker distribution
- judgement summaries now include reason text counts, so agents can see which positive signals are actually driving a shortlist instead of reading every returned row
- judgement summaries now include engagement-metric coverage counts, so agents can see whether the library has broad representative views/likes/comments/shares coverage before trusting a viral shortlist
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
