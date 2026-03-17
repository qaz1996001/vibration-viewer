# Rust 後端專家審查

**審查者視角：** Rust 系統工程師，評估慣用模式（idiomatic patterns）、效能、安全性及架構可擴展性。

**程式碼快照：** `src-tauri/src/` -- 8 個原始碼檔案，約 350 行應用程式 Rust 程式碼。

**結論：** 程式碼功能完整、精簡且能正確運行。以下問題依風險與效益排序；大多數是低成本修正，卻能在可靠性、可測試性和效能方面帶來顯著回報——尤其是當專案朝向多檔案 AIDPS 資料匯入和 FFT 分析發展時。

---

## 1. Rust 慣用寫法問題

### 1.1 `Mutex::lock().unwrap()` -- 鎖中毒時靜默 panic

**位置：** `commands/data.rs:45,65`、`commands/export.rs:14`、`commands/statistics.rs:12`（共四處）。

`lock().unwrap()` 會在*任何*先前持有者發生 panic 時終止 Tauri 程序。這是程式碼庫中最危險的模式，因為 `data.rs` 中的命令處理器已經包含可能*導致* panic 的 `.unwrap()` 鏈（見 1.2），而一旦鎖被毒化（poisoned），後續每次 IPC 呼叫都會崩潰。

```rust
// 目前寫法 -- 鎖中毒時 panic
let datasets = state.datasets.lock().unwrap();

// 修正 -- 從中毒狀態復原（內部資料仍然有效）
let datasets = state.datasets.lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());

// 或者，將錯誤向上傳播
let datasets = state.datasets.lock()
    .map_err(|_| "Internal state lock was poisoned".to_string())?;
```

**建議：** 使用 `unwrap_or_else(|p| p.into_inner())` 模式。Polars DataFrame 能在 panic 導致的不一致狀態中存活——它們是不可變值，而非有狀態的資源。如果不需要復原機制，至少應轉換為 `map_err`，讓前端收到乾淨的錯誤訊息，而非程序直接終止。

### 1.2 熱路徑中裸露的 `.unwrap()`

**位置（不完全列舉）：**

| 檔案 | 行號 | 運算式 |
|---|---|---|
| `data.rs` | 72-75 | `.column("time").unwrap().f64().unwrap()` |
| `data.rs` | 124-128 | `extract_f64_vec` -- 兩個 unwrap |
| `stats_engine.rs` | 51 | `series.f64().unwrap()` |
| `stats_engine.rs` | 72 | `sorted_series.f64().unwrap()` |

這些 unwrap 的防禦依據是「csv_reader 總是產生名為 `time` 的 f64 欄位」這個不變量（invariant）。這個不變量*目前*是正確的，但：
- 它並未在型別系統中被編碼。
- 未來的程式碼變更（例如新增 Parquet 匯入）可能會靜默地違反它。
- 在 `Mutex::lock()` 守衛（guard）內部的 panic 會毒化鎖（見 1.1）。

**修正模式：** 讓 `extract_f64_vec` 回傳 `Result`：

```rust
fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Result<Vec<f64>, String> {
    Ok(df.column(col_name).map_err(|e| e.to_string())?
        .f64().map_err(|e| e.to_string())?
        .into_no_null_iter()
        .collect())
}
```

### 1.3 到處使用 `Result<T, String>` -- 缺乏結構化錯誤

每個命令都回傳 `Result<T, String>`。這與 Tauri 的序列化機制相容，但：
- 呼叫端無法針對錯誤變體（variant）進行匹配。
- 錯誤訊息不一致（混用英文和中文：`"Dataset not found"` vs `"資料集不存在"` -- 中文版已被移除，但這說明了漂移風險）。
- 新增重試邏輯或遙測功能時需要解析字串。

**使用 `thiserror` 的範例：**

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Dataset not found: {0}")]
    DatasetNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("CSV parse error: {0}")]
    CsvParse(#[from] polars::error::PolarsError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("State lock poisoned")]
    LockPoisoned,
}

