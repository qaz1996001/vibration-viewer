# Research: Single Project File Format for Vibration Annotation Tool

> Research date: 2026-03-17
>
> Context: Tauri 2 (Rust) + SvelteKit + ECharts vibration time-series annotation tool.
> The project currently stores raw CSV data separately from annotations (.vibann.json).
> Goal: evaluate formats for bundling time-series data + annotations + metadata into one file.

---

## Executive Summary

**Key Findings:**

1. **ZIP-based container is the recommended approach** for this project -- it combines the lowest implementation complexity with excellent heterogeneous data support, leverages the project's existing Polars dependency for efficient CSV/Parquet inner files, and produces inspectable archives that users can open with standard tools.

2. **SQLite is the strongest alternative**, particularly if future requirements include incremental saves, undo/redo, or querying subsets of data without loading the full file. Audacity's .aup3 migration to SQLite validates this pattern for desktop apps with large binary payloads.

3. **Parquet excels at columnar time-series storage** and is already supported by Polars (which the project uses), but it lacks natural support for heterogeneous data (annotations, metadata) without workarounds. Best used *inside* a ZIP container rather than as the sole format.

4. **HDF5 is technically capable but operationally painful** in Rust -- the crate requires a C library dependency (libhdf5), has only 30% documentation coverage, and the last release was 2021. The cross-platform build complexity for a Tauri desktop app is a disqualifier.

5. **Custom binary formats and pure serialization formats (MessagePack/CBOR/Arrow IPC) are premature optimizations** -- they solve performance problems this project does not have while sacrificing inspectability and adding implementation burden.

---

## 1. Requirements Analysis

Based on the project's data models (from `docs/system-design.md`):

| Data Type | Structure | Typical Size |
|-----------|-----------|-------------|
| Time-series data | timestamp + X/Y/Z floats, up to millions of rows | 50-500 MB as CSV |
| Annotations | Array of Point/Range objects with labels, colors | < 100 KB typically |
| Metadata | Dataset info, settings, creation date | < 10 KB |

Key constraints:
- **Rust-first**: all file I/O happens in the Tauri backend
- **Already uses Polars 0.46** with CSV, lazy, temporal features
- **Desktop app**: single user, no concurrent access
- **YAGNI principle**: project philosophy explicitly rejects speculative features

---

## 2. Format Evaluations

### 2.1 ZIP-Based Container

**Concept:** A ZIP archive with extension `.vibproj` containing:
```
project.vibproj (ZIP)
  +-- meta.json           # VibrationDataset metadata, format version, settings
  +-- data.csv            # or data.parquet -- raw time-series
  +-- annotations.json    # AnnotationFile structure (already defined)
```

**Rust Ecosystem:**
- `zip` crate v8.2.0 -- mature (51 releases, 2,569 commits), actively maintained under zip-rs org, OpenSSF certified [docs.rs/zip, 2026]
- Supports Deflate, Zstandard, LZMA, Bzip2 compression
- Minimum Rust 1.88, Apache-2.0/MIT licensed
- Pure Rust (no C dependencies)

**Performance (1M+ rows):**
- Write: serialize CSV/Parquet to memory buffer, compress into ZIP entry -- I/O bound
- Read: decompress entry, parse with Polars -- dominated by parsing time, not ZIP overhead
- Zstandard compression typically achieves 3-5x ratio on numeric CSV data
- For 1M rows x 4 columns of f64: ~32 MB raw, ~8-12 MB compressed

**File Size Efficiency:**
- Compression ratio depends on algorithm choice
- Zstandard: excellent ratio with fast decompression
- Deflate: universal compatibility, slightly worse ratio
- Can optionally use Parquet inside ZIP for even better compression of columnar data

**Heterogeneous Data:** Excellent -- each sub-file uses its natural format (JSON for structured data, CSV/Parquet for tabular)

**Complexity:** Low
- 50-100 lines of Rust code for read/write
- No schema migration needed
- No new large dependencies beyond `zip`

