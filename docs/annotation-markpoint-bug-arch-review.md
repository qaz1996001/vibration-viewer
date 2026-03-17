# Architectural Review: Mark Point Annotation Display Bug

**Date:** 2026-03-17
**Scope:** Bugs 1-3 (HashMap ordering, multi-file mismatch, zoom-fetch transience)
**Reviewer:** Architecture review, guided by Thompson/Torvalds/Knuth/Fowler principles

---

## 1. Plan Evaluation: Is Plan A the Right Architectural Choice?

### Plan A: Remove axis filtering in overview chart

Drop the `axis` parameter from `buildMarkPoints` so the overview chart shows all Point
annotations regardless of which channel they belong to.

### Plan B: Use `data_columns` ordering instead of `Object.keys(chunk.channels)`

Iterate `dataset.column_mapping.data_columns` instead of `Object.keys(chunk.channels)` when
building series, guaranteeing deterministic order that matches annotation axis assignment.

### Plan C: Use `IndexMap` / `BTreeMap` in Rust to preserve insertion order

Replace `HashMap<String, Vec<f64>>` with an ordered map so JSON serialization preserves the
order from `data_columns`.

### Verdict: Plan A is correct for the overview chart, but Plan B is also needed

**Plan A alone is necessary but not sufficient.** Here is the reasoning:

1. **Plan A fixes the immediate symptom correctly.** The overview chart overlays all datasets
   and all channels onto a single value axis. A Point annotation placed at coordinates
   `(time, value)` is meaningful regardless of which channel it was placed on -- the Y value
   was captured from the cursor position on the shared axis. Filtering by channel name is
   semantically wrong for a multi-channel overlay. Thompson's principle: the overview chart's
   data structure is "all series overlaid"; filtering by axis contradicts that structure.

2. **Plan A eliminates an entire class of bugs.** By removing the axis match, the code no
   longer depends on HashMap ordering, `Object.keys` ordering, `firstSeries` identity,
   `activeDataset` vs `datasetOrder[0]` alignment, or any other indirect coupling. This is
   Torvalds' "good taste" -- eliminating the special case rather than patching around it.

3. **Plan B should still be applied as a general correctness measure.** Even though Plan A
   removes the dependency for mark points, the series iteration order still matters for:
   - Color assignment (colorIdx)
   - Legend ordering
   - Which series is "first" (receives markArea/markLine)
   - Future SingleAxisChart annotation display where axis filtering IS semantically correct

   Using `dataset.column_mapping.data_columns` as the iteration source instead of
   `Object.keys(chunk.channels)` costs nothing and eliminates a latent ordering hazard in all
   series construction. This is a one-line change per call site.

4. **Plan C (IndexMap) is over-engineering.** It adds a new crate dependency and changes the
   Rust data model to solve a problem that is better solved at the consumption site. The
   HashMap is not broken -- the consumer was making an invalid assumption about ordering. Per
   Fowler's YAGNI and Thompson's simplicity: do not change the data structure to compensate
   for a presentation-layer logic error.

**Recommendation:** Apply Plan A (overview chart) + Plan B (series iteration) together. Skip
Plan C.

---

## 2. Data Model Alignment

### 2.1 HashMap channels: sound but requires disciplined consumption

The `HashMap<String, Vec<f64>>` in `TimeseriesChunk` is the right structure. It provides O(1)
lookup by channel name, which is the primary access pattern when building per-channel series.
The alternative (a `Vec<(String, Vec<f64>)>`) would preserve order but degrade lookup to O(n)
and complicate the Rust insertion code.

The key discipline is: **never rely on iteration order of `chunk.channels` for semantic
decisions.** Always use `dataset.column_mapping.data_columns` as the authoritative ordered
list, and look up values from the HashMap by name.

This is already partially embodied in the Rust code at `src-tauri/src/commands/data.rs:96-99`
where channels are populated by iterating `data_columns`. The frontend should mirror this
pattern.

### 2.2 Annotation `axis` field: design smell but not yet harmful

The `axis: string` field on Point annotations stores the channel name (`data_columns[0]`) at
creation time. This has two issues:

**Issue A: The axis value is always `data_columns[0]`.** The annotation creation code at
`+page.svelte:168` unconditionally assigns the first data column. When the user clicks on the
overview chart, the Y value comes from the shared axis -- it is not channel-specific. The
`axis` field is therefore recording metadata that does not match reality. It claims the point
belongs to channel "acc_x" but the value was read from a shared grid position.

**Issue B: The axis field creates a false contract.** Downstream code (`buildMarkPoints`)
attempts to use it for filtering, but the overview chart has no concept of per-channel
annotation ownership. This mismatch between data model semantics and display semantics is the
root cause of the bug.

**Recommendation for now:** Keep the `axis` field in the data model. It will become
meaningful when SingleAxisChart shows per-channel annotations (the axis field correctly
indicates which sub-chart should display a given point). But the overview chart should ignore
it, which is exactly what Plan A does.

