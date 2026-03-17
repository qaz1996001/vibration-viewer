# 架構審查：Vibration Viewer 重新設計

> 審查日期：2026-03-17
> 範圍：從單檔 CSV 檢視器演進至具備 AIDPS 能力的專案導向振動分析平台之完整架構分析
> 哲學：Thompson（資料結構優先）、Torvalds（消除特殊情況）、Fowler（YAGNI、SRP）、Knuth（程式即文學）

---

## 1. 現有架構問題

### 1.1 程式碼異味與具體位置

**大型類別 / God Object（上帝物件）-- `+page.svelte`（352 行）**
`src/routes/+page.svelte` 統籌了檔案開啟、欄位對應對話框生命週期、標註建立/確認/取消、匯出（完整 + 視窗範圍）、縮放取值防抖、精度變更重新取值，以及版面渲染。它持有 7 項本地狀態（`pendingAnnotation`、`currentZoomStart`、`currentZoomEnd`、`showMappingDialog`、`currentPreview`、`pendingFilePaths`、衍生值）。這是典型的上帝物件，隨著專案模式、設備選擇、頻譜顯示和 WAV 控制元件的加入只會更加惡化。

**過長方法 -- `chartOptions.ts` 中的 `createOverviewOption`（第 36-193 行，共 158 行）**
此函式建構整個 ECharts 選項物件：迭代資料集、建構系列、建立標記點/標記區域/標記線、設定 tooltip/legend/grid/dataZoom/axes。它有 7 個參數（其中 3 個是包含多個欄位的可選物件）。依 Fowler 的標準（方法少於 20 行、參數少於 4 個），此函式在長度上超出約 8 倍，參數上超出約 2 倍。

**Feature Envy（依戀情結）-- `TimeseriesChart.svelte` 直接讀取 7 個 store**
`src/lib/components/Chart/TimeseriesChart.svelte`（第 5-8 行）從 dataStore 匯入並訂閱 `chunks`、`datasets`、`datasetOrder`、`globalTimeRange`、`fileColors`，加上 annotationStore 的 `annotations`、`selectedId`，以及 modeStore 的 `mode`、`rangeFirstClick`。圖表元件應透過 props 接收資料，而非橫跨整個 store 圖取值。這造成了隱式耦合：任何 store 的變更都會觸發圖表 `$effect` 區塊的重新運算。

**Primitive Obsession（基本型別偏執）-- Rust 後端全面以 `String` 處理錯誤**
每個 Tauri 命令都回傳 `Result<T, String>`。範例：`commands/data.rs:11`、`commands/annotation.rs:8`、`commands/statistics.rs:8`、`commands/export.rs:7`，以及 `csv_reader.rs`、`stats_engine.rs` 中的所有服務函式。字串錯誤失去型別資訊，阻礙前端程式化的錯誤處理，且無法在不進行字串比對的情況下區分「找不到檔案」、「解析錯誤」、「找不到欄位」或「找不到資料集」。

**重複程式碼 -- 時間篩選邏輯**
時間範圍篩選模式（`df.column("time").unwrap().f64().unwrap().into_iter().map(...)`）在 `commands/data.rs:71-78` 和 `commands/export.rs:20-27` 中完全相同。`data.rs:123-130` 的 `extract_f64_vec` 輔助函式是該模組的私有函式，但其他模組也需要使用。`formatTime` 在 `chartOptions.ts:21-29` 和 `ViewportDataTable.svelte:11-18` 之間重複——實作完全相同。

**重複程式碼 -- 色彩調色盤**
`dataStore.ts:27-29` 的 `FILE_COLOR_PALETTE` 和 `chartOptions.ts:5-15` 的 `COLOR_PALETTE` 是同一個 9 色陣列定義了兩次。修改其中一個會在不知不覺中與另一個產生差異。

**Dead Code（死碼）/ 職責邊界混亂 -- `AnnotationDialog.svelte`**
git status 顯示此檔案已修改（`M src/lib/components/Annotation/AnnotationDialog.svelte`），但該檔案不存在於磁碟上。對話框功能已被內聯到 `AnnotationPanel.svelte`，使該元件達到 425 行（其中一半是 CSS）。面板現在同時處理標註列表和建立/編輯表單——兩個截然不同的職責。

**Shotgun Surgery（散彈式修改）-- `dataStore.ts` 中的 `removeFile`（第 102-135 行）**
移除檔案需要更新 5 個獨立的可寫入 store（`datasets`、`chunks`、`statistics`、`fileColors`、`datasetOrder`），外加有條件地更新 `activeDatasetId`。每個 store 都透過各自的 `.update()` 呼叫獨立更新。新增任何按資料集區分的狀態（例如頻譜快取、WAV 關聯）都需要修改此函式。這是狀態分散的結構性症狀。

