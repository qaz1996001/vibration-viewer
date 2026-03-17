# 審查綜合報告：振動檢視器架構重新設計

> 日期：2026-03-17
> 輸入：4 份專家審查 + 差距分析 + 專案檔案研究 + 4 份哲學指南
> 目的：統一的物件導向、高效能、可維護架構行動方案

---

## 摘要

四份專家審查（架構師、程式碼審查、Rust 專家、資料管線）得出相同的核心結論：**目前的程式碼庫是結構良好的 MVP（最小可行產品），但必須從以檔案為中心演進為以專案為中心的架構，才能乾淨地加入 AIDPS 功能**。

### 跨審查共識矩陣

| 發現事項 | 架構師 | 程式碼審查 | Rust 專家 | 資料管線 |
|---------|:---------:|:-----------:|:-----------:|:-------------:|
| `unwrap()` 恐慌風險 | | x | x | |
| `fill_null(0.0)` 導致資料損毀 | | | | x |
| 手動遮罩 → Polars 惰性篩選 | | x | x | x |
| `Mutex` → `RwLock` | x | | x | |
| 時間範圍篩選重複實作 | x | x | x | x |
| `Result<T, String>` → `thiserror` | x | | x | |
| 以專案為中心的狀態階層 | x | | | x |
| `+page.svelte` 上帝元件 | x | x | | |
| `TimeseriesChart.svelte` 154 行 onMount | | x | | |
| `createOverviewOption` 157 行 | | x | | |
| 標註以裝置為單位（非全域） | x | | | |
| 時間排序 + 二分搜尋 | | | | x |
| 統計快取 | | | | x |
| 圖表程式碼中的 `any` 型別 | | x | | |
| 測試覆蓋率不足 | | x | x | |
| `Mutex` guard 持有期間執行檔案 I/O | x | | x | |
| JSON IPC 序列化瓶頸（50K 點 × 13 通道） | | | | x |
| 標註路徑計算應移至後端 | x | | | |
| `time_min`/`time_max` 不必要的 `Vec<f64>` 配置 | | | x | |
| `dataStore.ts` removeFile 巢狀 subscribe | | x | | |
| 日期時間格式硬編碼（單一格式） | | x | | |
| 裝置-感測器對應推論需驗證 UI | | | | x |

---

## 優先行動方案

### 第 0 階段：關鍵修正（不變更架構）

修正現有程式碼庫中的正確性錯誤與當機風險。

| # | 行動 | 來源 | 影響 |
|---|--------|--------|--------|
| 0.1 | **在 `csv_reader.rs` 中將 `fill_null(0.0)` 替換為 `fill_null(f64::NAN)`** | 資料管線 | 修正損毀的統計數據，防止 FFT 偽影 |
| 0.2 | **將命令處理器中所有 `unwrap()` 替換為 `?` + `map_err()`** | 程式碼審查 + Rust 專家 | 消除 8 個以上的當機路徑 |
| 0.3 | **在 `compute_shape_stats` 中防護 `std_dev == 0`** | 所有審查 | 修正常數資料的 NaN/Inf 統計問題 |
| 0.4 | **修正 `into_no_null_iter()` 與時間陣列的長度不匹配** | 資料管線 | 修正潛在的資料對齊錯誤 |
| 0.5 | **為 `handleSave`、`handleExport`、`saveAnnotations` 加入 try/catch** | 程式碼審查 | 使用者可獲得錯誤回饋 |
| 0.6 | **修正 `time_min`/`time_max` 提取**：改用 `series.min()`/`series.max()` 取代配置完整 `Vec<f64>` | Rust 專家 | 消除不必要的記憶體配置 |
| 0.7 | **修正 `preview_csv` 讀取整個檔案**：改為僅讀取前 N 行 | 資料管線 | 大檔案預覽效能從秒級降至毫秒級 |
| 0.8 | **修正 `dataStore.ts` removeFile 巢狀 subscribe 反模式** | 程式碼審查 | `activeDatasetId.update` 內不應包含巢狀 `subscribe` |