**Inspectability:** Excellent -- users can rename to .zip, open with any archive tool, inspect individual files. JSON files human-readable.

**Cross-Platform:** ZIP is universal. No platform-specific considerations.

**Precedents:** .docx (Microsoft Office), .odt (LibreOffice), .sketch (Sketch), .fig (Figma exports), .ora (OpenRaster) all use ZIP containers.

**Verdict: RECOMMENDED**

---

### 2.2 SQLite Single File

**Concept:** A SQLite database with extension `.vibproj` containing tables:
```sql
CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE timeseries (rowid INTEGER PRIMARY KEY, timestamp REAL, x REAL, y REAL, z REAL);
CREATE TABLE annotations (id TEXT PRIMARY KEY, data TEXT);  -- JSON blob
```

**Rust Ecosystem:**
- `rusqlite` v0.39.0 -- very mature, 100% documented, actively maintained [docs.rs/rusqlite, 2026]
- Full SQLite feature set: transactions, WAL mode, blob I/O, virtual tables
- Interfaces via `libsqlite3-sys` FFI bindings (bundles SQLite C source, so no external dep needed)

**Performance (1M+ rows):**
- SQLite is 35% faster than filesystem for small blobs, competitive for larger data [sqlite.org/fasterthanfs]
- Bulk insert of 1M rows: ~2-5 seconds with transaction batching and prepared statements
- Random access queries (time range selection) with index: sub-millisecond
- Sequential scan of 1M rows: ~1-2 seconds

**File Size Efficiency:**
- Moderate -- SQLite has page-based overhead (~20-30% larger than raw CSV for pure numeric data)
- No built-in compression (though pages can use zlib with custom VFS)
- 1M rows x 5 columns: ~60-80 MB typical

**Heterogeneous Data:** Good -- different tables for different data types. JSON columns for flexible structures.

**Complexity:** Medium
- More code than ZIP (schema creation, prepared statements, type mapping)
- Need to handle schema versioning/migration for format updates
- sqlite3 bundled compile adds ~30 seconds to clean build

**Inspectability:** Good -- `sqlite3` CLI tool can query any .vibproj file. DB Browser for SQLite provides GUI. Not as immediately accessible as ZIP+JSON though.

**Cross-Platform:** Excellent -- SQLite is the most widely deployed database engine. File format stable since 2004.

**Precedents:**
- **Audacity .aup3**: Migrated from XML+folder (.aup) to single SQLite file in v3.0. Stores audio blocks as BLOBs in SQLite. Benefits cited: single file portability, crash recovery via WAL, simplified file management. [audacityteam.org/man/audacity_projects]
- **Apple Photos, Apple Messages, Firefox, Chrome**: All use SQLite for local data
- SQLite's own documentation advocates this pattern: "a far better choice than either a custom file format" [sqlite.org/appfileformat]

**Unique Advantages:**
- Incremental save (only changed data written)
- ACID transactions (crash-safe)
- Queryable without loading everything into memory
- Natural undo/redo via savepoints
- Can index timestamp column for fast range queries

**Verdict: STRONG ALTERNATIVE -- best choice if requirements grow beyond simple save/load**

---

### 2.3 HDF5 (.h5)

**Concept:** Hierarchical data format with groups and datasets:
```
project.h5
  /metadata          (attributes: version, source_file, created_at)
  /timeseries
    /timestamp       (dataset: float64[N])
    /x               (dataset: float64[N])
    /y               (dataset: float64[N])
    /z               (dataset: float64[N])
  /annotations       (dataset: variable-length string, JSON)
```

**Rust Ecosystem:**
- `hdf5` crate v0.8.1 -- **last release November 2021** [docs.rs/hdf5]
- Only 30.8% documentation coverage (174 of 565 items)
- Zero examples in documentation
- **Requires HDF5 C library** (1.8.4+) installed on the system
- Windows: must ensure DLL is in PATH; Visual Studio version must match HDF5 binary build
- Static linking requires CMake