### 1.2 SRP 違反

| 位置 | 混合的職責 |
|------|-----------|
| `+page.svelte` | 檔案對話框統籌 + 標註工作流程 + 縮放管理 + 匯出邏輯 + 版面配置 |
| `AnnotationPanel.svelte` | 標註列表顯示 + 建立表單 + 編輯表單 + 色彩選擇 + 偏移控制 |
| `dataStore.ts` | Store 定義 + IPC 呼叫 + 檔案生命週期 + 色彩分配 + 衍生計算 |
| `commands/data.rs::get_timeseries_chunk` | 時間篩選 + LTTB 判斷 + 多通道擷取 + 回應建構 |
| `csv_reader.rs::read_csv_with_mapping` | 檔案驗證 + CSV 解析 + 日期時間格式偵測 + 型別轉換 + null 處理 + 欄位選取 |

### 1.3 狀態管理問題

**標註是全域的，而非按裝置區分。** `annotationStore.ts` 只有一個 `annotations` 可寫入值。載入檔案 A 的標註後再開啟檔案 B，會覆蓋 A 的標註。`+page.svelte:66-76` 中的 `handleMappingConfirm` 函式依序對多個檔案呼叫 `loadAnnotations`，意味著只有最後一個檔案的標註會保留。這是多檔案工作流程中的資料遺失 bug。

**縮放狀態不一致。** `TimeseriesChart.svelte:23` 中的 `currentZoom` 是一個普通變數（非響應式，刻意設計以避免回饋迴圈）。但 `+page.svelte:39-40` 中的 `currentZoomStart` 和 `currentZoomEnd` 是追蹤相同資訊的 `$state` 變數。這兩個表示法可能會產生漂移。

**沒有專案概念。** 狀態是以個別檔案為單位組織的（`datasets`、`chunks`、`statistics`——全部以 dataset ID 為鍵）。AIDPS 需要一個專案容器來持有多個設備，每個設備包含多個檔案，並具有設備到感測器的對應關係。將此改造到目前的扁平 map 結構中，需要每個 store 消費者都理解兩層式階層結構。

### 1.4 AIDPS 的可擴展性障礙

**所有資料集共用單一 Mutex。** `AppState` 使用 `Mutex<HashMap<String, DatasetEntry>>`。載入設備資料（連接 20+ 個 CSV）會在整個解析期間阻塞所有其他資料集的讀取。包含 5 個設備和 117 個 CSV 檔案的 AIDPS 專案將產生嚴重的競爭。

**沒有資料來源抽象層。** CSV 讀取是硬編碼的。AIDPS 需要 WAV 讀取（`hound`）、可能的 SQLite 資料庫讀取，以及 Parquet 讀取（用於 `.vibproj`）。每增加一種新格式都需要在命令層中穿引一條新的程式碼路徑。

**欄位對應是按檔案的，而非按 schema 的。** AIDPS 的 13 欄 schema 在所有 117 個 CSV 檔案中完全相同。目前的設計強制每次載入檔案都要進行欄位對應。沒有可套用至一組檔案的 schema 概念。

**`TimeseriesChunk` 使用 `HashMap<String, Vec<f64>>`** 作為通道。在 13 個 AIDPS 通道的情況下，這會造成每次 chunk 取值時 13 次獨立的堆積記憶體分配、13 次獨立的 IPC 序列化傳遞，且前端無法得知通道順序或分組（加速度 vs. 極值 vs. VRMS）。

---

## 2. 資料結構重新設計

遵循 Thompson 的原則：「如果有疑問，就用蠻力」以及 Torvalds 的原則：「好的程式設計師關注資料結構及其關係。」

核心洞察是系統有兩種運作模式，共享 90% 的機制但在資料組織方式上不同。與其對每種模式做特殊處理，資料結構應該將它們統一起來。

### 2.1 核心領域模型（Rust）