### 第 1 階段：程式碼品質（重構，不加新功能）

| # | 行動 | 來源 | 影響 |
|---|--------|--------|--------|
| 1.1 | **提取共用時間範圍篩選器**至 `services/time_filter.rs` | 全部 4 份審查 | DRY（不重複原則），單一錯誤處理點 |
| 1.2 | **將手動遮罩替換為 Polars 惰性篩選** | Rust 專家 + 資料管線 | 大型資料集加速 4-8 倍 |
| 1.3 | **引入 `AppError` 列舉搭配 `thiserror`** | 架構師 + Rust 專家 | 結構化錯誤，不再使用 `.map_err(|e| format!(...))` |
| 1.4 | **將 `Mutex` 改為 `RwLock`** | 架構師 + Rust 專家 | 並行讀取不再阻塞 |
| 1.5 | **將 `TimeseriesChart.svelte` 的 onMount 分解**為 5 個具名處理器 | 程式碼審查 | 154 行 → 每個約 30 行 |
| 1.6 | **將 `createOverviewOption` 拆分**為 4 個子函式 | 程式碼審查 | 157 行 → 每個約 40 行 |
| 1.7 | **提取重複項目**：`formatTime`、色彩配置、型別防護 | 程式碼審查 | DRY（不重複原則） |
| 1.8 | **修正 `dataStore.ts` 中的訂閱即執行反模式** | 程式碼審查 | 改用 `get()` |
| 1.9 | **新增單元測試**（按優先順序）：P0：`csv_reader` 時間格式分支、`stats_engine` 數值邊界（空序列、單值、常數）；P1：整合測試、標註往返；P2：純函式工具 | 程式碼審查 + Rust 專家 | 目標 30 個以上測試 |
| 1.10 | **為 ECharts 選項加上型別**——將 `any` 替換為 `LineSeriesOption` 等 | 程式碼審查 | 編譯期安全性 |
| 1.11 | **提取 `convert_time_column` 輔助函式**：`read_csv_with_mapping` 中 `match &time_dtype` 有 3 個非平凡分支，應獨立為函式 | 程式碼審查 | 降低函式複雜度 |
| 1.12 | **提取前端工具模組**：`useChart.ts`（ECharts 生命週期 init/destroy）+ `useAnnotationInteraction.ts`（click/drag 處理器），採用 Svelte action 模式 | 架構師 | 元件與互動邏輯分離 |
| 1.13 | **縮小 `Mutex` guard 作用域**：檔案 I/O 不可在持有 guard 期間執行，改用 block-scoped guard 模式 | Rust 專家 | 大型資料集載入時不再阻塞其他命令 |
| 1.14 | **確認 Polars 0.46 內建 `skew()`/`kurtosis()`**：若可用則移除手動迴圈計算 | Rust 專家 | 利用 Polars 內建 SIMD 最佳化 |

### 第 2 階段：架構重新設計（以專案為中心）

這是啟用所有 AIDPS 功能的結構性遷移。

#### 2A. 資料結構重新設計（Rust）

```
ProjectState
├── project_type: ProjectType (SingleFile | AidpsFolder | VibprojFile)
├── devices: HashMap<String, DeviceState>
│   ├── id: String
│   ├── name: String
│   ├── sources: Vec<DataSource>  // CSV 檔案、WAV 參照
│   ├── dataframe: Option<DataFrame>  // 合併後、依時間排序
│   ├── annotations: Vec<Annotation>  // 以裝置為單位，非全域
│   ├── statistics_cache: Option<StatisticsReport>
│   └── channel_schema: ChannelSchema  // 群組：加速度、極值、VRMS
├── sensor_mapping: HashMap<String, String>  // 裝置 → 感測器
├── metadata: ProjectMetadata
└── dirty: bool
```