// Tauri 要求命令錯誤實作 Serialize：
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
```

這消除了程式碼庫中所有的 `.map_err(|e| e.to_string())` 和 `.map_err(|e| format!(...))` 呼叫——大約 20 處。命令將簡化為：

```rust
pub fn load_vibration_data(...) -> Result<VibrationDataset, AppError> {
    let df = csv_reader::read_csv_with_mapping(&file_path, &column_mapping)?;
    let time_col = df.column("time")?.f64()?;
    // ...
}
```

### 1.4 自由函式 vs 領域型別上的方法

`stats_engine` 暴露了三個自由函式，全部接受 `(&Series, &str)` 參數。這是程序式風格（procedural style）；慣用的 Rust 寫法應該：

- 在 `StatisticsReport` 上加入 `impl` 區塊並提供建構子：
  ```rust
  impl StatisticsReport {
      pub fn from_dataframe(df: &DataFrame, columns: &[String]) -> Result<Self, AppError> { ... }
  }
  ```
- 或者使用 trait（見第 3 節）。

自由函式風格並沒有*錯*——根據 YAGNI 原則，在第二個使用場景出現前不需要重構。但當 AIDPS 多檔案統計功能到來時，就會需要結構化設計。

### 1.5 `DatasetEntry::metadata` 上的 `#[allow(dead_code)]`

```rust
pub struct DatasetEntry {
    #[allow(dead_code)]
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}
```

`metadata` 實際上在 `commands/data.rs:68`（`entry.metadata.column_mapping.data_columns`）和 `commands/statistics.rs:15` 中被使用。`#[allow(dead_code)]` 是不正確的——Rust 對該*欄位*發出警告，是因為 `DatasetEntry` 從未在 crate 外部透過公開 API 被建構，而非因為該欄位未被使用。修正方式是移除 allow 屬性，然後將 struct 設為 `pub(crate)`（它實際上就是如此）或新增一個建構子。這雖然是表面問題，但顯示警告被壓制時未經調查。

---

## 2. 效能分析

### 2.1 手動迭代器遮罩 vs Polars 延遲篩選（高影響）

**位置：** `commands/data.rs:71-78` 和 `commands/export.rs:20-28`。

時間範圍篩選透過在 Rust 中逐行迭代來建構 `BooleanChunked`：

```rust
let mask = df.column("time").unwrap().f64().unwrap()
    .into_iter()
    .map(|opt| opt.is_some_and(|t| t >= start_time && t <= end_time))
    .collect::<BooleanChunked>();
let filtered = df.filter(&mask).map_err(|e| e.to_string())?;
```

這抵消了 Polars 的 SIMD（單指令多資料流）加速比較核心。改用：

```rust
let filtered = df.clone().lazy()
    .filter(
        col("time").gt_eq(lit(start_time))
            .and(col("time").lt_eq(lit(end_time)))
    )
    .collect()
    .map_err(|e| e.to_string())?;
```

**為何重要：** 對於 1000 萬行的資料集（AIDPS 多檔案合併），手動迭代器是單執行緒純量比較。Polars 的延遲篩選使用 Arrow 的 SIMD 比較加上可選的多執行緒執行。預期加速：在支援 AVX2 的現代 x86 處理器上可達 4-8 倍。

**關於 `df.clone()` 的疑慮：** Polars 中 `DataFrame::clone()` 成本很低——它複製的是 `Arc<Vec<u8>>` 底層緩衝區（引用計數遞增，而非資料複製）。真正的複製發生在 `.filter()` 中，無論如何都會配置篩選後的輸出。因此 `df.clone().lazy()` 並非效能問題。

### 2.2 export.rs 中的完整 DataFrame 複製

```rust
_ => df.clone(), // 第 31 行 -- 無時間篩選時的完整複製
```

緊接著：