**Future consideration:** When implementing annotation in SingleAxisChart, the `axis` field
should be set from the specific channel being viewed, not defaulted to `data_columns[0]`.
This means the annotation creation path needs to know which chart view triggered it.

---

## 3. Separation of Concerns

### 3.1 Current structure

`chartOptions.ts` exports two option builders:
- `createOverviewOption()` -- multi-file overlay, shared Y axis, all channels visible
- `createSingleAxisOption()` -- single channel, per-dataset, no annotations yet

These serve fundamentally different display semantics:
- Overview: "show everything overlaid, annotations are global markers"
- SingleAxis: "show one channel in isolation, annotations are channel-specific"

### 3.2 Should `buildMarkPoints` be split?

**Yes.** The two contexts have different filtering requirements:

```
Overview:  show all Point annotations (no axis filter)
SingleAxis: show only Point annotations where axis === channelName
```

Two clean options:

**Option 1 (minimal):** Make the axis parameter optional.

```typescript
function buildMarkPoints(annotations: Annotation[], axis?: string): any[] {
    return annotations
        .filter((a) => a.annotation_type.type === 'Point'
            && (axis === undefined || a.annotation_type.axis === axis))
        .map(...)
}
```

**Option 2 (explicit):** Two named functions.

```typescript
function buildAllMarkPoints(annotations: Annotation[]): any[] { ... }
function buildChannelMarkPoints(annotations: Annotation[], axis: string): any[] { ... }
```

**Recommendation:** Option 1 is sufficient. It follows Thompson's "one function that handles
both cases cleanly" and avoids duplicating the mapping logic. The optional parameter makes
the call sites self-documenting:

```typescript
// Overview: all annotations
buildMarkPoints(annotations)

// SingleAxis: filtered by channel
buildMarkPoints(annotations, channelName)
```

This is also Torvalds' approach: the general case subsumes the special case. No `if`
branching at the call site.

### 3.3 The `firstSeries` pattern

The current code attaches markPoint/markArea/markLine to whichever series happens to be first
in the iteration. This is fragile because:

1. If the first dataset has no data in the current viewport, annotations vanish.
2. Mark points render at coordinates relative to the series they are attached to. On a shared
   value axis this works, but on a category axis it would break silently.
3. The intent ("annotations are global, attach to any series as a rendering vehicle") is not
   expressed in the code.

**Recommendation:** The `firstSeries` pattern is acceptable for now because ECharts requires
mark points to be attached to a series, and the overview uses a shared value axis. Add a
comment explaining the intent:

```typescript
// Attach global annotations to the first series as a rendering vehicle.
// On the shared value axis, mark coordinates are axis-absolute, not series-relative.
```

This is Knuth's literate programming: the code should explain the "why" of a non-obvious
design choice.

---

## 4. Multi-File Annotation Ownership

### 4.1 Current state

Annotations are stored in a single flat `annotations` store (Svelte writable). Loading a file
calls `loadAnnotations(filePath)` which **replaces** the entire store contents with that
file's annotations. In multi-file mode, loading file B after file A discards file A's
annotations silently.

This is a **pre-existing bug** separate from the mark point display issue, but architecturally
related.

### 4.2 Should annotations carry a dataset_id?

**Yes, eventually, but not in this fix.** The current file format (`AnnotationFile`) already
has `dataset_id: Option<String>` on the Rust side but it is always set to `None`.

The multi-file annotation model should be:

1. Annotations stored per-file (already the case via `.vibann.json` sidecar files).
2. In-memory store keyed by dataset ID: `Record<string, Annotation[]>`.
3. Overview chart receives merged annotations from all loaded files.
4. Save operation writes each dataset's annotations to its respective sidecar file.
5. Annotation creation records which dataset was active at creation time.

**Recommendation for this fix:** Do not change the annotation ownership model. The mark point
display bug is self-contained. But document this as a known architectural debt item:

> Multi-file annotation isolation is not yet implemented. Loading multiple files overwrites
> the annotation store with the last file's annotations. This should be addressed as a
> separate work item before multi-file annotation workflows are considered reliable.

### 4.3 Annotation store `loadAnnotations` is destructive

At `annotationStore.ts:63`, `annotations.set(loaded)` replaces all annotations. In the
multi-file loading loop at `+page.svelte:72-75`:

```typescript
for (const filePath of paths) {
    await addFile(filePath, mapping);
    await loadAnnotations(filePath);  // Overwrites previous file's annotations
}
```

Only the last file's annotations survive. This is a data integrity issue. A future fix should
change to `annotations.update((list) => [...list, ...loaded])` with deduplication, or move
to a per-dataset annotation map.

---

## 5. Regression Risk Assessment