```
Project
  |-- mode: ProjectMode (SingleFile | AidpsProject)
  |-- devices: Vec<Device>           // SingleFile 模式：1 個設備含 1 個來源
  |-- annotations: HashMap<DeviceId, Vec<Annotation>>  // 按設備區分
  |-- settings: ProjectSettings
  |
  +-- Device
       |-- id: DeviceId
       |-- name: String
       |-- sources: Vec<DataSource>
       |-- sensor_id: Option<SensorId>   // 用於 WAV 關聯
       |-- schema: ColumnSchema          // 所有來源共用
       |-- time_range: (f64, f64)        // 從合併資料計算而得
       |
       +-- DataSource
            |-- kind: SourceKind (CsvFile | WavFile | ParquetFile)
            |-- path: PathBuf
            |-- row_count: usize
            |-- time_range: (f64, f64)

 ColumnSchema
  |-- time_column: String
  |-- channels: Vec<ChannelDef>
  |
  +-- ChannelDef
       |-- name: String
       |-- group: ChannelGroup (Acceleration | Extreme | Vrms | Other)

ProjectMode
  |-- SingleFile       // 透過「開啟檔案」開啟——1 個設備、1 個 CSV、手動對應
  |-- AidpsProject     // 透過「開啟專案」開啟——N 個設備、自動偵測 schema

SpectrumResult
  |-- source_wav: PathBuf
  |-- timestamp: f64
  |-- sample_rate: u32
  |-- fft_size: usize
  |-- frequencies: Vec<f64>
  |-- amplitudes: Vec<f64>
```

### 2.2 資料模型的關鍵設計決策

**以設備為分析單位，而非檔案。** 在單檔模式中，一個 Device 包裝一個 DataSource。在 AIDPS 模式中，一個 Device 包裝 20+ 個 DataSource（CSV 檔案），它們會被串接起來。系統的其餘部分（chunk 取值、統計、標註、匯出）始終操作 Device，永遠不直接操作原始檔案。這消除了特殊情況。

**帶有 ChannelGroup 的 ColumnSchema。** 與其將所有資料欄位視為未分類的 `Vec<String>`，通道攜帶分組中繼資料。這讓前端可以將 13 個 AIDPS 通道分組為邏輯子圖表（3 個加速度、6 個極值、3 個 VRMS），無需硬編碼通道名稱。

**標註以 DeviceId 為鍵。** 每個設備擁有自己的標註列表。這修復了目前的全域標註 bug，且天然支援按設備的標註儲存/載入。

**DataSource 作為 enum-dispatch 分發點。** 當 WAV 和 Parquet 支援被加入時，`SourceKind` 可以擴展而不修改現有程式碼路徑。CSV、WAV 和 Parquet 各有自己的讀取器，但結果都是以相同方式儲存的 Polars DataFrame。

### 2.3 關係圖

```
                    +------------------+
                    |     Project      |
                    |  mode            |
                    |  settings        |
                    +--------+---------+
                             |
                    owns 1..N|
                             v
                    +------------------+
                    |     Device       |
                    |  id, name        |
                    |  schema          |
                    |  sensor_id?      |
                    +---+---------+----+
                        |         |
               owns 1..N|         | owns 0..N
                        v         v
               +-------------+  +-------------+
               | DataSource  |  | Annotation  |
               | kind, path  |  | type, label |
               | time_range  |  | color, ...  |
               +-------------+  +-------------+
```

### 2.4 前端鏡像型別（TypeScript）

前端型別透過 IPC 序列化來鏡像 Rust 領域模型。與目前的關鍵差異：

- `Project` 取代了分散的 `datasets`/`chunks`/`statistics` store
- `Device` 取代 `VibrationDataset`——它攜帶 schema 和來源列表
- `ChannelGroup` 使前端能自動佈局子圖表
- `TimeseriesChunk` 增加有序的 `channel_names: string[]`（取代 HashMap 鍵迭代，JS 的 `Record` 不保證順序）

---

## 3. Rust 後端架構

### 3.1 建議的模組佈局

```
src-tauri/src/
  main.rs                          // 入口點（不變）
  lib.rs                           // Tauri builder 設定
  error.rs                         // AppError 列舉、From 實作、IPC 序列化
  state.rs                         // ProjectState（取代 AppState）

  models/
    mod.rs
    project.rs                     // Project、ProjectMode、ProjectSettings
    device.rs                      // Device、DataSource、SourceKind
    schema.rs                      // ColumnSchema、ChannelDef、ChannelGroup
    annotation.rs                  // Annotation、AnnotationType、AnnotationFile
    statistics.rs                  // StatisticsReport、AxisStats（統一）
    spectrum.rs                    // SpectrumResult、FftParams
    chunk.rs                       // TimeseriesChunk（帶有有序通道）

  services/
    mod.rs
    csv_reader.rs                  // preview_csv、read_csv、read_csvs_concat
    wav_reader.rs                  // read_wav_samples（未來）
    fft_engine.rs                  // compute_fft（未來）
    downsampling.rs                // lttb_indices（不變）
    stats_engine.rs                // compute_stats（統一介面）
    project_scanner.rs             // scan_aidps_folder、detect_schema
    project_file.rs                // save_vibproj、load_vibproj（ZIP+Parquet+JSON）
    time_filter.rs                 // filter_by_time_range（從重複程式碼中擷取）

  commands/
    mod.rs
    project.rs                     // open_project_folder、save_project、load_project_file
    device.rs                      // load_device、get_chunk、compute_device_stats
    annotation.rs                  // 儲存/載入標註（按設備）
    export.rs                      // export_csv、export_viewport
    spectrum.rs                    // get_spectrum（未來）
```

