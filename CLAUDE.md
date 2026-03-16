# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Vibration time-series annotation tool built with **Tauri + Svelte + ECharts** (Option C). Replaces a Bokeh Python dashboard (`res/device3_vibration_dashboard.html`) with a native desktop app that adds interactive annotation capabilities for vibration data (X/Y/Z axes).

**Status:** Pre-implementation (design phase complete, no application code yet).

## Architecture

**Rust backend** (`src-tauri/`): CSV/data I/O via polars, LTTB downsampling, statistics computation, annotation persistence (JSON files).

**Svelte frontend** (`src/`): ECharts for time-series rendering (markPoint/markArea/brush for annotations, dataZoom for navigation), Svelte stores for state management.

**IPC:** 6 Tauri commands — `load_vibration_data`, `get_timeseries_chunk`, `compute_statistics`, `save_annotations`, `load_annotations`, `export_data`.

Detailed design in `docs/system-design.md` (data models, directory structure, data flows) and `docs/program-design.md` (implementation code).

## Build & Development (once scaffolded)

```bash
# Frontend
npm install
npm run dev          # Svelte dev server

# Tauri (runs both frontend + Rust backend)
cargo tauri dev      # Development mode
cargo tauri build    # Production build

# Rust checks
cd src-tauri
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Key Design Decisions

- **Annotation storage:** JSON files (`{datafile}.vibann.json`), not SQLite — YAGNI
- **Downsampling:** LTTB algorithm in Rust, cap frontend at 50K points
- **Operation modes:** browse / annotate_point / annotate_range (avoids brush/pan conflicts)
- **State:** Svelte stores (dataStore, annotationStore, viewStore, modeStore)
- **Zoom-fetch:** dataZoom events debounced 300ms, triggers new chunk from Rust

## Design Documents

- `docs/tech-selection.md` — Evaluated 5 options (A-E), chose C
- `docs/project-plan.md` — 6-phase roadmap with acceptance criteria
- `docs/system-design.md` — Data models, IPC interface, store architecture, UI layout
- `docs/program-design.md` — Rust + Svelte implementation details with code

## Coding Principles

Project follows philosophies from BIG-270 guides (Ken Thompson, Martin Fowler, Donald Knuth, Linus Torvalds):
- Data structures first, then algorithms
- Each function does one thing
- Correct before optimized
- YAGNI — no speculative features
- Validate only at system boundaries (file I/O, user input), trust internal interfaces