**Performance (1M+ rows):**
- HDF5 is designed for large scientific datasets -- excellent read/write for columnar numeric data
- Chunk-based storage with optional compression (gzip, szip)
- Partial I/O: can read slices without loading full dataset

**File Size Efficiency:**
- Good -- native chunk compression, columnar storage eliminates row overhead
- Comparable to or better than Parquet for pure numeric arrays

**Heterogeneous Data:** Good -- attributes for metadata, datasets for arrays, variable-length strings for JSON

**Complexity:** HIGH
- C library dependency makes Tauri distribution painful (must bundle libhdf5 DLL)
- Build configuration complex on Windows (HDF5_DIR env var, DLL path, VS version matching)
- Crate appears semi-maintained (5+ years since last release)

**Inspectability:** Moderate -- HDFView (Java GUI) or h5dump CLI can inspect. Not as universal as ZIP or SQLite tools.

**Cross-Platform:** The format itself is cross-platform, but the Rust crate's C dependency creates platform-specific build/distribution challenges.

**Precedents:**
- **MATLAB .mat v7.3**: HDF5-based since R2006b. Supports partial loading, compression, >2GB variables. [mathworks.com/help/matlab/import_export/mat-file-versions]
- Pandas HDF5Store, PyTables, scientific computing in general

**Verdict: NOT RECOMMENDED -- C dependency and stale Rust crate are disqualifiers for a Tauri desktop app**

---

### 2.4 Apache Parquet

**Concept:** Columnar file format storing time-series data with metadata in key-value pairs:
```
project.parquet
  Schema: timestamp (TIMESTAMP), x (DOUBLE), y (DOUBLE), z (DOUBLE)
  File metadata: {"annotations": "<json>", "version": "1", "source": "..."}
```

**Rust Ecosystem:**
- `parquet` crate v58.0.0 -- part of official Apache Arrow Rust project, very actively maintained [docs.rs/parquet]
- **Already available via Polars** -- the project uses Polars 0.46 which has `parquet` feature
- Rich API: row group statistics, predicate pushdown, columnar compression

**Performance (1M+ rows):**
- Excellent -- Parquet is designed specifically for large columnar datasets
- Polars notes "the layout of data in a Polars DataFrame in memory mirrors the layout of a Parquet file" [docs.pola.rs]
- Dictionary encoding, run-length encoding, delta encoding for timestamps
- Read speed: significantly faster than CSV for analytical queries

**File Size Efficiency:**
- Excellent -- best-in-class for columnar numeric data
- Snappy/Zstandard/gzip compression built-in
- 1M rows x 5 f64 columns: ~8-15 MB typical (vs ~50 MB CSV)
- Compression exploits column homogeneity (all f64 values compress well together)

**Heterogeneous Data:** POOR
- Designed for tabular data with uniform schema
- Custom key-value metadata exists (string pairs in file footer) but:
  - Limited to strings only -- must serialize annotations to JSON string
  - Not designed for large metadata payloads
  - Some tools may ignore or strip custom metadata
  - No standard way to store multiple "logical files" in one Parquet
- Annotations don't fit naturally into the columnar model

**Complexity:** Low-Medium
- Polars integration means almost zero new code for reading/writing timeseries
- But metadata/annotations shoe-horned into key-value pairs feels hacky
- Format version migration harder than with separate JSON files

**Inspectability:** Moderate -- `parquet-tools` CLI, DuckDB, Pandas can read. Not human-readable.

**Cross-Platform:** Excellent -- widely adopted standard.

**Verdict: EXCELLENT for time-series data, POOR for heterogeneous project files. Best used INSIDE a ZIP container.**

---

### 2.5 Custom Binary Format