### 3.2 錯誤處理策略

以結構化錯誤列舉取代所有 `Result<T, String>`：

```
AppError
  |-- Io(std::io::Error)              // 找不到檔案、權限被拒
  |-- CsvParse { path, detail }       // CSV 解析失敗
  |-- ColumnNotFound { name }         // 請求的欄位不存在
  |-- DeviceNotFound { id }           // 專案中無此設備
  |-- InvalidTimeRange { start, end } // end < start
  |-- SchemaDetectionFailed { path }  // 無法自動偵測 AIDPS schema
  |-- WavRead { path, detail }        // WAV 解析失敗（未來）
  |-- FftError { detail }             // FFT 運算失敗（未來）
  |-- ProjectFile { detail }          // .vibproj 讀寫失敗（未來）
  |-- Internal(String)                // 非預期錯誤的兜底
```

實作 `From<std::io::Error>`、`From<polars::error::PolarsError>` 以便符合人體工學地使用 `?`。為該列舉實作 `serde::Serialize`，使 Tauri 能透過 IPC 發送錯誤，前端可以對錯誤型別進行模式匹配。

這正是 Knuth 所說的關鍵 3% 的最佳化——不是效能最佳化，而是開發者體驗的最佳化。結構化錯誤能在不進行字串串接的情況下傳遞上下文，並使前端能顯示適當的 UI（例如「找不到檔案」vs.「解析錯誤」vs.「不支援的格式」）。

### 3.3 狀態管理重新設計

以下列結構取代 `AppState { datasets: Mutex<HashMap<String, DatasetEntry>> }`：

```
ProjectState
  |-- project: RwLock<Option<Project>>     // None = 尚未開啟專案
  |-- dataframes: RwLock<HashMap<DeviceId, DataFrame>>  // 每個設備載入的資料
  |-- spectrum_cache: Mutex<LruCache<(DeviceId, i64), SpectrumResult>>  // 未來
```

關鍵變更：

1. **`RwLock` 取代 `Mutex`。** 多個命令可以同時讀取（chunk 取值、統計）。只有載入/卸載設備需要寫入鎖。這消除了 AIDPS 專案的競爭問題。

2. **頂層的 `Option<Project>`。** 系統要麼處於「無專案」狀態，要麼處於「已開啟一個專案」狀態。這比隱式的「零或多個資料集」更清晰，後者要求每個消費者都處理空值情況。

3. **DataFrame 與 Project 中繼資料分開儲存。** `Project` 結構體成本低廉，可以複製和序列化（不包含大量資料）。DataFrame 存放在獨立的 map 中，因此可以按設備獨立載入/卸載，而不需透過 IPC 序列化數兆位元組的資料。

4. **頻譜結果的 LRU 快取。** FFT 運算代價高昂。快取最近 N 次頻譜結果，可在使用者於標註之間瀏覽時避免重複計算。

### 3.4 基於 Trait 的服務介面

目前的程式碼庫使用獨立函式（`csv_reader::preview_csv`、`stats_engine::compute_basic_stats`）。在重新設計中，**不建議**為服務使用 trait——它們會增加間接層卻沒有好處，因為桌面應用不需要執行期多型或測試模擬。這符合 YAGNI 原則。

然而，有一個 trait 是合理的：

```
trait DataSourceReader {
    fn read_to_dataframe(&self, path: &Path, schema: &ColumnSchema) -> Result<DataFrame, AppError>;
    fn preview(&self, path: &Path) -> Result<CsvPreview, AppError>;
}
```

此 trait 使得可以基於 `SourceKind` 進行分發，而不需要在每個命令中寫 match 語句。CSV、WAV 和 Parquet 讀取器各自實作它。分發點是一個 `fn reader_for(kind: SourceKind) -> Box<dyn DataSourceReader>` 工廠函式。這是消除 Torvalds 所警告之特殊情況問題的最小 trait 表面。

### 3.5 擷取共用邏輯

**`time_filter.rs`** -- 擷取重複的時間範圍篩選：

```
fn filter_by_time_range(df: &DataFrame, start: f64, end: f64) -> Result<DataFrame, AppError>
```

由 `get_chunk` 和 `export_data` 共同呼叫。單一事實來源。

**`extract_column`** -- 將 `extract_f64_vec` 從 `data.rs` 擷取到共用工具模組，或者如果此模式反覆出現，更好的做法是將其作為 `DataFrameExt` 擴展 trait 上的方法。