```rust
CsvWriter::new(&mut file)
    .finish(&mut export_df.clone()) // 第 37 行 -- 第二次複製
```

兩個問題：
1. 無篩選路徑上的 `df.clone()` 是 `Arc` 遞增（成本低），但*語義上*是不必要的——`CsvWriter::finish` 接受 `&mut DataFrame` 但實際上不會修改資料。雙重複製的原因是寫入器需要 `&mut`，而我們持有的是來自 `MutexGuard` 的不可變借用。
2. 修正：只複製一次，同時用於可選的篩選和寫入。

```rust
let mut export_df = match (start_time, end_time) {
    (Some(start), Some(end)) => { /* 延遲篩選 */ },
    _ => df.clone(),
};
CsvWriter::new(&mut file).finish(&mut export_df)?;
```

### 2.3 LTTB 單通道索引選擇應用於多通道資料

`get_timeseries_chunk` 使用 `data_columns[0]` 作為 LTTB（最大三角形三桶演算法）索引選擇的代表通道，然後將這些索引套用到所有通道（X、Y、Z）。

**正確性疑慮：** 如果 Z 通道在索引 5000 處有尖峰，但 X 通道（代表通道）在該處是平坦的，則降採樣輸出中不會保留該尖峰。對於 X/Y/Z 通常具有相關性（同一物理事件）的振動資料，這通常是可接受的。但對於 13 欄的 AIDPS 結構（x/y/z + max/min + vrms），各通道可能會顯著分歧。

**替代方案（目前不建議實作——根據 YAGNI 原則）：**
- **索引聯集：** 對每個通道執行 LTTB，聯集索引集，排序。最壞情況產生 `threshold * num_channels` 個點（因重疊通常約為 1.5 倍）。
- **複合訊號：** 使用 `sqrt(x^2 + y^2 + z^2)` 作為代表。對振動幅度較佳，但會失去軸向特徵。
- **逐通道 LTTB：** 為每個通道回傳獨立的索引集。增加前端複雜度。

**建議：** 維持目前的做法。以註解記錄此取捨。若 AIDPS 使用者反映特定通道遺漏峰值，再行檢討。

### 2.4 統計計算：手動迴圈 vs Polars 運算式

`compute_shape_stats` 手動迭代 Series 來計算偏度（skewness）和峰度（kurtosis）。這是正確的，但遺漏了 Polars 自 0.30+ 版本起提供的內建 `.skew()` 和 `.kurtosis()` 方法：

```rust
// 目前：15 行手動迭代
let ca = series.f64().unwrap();
let mut sum3 = 0.0;
let mut sum4 = 0.0;
for val in ca.into_iter().flatten() { ... }

// Polars 內建（若啟用 "moment" 或適當的 feature）：
// series.skew(false)  -- bias=false 為樣本偏度
// series.kurtosis(false, false)  -- fisher=false 為超值峰度
```

請確認 Polars 0.46 是否暴露 `Series::skew()` 和 `Series::kurtosis()`。若有，手動迴圈可以替換為兩個函式呼叫。若無，手動迴圈也沒問題——它是 O(n) 且無配置，已經是最佳解。

**關於 `compute_distribution_stats` 的備註：** 它呼叫 `series.sort()` 來計算百分位數。這是 O(n log n)。對於百分位數計算，選擇演算法（selection algorithm）為 O(n) 會更快，但 Polars 未在 Series 上暴露此功能。排序方式對最多約 1000 萬行的資料集是可接受的。

### 2.5 `preview_csv` 讀取整個檔案

```rust
pub fn preview_csv(file_path: &str) -> Result<CsvPreview, String> {
    let df = CsvReadOptions::default()
        .try_into_reader_with_file_path(Some(path.into()))
        .map_err(|e| ...)?
        .finish()   // <-- 讀取所有列
        .map_err(|e| ...)?;
    let columns = df.get_column_names();
    let row_count = df.height();
```

