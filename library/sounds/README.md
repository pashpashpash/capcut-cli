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