---

## 4. 前端架構

### 4.1 Store 重新設計：以專案為中心

以分層架構取代目前分散的 4 個 store：

```
projectStore.ts          // 專案中繼資料、設備列表、啟用的設備
  |-- project: Project | null
  |-- activeDeviceId: string | null
  |-- derived: activeDevice, deviceList
  |-- actions: openSingleFile, openAidpsProject, closeProject, switchDevice

deviceDataStore.ts       // 按設備載入的資料（chunks、stats）
  |-- chunks: Map<DeviceId, TimeseriesChunk>
  |-- statistics: Map<DeviceId, StatisticsReport>
  |-- actions: fetchChunk, fetchStats, clearDevice

annotationStore.ts       // 按設備的標註（修復目前的全域 bug）
  |-- annotations: Map<DeviceId, Annotation[]>
  |-- selectedId: string | null
  |-- dirty: Map<DeviceId, boolean>
  |-- actions: add, update, remove, save, load

viewStore.ts             // UI 狀態（精度、縮放、版面偏好）
  |-- precision: PrecisionLevel
  |-- zoom: { start: number, end: number }
  |-- actions: setPrecision, setZoom

modeStore.ts             // 互動模式（不變，已經很乾淨）
  |-- mode: AppMode
  |-- rangeFirstClick: number | null

spectrumStore.ts         // 未來：頻譜資料快取
  |-- spectra: Map<string, SpectrumResult>
  |-- activeSpectrumKey: string | null
```

**關鍵修復：** 標註變為 `Map<DeviceId, Annotation[]>`。儲存和載入操作範圍限定在啟用的設備。這消除了資料遺失 bug。

**縮放狀態整合：** 將縮放追蹤移至 `viewStore.ts` 作為單一事實來源。圖表元件從中讀取；datazoom 處理器向其寫入。不再有重複的 `currentZoom` 變數。

### 4.2 元件拆分

目標：每個元件低於 200 行（包括模板和樣式）。

**目前的 `+page.svelte`（352 行）拆分為：**

| 新元件 | 職責 | 預估行數 |
|--------|------|----------|
| `+page.svelte` | 版面外殼，將元件分配到 grid 中 | ~60 |
| `ProjectController.svelte` | 檔案/專案開啟統籌、對話框生命週期 | ~80 |
| `ChartSection.svelte` | 總覽圖表 + 縮放取值接線 | ~60 |
| `ChannelAnalysisSection.svelte` | 啟用設備的按通道子圖表 | ~40 |
| `AnnotationController.svelte` | 待確認標註狀態機、確認/取消 | ~80 |
| `ExportController.svelte` | 完整匯出 + 視窗範圍匯出邏輯（或移至工具模組） | ~40 |

**目前的 `AnnotationPanel.svelte`（425 行）拆分為：**

| 新元件 | 職責 | 預估行數 |
|--------|------|----------|
| `AnnotationPanel.svelte` | 列表容器，渲染項目 | ~80 |
| `AnnotationCreateForm.svelte` | 待確認標註表單（標籤 + 色彩） | ~80 |
| `AnnotationEditForm.svelte` | 內聯編輯表單（標籤 + 色彩 + 偏移） | ~100 |
| `AnnotationListItem.svelte` | 單一標註列（顯示模式） | ~50 |
| `ColorPicker.svelte` | 預設色卡 + 自訂色彩輸入 | ~60 |

**目前的 `chartOptions.ts`（350 行）拆分為：**

| 新模組 | 職責 | 預估行數 |
|--------|------|----------|
| `chartOptions.ts` | `createOverviewOption`、`createSingleAxisOption`（骨架組裝） | ~80 |
| `chartSeries.ts` | 從 chunks + datasets 建構系列 | ~60 |
| `chartAnnotations.ts` | `buildMarkPoints`、`buildMarkAreas`、`buildSelectedRangeHandles` | ~80 |
| `chartFormatters.ts` | `formatTime`、tooltip 格式化器 | ~40 |
| `chartColors.ts` | 單一色彩調色盤定義、`getChannelColor` | ~20 |

### 4.3 圖表邏輯分離

`TimeseriesChart.svelte` 目前混合了 ECharts 生命週期管理與互動處理（用於標註放置和範圍邊界拖曳的 click、mousedown、mouseup、mousemove）。應拆分為：

1. **`useChart.ts`** -- 一個工具模組（或 Svelte action），處理 `echarts.init`、`ResizeObserver`、銷毀。回傳圖表實例參考。約 30 行。

2. **`useAnnotationInteraction.ts`** -- 封裝用於點放置、範圍邊界拖曳和游標變更的 click/mousedown/mouseup/mousemove 處理器。以圖表實例 + store 參考作為輸入。約 100 行。