**關鍵洞見**（Torvalds）：單檔案模式是專案模式的退化情形——一個裝置搭配一個資料來源。不需要特殊處理程式碼。

**NULL 處理策略**：DataFrame 應保留 Polars null（非 0.0 亦非 NaN），各消費者分別處理：
- CSV reader：移除 `fill_null`，保留原始 null
- 圖表：null 值自動斷線（ECharts 原生支援）
- 統計：使用 Polars 內建函式自動跳過 null
- LTTB：跳過 null 索引
- FFT：null 區段不計算，或以插值填補
- 匯出：保留原始 null，由使用者決定填補策略

**記憶體管理策略**：多裝置場景下，每裝置 1M 列 × 13 通道 ≈ 112 MB。需引入 LRU 快取驅逐：
- `spectrumStore.ts`：LRU 快取，最多保留 N 筆 FFT 結果
- `DeviceState.dataframe`：非活動裝置可卸載，僅保留中繼資料
- 載入門檻：> 1M 列時啟用延遲載入（3.1 MB 中繼 vs 112 MB 完整載入）

#### 2B. Rust 模組配置

```
src-tauri/src/
├── error.rs          ← AppError 列舉（thiserror）
├── state.rs          ← ProjectState + RwLock
├── commands/
│   ├── project.rs    ← open_project, save_project, load_project, close_project
│   ├── device.rs     ← load_device_data, get_device_chunk, get_device_stats
│   ├── annotation.rs ← save_annotations, load_annotations（以裝置為單位）
│   ├── export.rs     ← export_device_data
│   └── spectrum.rs   ← get_spectrum（WAV + FFT）
├── services/
│   ├── csv_reader.rs ← 預覽 + 讀取 + 串接
│   ├── time_filter.rs ← 共用篩選邏輯（Polars 惰性）
│   ├── downsampling.rs ← LTTB（不變）
│   ├── stats_engine.rs ← 計算 + 快取
│   ├── project_scanner.rs ← 掃描 AIDPS 資料夾結構
│   ├── project_file.rs ← .vibproj ZIP 讀寫
│   ├── wav_reader.rs ← hound WAV 讀取
│   └── fft_engine.rs ← rustfft FFT 計算
└── models/
    ├── project.rs    ← ProjectState, DeviceState, ChannelSchema
    ├── vibration.rs  ← ColumnMapping, CsvPreview, TimeseriesChunk
    ├── annotation.rs ← Annotation, AnnotationType（不變）
    ├── statistics.rs ← StatisticsReport（不變）
    └── spectrum.rs   ← SpectrumData { frequencies, amplitudes }
```

**標註路徑計算移至後端**：目前前端計算 `filePath + '.vibann.json'`，應改由後端根據專案模式（AIDPS vs. 單檔案）決定儲存位置。

**後端職責變更**：
- 標註檔案路徑由後端評估專案模式後決定
- `concat_df_horizontal` 用於同裝置多通道合併；`concat`（垂直）用於 AIDPS 多 CSV 時間串接
- WAV/FFT 計算使用 `tokio::task::spawn_blocking` 避免阻塞 async runtime

#### 2C. 前端 Store 重新設計

```
projectStore.ts     ← 根：專案類型、裝置列表、目前裝置、感測器對應
deviceDataStore.ts  ← 各裝置的資料區塊、載入狀態
annotationStore.ts  ← 各裝置的標註（以 device_id 為鍵）
spectrumStore.ts    ← FFT 快取（以 annotation_id 為鍵，LRU 驅逐策略）
uiStore.ts          ← 模式、精度、縮放狀態（合併自 modeStore + viewStore）
```

#### 2D. IPC 命令（共 13 個）