### 5.1 Plan A regression risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Annotations from wrong channel rendered on overview | None -- overview has shared Y axis, all annotations are coordinate-absolute | N/A |
| Duplicate mark points if multiple annotations share same coords | Low -- each annotation has unique ID and coordinates from distinct click events | Acceptable |
| Performance with many annotations | Low -- `buildMarkPoints` iterates all annotations without axis filter; linear scan is O(n) where n = annotation count, typically < 100 | Acceptable |
| SingleAxisChart shows wrong annotations | None -- Plan A only changes overview; SingleAxisChart does not yet show annotations | Verify when SingleAxisChart annotations are implemented |

### 5.2 Plan B regression risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `data_columns` contains a name not in `chunk.channels` | Low -- both are populated from the same `ColumnMapping`; would indicate a deeper data corruption | Add null check: `if (!chunk.channels[channelName]) continue` |
| Series order changes from current behavior | Low -- for most users this is the first load, so there is no "current order" expectation; the change makes order deterministic and predictable | Acceptable |

### 5.3 Bug 3 (zoom-fetch transience) risk

Plan A does not address Bug 3. During zoom-fetch, the chunks store is temporarily empty or
stale, which causes a brief flash. This is an existing UX issue unrelated to annotation
correctness. The standard mitigation is:

- Keep the previous chunk in the store until the new one arrives (optimistic retention).
- Or display a loading indicator overlay on the chart during fetch.

This should be tracked separately.

---

## 6. Future-Proofing: SingleAxisChart Annotation Display

### 6.1 Will Plan A constrain SingleAxisChart?

**No.** Plan A makes `buildMarkPoints` accept an optional `axis` parameter. When
SingleAxisChart needs annotations:

```typescript
// In createSingleAxisOption, future addition:
const singleSeries = {
    ...existingSeriesConfig,
    markPoint: {
        data: buildMarkPoints(annotations, channelName),  // axis filter active
        animation: false
    }
};
```

The function signature `buildMarkPoints(annotations, axis?)` naturally supports both use
cases. The overview calls it without `axis`; SingleAxisChart calls it with the specific
channel name.

### 6.2 What needs to change for SingleAxisChart annotations?

1. `createSingleAxisOption` needs an `annotations` parameter added to its signature.
2. `SingleAxisChart.svelte` needs to subscribe to the annotation store.
3. The annotation creation path needs to know whether the user clicked on the overview chart
   or a SingleAxisChart, and set `axis` accordingly.
4. Point 3 requires either passing the source chart identity through the event chain, or
   having SingleAxisChart emit its own annotation events with the channel name pre-filled.

None of these are affected by Plan A.

---

## 7. Summary of Recommendations

### Immediate (this fix)

1. **Apply Plan A:** Remove axis filtering in `buildMarkPoints` for overview chart. Make the
   `axis` parameter optional.
2. **Apply Plan B:** In `createOverviewOption`, iterate
   `dataset.column_mapping.data_columns` instead of `Object.keys(chunk.channels)`.
3. **Add a guard:** `if (!chunk.channels[channelName]) continue` in the series loop to
   handle any data_columns/channels mismatch gracefully.
4. **Add a comment** on the `firstSeries` pattern explaining why annotations are attached to
   the first series.

### Deferred (separate work items)

5. **Multi-file annotation isolation:** Change annotation store to
   `Record<string, Annotation[]>` keyed by dataset ID. Merge for overview display, isolate
   for save/load.
6. **Zoom-fetch transience (Bug 3):** Implement optimistic chunk retention -- do not clear
   old chunk until new one arrives.
7. **SingleAxisChart annotations:** Add annotation display with axis filtering, and ensure
   the creation path sets axis from the originating chart context.

### Not recommended

8. **Plan C (IndexMap):** Rejected. Adds dependency to solve a presentation-layer issue.
   Violates YAGNI.

---

## 8. Architectural Compliance

| Principle | Assessment |
|-----------|-----------|
| **Thompson (data structures first)** | HashMap is the right structure for channel lookup. The bug is in consumption, not structure. Plan B (use data_columns as iteration source) aligns data flow with data structure. |
| **Torvalds (eliminate special cases)** | Plan A removes a special case (axis filtering in a context where axes are not meaningful). The general case (show all points) is simpler and more correct. |
| **Knuth (correct before optimized)** | The fix prioritizes correctness (show annotations reliably) over any performance concern. No premature optimization. |
| **Fowler (YAGNI, simple design)** | Plan C rejected as speculative. The fix is minimal: one parameter made optional, one iteration source changed. No new abstractions introduced. |

---

## Appendix: Affected Files

| File | Change |
|------|--------|
| `src/lib/components/Chart/chartOptions.ts` | `buildMarkPoints` axis parameter becomes optional; `createOverviewOption` series loop iterates `data_columns` instead of `Object.keys` |
| `src/routes/+page.svelte` | No change needed for this fix (annotation axis assignment remains; will be revisited when SingleAxisChart annotations are added) |
| `src-tauri/src/models/vibration.rs` | No change (HashMap is correct) |
| `src/lib/stores/annotationStore.ts` | No change for this fix (multi-file isolation is deferred) |
