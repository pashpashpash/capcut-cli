# Sound library

This directory holds committed sound metadata plus imported TikTok trend assets.

## Goals

- keep a growing in-repo sound library
- store normalized metadata for every sound import
- preserve both the representative source video and the extracted audio
- keep the pipeline inspectable through committed schemas and step artifacts

## Structure

- `manifest.json` — top-level library index
- `imported/` — per-sound directories created by the TikTok Apify importer
- `samples/` — committed seed assets for repo scaffolding

Each imported sound directory should include:

- `trend.json` — raw trending sound record plus trend actor run metadata
- `posts.json` — raw Novi resolver dataset plus the exact resolver input profile
- `selection.json` — normalized candidate posts ranked by like count
- `download.json` — direct-media download attempts and the winning media URL
- `metadata.json` — final imported sound summary, provenance, rights note, and local file paths
- `video.mp4`
- `audio.mp3`

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
- local metadata path
- per-step metadata paths when present
- acquisition method / provenance
- rights/provenance note
- representative engagement metrics when present
- resolver actor id and download method when present
