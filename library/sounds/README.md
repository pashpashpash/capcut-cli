# Sound library

This directory holds committed TikTok sound metadata and selected audio samples for the first deliverable.

## Goals

- keep a growing library of popular sounds in-repo
- store normalized metadata for every sound
- keep at least a small committed sample set so the pipeline is inspectable
- make it easy for an agent to add more sounds over time

## Structure

- `manifest.json` — top-level library index
- `seed/` — manually curated or initially imported sounds
- `samples/` — committed audio files that can be previewed and shared for feedback

## Metadata expectations

Each sound entry should track:

- stable local id
- source platform
- source URL or source identifier
- title or inferred label
- creator/uploader when known
- duration
- local committed path if present
- acquisition method
- rights/provenance note
- tags
- status