**Concept:** Application-specific binary layout:
```
[4 bytes: magic "VIBR"]
[4 bytes: format version]
[4 bytes: metadata section offset]
[4 bytes: annotations section offset]
[4 bytes: data section offset]
--- metadata section ---
[4 bytes: JSON length][N bytes: JSON metadata]
--- annotations section ---
[4 bytes: JSON length][N bytes: JSON annotations]
--- data section ---
[8 bytes: row count]
[N * 32 bytes: (f64 timestamp, f64 x, f64 y, f64 z) per row]
```

**Rust Ecosystem:**
- No crate needed -- use `std::io::{Read, Write}`, `byteorder`, or `bincode`
- `bincode` v2.0: mature serde-based binary serialization
- Complete control over layout

**Performance (1M+ rows):**
- Potentially the fastest -- zero parsing overhead for fixed-width numeric data
- Memory-mapped I/O possible for near-zero-copy reads
- But: unlikely to be meaningfully faster than Parquet for real-world use

**File Size Efficiency:**
- Raw: 32 bytes/row = 32 MB for 1M rows (no compression)
- With compression layer: comparable to Parquet
- But must implement compression yourself

**Heterogeneous Data:** Whatever you design -- full flexibility, full responsibility

**Complexity:** HIGH
- Must design, document, implement, and maintain the format specification
- Must handle versioning, backward compatibility, endianness
- Must implement compression if desired
- Every new field requires format revision
- No tooling ecosystem -- debugging requires custom tools

**Inspectability:** None -- completely opaque binary blob. Only your app can read it.

**Cross-Platform:** Must handle endianness explicitly (though most platforms are little-endian now).

**Precedents:**
- **NI TDMS** (LabVIEW): Custom binary with segments containing lead-in + metadata + raw data. Hierarchical (file/group/channel). Supports incremental writes. Separate index file for fast seeking. [ni.com TDMS documentation]
  - Lesson learned: NI invested massive engineering effort maintaining this format across decades. Not appropriate for a small project.

**Verdict: NOT RECOMMENDED -- violates YAGNI principle, massive implementation burden for minimal gain**

---

### 2.6 MessagePack / CBOR

**Concept:** Binary serialization of the entire project state:
```rust
#[derive(Serialize, Deserialize)]
struct ProjectFile {
    version: u32,
    metadata: VibrationDataset,
    annotations: Vec<Annotation>,
    timeseries: TimeseriesData,  // the big part
}
// Serialize entire struct to MessagePack/CBOR bytes
```

**Rust Ecosystem:**
- MessagePack: `rmp-serde` v1.3.1 -- stable, 100% documented, MIT licensed [docs.rs/rmp-serde]
- CBOR: `ciborium` v0.2.2 -- adequate, 100% documented, IETF standard (RFC 8949) [docs.rs/ciborium]
- Both integrate with serde, so existing `#[derive(Serialize, Deserialize)]` structs work immediately

**Performance (1M+ rows):**
- Serialization: must traverse entire data structure -- O(N) for N data points
- MessagePack is ~10x faster than JSON for serialization [msgpack.org]
- But: cannot do partial reads -- must deserialize the entire file to access any part
- For 1M rows: serialization takes ~1-3 seconds, deserialization ~1-2 seconds

**File Size Efficiency:**
- MessagePack: ~20-30% smaller than JSON for mixed data
- For numeric arrays: much less efficient than columnar formats (Parquet)
- f64 values: 9 bytes each in MessagePack (1 byte type + 8 bytes value) vs 8 bytes raw
- 1M rows x 5 columns: ~45 MB (MessagePack) vs ~32 MB (raw binary) vs ~10 MB (Parquet)

**Heterogeneous Data:** Excellent -- serializes any serde-compatible Rust struct directly

**Complexity:** Very Low
- Literally `rmp_serde::to_vec(&project)` / `rmp_serde::from_slice(&bytes)`
- ~10 lines of code total
- But: lack of partial read capability means loading 500 MB into memory at once

