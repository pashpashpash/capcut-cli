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
capcut-cli library sound judge --manifest library/sounds/manifest.json --min-score 75 --top 3
```

The command should be offline and deterministic. It should read the committed manifest plus per-sound metadata and output JSON with:

- `sound_id`
- `trend_rank`
- `title`
- `author`
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

## Provider ladder

Keep the acquisition ladder explicit:

1. **Current default:** Apify trend actor plus Novi resolver.
2. **Validation layer:** repeated runs across countries/periods, then compare rank persistence.
3. **Rights layer:** mark whether a candidate appears in CML or otherwise needs manual rights review.
4. **Future provider adapter:** only add another provider if it improves coverage, reliability, cost, or rights metadata.

Do not add browser scraping until the local judgement/report command exists. More raw sounds are less useful than a smaller set with stable scoring and provenance.
