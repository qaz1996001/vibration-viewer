# Rust Expert Review: HashMap Serialization Bug in TimeseriesChunk

**Date:** 2026-03-17
**Reviewer:** Rust expert analysis (Claude Opus 4.6)
**Scope:** HashMap non-deterministic serialization in Tauri IPC, data model design
**Guiding principles:** Ken Thompson (data structures first), Linus Torvalds (good taste, eliminate special cases), Donald Knuth (correctness before optimization), Martin Fowler (simple design, YAGNI)

---

## 1. HashMap vs IndexMap Analysis

### 1.1 Is HashMap appropriate for this use case?

**No.** The channels in `TimeseriesChunk` are not a "set of key-value pairs with no meaningful order." They are an ordered sequence of named data columns whose order is defined by the user's `ColumnMapping.data_columns: Vec<String>`. The data has inherent order -- it comes from a CSV where the user explicitly chose columns in a specific sequence, and that order is preserved through `ColumnMapping`, through the statistics computation loop, and through the `SingleAxisChart` rendering. The only place where order is lost is in the `HashMap<String, Vec<f64>>` inside `TimeseriesChunk`.

Ken Thompson's principle applies directly here: **the data structure is wrong.** When your data has order and your container discards it, you have chosen the wrong container. The bug is not in the frontend's `Object.keys()` reliance -- it is in the Rust type system failing to encode a property (ordering) that the domain requires.

### 1.2 Performance characteristics

For 3-10 channels (the typical vibration dataset), the performance difference between `HashMap` and `IndexMap` is irrelevant:

| Operation | HashMap | IndexMap | Notes |
|-----------|---------|----------|-------|
| Insert | O(1) amortized | O(1) amortized | IndexMap appends to internal Vec |
| Lookup by key | O(1) | O(1) | Both use hash table |
| Iteration | Arbitrary order | Insertion order | IndexMap iterates the Vec |
| Serialization | Hash order | Insertion order | The critical difference |
| Memory | ~same | ~same + one Vec pointer per entry | Negligible at n<=10 |

At n=3 to n=10, both containers fit entirely in L1 cache. There is zero measurable performance difference. The Knuth principle says: do not even think about micro-optimizing a 10-element map. Get correctness first.

### 1.3 Serde compatibility

`IndexMap` with the `serde` feature flag has first-class serde support. The integration is battle-tested:

```toml
indexmap = { version = "2", features = ["serde"] }
```

After this, `#[derive(Serialize, Deserialize)]` works identically to HashMap. The serialized JSON preserves insertion order. There are no gotchas, no special annotations needed, no custom serializer required.

One note: `serde_json` already serializes `BTreeMap` in sorted key order and `IndexMap` in insertion order. Both are deterministic. `HashMap` is the only standard map type that produces non-deterministic JSON.

### 1.4 Should the project adopt IndexMap regardless of the frontend fix?

**Yes, for `TimeseriesChunk.channels`.** Even if the frontend fix (Plan A) removes the immediate annotation bug, the HashMap is still wrong at the type level. It communicates to every future reader of the code -- and to every future consumer of the IPC data -- that channel order does not matter. But it does. The `ColumnMapping.data_columns` ordering is meaningful, and `TimeseriesChunk` should preserve it.

However: the `HashMap<String, DatasetEntry>` in `AppState` is correct. Dataset lookup by ID has no meaningful order, and the frontend maintains its own `datasetOrder: string[]` for display ordering. That is the right separation of concerns.

---

## 2. Data Structure Design Review

### 2.1 Evaluating the three options

**Option C1: IndexMap** -- The correct fix. Matches the domain semantics (ordered named channels), requires one small dependency, zero API change on the TypeScript side (`Record<string, number[]>` works identically), and insertion-order preservation is guaranteed by the type itself.

**Option C2: channel_order field** -- This is the "parallel arrays" anti-pattern. You have two sources of truth for the same information: the HashMap keys and the `channel_order` Vec. They can drift. Every consumer must remember to use `channel_order` instead of `Object.keys()`. Every serializer/deserializer must keep them in sync. This violates Linus Torvalds' taste principle: if you need a comment or a convention to prevent misuse, the design is wrong.