**Inspectability:** Poor -- binary format, no standard inspection tools for most users. MessagePack has some web-based decoders but they choke on large files.

**Cross-Platform:** Format is cross-platform; serde handles endianness.

**Verdict: NOT RECOMMENDED for large datasets -- no partial read, poor compression for numeric data, must load entire file into memory**

---

### 2.7 Apache Arrow IPC

**Concept:** Arrow's file format for serializing RecordBatches:
```
[ARROW1 magic]
[Schema message with custom metadata]
[RecordBatch 1: timeseries data]
[RecordBatch 2: annotations as table]
[Footer with offsets]
[ARROW1 magic]
```

**Rust Ecosystem:**
- `arrow-ipc` v58.0.0 -- part of official Apache Arrow project, 100% documented [docs.rs/arrow-ipc]
- **Available via Polars** -- `ipc` and `ipc_streaming` features [docs.pola.rs]
- Mature and well-maintained

**Performance (1M+ rows):**
- Designed for zero-copy interprocess communication -- extremely fast read
- Footer enables random access to specific RecordBatches
- Supports LZ4 and Zstandard compression
- In-memory layout matches on-disk layout -- minimal deserialization overhead

**File Size Efficiency:**
- Moderate-Good -- columnar layout with optional compression
- Without compression: larger than Parquet (alignment padding, metadata overhead)
- With Zstandard: comparable to Parquet

**Heterogeneous Data:** Moderate
- Schema-level custom metadata (key-value string pairs) exists
- Can store multiple RecordBatches with different schemas (using union types or separate batches)
- But the format is fundamentally oriented toward tabular data, not arbitrary structures
- Annotations would need to be flattened into a table or serialized as metadata strings

**Complexity:** Medium
- Polars IPC support makes basic read/write easy
- But annotations/metadata require additional handling outside the tabular model
- Format is spec-heavy -- debugging issues requires understanding Arrow internals

**Inspectability:** Poor-Moderate -- requires Arrow-aware tools (DuckDB, Polars, pyarrow)

**Cross-Platform:** Excellent -- Arrow specification is cross-platform and cross-language.

**Verdict: OVERKILL -- designed for IPC between processes, not for project file persistence. Parquet is better for on-disk storage.**

---

## 3. Precedent Analysis: How Other Tools Handle This

### 3.1 Audacity (.aup3) -- SQLite

- **Before v3.0**: XML manifest (.aup) + folder of audio block files (.au)
  - Problem: moving/renaming broke the project; users accidentally deleted the data folder
- **After v3.0**: Single SQLite database (.aup3)
  - Audio blocks stored as BLOBs in `sampleblocks` table
  - Project metadata, tracks, labels, envelope points in separate tables
  - WAL mode for crash recovery
  - Result: simpler file management, better crash safety, single portable file
- **Trade-off**: Files are larger (SQLite overhead on audio BLOBs) but compressible with standard ZIP utilities
- **Lesson**: For desktop apps with mixed data types, SQLite's single-file + ACID properties are compelling

[Sources: audacityteam.org project documentation; sqlite.org/appfileformat]

### 3.2 MATLAB (.mat) -- Custom Binary / HDF5

- **v4-v7**: Custom binary format with type tags, dimension headers, and raw data blocks
  - Compact, fast for MATLAB's use case
  - Compression added in v7
- **v7.3**: Switched to HDF5 internally
  - Needed for variables >2 GB
  - Enabled partial loading (`matfile` API)
  - Trade-off: HDF5 overhead makes small files larger
- **Lesson**: When data exceeds certain size thresholds, migrating to an established hierarchical format (HDF5) was preferable to extending a custom binary format. But the HDF5 dependency is acceptable for MATLAB because they control the entire toolchain.

[Source: mathworks.com MAT-file versions documentation]

### 3.3 NI LabVIEW (.tdms) -- Custom Binary