| 命令 | 階段 | 輸入 | 輸出 |
|---------|-------|-------|--------|
| `open_single_file` | 2 | file_path, column_mapping | ProjectState |
| `open_aidps_project` | 3 | folder_path | ProjectState |
| `save_project` | 4 | output_path | () |
| `load_project` | 4 | file_path | ProjectState |
| `load_device_data` | 2 | device_id | DeviceState |
| `get_device_chunk` | 2 | device_id, start, end, max_points, channels? | TimeseriesChunk |
| `get_device_stats` | 2 | device_id | StatisticsReport |
| `save_annotations` | 2 | device_id | () |
| `load_annotations` | 2 | device_id | Vec<Annotation> |
| `export_device_data` | 2 | device_id, output_path, time_range? | String |
| `get_spectrum` | 3 | device_id, timestamp, fft_size? | SpectrumData |
| `preview_csv` | 2 | file_path | CsvPreview |
| `close_project` | 2 | | () |

### 第 3 階段：AIDPS 整合

建構於第 2 階段以專案為中心的架構之上：
- `project_scanner.rs`：掃描 `history/` → 依裝置分組 → 偵測 13 欄位架構
- `csv_reader.rs`：新增 `concat_csvs(paths, mapping)`，使用 Polars `concat` + 時間排序 + **去重**（時間欄位）
- `DeviceSelector.svelte`：以裝置導向的導覽取代 FileList
- 偵測到架構時自動跳過 ColumnMappingDialog
- **裝置-感測器對應驗證 UI**：推論的對應關係需使用者確認/修改（編號推論可能不正確）
- **CSV 時間重疊/缺口處理**：串接時排序 + 去重，偵測並警告時間不連續區段

### 第 4 階段：WAV + FFT + .vibproj

建構於第 3 階段之上：

**WAV + FFT：**
- `wav_reader.rs`：使用 hound 讀取，**取樣率從 WAV header 自動偵測**（不可硬編碼）
- `fft_engine.rs`：可參數化——視窗函式（Hanning/Hamming）、FFT 大小、重疊率
- `SpectrumChart.svelte`：ECharts 頻域圖表
- 在 `AnnotationPanel` 中建立標註 → 頻譜的連結
- 使用 `tokio::task::spawn_blocking` 執行 WAV/FFT 計算，避免阻塞
- **多裝置頻譜比較模式**（未來）：同一時間點不同裝置的頻譜疊加

**`.vibproj` 檔案格式：**

```
project.vibproj (ZIP + Zstandard 壓縮)
├── meta.json              # 版本、日期、專案後設資料
├── annotations.json       # AnnotationFile 結構
└── data.parquet           # 時間序列（timestamp + 各通道）

壓縮效益：1M 行 × 4 列 f64 → 32 MB 原始 → 8-12 MB 壓縮（Zstandard 3-5 倍）
```

**已知技術限制：**
- `zip` crate 的 `ZipFile` 未實作 `Seek`，必須完整緩衝到 `Vec<u8>` 後才能傳給 Polars
- 記憶體映射 Parquet + `scan_parquet()` 的 predicate pushdown 在 ZIP 擷取後失效
- MessagePack/CBOR 無法部分讀取，對 1M+ 行不可行（已排除）
- Arrow IPC 適用於進程間通訊，不適合專案檔案持久化（已排除）

**SQLite 升級觸發條件**（若未來需求超出 ZIP 容器能力）：
1. 增量保存 2. 撤銷/重做 3. 範圍查詢 4. 並行存取 5. 當機恢復
6. 時間戳索引 7. WAL 模式 8. 多表關聯 9. 交易完整性

### 第 5 階段：效能最佳化（視規模需求）

當資料規模達到 1M+ 列或 AIDPS 多裝置場景時啟動：

#### 效能預算與瓶頸分析

| 場景 | 篩選延遲 | 主要瓶頸 |
|------|----------|----------|
| 28K 列 × 3 通道 | ~4 ms | 無（可接受） |
| 28K 列 × 13 通道 | ~15 ms | 無（可接受） |
| 1M 列 × 13 通道 | ~70 ms | JSON 序列化 + 解析 |
| 10M+ 列 | > 200 ms | LTTB + JSON 序列化 |