**Option C3: Frontend-only fix** -- Removes the immediate symptom but leaves the wrong data structure in place. Acceptable as a quick patch, but it sweeps the real problem under the rug. The next developer who writes `Object.keys(chunk.channels)` will hit the same class of bug.

### 2.2 Is `HashMap<String, Vec<f64>>` the right choice for channels?

As argued above: no. But the question is what to replace it with.

**`IndexMap<String, Vec<f64>>`** -- Best option. Named access by channel name is preserved (for `createSingleAxisOption` which does `chunk.channels[channelName]`), order is preserved, serde works out of the box.

**`Vec<(String, Vec<f64>)>`** -- Simpler in theory, but loses O(1) key lookup. The frontend would need to iterate to find a channel by name, or convert to a Map on arrival. The TypeScript type would become `[string, number[]][]` instead of `Record<string, number[]>`, requiring changes to every consumer. More churn for less expressiveness.

**`BTreeMap<String, Vec<f64>>`** -- Deterministic (alphabetical order), zero new dependencies. But alphabetical order is not the same as user-specified order. If the user maps columns as `["amplitude", "z_accel", "x_accel"]`, a BTreeMap would serialize as `["amplitude", "x_accel", "z_accel"]`. This is wrong for the same reason HashMap is wrong: it does not preserve the user's intent.

**Recommendation: IndexMap.** It is the idiomatic Rust answer to "I need a map that remembers insertion order."

### 2.3 Rust-idiomatic alternatives

One alternative worth mentioning: a newtype wrapper.

```rust
/// Ordered channel data. Iteration order matches ColumnMapping.data_columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelData(IndexMap<String, Vec<f64>>);
```

This is more Rust-idiomatic (newtype pattern for domain semantics) and allows adding methods like `channel_names() -> &[String]` or `first_channel() -> Option<(&str, &[f64])>`. However, for the current codebase size and YAGNI principles, the bare `IndexMap` is sufficient. The newtype can be introduced later if the API surface grows.

---

## 3. Serialization Safety Audit

### 3.1 All HashMap usages in the codebase

| Location | Type | Order-sensitive? | Verdict |
|----------|------|-----------------|---------|
| `vibration.rs` `TimeseriesChunk.channels` | `HashMap<String, Vec<f64>>` | **YES** -- frontend uses key order for annotation matching, color assignment, display | **Must fix: use IndexMap** |
| `state.rs` `AppState.datasets` | `Mutex<HashMap<String, DatasetEntry>>` | No -- never serialized over IPC, lookup by UUID only | Correct as-is |
| `data.rs` `get_timeseries_chunk` local | `HashMap::new()` used to build channels | Insertion order matches `data_columns` iteration | Will be correct once type is IndexMap |

The `AppState.datasets` HashMap is an internal lookup table, never crosses the IPC boundary as a map (individual `VibrationDataset` values are returned, not the whole map), and has no ordering requirement. It is correctly a HashMap.

### 3.2 Frontend Record types

The frontend uses `Record<string, T>` in several places:

- `Record<string, TimeseriesChunk>` in `chunks` store -- keyed by dataset ID, order maintained by `datasetOrder: string[]`. Correct pattern.
- `Record<string, VibrationDataset>` in `datasets` store -- same pattern. Correct.
- `Record<string, number[]>` in `TimeseriesChunk.channels` -- **the problematic one.** No separate order array exists on the frontend side. Order comes entirely from JSON key order.

### 3.3 Policy recommendation

**Any Rust struct that crosses the Tauri IPC boundary and contains a map whose key order is semantically meaningful MUST use `IndexMap`, not `HashMap`.** This should be documented in `CLAUDE.md` under Design Decisions.

The general rule: `HashMap` is for internal lookup tables. `IndexMap` (or `BTreeMap` when alphabetical is desired) is for data that gets serialized. This is a common Rust-in-web-context pattern -- the `serde_json` crate itself documents that HashMap serialization order is non-deterministic.