對於僅需要欄位名稱和列數的預覽功能，讀取整個檔案是浪費的。對於 500MB 的 CSV，這可能需要數秒。

**修正：** 使用 `n_rows` 僅讀取少量列以取得欄位名稱，並使用獨立的列數機制：

```rust
let df = CsvReadOptions::default()
    .with_n_rows(Some(0))  // 僅讀取結構描述，零資料列
    .try_into_reader_with_file_path(Some(path.into()))
    .map_err(|e| ...)?
    .finish()?;
let columns = df.get_column_names();

// 對於列數，掃描行數（或在完整載入前接受「未知」）
```

替代方案是接受列數為近似值，將其回傳為 `Option<usize>`，延遲到 `load_vibration_data` 時再取得精確數值。

### 2.6 並行 Tauri 命令下的 Mutex 競爭

Tauri 在執行緒池上調用命令。如果前端在 `compute_statistics` 執行時發送 `get_timeseries_chunk`，其中一個會被另一個阻塞。兩者都是唯讀操作。這是使用 `RwLock`（讀寫鎖）的經典場景：

```rust
pub struct AppState {
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
}
```

只有 `load_vibration_data` 需要寫入鎖。所有其他命令（chunk、stats、export）取得讀取鎖即可並行執行。

**影響：** 目前影響不大（單一使用者、循序的 UI 互動），但在 AIDPS 批次處理或前端為多個面板並行化 chunk 請求時會變得顯著。

---

## 3. 基於 Trait 的架構提案

目前的程式碼非常適合其範圍。Trait 在**今天不是必要的**——根據 YAGNI 原則，當第二個實作變體出現時再引入。以下是 AIDPS 多檔案和 WAV/FFT 功能新增時的架構草稿。

### 3.1 DataLoader trait

```rust
pub trait DataLoader: Send + Sync {
    fn load(&self, source: &DataSource) -> Result<DataFrame, AppError>;
    fn preview(&self, source: &DataSource) -> Result<CsvPreview, AppError>;
}

pub enum DataSource {
    SingleCsv { path: PathBuf, mapping: ColumnMapping },
    DirectoryScan { dir: PathBuf, pattern: String },
    WavFile { path: PathBuf },
    ProjectFile { path: PathBuf },  // .vibproj
}
```

**實作：** `CsvLoader`（目前的程式碼）、`DirectoryLoader`（AIDPS：glob + 合併 + 排序）、`WavLoader`（hound）、`ProjectLoader`（.vibproj ZIP）。

**測試效益：** Mock `DataLoader` 回傳已知的 DataFrame，無需存取檔案系統：

```rust
struct MockLoader;
impl DataLoader for MockLoader {
    fn load(&self, _: &DataSource) -> Result<DataFrame, AppError> {
        Ok(df!("time" => &[0.0, 1.0, 2.0], "x" => &[1.0, 2.0, 3.0])?)
    }
}
```

### 3.2 Downsampler trait

```rust
pub trait Downsampler: Send + Sync {
    fn downsample(
        &self,
        time: &[f64],
        channels: &HashMap<String, Vec<f64>>,
        max_points: usize,
    ) -> DownsampleResult;
}

pub struct DownsampleResult {
    pub indices: Vec<usize>,
    pub method: &'static str,  // "lttb", "min-max", "average"
}
```

這使得可以在不更改命令處理器的情況下，將 LTTB 替換為 min-max（更適合包絡線視覺化）或逐通道 LTTB。

### 3.3 StatisticsEngine trait

```rust
pub trait StatisticsEngine: Send + Sync {
    fn compute(&self, df: &DataFrame, columns: &[String]) -> Result<StatisticsReport, AppError>;
}
```

預設實作即為目前的程式碼。FFT 增強版實作可新增頻域統計（主頻率、THD（總諧波失真）、波峰因數），而無需修改命令層。

