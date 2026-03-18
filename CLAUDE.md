# CLAUDE.md

## Project Overview

Vibration time-series annotation tool built with **Tauri 2 + SvelteKit + ECharts 6 + Polars** (Option C). Replaces a Bokeh Python dashboard (`res/device3_vibration_dashboard.html`) with a native desktop app that adds interactive annotation capabilities for vibration data.

**Status:** Multi-file + dynamic CSV columns implemented. In refinement phase.

## Architecture

**Rust backend** (`src-tauri/src/`):
- `commands/` — 7 IPC endpoints: `data.rs` (preview_csv_columns, load_vibration_data, get_timeseries_chunk), `statistics.rs`, `annotation.rs`, `export.rs`
- `models/` — `vibration.rs` (ColumnMapping, CsvPreview, VibrationDataset, TimeseriesChunk with IndexMap channels), `annotation.rs`, `statistics.rs`
- `services/` — `csv_reader.rs` (preview_csv + read_csv_with_mapping), `downsampling.rs` (lttb_indices), `stats_engine.rs`, `time_filter.rs` (shared Polars lazy time range filter)
- `state.rs` — `AppState` with `RwLock<HashMap<id, DatasetEntry>>`
- `error.rs` — `AppError` enum (thiserror + Serialize) for typed IPC errors
- `lib.rs` — Tauri builder entry point

**Svelte frontend** (`src/lib/`):
- `components/Chart/` — `TimeseriesChart.svelte`, `SingleAxisChart.svelte`, `chartOptions.ts`
- `components/Annotation/` — `AnnotationPanel.svelte`
- `components/ColumnMapping/` — `ColumnMappingDialog.svelte`
- `components/DataTable/` — `ViewportDataTable.svelte`
- `components/Statistics/` — `BasicStatsTable.svelte`
- `components/Layout/` — `Toolbar.svelte`, `FileList.svelte`
- `stores/` — `dataStore.ts` (multi-file: datasets/chunks/statistics maps), `annotationStore.ts`, `modeStore.ts`, `viewStore.ts`
- `types/` — `vibration.ts` (ColumnMapping, CsvPreview, channels: Record<string,number[]>), `annotation.ts`, `statistics.ts`
- `utils/` — `debounce.ts`, `formatTime.ts`
- `constants/` — `colors.ts` (dataset color palette)

**IPC:** 7 Tauri commands — `preview_csv_columns`, `load_vibration_data` (accepts ColumnMapping), `get_timeseries_chunk`, `compute_statistics`, `save_annotations`, `load_annotations`, `export_data`.

## Build & Development

**Prerequisites:** Rust toolchain, Node.js 20+, `npm install` in root, `@tauri-apps/cli` (installed as devDep)

```bash
cargo tauri dev                              # Dev mode (frontend + backend)
cargo tauri build                            # Production build

# Checks
cd src-tauri && cargo clippy -- -D warnings  # Rust lint
cd src-tauri && cargo test                   # Rust tests
npx svelte-check                             # Svelte/TS check
```

## Key Design Decisions

- **Multi-file overlay:** Multiple CSVs on same chart with time-aligned value axis (epoch seconds)
- **Dynamic CSV columns:** User maps columns via ColumnMappingDialog after opening file; no hardcoded x/y/z
- **TimeseriesChunk.channels:** `IndexMap<String, Vec<f64>>` (preserves column order) instead of fixed x/y/z/amplitude fields
- **Two-step file open:** preview_csv_columns → ColumnMappingDialog → load_vibration_data with mapping
- **Value axis:** xAxis type='value' (epoch seconds) enables multi-file time alignment
- **LTTB index-based:** `lttb_indices` returns indices, applies to all channels for aligned downsampling
- **Annotation storage:** JSON files (`{datafile}.vibann.json`), frontend derives path
- **Downsampling:** LTTB algorithm in Rust, cap frontend at 50K points per dataset
- **Operation modes:** browse / annotate_point / annotate_range (avoids brush/pan conflicts)
- **State:** Multi-file stores (datasets, chunks, statistics as Record maps), activeDatasetId for single-axis views
- **Zoom-fetch:** dataZoom events debounced 300ms, triggers fetchAllChunks for all loaded datasets

## Known Issues

- Label offset drag UI not yet implemented (data model supports it)

## Design Documents

- `docs/tech-selection.md` — Evaluated 5 options (A-E), chose C
- `docs/project-plan.md` — 6-phase roadmap with acceptance criteria
- `docs/system-design.md` — Data models, IPC interface, store architecture, UI layout
- `docs/program-design.md` — Rust + Svelte implementation details with code
- `docs/architecture-review.md` — Architecture review with recommendations

## Gotchas

- **SPA mode:** `ssr = false`, `prerender = false` in `+layout.ts` — no server-side code, static adapter only
- **Svelte 5:** Uses runes (`$state`, `$derived`, `$effect`) — no `let` reactivity or `$:` syntax
- **ECharts 6:** API differences from v5 — `init()` options changed, some option keys renamed
- **No eslint/prettier configured** — no auto-format command available
- **Tauri file dialogs:** Use `@tauri-apps/plugin-dialog` (`open()`) not browser file input

## Coding Principles

Project follows philosophies from BIG-270 guides (Ken Thompson, Martin Fowler, Donald Knuth, Linus Torvalds):
- Data structures first, then algorithms
- Each function does one thing
- Correct before optimized
- YAGNI — no speculative features
- Validate only at system boundaries (file I/O, user input), trust internal interfaces