- **Structure**: Segments with lead-in header + metadata + raw data
- **Hierarchy**: File > Group > Channel (three levels)
- **Features**: Incremental metadata writing (only changed data written), optional index files for fast seeking, interleaved or non-interleaved data layout
- **Timestamps**: Custom format (seconds since 1904-01-01 + fractional 2^-64 seconds)
- **Lesson**: NI invested enormous engineering effort over decades to build and maintain TDMS. Appropriate for a platform vendor; inappropriate for a small desktop tool.

[Source: ni.com TDMS file format internal structure documentation]

---

## 4. Comparative Matrix

| Criterion | ZIP+CSV/Parquet | SQLite | HDF5 | Parquet | Custom Binary | MsgPack/CBOR | Arrow IPC |
|-----------|:-:|:-:|:-:|:-:|:-:|:-:|:-:|
| **Rust crate maturity** | Excellent | Excellent | Poor | Excellent | N/A | Good | Excellent |
| **C dependency** | None | Bundled | Required | None | None | None | None |
| **1M row read perf** | Good | Good | Excellent | Excellent | Excellent | Poor* | Excellent |
| **1M row write perf** | Good | Good | Good | Good | Good | Poor* | Good |
| **File size (1M rows)** | 10-15 MB | 60-80 MB | 15-20 MB | 8-15 MB | 32 MB raw | 45 MB | 20-30 MB |
| **Heterogeneous data** | Excellent | Good | Good | Poor | Full control | Excellent | Moderate |
| **Partial read** | Per-entry | SQL queries | Slice read | Row groups | Manual offsets | No | RecordBatch |
| **Implementation effort** | Low | Medium | High | Low-Med | High | Very Low | Medium |
| **Inspectability** | Excellent | Good | Moderate | Moderate | None | Poor | Poor |
| **Cross-platform build** | Easy | Easy | Hard | Easy | Easy | Easy | Easy |
| **Crash safety** | None | ACID | None | None | None | None | None |
| **Incremental save** | No (rewrite) | Yes | Append | No | Manual | No | Append |

*MsgPack/CBOR "Poor" for 1M rows because entire file must be deserialized; cannot do partial reads.

---

## 5. Recommendation

### Primary Recommendation: ZIP Container with Parquet + JSON

**Format:** `.vibproj` file (ZIP archive) containing:
```
meta.json              -- project metadata (VibrationDataset, format version, settings)
annotations.json       -- annotation data (existing AnnotationFile structure)
data.parquet           -- time-series data (timestamp, x, y, z columns)
```

**Rationale:**

1. **Leverages existing stack**: Polars already supports Parquet read/write. The project already uses serde_json. Only new dependency is the `zip` crate.

2. **Best compression for time-series**: Parquet's columnar encoding + Snappy/Zstd compression yields 3-5x better compression than ZIP+CSV, and the data is directly loadable into a Polars DataFrame.

3. **Clean separation of concerns**: Each sub-file uses its natural format. Tabular data in Parquet, structured metadata in JSON. No shoe-horning annotations into column metadata or SQL schemas.

4. **Inspectable**: Users can rename `.vibproj` to `.zip`, extract files, inspect JSON in a text editor, load Parquet in DuckDB/Pandas/Polars. This matters for a scientific/engineering tool.

5. **Low complexity**: Estimated ~80 lines of Rust for save/load, aligning with YAGNI.

6. **Future-proof**: Adding new sub-files (e.g., `settings.json`, `thumbnails/preview.png`) is trivial.