**JSON IPC 序列化瓶頸**：50K 點 × 13 通道 = 700K 浮點數 → 7-10 MB 承載量，序列化 ~20 ms + 解析 ~40 ms。

| # | 行動 | 影響 |
|---|--------|--------|
| 5.1 | **`get_device_chunk` 加入 `channels?: Vec<String>` 參數**：僅傳回可見通道，減少 IPC 資料量 | 13 通道 → 3 通道可減少 ~77% |
| 5.2 | **截斷浮點精度**：序列化時限制小數位數（如 6 位） | 減少 ~20% JSON 大小 |
| 5.3 | **評估二進位 IPC**（MessagePack 或自定義 binary protocol） | 消除序列化/解析開銷 |
| 5.4 | **排序 + 二分搜尋**取代線性篩選：時間排序後的 DataFrame 使用 `search_sorted` | 1000 倍篩選加速（微秒 vs 毫秒） |
| 5.5 | **多解析度金字塔預計算**（5K/15K/50K 點）：載入時預先降採樣 | 10M+ 列時消除即時 LTTB 成本 |
| 5.6 | **LTTB 複製開銷最佳化**：目前 14 次配置、80 KB/次、1.1 MB 總計 | 1M+ 列時節省 ~7 ms |

---

## 新增 Rust 依賴套件

| 套件 | 版本 | 用途 | 階段 |
|-------|---------|---------|-------|
| `thiserror` | 2.x | 結構化錯誤型別 | 1 |
| `hound` | 3.5 | WAV 檔案讀取 | 4 |
| `rustfft` | 6.2 | FFT 計算 | 4 |
| `zip` | 8.2 | .vibproj 容器 | 4 |
| polars `parquet` 功能 | - | .vibproj 中的 Parquet | 4 |

---

## 設計反模式警告

以下為各審查明確建議**避免**的做法：

| 反模式 | 來源 | 說明 |
|--------|------|------|
| 通用 Trait 多型 | 架構師 + Rust 專家 | 僅 `DataSourceReader` factory 用於 reader dispatch，不做通用 service interface。YAGNI——等到第二種實作變體再引入。 |
| `DashMap` | Rust 專家 | 過度設計——讀多寫少場景用 `RwLock` 即足夠 |
| `anyhow` | Rust 專家 | 適用於 CLI 工具，不適用於函式庫/應用程式——改用 `thiserror` 定義結構化錯誤 |
| `tokio` 直接依賴 | Rust 專家 | Tauri 已內建 async runtime，額外引入 tokio 增加複雜度且可能衝突 |
| 雙重型別轉換 | 程式碼審查 | `Toolbar.svelte:22` 的 `as HTMLSelectElement` + `as PrecisionLevel` 連續轉換——應使用型別安全的解析 |
| `DataFrameExt` extension trait | 架構師 | 已草擬但暫不實作——目前 `extract_f64_vec` helper 已足夠，等功能擴展再評估 |

---

## Crate 評估矩陣

| Crate | 建議 | 成熟度 | 風險 | 階段 |
|-------|------|--------|------|------|
| `thiserror` 2.x | **採用** | 穩定 | 低 | 1 |
| `hound` 3.5 | **採用** | 穩定 | 低 | 4 |
| `rustfft` 6.2 | **採用** | 穩定 | 低 | 4 |
| `zip` 8.2 | **採用** | 穩定 | 中（ZipFile 無 Seek） | 4 |
| polars `parquet` | **採用** | 穩定 | 低 | 4 |
| `DashMap` | **不採用** | 穩定 | 過度設計 | - |
| `anyhow` | **不採用** | 穩定 | 語義不足 | - |
| `tokio` | **不採用** | 穩定 | 與 Tauri runtime 衝突 | - |

---

## 已知 Bug 完整清單

