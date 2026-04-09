# capcut-cli

An open source, agent-first video editing CLI for generating short social clips without touching a timeline.

## What this is

`capcut-cli` is a Rust project for agents that need to assemble short-form videos programmatically.

The goal is not to recreate a full nonlinear editor. The goal is to expose the primitives an agent actually needs:

- discover and collect candidate media
- ingest audio and video assets into a local library
- trim and normalize clips
- align visuals to audio
- compose short videos from reusable pipelines
- export social-ready outputs for surfaces like Twitter/X
- operate entirely from a command line interface

## Project goals

This project starts from four immediate requirements:

1. Research how to pull trending sounds from TikTok programmatically
2. Research how to pull viral video clips from Twitter/X
3. Build a prototype that combines trending audio with relevant video into short clips suitable for posting on Twitter/X
4. Package the whole thing as an agent-first CLI

## Design principles

### Agent-first

Every important action should be scriptable, composable, and inspectable.

That means:

- stable CLI commands
- machine-readable JSON output where useful
- predictable file layouts
- explicit inputs and outputs
- no GUI dependency
- no hidden timeline state

### Library-backed

Part of this repository will become a large library of sounds and clips.

The CLI should eventually manage:

- metadata for downloaded and curated sounds
- metadata for source clips
- tags, themes, and semantic relevance
- deduplication
- provenance tracking
- prepared intermediates for fast recomposition

### Rust core

Rust is the implementation language for reliability, portability, and strong CLI ergonomics.

Likely building blocks include:

- `clap` for CLI structure
- `serde` and `serde_json` for config and machine-readable output
- `tokio` for async network and pipeline orchestration
- `reqwest` for HTTP/API access
- `ffmpeg` invoked as a system dependency for actual media transforms

## Proposed shape

### Commands

Possible early command surface:

- `capcut-cli research tiktok-sounds`
- `capcut-cli research twitter-clips`
- `capcut-cli library import-sound`
- `capcut-cli library import-clip`
- `capcut-cli compose short`
- `capcut-cli export twitter`

### Repository layout

Possible initial layout:

- `src/cli/` for command definitions
- `src/research/` for source-specific acquisition logic
- `src/library/` for asset registry and metadata
- `src/media/` for ffmpeg pipeline generation
- `src/compose/` for clip assembly logic
- `library/` for local asset manifests and indexes
- `notes/` for ongoing research findings

## Immediate next steps

- put up this README
- research the acquisition paths for TikTok sounds and Twitter/X clips
- map the legal and technical constraints around each source
- sketch the MVP architecture
- build the first committed sound library deliverable
- post progress updates as the work becomes concrete

## First deliverable

The first concrete deliverable is a committed library of popular TikTok sounds, plus a pipeline for adding more over time.

That means:

- committed sound metadata in the repo
- committed sample audio files for preview and feedback
- a documented acquisition pipeline
- CLI primitives that will eventually automate discovery and refresh

## Status

Day one, but no longer just a placeholder.

Current state:

- README and initial research notes are in place
- first Rust CLI scaffold exists
- commands now emit structured JSON for discovery, library planning, and composition planning
- next step is wiring real source adapters and ffmpeg-backed rendering

## Current CLI surface

```bash
capcut-cli discover tiktok-sounds --limit 10
capcut-cli discover x-clips --query "ai agents" --limit 10
capcut-cli library sound --from <url-or-provider> --id <optional-id>
capcut-cli library clip --from <url-or-provider> --id <optional-id>
capcut-cli compose --sound sound_123 --clip clip_a --clip clip_b --duration-seconds 30
```

Each command currently returns machine-readable JSON so an agent can inspect the plan before the implementation becomes fully operational.