3. **`TimeseriesChart.svelte`** -- 組合 `useChart` 和 `useAnnotationInteraction`。透過 props 接收資料（非 store 訂閱）。`$effect` 區塊呼叫 `chart.setOption`。約 60 行。

此拆分遵循原則：Svelte 元件擁有 DOM 元素和響應性；工具模組擁有互動邏輯和 ECharts API 呼叫。

### 4.4 擷取共用工具模組

| 工具 | 重複來源 |
|------|----------|
| `formatTime(epochSeconds: number): string` | `chartOptions.ts:21` 和 `ViewportDataTable.svelte:11` |
| `COLOR_PALETTE` | `dataStore.ts:27` 和 `chartOptions.ts:5` |
| `debounce` | 已擷取；無需變更 |

---

## 5. IPC 介面重新設計

### 5.1 目前的命令（7 個）

```
preview_csv_columns(file_path) -> CsvPreview
load_vibration_data(file_path, column_mapping) -> VibrationDataset
get_timeseries_chunk(dataset_id, start_time, end_time, max_points) -> TimeseriesChunk
compute_statistics(dataset_id) -> StatisticsReport
save_annotations(annotation_path, annotations) -> ()
load_annotations(annotation_path) -> Vec<Annotation>
export_data(dataset_id, output_path, start_time?, end_time?) -> String
```

### 5.2 建議的命令（共 13 個：7 個對應現有功能 + 6 個新增）

**專案生命週期（取代直接的檔案載入）：**

```
open_single_file(file_path) -> CsvPreview
  -- 預覽 CSV 以進行欄位對應（與目前的 preview_csv_columns 相同）

load_single_file(file_path, column_mapping) -> Project
  -- 建立單檔專案、載入資料、回傳完整 Project 中繼資料
  -- 取代 load_vibration_data；回傳 Project 而非 VibrationDataset

open_aidps_project(folder_path) -> Project
  -- 掃描 history/ 資料夾、偵測設備、自動偵測 schema
  -- 不載入資料（延遲載入）；回傳包含設備列表的 Project

close_project() -> ()
  -- 釋放所有記憶體中的資料
```

**設備資料（取代按 dataset-id 的命令）：**

```
load_device(device_id) -> DeviceSummary
  -- 為設備載入（或串接）所有 DataSource 到記憶體
  -- 回傳 time_range、total_points、channel_names（非資料本身）

get_chunk(device_id, start_time, end_time, max_points) -> TimeseriesChunk
  -- 與目前 get_timeseries_chunk 語意相同，以 device_id 為鍵

compute_statistics(device_id) -> StatisticsReport
  -- 語意相同，以 device_id 為鍵
```

**標註（範圍限定在設備）：**

```
save_annotations(device_id, annotations) -> ()
  -- 從專案 + 設備中繼資料推導檔案路徑
  -- AIDPS 模式：儲存到專案資料夾；單檔模式：旁檔 .vibann.json

load_annotations(device_id) -> Vec<Annotation>
  -- 與 save 對稱
```

**匯出：**

```
export_csv(device_id, output_path, start_time?, end_time?) -> ()
  -- 與目前的 export_data 相同
```

**頻譜（未來，為了介面完整性而包含）：**

```
get_spectrum(device_id, timestamp, fft_size?) -> SpectrumResult
  -- 透過設備的 sensor_id 找到最近的 WAV，讀取取樣、計算 FFT
```

**專案檔案（未來）：**

```
save_project_file(output_path) -> ()
  -- 將目前專案序列化為 .vibproj（ZIP + Parquet + JSON）

load_project_file(file_path) -> Project
  -- 反序列化 .vibproj，載入到狀態中
```

### 5.3 IPC 設計原則

1. **以設備為中心，而非以檔案為中心。** 每個資料命令接受 `device_id`，而非 `dataset_id` 或 `file_path`。後端負責將設備解析為檔案。

2. **Project 僅作為中繼資料回傳。** 透過 IPC 傳送的 `Project` 結構體包含設備列表、schema、時間範圍——但永遠不包含大量資料。大量資料透過 `get_chunk` 流動。

3. **延遲載入。** `open_aidps_project` 掃描資料夾結構但不解析 CSV 資料。`load_device` 解析並儲存到記憶體。`get_chunk` 提供切片。這防止在專案開啟時載入 117 個 CSV 檔案。

4. **標註路徑推導移至後端。** 目前前端計算 `filePath + '.vibann.json'`（`annotationStore.ts:46`）。此邏輯應存在於後端，因為後端能考慮專案模式（AIDPS 與單檔模式的標註儲存方式不同）。