### 3.4 SpectrumAnalyzer trait（未來）

```rust
pub trait SpectrumAnalyzer: Send + Sync {
    fn fft(&self, signal: &[f64], sample_rate: f64) -> FrequencySpectrum;
    fn spectrogram(
        &self,
        signal: &[f64],
        sample_rate: f64,
        window_size: usize,
        overlap: usize,
    ) -> Spectrogram;
}
```

### 3.5 將 trait 接入 Tauri 狀態

```rust
pub struct AppState {
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
    pub loader: Box<dyn DataLoader>,
    pub downsampler: Box<dyn Downsampler>,
    pub stats_engine: Box<dyn StatisticsEngine>,
}
```

命令變成輕量的分派器：

```rust
#[tauri::command]
fn compute_statistics(id: String, state: State<AppState>) -> Result<StatisticsReport, AppError> {
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
    let entry = datasets.get(&id).ok_or(AppError::DatasetNotFound(id))?;
    state.stats_engine.compute(&entry.dataframe, &entry.metadata.column_mapping.data_columns)
}
```

**何時引入此架構：** 當第一個非 CSV 資料來源（WAV 或 AIDPS 目錄掃描）被實作時。不要提前引入。

---

## 4. Polars 使用審查

### 4.1 延遲運算式 -- 部分使用

`csv_reader.rs` 正確地使用了 `.lazy().with_columns([...]).collect()` 進行欄位轉換。很好。

`data.rs` 和 `export.rs` **未**使用延遲模式進行時間篩選——它們建構手動的 `BooleanChunked` 遮罩。這應該改用延遲模式（見 2.1）。

### 4.2 可簡化程式碼的缺漏 Polars 功能

| 目前的程式碼 | Polars 替代方案 | 效益 |
|---|---|---|
| 手動 `BooleanChunked` 篩選 | `df.lazy().filter(col("time").is_between(...))` | SIMD、多執行緒 |
| `time_vec.iter().cloned().reduce(f64::min)` | `series.min::<f64>()` | 已由 Polars 計算 |
| 手動偏度/峰度迴圈 | `series.skew()` / `series.kurtosis()` | 更少程式碼，可能使用 SIMD |
| `preview_csv` 讀取所有列 | `CsvReadOptions::default().with_n_rows(Some(0))` | 僅讀取結構描述 |
| 未來多檔案合併 | `polars::functions::concat` | 內建合併 + 排序 |

### 4.3 `load_vibration_data` 中的時間範圍提取

```rust
let time_vec: Vec<f64> = time_ca.into_no_null_iter().collect();
let time_min = time_vec.iter().cloned().reduce(f64::min).unwrap_or(0.0);
let time_max = time_vec.iter().cloned().reduce(f64::max).unwrap_or(0.0);
```

這將整個時間欄位收集到 `Vec<f64>` 中，僅僅是為了求最小值/最大值。直接使用 Polars：

```rust
let time_min = time_col.min::<f64>().map_err(|e| e.to_string())?
    .unwrap_or(0.0);
let time_max = time_col.max::<f64>().map_err(|e| e.to_string())?
    .unwrap_or(0.0);
```

這避免了潛在的大量記憶體配置（8 位元組 * N 列），並在單次 Polars 核心傳遞中完成。

### 4.4 用於 AIDPS 多檔案匯入的 `concat`

當 AIDPS 到來時，使用：

```rust
use polars::functions::concat_df_horizontal;  // 或 concat 用於垂直合併
let frames: Vec<LazyFrame> = paths.iter().map(|p| read_single_csv(p)).collect();
let combined = polars::functions::concat(frames, UnionArgs::default())?
    .sort(["time"], SortMultipleOptions::default())
    .collect()?;
```

### 4.5 用於 .vibproj 的 Parquet

實作 .vibproj 時，在 Polars feature flags 中新增 `"parquet"`：

```toml
polars = { version = "0.46", features = ["lazy", "csv", "parquet", "temporal", ...] }
```