**Implementation sketch:**
```rust
// Save project
fn save_project(path: &Path, dataset: &VibrationDataset, df: &DataFrame, annotations: &AnnotationFile) -> Result<()> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);

    // Write metadata
    zip.start_file("meta.json", FileOptions::default().compression_method(Deflated))?;
    serde_json::to_writer(&mut zip, &dataset)?;

    // Write annotations
    zip.start_file("annotations.json", FileOptions::default().compression_method(Deflated))?;
    serde_json::to_writer(&mut zip, &annotations)?;

    // Write time-series as Parquet
    zip.start_file("data.parquet", FileOptions::default().compression_method(Stored))?; // Parquet already compressed
    ParquetWriter::new(&mut zip).finish(&mut df.clone())?;

    zip.finish()?;
    Ok(())
}

// Load project
fn load_project(path: &Path) -> Result<(VibrationDataset, DataFrame, AnnotationFile)> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file)?;

    let meta: VibrationDataset = serde_json::from_reader(zip.by_name("meta.json")?)?;
    let annotations: AnnotationFile = serde_json::from_reader(zip.by_name("annotations.json")?)?;

    let parquet_bytes = {
        let mut entry = zip.by_name("data.parquet")?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        buf
    };
    let df = ParquetReader::new(Cursor::new(parquet_bytes)).finish()?;

    Ok((meta, df, annotations))
}
```

**New dependencies:**
```toml
zip = { version = "8.2", features = ["deflate-zlib-rs"] }
# Polars parquet feature:
polars = { version = "0.46", features = ["lazy", "csv", "temporal", "strings", "dtype-datetime", "parquet"] }
```

### When to Consider SQLite Instead

Upgrade to SQLite (`.vibproj` as SQLite database) if ANY of these requirements emerge:

- **Incremental save**: Users work on very large files and expect fast save (not full rewrite)
- **Undo/redo**: Need transaction-based undo with savepoints
- **Querying**: Need to extract time ranges without loading entire dataset
- **Concurrent access**: Multiple processes reading the same project file
- **Crash recovery**: Need WAL-based protection against data loss during writes

These are real features that real users request, but per YAGNI, don't build for them until needed.

---

## 6. Bibliography

1. zip crate v8.2.0 documentation. docs.rs/zip/8.2.0. Accessed 2026-03-17.
2. rusqlite v0.39.0 documentation. docs.rs/rusqlite/0.39.0. Accessed 2026-03-17.
3. hdf5 v0.8.1 documentation. docs.rs/hdf5/0.8.1. Accessed 2026-03-17.
4. parquet v58.0.0 documentation. docs.rs/parquet/58.0.0. Accessed 2026-03-17.
5. arrow-ipc v58.0.0 documentation. docs.rs/arrow-ipc/58.0.0. Accessed 2026-03-17.
6. rmp-serde v1.3.1 documentation. docs.rs/rmp-serde/1.3.1. Accessed 2026-03-17.
7. ciborium v0.2.2 documentation. docs.rs/ciborium/0.2.2. Accessed 2026-03-17.
8. "SQLite As An Application File Format." sqlite.org/appfileformat.html. Accessed 2026-03-17.
9. "35% Faster Than The Filesystem." sqlite.org/fasterthanfs.html. Accessed 2026-03-17.
10. "Appropriate Uses For SQLite." sqlite.org/whentouse.html. Accessed 2026-03-17.
11. Audacity Project Files documentation. manual.audacityteam.org/man/audacity_projects.html. Accessed 2026-03-17.
12. "MAT-File Versions." mathworks.com/help/matlab/import_export/mat-file-versions.html. Accessed 2026-03-17.
13. "TDMS File Format Internal Structure." ni.com. Accessed 2026-03-17.
14. HDF Group. "HDF5." hdfgroup.org/solutions/hdf5/. Accessed 2026-03-17.
15. Apache Parquet documentation. parquet.apache.org/docs/overview/. Accessed 2026-03-17.
16. Apache Arrow Columnar Format. arrow.apache.org/docs/format/Columnar.html. Accessed 2026-03-17.
17. MessagePack specification. msgpack.org. Accessed 2026-03-17.
18. CBOR specification. cbor.io. Accessed 2026-03-17.
19. Polars I/O documentation. docs.pola.rs/user-guide/io/. Accessed 2026-03-17.
20. zip-rs/zip2 GitHub repository. github.com/zip-rs/zip2. Accessed 2026-03-17.