---

## 6. 遷移策略

### 階段 0：準備性重構（無行為變更）

這些變更可以在目前架構上完成，不會破壞任何功能。每項都可以獨立合併。

**0a. 擷取共用工具模組。**
- 將 `formatTime` 移至 `src/lib/utils/formatTime.ts`。更新 `chartOptions.ts` 和 `ViewportDataTable.svelte` 的匯入。
- 將 `COLOR_PALETTE` 移至 `src/lib/constants/colors.ts`。更新 `dataStore.ts` 和 `chartOptions.ts` 的匯入。
- 從 `commands/data.rs` 和 `commands/export.rs` 的重複程式碼中擷取 `time_filter.rs`。

**0b. 引入 `AppError` 列舉。**
- 建立 `src-tauri/src/error.rs`，包含錯誤列舉和 `From` 實作。
- 逐一將命令從 `Result<T, String>` 變更為 `Result<T, AppError>`。
- 由於 `AppError` 實作了 `Serialize`，Tauri IPC 繼續正常運作。

**0c. 拆分 `chartOptions.ts`。**
- 將 `buildMarkPoints`、`buildMarkAreas`、`buildSelectedRangeHandles` 擷取到 `chartAnnotations.ts`。
- 將 `formatTime` 和 tooltip 格式化器擷取到 `chartFormatters.ts`。
- 將色彩調色盤和 `getChannelColor` 擷取到 `chartColors.ts`。
- `createOverviewOption` 和 `createSingleAxisOption` 保留在 `chartOptions.ts` 中，但變為更簡短的組裝函式。

**0d. 拆分 `AnnotationPanel.svelte`。**
- 擷取 `AnnotationCreateForm.svelte`、`AnnotationEditForm.svelte`、`AnnotationListItem.svelte`。
- `AnnotationPanel.svelte` 變為渲染這些子元件的容器。

### 階段 1：狀態架構遷移

**1a. 引入 `projectStore.ts`。**
- 建立 store，包含 `project: Writable<Project | null>` 和 `activeDeviceId`。
- 初始時，`dataStore.ts` 中的 `addFile` 建立一個合成的 `Project`（含一個 `Device`）。現有功能不變。
- 其他 store 從 `projectStore` 衍生，而非維護各自的 `datasets`/`datasetOrder`。

**1b. 修復按設備的標註。**
- 將 `annotationStore.ts` 從 `writable<Annotation[]>` 變更為 `writable<Map<string, Annotation[]>>`。
- 儲存/載入範圍限定在 `activeDeviceId`。
- 這修復了多檔案標註的資料遺失 bug。

**1c. 整合縮放狀態。**
- 將 `currentZoomStart`/`currentZoomEnd` 從 `+page.svelte` 移至 `viewStore.ts`。
- `TimeseriesChart.svelte` 從 `viewStore` 讀取縮放值，而非維護自己的 `currentZoom` 變數。使用「寫穿」模式：datazoom 處理器寫入 viewStore，`$effect` 從中讀取，並設有防護以避免回饋迴圈。

### 階段 2：Rust 後端重構

**2a. 建立 `models/device.rs`、`models/project.rs`、`models/schema.rs`。**
- 在現有型別旁定義新的領域模型型別。
- 現有的 `VibrationDataset` 暫時保留；新命令使用新型別。

**2b. 建立 `commands/project.rs` 和 `commands/device.rs`。**
- 將 `open_single_file` 和 `load_single_file` 實作為薄包裝器，建立一個含有一個 `Device` 的 `Project`，呼叫現有的 `csv_reader` 函式。
- 在 `lib.rs` 中將新命令與現有命令一併註冊。
- 前端可以開始對新功能使用新命令，同時舊命令仍然有效。

**2c. 以 `ProjectState` 取代 `AppState`。**
- 從 `Mutex` 切換到 `RwLock`。
- 將 `datasets: HashMap<String, DatasetEntry>` 遷移為 `dataframes: HashMap<DeviceId, DataFrame>`。
- 更新所有命令函式。由於所有命令都存取 state，這是一個單次提交的遷移。

**2d. 棄用舊命令。**
- 一旦所有前端程式碼使用新命令，移除 `load_vibration_data`、`get_timeseries_chunk`、`compute_statistics`（舊簽名）、`preview_csv_columns`。
- 移除 `models/vibration.rs::VibrationDataset`、`state.rs::DatasetEntry`。

### 階段 3：AIDPS 功能

**3a. 實作 `project_scanner.rs`。**
- `scan_aidps_folder(path)` 回傳 `Project`，設備從 `history/` 子目錄自動偵測。
- `detect_schema(csv_path)` 檢查 13 欄 AIDPS 格式。