Parquet 是欄式儲存、壓縮且保留型別——非常適合歸檔已處理的 DataFrame。讀取速度約為相同資料 CSV 的 10 倍。

---

## 5. 並行與安全性

### 5.1 `Mutex<HashMap>` -> `RwLock<HashMap>`

如 2.6 所述，除 `load_vibration_data` 外的所有命令都是唯讀的。使用 `std::sync::RwLock`：

```rust
use std::sync::RwLock;

pub struct AppState {
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
}

// 讀取路徑（chunk、stats、export）：
let datasets = state.datasets.read().unwrap_or_else(|p| p.into_inner());

// 寫入路徑（load）：
let mut datasets = state.datasets.write().unwrap_or_else(|p| p.into_inner());
```

`DashMap` 是替代方案，但新增了依賴項，而帶來的效益微乎其微。`RwLock` 是零成本的（標準函式庫）且足以應對此存取模式（不頻繁的寫入、頻繁的讀取）。

### 5.2 WAV/FFT 的非同步考量

WAV 讀取（`hound`）和 FFT（`rustfft`）是 CPU 密集型操作。Tauri 2 命令預設為同步（在執行緒池上執行），因此不會阻塞非同步執行環境（async runtime）。這實際上沒問題——Tauri 的 `invoke_handler` 已經分派到阻塞執行緒池。

如果後續需要非同步命令（例如串流處理大型 WAV 檔案），使用 `tokio::task::spawn_blocking`：

```rust
#[tauri::command]
async fn analyze_wav(path: String) -> Result<FrequencySpectrum, AppError> {
    tokio::task::spawn_blocking(move || {
        let samples = hound_reader::read_wav(&path)?;
        let spectrum = fft_engine::compute_fft(&samples, sample_rate)?;
        Ok(spectrum)
    }).await.map_err(|e| AppError::TaskJoin(e.to_string()))?
}
```

### 5.3 狀態生命週期管理

目前，資料集載入到記憶體後永遠不會被驅逐。對於單一檔案使用場景，這是可以的。對於 AIDPS（多個 CSV 合併），記憶體使用量可能會很可觀。

**未來考量（暫時不需要）：** 新增 LRU 驅逐策略或明確的 `unload_dataset` 命令。`HashMap<String, DatasetEntry>` 已透過 `datasets.remove(&id)` 支援移除。

### 5.4 跨 I/O 持有 `Mutex` 守衛

在 `export_data` 中，`MutexGuard`（或未來的 `RwLockReadGuard`）在將 CSV 檔案寫入磁碟期間一直被持有：

```rust
let datasets = state.datasets.lock().unwrap();  // 取得守衛
let entry = datasets.get(&dataset_id).ok_or("...")?;
// ... 篩選 ...
CsvWriter::new(&mut file).finish(&mut export_df.clone())?;
// 守衛在此釋放（函式結束）
```

`export_df` 是一個複本，因此守衛可以更早釋放：

```rust
let export_df = {
    let datasets = state.datasets.read()?;
    let entry = datasets.get(&dataset_id).ok_or(...)?;
    // 複製或篩選
    filtered_df
};  // 守衛在此釋放

// 檔案 I/O 在鎖外進行
let mut file = File::create(&output_path)?;
CsvWriter::new(&mut file).finish(&mut export_df)?;
```

這是一個小改動，卻有顯著影響：檔案寫入在大型資料集上可能需要數秒，而持有鎖會阻塞所有其他命令。

---

## 6. 新增依賴項評估

### 6.1 `thiserror`（建議：是）

- **成熟度：** 超過 5000 萬次下載，由 dtolnay（serde、syn、proc-macro2 的作者）維護。
- **成本：** 僅編譯時期的程序巨集（proc macro），零執行時期開銷。
- **效益：** 消除約 20 處 `.map_err(|e| format!(...))` 呼叫，啟用結構化錯誤匹配。
- **風險：** 無。這是 Rust 中函式庫風格錯誤的事實標準。