| # | Bug | 嚴重度 | 位置 | 階段 |
|---|-----|--------|------|------|
| B1 | `fill_null(0.0)` 導致統計數據損毀 + FFT 偽影 | **高** | `csv_reader.rs` | 0 |
| B2 | 命令處理器中 8+ 個 `unwrap()` 可導致 panic + Mutex 中毒 | **高** | `commands/*.rs` | 0 |
| B3 | `std_dev == 0` 時 shape stats 產生 NaN/Inf | **中** | `stats_engine.rs` | 0 |
| B4 | `into_no_null_iter()` 與時間陣列長度不匹配導致資料錯位 | **中** | `csv_reader.rs` | 0 |
| B5 | `time_min`/`time_max` 配置完整 `Vec<f64>` 浪費記憶體 | **低** | `data.rs` | 0 |
| B6 | `preview_csv` 讀取整個檔案而非前 N 行 | **低** | `csv_reader.rs` | 0 |
| B7 | 單通道 LTTB 在代表性通道變異度低時可能遺漏其他通道尖峰 | **低** | `downsampling.rs` | 已知限制 |
| B8 | 點標註指派給活動資料集的第一欄 | **低** | 前端 | 已知問題 |

---

## 哲學對齊檢核表

| 原則 | 如何應用 |
|-----------|------------|
| **Thompson：資料結構優先** | 在撰寫任何程式碼前先設計 ProjectState 階層。單檔案是退化情形，不是特殊情形。 |
| **Thompson：簡單、由下而上** | 每個服務都是獨立單元。不用框架、不用 DI 容器。純函式 + 結構體。 |
| **Torvalds：消除特殊情形** | 一個裝置搭配一個資料來源 = 單檔案模式。不需要 `if (isProject)` 分支。 |
| **Torvalds：函式短小精悍** | 所有函式目標 < 24 行。`onMount` 拆分為 5 個。`createOverviewOption` 拆分為 4 個。 |
| **Fowler：YAGNI（你不會需要它）** | Trait 已草擬但不實作，直到需要第二種資料來源變體時才實作。 |
| **Fowler：程式碼異味引導重構** | 辨識出 30 個異味 → 在第 0-1 階段進行優先修正。 |
| **Knuth：先求正確再求最佳化** | 第 0 階段修正正確性（null 處理、當機路徑）。第 1 階段最佳化（惰性篩選、RwLock）。 |
| **Knuth：不要忽略關鍵的 3%** | 時間範圍篩選是熱點路徑。Polars 惰性篩選 + 排序二分搜尋加以處理。手動遮罩 → 惰性篩選可達 4-8 倍加速（AVX2 SIMD + 多執行緒）。JSON IPC 序列化在 1M+ 列時成為主要瓶頸（60 ms），需評估二進位 IPC。 |

---

## 審查文件

| 文件 | 重點 | 位置 |
|----------|-------|----------|
| 架構審查 | 結構、資料模型、遷移 | `docs/architecture-review.md` |
| 程式碼品質審查 | 30 個程式碼異味、函式稽核、建議 | `docs/code-quality-review.md` |
| Rust 專家審查 | 慣用 Rust、效能、trait、安全性 | `docs/rust-expert-review.md` |
| 資料管線審查 | 查詢效能、記憶體、null 處理、可擴展性 | `docs/data-pipeline-review.md` |
| AIDPS 差距分析 | AIDPS 資料整合的功能缺口 | `docs/aidps-gap-analysis.md` |
| 專案檔案研究 | .vibproj 格式評估（比較 7 種格式） | `docs/research-project-file-format.md` |
| HashMap 序列化審查 | Rust HashMap 序列化問題與解法 | `docs/rust-hashmap-serialization-review.md` |
| Markpoint Bug 架構審查 | 標註 markpoint 錯誤的架構分析 | `docs/annotation-markpoint-bug-arch-review.md` |
| Markpoint 修復設計 | markpoint 重繪修復方案 | `docs/design-markpoint-refresh-fix.md` |