---

## 4. Recommendation

### 4.1 Primary recommendation: IndexMap (Option C1)

Adopt IndexMap for `TimeseriesChunk.channels`. This is a 4-line change:

**Cargo.toml** -- add one dependency:
```toml
indexmap = { version = "2", features = ["serde"] }
```

**`src-tauri/src/models/vibration.rs`** -- change the import and field type:
```rust
use indexmap::IndexMap;
// ...
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub channels: IndexMap<String, Vec<f64>>,
    pub is_downsampled: bool,
    pub original_count: usize,
}
```

**`src-tauri/src/commands/data.rs`** -- change local construction (2 places):
```rust
use indexmap::IndexMap;
// ...
let mut channels = IndexMap::new();
```

No frontend changes required. `Record<string, number[]>` in TypeScript consumes both ordered and unordered JSON objects identically.

### 4.2 Combine with Plan A (frontend fix) if desired

The IndexMap fix and the frontend Plan A fix are orthogonal. Plan A (removing axis-based filtering in `buildMarkPoints` for the overview chart) solves the immediate user-facing annotation visibility bug. The IndexMap fix prevents the entire class of order-dependent bugs from recurring.

If both are applied:
- IndexMap ensures that `Object.keys(chunk.channels)[0]` always equals `data_columns[0]`
- Plan A ensures that even if some future code path reintroduces order ambiguity, annotations still display correctly in the overview

This is defense in depth, which is appropriate for a data analysis tool where silent data misrepresentation is the worst failure mode.

### 4.3 If Plan A alone is adopted

Add a comment in `vibration.rs`:

```rust
/// Channel data keyed by column name.
///
/// WARNING: HashMap serialization order is non-deterministic.
/// Do NOT rely on JSON key order in the frontend.
/// The authoritative channel order is in VibrationDataset.column_mapping.data_columns.
pub channels: HashMap<String, Vec<f64>>,
```

This is the minimum acceptable documentation, but it is a code smell. A comment that says "do not rely on a property of this type" means the type is wrong.

---

## 5. Broader Rust Code Quality Review

### 5.1 Data model design in vibration.rs

The data model is clean and minimal. Four structs, each with a clear purpose. The `ColumnMapping` / `CsvPreview` / `VibrationDataset` / `TimeseriesChunk` hierarchy maps directly to the application's data flow (preview -> mapping -> load -> chunk). This follows Ken Thompson's "data structures first" principle well.

**One concern:** `TimeseriesChunk` contains both data (`time`, `channels`) and metadata (`is_downsampled`, `original_count`). For a 50,000-point chunk, the metadata is negligible, but conceptually these are different concerns. At the current scale this is fine (YAGNI), but if the chunk type grows more metadata fields, consider:

```rust
pub struct TimeseriesChunk {
    pub data: ChannelData,
    pub meta: ChunkMeta,
}
```

### 5.2 Error handling

The codebase uses `.unwrap()` in several places inside `get_timeseries_chunk` and `extract_f64_vec`:

```rust
fn extract_f64_vec(df: &polars::prelude::DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name)
        .unwrap()  // panics if column missing
        .f64()
        .unwrap()  // panics if not f64
        .into_no_null_iter()
        .collect()
}
```

These unwraps are defended by the earlier validation in `read_csv_with_mapping` (columns are checked to exist and cast to Float64). Per the CLAUDE.md principle "trust internal interfaces," this is acceptable. The validation boundary is at CSV load time, and once data is in the `DatasetEntry`, the invariant "columns exist and are f64" is guaranteed.

However, if the function were `pub` or used across module boundaries, it should return `Result`. Currently it is `fn` (private to the module), so the unwraps are tolerable.

### 5.3 The `Mutex<HashMap>` in AppState

```rust
pub struct AppState {
    pub datasets: Mutex<HashMap<String, DatasetEntry>>,
}
```

This is fine for a single-user desktop app. The mutex is held briefly during each IPC call. Two potential improvements to note for the future but not act on now (YAGNI):