### 6.2 `zip` crate 用於 .vibproj（建議：需要時再加入）

- **成熟度：** 超過 3500 萬次下載，純 Rust，無 C 依賴。
- **API：** `ZipArchive` 用於讀取，`ZipWriter` 用於寫入。支援 deflate、zstd。
- **風險：** 低。維護良好。建議使用 `zip = { version = "2", default-features = false, features = ["deflate"] }` 以最小化二進位檔大小。

### 6.3 `hound` 用於 WAV（建議：需要時再加入）

- **成熟度：** 約 500 萬次下載，穩定的 API，純 Rust。
- **支援：** PCM 8/16/24/32 位元、IEEE 浮點 32/64 位元。支援讀取和寫入。
- **限制：** 不支援壓縮 WAV 格式（ADPCM、MP3）。對於振動感測器資料（通常為 PCM），這不是問題。
- **風險：** 低。單一用途的 crate，最少的依賴。

### 6.4 `rustfft` 用於 FFT（快速傅立葉轉換）（建議：需要時再加入）

- **成熟度：** 約 700 萬次下載，純 Rust，無 C 依賴。
- **效能：** 對 2 的冪次大小與 FFTW 相當。在 x86 上使用 AVX2。
- **API：** `FftPlanner::plan_fft_forward(len)` 回傳可重複使用的計畫。執行緒安全。
- **替代方案：** `realfft`（建構於 `rustfft` 之上）適用於實數輸入——因振動資料為實數，可將計算量減半。建議優先於原始的 `rustfft` 使用。
- **風險：** 低。注意 FFT 輸出的解讀（頻率分箱映射、加窗處理）需要領域知識——該 crate 不處理漢寧窗（Hanning）/漢明窗（Hamming），你必須自行套用。

### 6.5 Polars `parquet` feature（建議：需要時再加入）

- **成本：** 增加約 500KB 的二進位檔大小（引入 `parquet2` 或 Arrow 的 Parquet 實作）。
- **效益：** 讀寫速度為 CSV 的 10 倍，欄式壓縮，型別保留。
- **用法：** `df.lazy().sink_parquet("file.parquet", ParquetWriteOptions::default())?`
- **風險：** 無。這是 Polars 的第一方 feature。

### 6.6 不建議的依賴項

- **`DashMap`：** `RwLock<HashMap>` 對此存取模式已經足夠。DashMap 增加了複雜度和依賴項，對於個位數數量的資料集沒有可衡量的效益。
- **`anyhow`：** 對應用程式而言，`anyhow` 是可以的，但 Tauri 命令需要錯誤型別實作 `Serialize`。`thiserror` 在此更合適，因為它產生具名的變體，可以被序列化。
- **`tokio`：** Tauri 2 內部已使用 tokio。除非你需要 `spawn_blocking` 來處理非同步命令，否則不要將其作為直接依賴項新增。改用 `tauri::async_runtime` 存取。

---

## 7. Rust 專屬建議前 10 名

依影響力排序（安全性 x 效能 x 投入比）：

### 1. 將手動 `BooleanChunked` 遮罩替換為 Polars 延遲篩選

**影響：** 高（效能）。**投入：** 低（2 個檔案中變更 4 行）。

影響 `data.rs` 和 `export.rs`。啟用 SIMD 加速篩選。對大型資料集而言是單一最高價值的變更。

### 2. 引入帶有 `thiserror` 的 `AppError` 列舉

**影響：** 高（安全性 + 可維護性）。**投入：** 中（新增檔案 + 更新約 20 處呼叫點）。

消除 `.unwrap()` 鏈、`.map_err(|e| format!(...))` 樣板程式碼，以及錯誤訊息不一致的風險。使錯誤處理變得機械化而非臨時性。

