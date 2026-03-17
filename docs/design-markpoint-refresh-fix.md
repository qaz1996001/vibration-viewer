# Design: Mark Point Refresh Failure Fix

> Synthesized from architect-review, code-reviewer, and rust-expert analysis.
> Reference: `claudedocs/research_markpoint_refresh_failure.md`

---

## Problem Statement

Mark Point annotations intermittently fail to display on the ECharts overview chart. Root cause is a data structure mismatch: Rust `HashMap<String, Vec<f64>>` serialization order is non-deterministic, but the frontend uses the first key from `Object.keys()` to filter annotations by axis name.

**Three bugs identified:**

| Bug | Severity | Trigger | Root Cause |
|-----|----------|---------|------------|
| Bug 1 | Critical | Single-file | HashMap serialization order != `data_columns` order |
| Bug 2 | High | Multi-file | `activeDataset` != `firstSeries` owner dataset |
| Bug 3 | Low | Zoom | Transient chunk store state during fetch |

---

## Design Decision

### Adopted: Plan A + C1 (Two-Layer Fix)

**Layer 1 — Frontend (Plan A): Remove axis filtering in overview chart's `buildMarkPoints`**

The overview chart overlays all channels on a shared value axis. Point annotations use absolute coordinates `[time, value]` — they are axis-independent in this context. Filtering by axis in the overview chart is a design error.

- Fixes Bug 1 + Bug 2 simultaneously
- Zero regression risk (coordinates are absolute)
- Consistent with `buildMarkAreas` which already has no axis filter

**Layer 2 — Rust (C1): Replace `HashMap` with `IndexMap`**

`HashMap` is the wrong data structure for `TimeseriesChunk.channels`. Channel data has a user-defined order (from `ColumnMapping.data_columns`), and `HashMap` discards that ordering invariant. This is a Linus Torvalds "data structures first" fix — encode the domain constraint in the type.

- Eliminates the entire class of ordering bugs at the source
- Prevents future bugs if other code depends on channel order (legend, color assignment)
- One dependency, zero API changes

### Rejected Alternatives

| Plan | Reason for Rejection |
|------|---------------------|
| Plan B (use `data_columns` instead of `Object.keys`) | Fixes Bug 1 but not Bug 2. Band-aid over wrong data structure. |
| Plan C2 (add `channel_order: Vec<String>`) | Parallel-arrays anti-pattern. Two sources of truth that can drift. |
| Frontend-only without Rust fix | Treats symptom; leaves wrong data structure in place. |

---

## Implementation

### Change 1: Rust — IndexMap for TimeseriesChunk.channels

**`src-tauri/Cargo.toml`**
```toml
[dependencies]
indexmap = { version = "2", features = ["serde"] }
```

**`src-tauri/src/models/vibration.rs`**
```rust
use indexmap::IndexMap;

pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub channels: IndexMap<String, Vec<f64>>,  // was: HashMap
    pub is_downsampled: bool,
    pub original_count: usize,
}
```

**`src-tauri/src/commands/data.rs`** — update `HashMap::new()` → `IndexMap::new()` (2 sites) and import.

**Frontend**: No TypeScript type change needed. `Record<string, number[]>` consumes ordered JSON identically.

### Change 2: Frontend — buildMarkPoints removes axis filter

**`src/lib/components/Chart/chartOptions.ts`**

Extract shared mapping logic, create two functions:

```typescript
// Shared mapping: Annotation → ECharts markPoint data item
function mapPointToMarkData(a: Annotation) {
    const pt = a.annotation_type as {
        type: 'Point'; time: number; value: number; axis: string
    };
    return {
        coord: [pt.time, pt.value] as [number, number],
        name: a.label,
        symbol: 'pin',
        symbolSize: 30,
        itemStyle: {
            color: a.color,
            borderColor: '#fff',
            borderWidth: 2,
            shadowBlur: 4,
            shadowColor: 'rgba(0,0,0,0.3)'
        },
        label: {
            show: true,
            formatter: a.label,
            offset: [a.label_offset_x, a.label_offset_y],
            backgroundColor: 'rgba(255,255,255,0.9)',
            borderColor: a.color,
            borderWidth: 1,
            borderRadius: 4,
            padding: [4, 8],
            color: '#333',
            fontSize: 12
        }
    };
}

// Overview chart: ALL point annotations (no axis filter)
function buildMarkPoints(annotations: Annotation[]): any[] {
    return annotations
        .filter((a) => a.annotation_type.type === 'Point')
        .map(mapPointToMarkData);
}

// Future: SingleAxisChart — filter by specific channel
export function buildMarkPointsForAxis(
    annotations: Annotation[],
    axis: string
): any[] {
    return annotations
        .filter((a) => a.annotation_type.type === 'Point'
                    && a.annotation_type.axis === axis)
        .map(mapPointToMarkData);
}
```

**Call site** (line ~106):
```typescript
// Before:
data: buildMarkPoints(annotations, channelName)

// After:
data: buildMarkPoints(annotations)
```

### Change 3: Channel iteration order (hygiene)

In `createOverviewOption`, use `data_columns` for deterministic iteration:

```typescript
// Before:
const channelNames = Object.keys(chunk.channels);

// After:
const channelNames = ds.column_mapping.data_columns.filter(
    (col) => col in chunk.channels
);
```

This makes legend order, color assignment, and series ordering deterministic regardless of JSON key order.

---

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `indexmap` dependency |
| `src-tauri/src/models/vibration.rs` | `HashMap` → `IndexMap` for channels |
| `src-tauri/src/commands/data.rs` | `HashMap::new()` → `IndexMap::new()`, update import |
| `src/lib/components/Chart/chartOptions.ts` | Refactor `buildMarkPoints`, add `buildMarkPointsForAxis`, use `data_columns` for iteration |

---

## Validation

1. **Single-file**: Load CSV with 3+ data columns. Create point annotations. Zoom in/out 10+ times. All annotations must remain visible on every render.
2. **Multi-file**: Load File A, create annotations. Load File B. File A annotations must still display on overview.
3. **Persistence round-trip**: Create annotations → save → reload file → load annotations → all display correctly.
4. **LTTB downsampling**: Load >50K point file. Verify annotations display correctly after downsampling.

---

## Known Issues (Out of Scope)

Identified during review but not addressed by this fix:

| Issue | Severity | Description |
|-------|----------|-------------|
| Annotation store overwrite | High | `loadAnnotations` replaces all annotations globally. Multi-file mode loses prior file's annotations. Needs per-file store (`Record<datasetId, Annotation[]>`). |
| `extract_f64_vec` null handling | Medium | `into_no_null_iter()` skips nulls, producing misaligned vectors. Should use `into_iter()` with fallback. |
| `pendingAnnotation.data: any` | Low | Should be discriminated union type for type safety. |
| Subscribe-then-unsubscribe anti-pattern | Low | `dataStore.ts` lines 128-131, 143 should use `get()` from svelte/store. |
| Missing error handling | Low | `handleExport`/`handleExportViewport` have no try/catch. |

---

## Philosophy Alignment

- **Ken Thompson**: Data structures first — `IndexMap` encodes the ordering invariant at the type level.
- **Linus Torvalds**: Eliminate special cases — removing the axis filter removes a special-case branch that was the root cause.
- **Donald Knuth**: Correct before optimized — fix the logical error before considering performance.
- **Martin Fowler**: YAGNI + Simple Design — minimal changes, no speculative features, extract method to avoid duplication.

---

## Related Documents

- `claudedocs/research_markpoint_refresh_failure.md` — Original bug research
- `docs/annotation-markpoint-bug-arch-review.md` — Architect review
- `docs/rust-hashmap-serialization-review.md` — Rust expert review
