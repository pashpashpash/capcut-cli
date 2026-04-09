# TikTok sounds pipeline - first deliverable

## Goal

Establish a repeatable path for building a committed library of popular TikTok sounds, with a pipeline for adding more over time.

## Deliverable definition

The first deliverable is not a full autonomous trend harvester yet.
It is:

1. a sound library structure committed to the repo
2. a manifest format for sound metadata
3. at least one committed sample that can be shown in Discord for feedback
4. a documented pipeline for growing the library

## Acquisition strategy ladder

### Stage 1: manual seed

Use manually sourced sound URLs or sound identifiers to populate the manifest and commit representative audio previews.

Why:
- unblocks the library shape immediately
- gives the community something concrete to react to
- avoids pretending the automated pipeline is already solved

### Stage 2: provider adapters

Add provider-backed discovery/import adapters for trending sounds.

Candidate paths:
- managed unofficial TikTok APIs
- trend-specific third-party services
- browser-automation-backed scraping adapters

Design rule:
- each provider normalizes into the same local metadata schema
- no provider-specific weirdness should leak into the library interface

### Stage 3: ranking and refresh

Build commands that:
- fetch candidate trending sounds
- score them
- detect duplicates
- commit new metadata entries and optional previews

## Repo primitives we need

### Library files

- `library/sounds/manifest.json`
- committed sample audio under `library/sounds/samples/`

### Future CLI surface

- `capcut-cli sounds list`
- `capcut-cli sounds add --source <provider-or-url>`
- `capcut-cli sounds seed --file <metadata.json>`
- `capcut-cli sounds fetch tiktok-trending --provider <name> --limit <n>`

## Selection heuristics for popular sounds

When proper trend data is available, track:
- reuse count across videos
- growth over recent intervals
- presence across multiple creators
- recency
- genre/topic fit for the intended clip domain

## Immediate next steps

- commit library structure
- commit a seed preview sound
- show the sample in Discord and ask for feedback on library direction
- replace placeholder seeds with real TikTok-derived sounds as soon as the acquisition path stabilizes