### 3. 將 `Mutex` 改為 `RwLock`

**影響：** 中（並行性）。**投入：** 低（更改型別 + 將 `lock()` 改為 `read()`/`write()`）。

允許並行讀取操作。程式碼改動最小，零新增依賴。

### 4. 移除求時間最小值/最大值時不必要的 `Vec<f64>` 配置

**影響：** 中（大型檔案效能）。**投入：** 低（2 行）。

將 `into_no_null_iter().collect::<Vec<f64>>()` + 手動 reduce 替換為 `series.min()` / `series.max()`。節省 8*N 位元組的配置。

### 5. 在 `export_data` 中於檔案 I/O 前釋放鎖

**影響：** 中（並行性）。**投入：** 低（以區塊作用域重構一個函式）。

防止在可能較慢的磁碟寫入期間產生鎖競爭。

### 6. 修正 `preview_csv` 使其不讀取整個檔案

**影響：** 中（大型檔案的使用體驗）。**投入：** 低（加入 `.with_n_rows(Some(0))` 或一個小數值）。

預覽是使用者選擇檔案後看到的第一個畫面。對於 500MB 的 CSV，為了取得欄位名稱而讀取所有列是不必要地緩慢。

### 7. 讓 `extract_f64_vec` 回傳 `Result`

**影響：** 中（安全性）。**投入：** 低（更改簽章 + 在呼叫點加入 `?`）。

消除在鎖守衛內部執行的兩個 `.unwrap()` 呼叫。防止型別不匹配導致的鎖中毒。

### 8. 移除 `DatasetEntry::metadata` 上的 `#[allow(dead_code)]`

**影響：** 低（程式碼整潔）。**投入：** 極小。

該欄位有被使用。該屬性壓制了一個應該被調查而非靜默處理的警告。

### 9. 為 `csv_reader` 和 `stats_engine` 新增 `#[cfg(test)]` 模組

**影響：** 中（長期品質）。**投入：** 中。

目前只有 `downsampling.rs` 有測試。CSV 讀取器（日期時間解析邊界案例）和統計引擎（極端值下偏度/峰度的數值穩定性）是最可能出現細微 bug 的來源。

關鍵測試案例：
- 混合日期時間格式的 CSV
- 資料欄位中包含 NaN/Inf 值的 CSV
- 常數序列的統計（std_dev = 0 導致 `compute_shape_stats` 中除以零）
- 單一元素序列的統計

**發現的關鍵 bug：** 在 `compute_shape_stats` 中，如果 `std_dev` 為 0（常數序列），`z = (val - mean) / std_dev` 會產生 `NaN` 或 `Inf`。該函式應加入防護：
```rust
if std_dev < f64::EPSILON {
    return AxisShapeStats { axis: axis_name.to_string(), skewness: 0.0, kurtosis: 0.0 };
}
```

### 10. 記錄 LTTB 單通道取捨

**影響：** 低（可維護性）。**投入：** 極小（加入一段文件註解）。

未來的開發者（或未來的你）會疑惑為何只用 `data_columns[0]` 驅動索引選擇。一段 3 行的註解解釋此取捨，可避免不必要的「修正」PR。

---

## 附錄：發現的 Bug 摘要

| 嚴重度 | 位置 | 問題 |
|---|---|---|
| **中** | `stats_engine.rs:48` | 當 `std_dev == 0`（常數序列）時除以零，產生 `NaN` 偏度/峰度 |
| **低** | `state.rs:9` | 在實際被使用的欄位上標記 `#[allow(dead_code)]` |
| **低** | `export.rs:37` | 對 `export_df` 進行不必要的第二次 `.clone()` |
| **低** | `data.rs:26-28` | 僅為計算最小值/最大值而配置完整的 `Vec<f64>` |

---

*審查產生於 2026-03-17。適用於 `claude/tauri-vibration-dashboard-d6Qx2` 分支上的 commit `5a454eb`。*