**3b. 實作 `csv_reader::read_csvs_concat`。**
- 接受 `Vec<PathBuf>` + `ColumnSchema`，讀取每個 CSV，使用 Polars `concat` 串接，依時間排序。
- `load_device` 命令為 AIDPS 設備呼叫此函式。

**3c. 前端：設備選擇器 UI。**
- 在側邊欄 `FileList` 上方建立 `DeviceSelector.svelte` 元件。
- 選擇設備觸發 `load_device` + `get_chunk`。

### 階段 4：WAV + FFT（未來）

**4a. 新增 `hound` 和 `rustfft` 依賴。**
**4b. 實作 `wav_reader.rs` 和 `fft_engine.rs`。**
**4c. 實作 `get_spectrum` 命令。**
**4d. 前端：`SpectrumChart.svelte` + `spectrumStore.ts`。**

### 階段 5：專案檔案（未來）

**5a. 新增 `zip` 依賴，並在 Polars 上啟用 Parquet 功能。**
**5b. 實作 `project_file.rs`（儲存/載入 .vibproj）。**
**5c. 前端：儲存/載入專案選單項目。**

### 遷移原則

1. **平行共存。** 新命令和 store 與舊的並列新增。舊的僅在所有消費者遷移後才移除。在任何時間點，現有功能都不會中斷。

2. **每次合併一項結構性變更。** 每個階段子步驟（0a、0b、...）是一個可以獨立審查和測試的單一 PR。

3. **前端先行，後端跟進。** 前端 store 重構（階段 1）可以透過包裝舊後端命令的合成資料轉接器來進行。Rust 重構（階段 2）隨後提供真正的實作。

4. **在邊界處測試。** 服務的 Rust 單元測試（`csv_reader`、`downsampling`、`stats_engine`）在每個階段都會保留。新服務（`project_scanner`、`time_filter`）在建立時加入測試。前端 store 可以透過模擬的 `invoke` 呼叫進行測試。

---

## 附錄 A：本審查中參考的檔案

### Rust 後端
- `src-tauri/src/lib.rs` -- Tauri builder、命令註冊
- `src-tauri/src/state.rs` -- AppState 定義
- `src-tauri/src/commands/data.rs` -- 資料載入和 chunk 提供
- `src-tauri/src/commands/annotation.rs` -- 標註儲存/載入
- `src-tauri/src/commands/statistics.rs` -- 統計運算
- `src-tauri/src/commands/export.rs` -- CSV 匯出
- `src-tauri/src/services/csv_reader.rs` -- CSV 解析和欄位對應
- `src-tauri/src/services/downsampling.rs` -- LTTB 演算法
- `src-tauri/src/services/stats_engine.rs` -- 統計計算
- `src-tauri/src/models/vibration.rs` -- 資料模型型別
- `src-tauri/src/models/annotation.rs` -- 標註型別
- `src-tauri/src/models/statistics.rs` -- 統計型別
- `src-tauri/Cargo.toml` -- 依賴宣告

### 前端
- `src/routes/+page.svelte` -- 主頁面統籌器
- `src/lib/stores/dataStore.ts` -- 資料集狀態和 IPC 呼叫
- `src/lib/stores/annotationStore.ts` -- 標註狀態
- `src/lib/stores/modeStore.ts` -- 互動模式
- `src/lib/stores/viewStore.ts` -- 精度/檢視設定
- `src/lib/components/Chart/TimeseriesChart.svelte` -- 主圖表元件
- `src/lib/components/Chart/chartOptions.ts` -- ECharts 選項建構器
- `src/lib/components/Chart/SingleAxisChart.svelte` -- 按通道圖表
- `src/lib/components/Annotation/AnnotationPanel.svelte` -- 標註 UI
- `src/lib/components/Statistics/BasicStatsTable.svelte` -- 統計顯示
- `src/lib/components/DataTable/ViewportDataTable.svelte` -- 資料表格
- `src/lib/components/Layout/Toolbar.svelte` -- 工具列
- `src/lib/components/Layout/FileList.svelte` -- 檔案列表側邊欄
- `src/lib/components/ColumnMapping/ColumnMappingDialog.svelte` -- 欄位對應
- `src/lib/types/vibration.ts` -- TypeScript 資料型別
- `src/lib/types/annotation.ts` -- TypeScript 標註型別
- `src/lib/types/statistics.ts` -- TypeScript 統計型別
- `src/lib/utils/debounce.ts` -- debounce 工具

### 設計文件
- `docs/aidps-gap-analysis.md` -- AIDPS 整合需求
- `docs/research-project-file-format.md` -- .vibproj 格式研究