1. **`RwLock` instead of `Mutex`**: Read-heavy workload (many `get_timeseries_chunk` calls, rare `load_vibration_data` calls) would benefit from concurrent readers. But Tauri IPC calls are serialized per-window anyway, so this provides no practical benefit today.

2. **Poisoning**: The `.unwrap()` on `lock()` will panic if another thread panicked while holding the lock. In a Tauri desktop app with a single UI thread driving IPC calls, mutex poisoning is effectively impossible. The unwrap is acceptable.

### 5.4 The `data_columns` iteration in `get_timeseries_chunk`

```rust
for col_name in data_columns {
    let raw = extract_f64_vec(&filtered, col_name);
    let sampled: Vec<f64> = indices.iter().map(|&i| raw[i]).collect();
    channels.insert(col_name.clone(), sampled);
}
```

This iterates `data_columns` (a `Vec<String>` from `ColumnMapping`) in order, inserting into the channels map. With `HashMap`, the insertion order is lost. With `IndexMap`, the insertion order is preserved, and since `data_columns` is iterated in its natural order, the channels will serialize in the user-specified column order. This is exactly right.

### 5.5 Clone cost in `load_vibration_data`

```rust
let metadata = VibrationDataset { ... };
// ...
datasets.insert(id, DatasetEntry { metadata: metadata.clone(), dataframe: df });
Ok(metadata)
```

The `VibrationDataset` is cloned to return one copy to the frontend and store another in state. The struct contains only Strings and small scalar values -- the clone cost is negligible. This is fine.

### 5.6 LTTB using only the first channel

```rust
let representative = if !data_columns.is_empty() {
    extract_f64_vec(&filtered, &data_columns[0])
} else {
    vec![0.0; time_raw.len()]
};
let indices = lttb_indices(&time_raw, &representative, max_points);
```

This uses the first data column as the LTTB representative. The downsampled indices are then applied to all channels, ensuring time alignment. This is a sound engineering trade-off: running LTTB per-channel and merging index sets would be more accurate but dramatically more complex, and the visual difference for correlated vibration channels is minimal. The design decision is documented in CLAUDE.md. Good.

---

## 6. Summary of Actionable Items

| Priority | Action | Effort | Files changed |
|----------|--------|--------|---------------|
| **P0** | Replace `HashMap` with `IndexMap` in `TimeseriesChunk.channels` | 10 min | `Cargo.toml`, `vibration.rs`, `data.rs` |
| **P0** | Apply Plan A frontend fix (remove axis filter in `buildMarkPoints`) | 5 min | `chartOptions.ts` |
| **P1** | Add IPC serialization policy to `CLAUDE.md` | 2 min | `CLAUDE.md` |
| **P2** | Consider making `extract_f64_vec` return `Result` if it becomes pub | -- | deferred |
| **P2** | Consider `RwLock` if concurrent reads become a bottleneck | -- | deferred |

The P0 items are the minimum to fix the bug correctly. The IndexMap change takes under 10 minutes, adds one well-maintained dependency (indexmap is already a transitive dependency of serde_json and many other crates), and eliminates the entire class of serialization-order bugs.

---

## Appendix: Exact Code Changes for IndexMap Fix

### Cargo.toml

Add after the `uuid` line:

```toml
indexmap = { version = "2", features = ["serde"] }
```

### src-tauri/src/models/vibration.rs

```rust
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub time_column: String,
    pub data_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPreview {
    pub file_path: String,
    pub columns: Vec<String>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub total_points: usize,
    pub time_range: (f64, f64),
    pub column_mapping: ColumnMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    /// Channel data keyed by column name. Iteration/serialization order
    /// matches ColumnMapping.data_columns (guaranteed by IndexMap).
    pub channels: IndexMap<String, Vec<f64>>,
    pub is_downsampled: bool,
    pub original_count: usize,
}
```

### src-tauri/src/commands/data.rs

Replace `use std::collections::HashMap;` with `use indexmap::IndexMap;` and change both `HashMap::new()` calls to `IndexMap::new()`.

No changes needed to any other Rust files, any TypeScript files, or any TypeScript types.
