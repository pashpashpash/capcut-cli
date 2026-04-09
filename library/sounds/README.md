# Sound library

This directory holds committed sound metadata plus imported TikTok trend assets.

## Goals

- keep a growing in-repo sound library
- store normalized metadata for every sound import
- preserve a representative post plus the wider downloaded asset bag per sound
- keep the pipeline inspectable through committed schemas and step artifacts

## Structure

- `manifest.json` — top-level library index
- `imported/` — per-sound directories created by the TikTok Apify importer
- `samples/` — committed seed assets for repo scaffolding

Each imported sound directory should include:

- `trend.json` — raw trending sound record plus trend actor run metadata
- `posts.json` — raw Novi resolver dataset plus the exact resolver input profile
- `selection.json` — normalized candidate posts ranked by like count
- `download.json` — per-candidate direct-media download results plus local file paths
- `metadata.json` — final imported sound summary, provenance, rights note, representative media paths, and the downloaded asset inventory
- `videos/`
- `audios/`

## Metadata expectations

Each sound entry should track:

- stable local id
- source platform
- trend/source URL
- representative source video URL
- title or inferred label
- creator/uploader when known
- duration
- local video path
- local audio path
- local videos directory
- local audios directory
- local metadata path
- per-step metadata paths when present
- acquisition method / provenance
- rights/provenance note
- representative engagement metrics when present
- resolver actor id, download method, and asset counts when present
