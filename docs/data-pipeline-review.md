# 資料管線審查：Vibration Viewer

> 審查日期：2026-03-17
> 範圍：Rust 後端資料結構、查詢模式、記憶體管理與可擴展性
> 背景：目前單一 CSV 架構轉型為支援多裝置 AIDPS 專案

---

## 1. 資料結構最佳化

### 1.1 目前結構：`HashMap<String, DatasetEntry>`

```rust
pub struct AppState {
    pub datasets: Mutex<HashMap<String, DatasetEntry>>,
}
pub struct DatasetEntry {
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}
```

**評估：** 此結構對於單一檔案的使用情境是正確的。但在多裝置專案模型下會產生問題，原因有三：

1. **扁平命名空間。** 以 UUID 為鍵的 HashMap 沒有「這 24 個 DataFrame 屬於 device3」的概念。每次查詢都必須迭代或維護另一個索引來回答「給我 device X 的所有資料」。

2. **粒度不匹配。** 規劃中的 AIDPS 工作流程會為每個裝置載入 23-24 個 CSV，並將它們合併為一條連續的時間序列。目前的 HashMap 每個 CSV 檔案儲存一個 DataFrame。合併後，map 中每個裝置會有一個大型 DataFrame，但鍵仍然是沒有語義意義的隨機 UUID。

3. **單一 Mutex（互斥鎖）。** 所有資料集存取——讀取與寫入——都經由同一個 `Mutex<HashMap>`。載入 device2 的資料會阻塞 device1 的 chunk 查詢。在目前的規模下這是可以接受的（鎖持有時間為亞毫秒級），但若資料載入或 FFT 計算耗時顯著，就會成為競爭瓶頸。

**建議的專案導向資料結構：**

```
AppState
  +-- active_project: Option<ProjectState>
        +-- metadata: ProjectMetadata (devices, sensor mapping, paths)
        +-- devices: HashMap<DeviceId, DeviceState>
              +-- metadata: DeviceMetadata
              +-- timeseries: Option<DataFrame>    // loaded on demand
              +-- stats_cache: Option<StatisticsReport>
              +-- loaded_at: Instant               // for LRU eviction
```

這直接對應到領域模型：一個專案包含多個裝置，每個裝置擁有一條合併後的時間序列。`Option<DataFrame>` 啟用了延遲載入（lazy loading）——只有目前活動裝置的資料需要在記憶體中。`loaded_at` 時間戳記啟用 LRU 驅逐策略（第 3 節）。

**鎖粒度：** 考慮使用 `RwLock` 取代 `Mutex`。Chunk 查詢是唯讀的，可以並行處理。只有資料載入和標註寫入需要獨佔存取。另一種方案是保留單一 `Mutex`，但確保鎖內的所有操作都很快（鎖內不做 I/O）。

### 1.2 記憶體佈局

Polars DataFrame 已經是欄式儲存（Arrow-backed），對於此工作負載接近最佳：

- **時間範圍篩選**只觸及時間欄位，不觸及資料欄位（良好的快取行為）。
- **LTTB** 依序讀取時間 + 一個資料欄位（每個欄位內為 stride-1 存取）。
- **統計**對各欄位獨立操作。

此處不需要變更。Polars 欄式佈局是正確的。

### 1.3 時間欄位：已排序 + 索引化

**目前狀態：** 時間欄位在 CSV 解析後以原樣儲存。每次 `get_timeseries_chunk` 呼叫都執行完整的線性掃描來建立布林遮罩：

```rust
// data.rs:71-78 -- O(N) scan for every chunk request
let mask = df.column("time").unwrap().f64().unwrap()
    .into_iter()
    .map(|opt| opt.is_some_and(|t| t >= start_time && t <= end_time))
    .collect::<BooleanChunked>();
```

對 1,300 列而言這幾乎無感。對 28,000 列（24 個 CSV 合併）也仍然很快。但這是時間序列資料——本質上是已排序或接近排序的。應該利用這個特性。

**建議：** 在合併裝置的 CSV 後，依時間排序並去重。然後將 chunk 擷取從線性遮罩掃描改為對起止索引進行二元搜尋（binary search），接著使用 `DataFrame::slice()`。在 28,000 列的已排序 f64 欄位上進行二元搜尋約需 ~15 次比較，而非 28,000 次迭代。

在目前的 AIDPS 資料規模（每裝置 28K 列）下，兩種方法都在微秒級完成。二元搜尋方法在 1M+ 列時才有意義，同時也省去了布林遮罩的整欄記憶體配置。

---

## 2. 查詢效能

### 2.1 時間範圍篩選

三種方法比較：

| 方法 | 機制 | 28K 列 | 1M 列 | 10M 列 |
|------|------|--------|-------|--------|
| 目前的手動遮罩 | `into_iter().map().collect::<BooleanChunked>` 然後 `df.filter()` | ~50 us | ~5 ms | ~50 ms |
| Polars lazy filter（惰性篩選） | `df.lazy().filter(col("time").gte(start).and(...)).collect()` | ~100 us | ~3 ms | ~30 ms |
| 排序 + 二元搜尋 + slice | `partition_point()` 然後 `df.slice(offset, len)` | ~1 us | ~5 us | ~10 us |

手動遮罩方法有兩項成本：(a) 迭代整個欄位以產生 `BooleanChunked`，(b) `df.filter()` 將選取的列複製到新的 DataFrame。Polars lazy filter 內部做的是相同的工作，但可以透過 SIMD 最佳化篩選謂詞。排序+slice 方法兩者都避免了——`slice()` 是 O(1)，回傳現有 DataFrame 的零複製視圖。

**結論：** 排序 + 二元搜尋 + slice 在大規模下快 1000 倍，且零配置開銷。它要求時間欄位已排序，這應在載入時建立保證。

### 2.2 LTTB 效能

LTTB 為 O(N)，其中 N 是篩選範圍內的點數。目前在 `downsampling.rs` 中的實作簡潔且高效——內部迴圈無記憶體配置，簡單的算術運算。主要成本在 `extract_f64_vec()`，它將整個欄位複製到 `Vec<f64>`：

```rust
// data.rs:123-130 -- allocates and copies
fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name).unwrap().f64().unwrap()
        .into_no_null_iter().collect()
}
```

這會為時間欄位呼叫一次，為代表性資料欄位（用於索引選擇）呼叫一次，然後為每個額外的資料欄位（用於套用索引）各呼叫一次。對於篩選到 10K 列範圍的 13 個 AIDPS 欄位，共 14 次配置，每次 80 KB，總計 ~1.1 MB。很快，但不必要——資料已經連續儲存在 DataFrame 的 Arrow buffer 中。基於 slice 的 LTTB 直接操作 `ChunkedArray<Float64Type>` slice，可以完全避免這些複製。

**預估 LTTB 耗時：**

| 篩選後列數 | 閾值 | 耗時（目前） | 耗時（零複製） |
|------------|------|-------------|---------------|
| 10,000 | 50,000 | <1 ms（不降採樣） | <1 ms |
| 100,000 | 50,000 | ~2 ms | ~1.5 ms |
| 1,000,000 | 50,000 | ~15 ms | ~8 ms |

LTTB 不是瓶頸。複製開銷只在 1M+ 篩選列時才有影響。

### 2.3 序列化與 IPC

`TimeseriesChunk` 結構體由 Tauri 的 IPC 層（serde_json）序列化為 JSON：

```rust
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub channels: HashMap<String, Vec<f64>>,
    pub is_downsampled: bool,
    pub original_count: usize,
}
```

對於 50K 點、13 個通道的 chunk：14 個陣列，每個 50,000 個 f64 值 = 700,000 個數字。每個 f64 序列化為 JSON 約 8-15 個字元。JSON 總承載大小：~7-10 MB。

**這才是大型 chunk 請求的實際瓶頸。** 700K 浮點數的 JSON 序列化成本很高：

| 點數 | 通道數 | JSON 大小 | 序列化耗時 | JS 解析耗時 |
|------|--------|----------|-----------|------------|
| 5,000 | 3 | ~250 KB | <1 ms | <1 ms |
| 50,000 | 3 | ~2.5 MB | ~5 ms | ~10 ms |
| 50,000 | 13 | ~10 MB | ~20 ms | ~40 ms |
| 150,000 | 13 | ~30 MB | ~60 ms | ~120 ms |

在「精細」精度等級（150K 點）搭配 13 個通道時，光是序列化就可能消耗掉整個 100ms 的延遲預算。「完整資料」模式若無降採樣上限，則實際上沒有上界。

**緩解方案（依影響程度排序）：**
1. 減少傳送的通道數：只傳送可見/請求的通道，而非全部 13 個。
2. 降低精度：在 JSON 中將 f64 截斷為 4-6 位有效數字（字串長度減少 ~40%）。
3. Binary IPC（二進位 IPC）：Tauri 支援原始位元組回應。將通道以打包的 f64 陣列傳送（每個值 8 bytes，而非 JSON 中的 10-15 個字元）。這可減少 2 倍的承載量，並消除序列化和解析的開銷。

### 2.4 延遲預算

目標：每次縮放/平移 chunk 請求 <100ms。

| 階段 | 目前（28K 列，3 通道） | 預估（28K 列，13 通道） | 預估（1M 列，13 通道） |
|------|----------------------|------------------------|----------------------|
| 鎖取得 | <1 us | <1 us | <1 us |
| 時間範圍篩選 | ~50 us | ~50 us | ~5 ms |
| LTTB（如需要） | ~1 ms | ~1 ms | ~15 ms |
| extract_f64_vec | ~0.5 ms | ~2 ms | ~10 ms |
| Serde JSON 序列化 | ~1 ms | ~5 ms | ~20 ms |
| IPC 傳輸 | ~0.5 ms | ~2 ms | ~5 ms |
| JS JSON 解析 | ~1 ms | ~5 ms | ~15 ms |
| **總計** | **~4 ms** | **~15 ms** | **~70 ms** |

在目前的 AIDPS 規模（28K 列，13 通道）下，管線充分在預算內。在 1M 列搭配 13 通道時，接近邊界。大規模下的主要成本是 JSON 序列化和解析，而非實際的資料處理。

---

## 3. 記憶體管理

### 3.1 目前狀態

每個載入的 DataFrame 永遠留在 `HashMap` 中。前端存在 `removeFile()`，但只移除前端 store 的項目——Rust 端的 `DatasetEntry` 永遠不會釋放（沒有對應的 Tauri 命令來卸載資料集）。

### 3.2 預估記憶體佔用

每個裝置（24 個合併的 CSV，~28,000 列，14 個 f64 欄位）：

```
28,000 rows x 14 columns x 8 bytes/f64 = 3.1 MB per device
```

同時載入全部 5 個 AIDPS 裝置：~15.5 MB。這微不足道。即使是目前資料量的 10 倍，五個裝置各 280K 列也只有 ~155 MB——對桌面應用程式而言仍然輕鬆。

**何時記憶體會成為問題：** 在每裝置 1M 列搭配 14 個欄位時，每個裝置 DataFrame 消耗 ~112 MB。五個裝置同時：~560 MB。這是延遲載入變得重要的門檻。

### 3.3 建議策略

鑑於目前的 AIDPS 規模（總計 5.4 MB CSV 資料），**現在不要實作驅逐機制。** 基礎設施的成本（LRU 追蹤、按需重載、過期快取失效）超過其效益。

但是，設計狀態結構時應*允許*未來加入驅逐：

- 每個裝置以 `Option<DataFrame>` 包裝 DataFrame（如第 1.1 節所示）。
- 切換到新裝置時，若總記憶體超過可設定的閾值（例如 500 MB），驅逐最近最少存取的裝置的 DataFrame。
- 下次存取時從 `.vibproj` Parquet 檔案重新載入（快速：Polars 讀取 28K 列的 Parquet <10 ms）。

**記憶體映射 Parquet（Memory-mapped Parquet）**（透過 Polars `scan_parquet`）是未來在資料集達到 100M+ 列時的選項，但它增加了複雜度（Windows 上的記憶體映射 I/O 需要小心的檔案控制代碼管理），在目前規模下不需要。

### 3.4 匯出時的 DataFrame Clone

`export.rs:30` 在未套用時間篩選時複製整個 DataFrame：

```rust
_ => df.clone(),
```

Polars DataFrame clone 成本很低——它複製的是底層 Arrow 陣列的 Arc 指標，而非資料本身。這不是真正的效能問題。後續在第 37 行的 `CsvWriter::new(&mut file).finish(&mut export_df.clone())` 執行了*第二次* clone，同樣只是 Arc 指標複製，成本低但不必要。`finish()` 上的 `&mut` 是 API 要求，但實際上不會修改資料。

**嚴重度：** 低。Clone 是 O(欄位數)，不是 O(列數)。不需要處理。

---

## 4. NULL 值策略

### 4.1 目前的問題

```rust
// csv_reader.rs:96 -- fills all nulls with 0.0
col(c).cast(DataType::Float64).fill_null(lit(0.0))
```

這對圖表渲染是正確的（ECharts 需要數字，不是 null），但會扭曲每個下游消費者：

| 消費者 | fill(0.0) 的影響 | 嚴重度 |
|--------|-----------------|--------|
| **圖表（ECharts）** | 零值顯示為 y=0 的真實資料點。對於典型範圍為 0.001-0.05 g 的振動資料，零值會產生朝向零的視覺尖峰，看起來像異常。 | 中 |
| **統計（平均值、標準差）** | 零值將平均值拉向零並膨脹標準差。在平均值=0.02、標準差=0.005 的資料集上，5% 的 null 率會使平均值下降 ~5%，標準差增加 ~10%。 | 高 |
| **統計（分布）** | 零值顯示為最小值。Q1 向下偏移。IQR 增大。 | 高 |
| **統計（偏態、峰態）** | 零值在零處產生質點，顯著扭曲兩個動差。 | 高 |
| **FFT（未來）** | 時間序列輸入中的零值會產生不連續，導致虛假的高頻成分。這是眾所周知的訊號處理錯誤。 | 嚴重 |
| **匯出** | 匯出 CSV 中的零值與真實的零測量值無法區分。下游工具無法恢復原始的 null 語義。 | 中 |

### 4.2 建議方法

**保留 null 為 Polars null（不是 NaN，不是 0.0）。** Polars 具有一流的 null 支援——每個 ChunkedArray 都帶有有效性位圖（validity bitmap），每個元素零額外成本。Polars 的統計函式（`mean()`、`std()`、`median()`）已經正確跳過 null。

每個消費者所需的變更：

**CSV reader（CSV 讀取器）：** 移除 `fill_null(lit(0.0))`。資料欄位已經轉型為 Float64，原生支援 null。

**圖表渲染：** 在 `extract_f64_vec()` 中，從 `into_no_null_iter()`（會跳過 null，默默地縮短陣列！）改為發出 `f64::NAN` 對應 null 值的迭代器。ECharts 透過在折線中產生間隙來處理 NaN——這是缺失資料的正確視覺表示。

```rust
// Replace:
fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name).unwrap().f64().unwrap()
        .into_no_null_iter().collect()  // BUG: skips nulls, array shorter than time array
}
// With:
fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name).unwrap().f64().unwrap()
        .into_iter()
        .map(|opt| opt.unwrap_or(f64::NAN))
        .collect()
}
```

**統計：** 不需要變更。Polars 的 `mean()`、`std()`、`min()`、`max()`、`median()` 已排除 null。`compute_shape_stats()` 函式在迭代器上使用 `.flatten()`，已經跳過 null——這對偏態/峰態是正確的。`compute_basic_stats()` 中的 `count` 應從 `series.len()`（包含 null 的總列數）改為減去 `series.null_count()`，或使用 Polars 的非 null 計數。

**LTTB：** LTTB 演算法假設資料是連續且非 null 的。兩個選項：(a) 在載入時過濾掉 null 時間的列（已完成），並在 LTTB 中使用 NaN 感知的三角面積計算；(b) 將 NaN 值視為零面積貢獻（索引選擇時實質上跳過它們，但在輸出中保留）。選項 (b) 更簡單且對視覺化而言是正確的。

**FFT（未來）：** 對於 FFT 輸入，訊號中的 null/NaN 必須明確處理。選項：(a) FFT 前插值缺失值（線性插值是短間隙的標準做法），(b) 拒絕 null 過多的片段，(c) 使用只在連續非 null 片段上操作的窗函式 FFT。

**匯出：** 將 null 匯出為 CSV 中的空儲存格（Polars CsvWriter 對 null 值預設就是這樣做的）。這保留了原始語義。

### 4.3 關於 `into_no_null_iter()` Bug 的說明

目前的 `extract_f64_vec()` 使用 `into_no_null_iter()`，會默默跳過 null 值。若一個資料欄位在 1,000 列中有 3 個 null，回傳的 Vec 有 997 個元素，而時間 Vec 有 1,000 個元素。從 1,000 元素時間陣列計算的 LTTB 索引接著被套用到 997 元素的資料陣列上，導致 panic（index out of bounds，索引越界）或默默的資料錯位。這是一個潛在 bug，會在資料欄位含有 null 值時顯現（不是時間欄位——時間 null 在載入時已被過濾）。

---

## 5. Parquet 作為儲存層

### 5.1 對此工作負載的好處

`docs/research-project-file-format.md` 中描述的 `.vibproj` ZIP+Parquet 格式選擇得當。具體好處：

1. **原生 Polars 親和性。** Polars 的內部記憶體佈局映射 Parquet 的欄式結構。`ParquetReader::new(cursor).finish()` 產生的 DataFrame 無需中間表示。CSV 讀取需要對每個儲存格進行字串轉浮點數的解析。

2. **壓縮。** 28,000 列 x 14 個 f64 欄位 = 3.1 MB 原始大小。Parquet 搭配 Snappy 壓縮：~0.8-1.2 MB。Parquet 搭配 Zstandard：~0.5-0.8 MB。CSV：~1.5-2.0 MB 未壓縮。壓縮是欄式的，利用感測器欄位中相鄰值通常相似的特性（delta encoding，差值編碼，效果良好）。

3. **型別保留。** CSV 會遺失型別資訊。Parquet 保留 f64 精度、null 位圖和欄位名稱。不需要在載入時進行 `ColumnMapping` 的對應——schema 已嵌入。

4. **讀取速度。** 28K 列的 Parquet 讀取：~2-5 ms。相同資料的 CSV 解析：~10-30 ms。在 1M 列時：Parquet ~50-100 ms，CSV ~300-500 ms。

### 5.2 Row Group（列群組）作為 Chunk 邊界

Parquet 檔案被劃分為 row group（通常每個 64-128 MB 未壓縮資料）。對於目前的 AIDPS 資料（每裝置 3.1 MB），整個資料集適合放在單一 row group 中。在此規模下 row group 邊界無關緊要。

在 1M+ 列時，row group 變得有用。Polars `scan_parquet().filter()` 可以使用 row group 統計資訊（每個群組每欄的 min/max）來跳過完全落在請求時間範圍外的整個群組。這就是 predicate pushdown（謂詞下推）——篩選發生在 I/O 層，而非載入記憶體之後。

**然而：** Row group predicate pushdown 只在 `scan_parquet`（lazy mode，惰性模式）下有效，在 `ParquetReader`（eager mode，即時模式）下無效。而且它只在 Parquet 檔案可在檔案系統上直接存取時有效——從 ZIP 壓縮檔擷取到記憶體緩衝區後讀取則無法使用。

這產生了一個張力：`.vibproj` ZIP 容器需要在讀取前將 Parquet 檔案擷取到記憶體緩衝區中，這抵消了 row group 隨機存取的好處。兩種緩解方案：

1. **小檔案（< 50 MB 未壓縮）：** 將整個 Parquet 緩衝到記憶體中，使用 `ParquetReader::new(Cursor::new(bytes))`。這是目前的規劃，且是正確的。

2. **大檔案（> 50 MB）：** 將 Parquet 擷取到暫存檔案，然後使用 `scan_parquet()` 搭配 predicate pushdown。或完全跳過 ZIP 容器，將 Parquet 與專案中繼資料檔案一起儲存。

在 AIDPS 規模下，方案 (1) 已足夠。方案 (2) 在此記錄供未來參考。

### 5.3 ZIP 中的 Parquet：Seek 限制

ZIP 項目是循序位元組流。`zip` crate 的 `ZipFile` reader 實作了 `Read` 但未實作 `Seek`。Parquet reader 需要 `Read + Seek`，因為檔案格式將 footer（schema + row group offset）放在檔案末尾。

標準解決方法是將整個 ZIP 項目讀入 `Vec<u8>` 並包裝在 `std::io::Cursor` 中，後者同時實作 `Read + Seek`。這就是 `research-project-file-format.md` 中實作草圖已經展示的做法。記憶體成本是 Parquet 資料的一份額外複本（通常是已壓縮的，所以比記憶體中的 DataFrame 小）。可接受。

---

## 6. FFT 資料管線

### 6.1 資料流

```
User clicks annotation at time T
  --> Frontend calls get_spectrum(device_id, timestamp)
  --> Rust: look up device -> sensor mapping
  --> Rust: find WAV file closest to timestamp T
      (binary search on sorted filename-derived timestamps)
  --> Rust: hound::WavReader reads single WAV file (~1.3 MB)
  --> Rust: extract f32/f64 samples from WAV
  --> Rust: apply window function (Hanning)
  --> Rust: rustfft computes FFT
  --> Rust: compute magnitude spectrum (|FFT[k]| / N)
  --> Return SpectrumData { frequencies: Vec<f64>, amplitudes: Vec<f64> }
```

### 6.2 效能預估

一個 10 秒、44.1 kHz 的 WAV = 441,000 個取樣。使用 rustfft 對 441K 取樣進行 FFT：

- FFT 計算：~5-10 ms（rustfft 高度最佳化，在 x86_64 上使用 AVX2/SSE4）
- WAV 檔案讀取 + 解碼：~5-15 ms（hound 直接讀取 PCM，無需解壓縮）
- 頻譜計算（幅度、頻率區間）：~2 ms
- 結果的 JSON 序列化（220K 頻率區間）：~5-10 ms
- **總計：每次頻譜請求 ~20-40 ms**

這在 100ms 預算內。此規模下不需要非同步計算。

### 6.3 快取策略

`.vibproj` 格式已指定 `spectra/` 目錄用於快取 FFT 結果：

```
spectra/
  {annotation_id}.json
```

這是正確的方法。FFT 是確定性的（相同 WAV + 相同參數 = 相同結果），所以快取是安全的。快取鍵應為 `(wav_file_path, fft_size, window_function)` 而非 `annotation_id`，因為相同時間戳的多個標註共享相同的 WAV 檔案和頻譜。

**快取大小：** 每個頻譜 JSON 約 ~4 MB（220K 頻率-幅度對）。50 個快取頻譜：磁碟上 ~200 MB。記憶體中只保留最近檢視的頻譜（最多 2-3 個，~12 MB）。

**建議：** 在儲存時快取到 `.vibproj` ZIP 內的磁碟上。會話期間使用以 `(wav_path, fft_params)` 為鍵的簡單 HashMap 在記憶體中快取。不要預先計算頻譜——使用者只會檢視 1262 個 WAV 檔案中的一小部分。

### 6.4 記憶體考量

單一 1.3 MB 的 WAV 檔案產生 ~441K 個 f32 取樣 = 1.7 MB。FFT 計算需要相同長度的複數值緩衝區：441K x 16 bytes（Complex64）= 7 MB。單次 FFT 計算的工作記憶體總計：~10 MB。結果擷取後立即釋放。

關鍵限制是：**永遠不要同時載入多個 WAV 檔案。** 一次處理一個，回傳結果，釋放緩衝區。1.6 GB 的 WAV 語料庫必須留在磁碟上。

---

## 7. 可擴展性評估

### 7.1 目前架構限制

| 規模 | 列數/裝置 | 總記憶體 | 瓶頸 | 狀態 |
|------|----------|---------|------|------|
| AIDPS 目前 | 28K | ~15 MB | 無 | 運作良好 |
| 10x AIDPS | 280K | ~155 MB | 無 | 運作良好 |
| 每裝置 1M 列 | 1M | ~560 MB | 150K+ 點請求時的 JSON 序列化 | 可管理 |
| 每裝置 10M 列 | 10M | ~5.6 GB | 記憶體、載入時間 | 需要變更 |
| 每裝置 100M 列 | 100M | ~56 GB | 所有環節 | 需要架構變更 |

### 7.2 最先崩潰的環節

**在 1M 列（第一個壓力點）：**

1. **JSON IPC 序列化**在 50K 點 x 13 通道時達到 100ms 預算。這是第一個瓶頸，因為它隨（點數 x 通道數）線性擴展，且無法在不更改傳輸格式的情況下最佳化。

2. **線性時間範圍篩選**耗時 ~5 ms。尚未成為瓶頸，但在已排序資料上是不必要的。

3. **CSV 載入時間** 1M 列：~500 ms-1 秒。對初始載入可接受，但會阻塞 UI 執行緒。應改為非同步。

4. **統計計算** 1M 列：所有軸 ~50-100 ms。每次請求都重新計算——應加入快取。

**在 10M 列：**

5. **記憶體**成為約束條件。5 個裝置 5.6 GB 在 16 GB 機器上可行，但餘裕不多。需要延遲載入（只載入活動裝置）。

6. **Parquet 載入時間** 10M 列：~500 ms-1 秒。必須非同步並顯示載入指示器。

7. **LTTB 處理 10M 列**（完全縮小）：~100-150 ms。接近延遲預算。預先計算的多解析度金字塔（以 5K、15K、50K 點儲存的 LTTB，與完整資料並存）可消除此成本。

**在 100M 列：**

8. **所有環節崩潰。** 整個 DataFrame 在典型桌面機器上無法放入記憶體。此規模需要：記憶體映射 Parquet 搭配 predicate pushdown、分塊/漸進式渲染、伺服器端預聚合。這是根本不同的架構，超出桌面標註工具的範圍。

### 7.3 實際的規模上限

鑑於 AIDPS 資料特性（每秒 1 列，13 個欄位），達到 1M 列需要 ~11.5 天的連續錄製。達到 10M 需要 ~115 天。對於預期的使用情境（每次會話數小時到數天的振動監測），**實際的上限是每裝置 100K-500K 列。** 目前的架構無需修改即可處理此範圍。

---

## 8. 資料管線十大建議

依影響程度排序，考量 AIDPS 資料規模與近期需求。

### 排名 1：修正 NULL 處理（移除 `fill_null(0.0)`）

**影響：** 正確性——影響每個下游消費者。
**工作量：** 小（移除一行，調整 `extract_f64_vec` 以發出 NaN）。
**為何排第一：** 這是資料完整性問題。統計、圖表和未來的 FFT 在 null 被替換為零時都會產生錯誤結果。`into_no_null_iter()` bug（第 4.3 節）可在含有 null 值的資料上導致 index-out-of-bounds panic。在新增任何新功能之前先修正此問題。

### 排名 2：排序 + 二元搜尋用於時間範圍查詢

**影響：** 在大規模下範圍篩選加速 1000 倍；零複製 slice 取代 filter+allocate。
**工作量：** 小。在載入時依時間排序 DataFrame。將手動遮罩替換為 `partition_point()` + `df.slice()`。
**為何現在做：** 合併 CSV 功能（差距分析中的 P0）無論如何都必須依時間排序以去重。在此基礎上加入二元搜尋幾乎不需額外成本。

### 排名 3：重構 AppState 為專案/裝置階層

**影響：** 啟用整個 AIDPS 多裝置工作流程。沒有這個，就沒有地方儲存裝置分組、感測器對應或每裝置狀態。
**工作量：** 中。重新設計 `AppState`，新增 `ProjectState` 和 `DeviceState` 結構體，更新所有命令。
**為何現在做：** 這是差距分析中每個 P0 功能的前置條件。所有後續工作都建立在此結構之上。

### 排名 4：每裝置快取統計資訊

**影響：** 消除每次 `compute_statistics` 呼叫的重複計算。
**工作量：** 小。在 `DeviceState` 中儲存 `Option<StatisticsReport>`，資料變更時失效。
**為何現在做：** 統計計算涉及排序（百分位數）和全欄迭代（動差）。在 28K 列下這不到 5 ms，但每次面板檢視都會呼叫。快取是瑣碎的，可消除不必要的工作。

### 排名 5：在 Chunk 回應中只傳送可見通道

**影響：** 將 IPC 承載量減少最多 4 倍（3 個可見通道 vs 13 個全部）。
**工作量：** 小。在 `get_timeseries_chunk` 中新增 `channels: Vec<String>` 參數。
**為何現在做：** AIDPS 格式有 13 個欄位，但使用者通常一次檢視 3 個（x/y/z 或 vrms 群組）。在每次縮放/平移事件中序列化全部 13 個通道浪費了 ~75% 的 IPC 頻寬。

### 排名 6：實作多 CSV 串接與去重

**影響：** 啟用核心 AIDPS 工作流程（每裝置 24 個 CSV 合併為一條時間序列）。
**工作量：** 中。`Polars concat + sort_by("time") + unique(subset=["time"])`。
**為何現在做：** 這是 P0 關鍵功能。排序步驟（排名 2）和專案結構（排名 3）是前置條件。

### 排名 7：新增 `unload_dataset` 命令

**影響：** 允許前端在關閉檔案或切換裝置時釋放 Rust 端的記憶體。
**工作量：** 小。一個從 HashMap 移除項目的 Tauri 命令。
**為何現在做：** 目前載入的 DataFrame 永遠累積。載入 5 個裝置但使用者只操作一個時，~80% 的記憶體被浪費。這成為延遲載入的機制。

### 排名 8：使用 Parquet 作為專案檔案儲存

**影響：** 相比 CSV 有 3-5 倍更快的載入時間、型別保留、null 保留、更小的檔案大小。
**工作量：** 中。在 Polars 中新增 `parquet` feature，實作帶有 ZIP 容器的 save/load_project 命令。
**為何現在做：** 這在專案檔案格式研究中已有規劃。它也解決了「每次開啟專案都重新解析 24 個 CSV」的問題——解析一次，存為 Parquet，之後快速載入。

### 排名 9：實作 FFT 管線搭配逐 WAV 快取

**影響：** 啟用頻域分析功能（差距分析中的 P0）。
**工作量：** 大。新依賴（hound、rustfft）、新命令、新前端元件。
**為何現在做：** 這是工作量最大的項目，但也是核心功能需求。快取策略（第 6.3 節）確保對相同 WAV 檔案的重複查詢是即時的。

### 排名 10：考慮 Binary IPC 用於大型承載（未來）

**影響：** 大型 chunk 回應的序列化時間減少 2-3 倍。
**工作量：** 中。需要 Rust 端的自訂序列化和 JS 端的 ArrayBuffer 解析。
**為何不是現在：** 在 AIDPS 規模下，28K 列 x 3 通道的 JSON 序列化約 ~1 ms。此最佳化只在 1M+ 列或同時以高精度傳送全部 13 個通道時才有意義。當使用者回報縮放/平移操作有延遲時再重新評估。

---

## 附錄：審查中發現的 Bug 摘要

1. **`fill_null(0.0)` 扭曲所有統計**（`csv_reader.rs:96`）。在 null 存在時，每個平均值、標準差、百分位數、偏態和峰態都是錯誤的。

2. **`into_no_null_iter()` 默默丟棄 null 值**（`data.rs:128`），產生比時間陣列更短的資料陣列。從時間陣列計算的 LTTB 索引被套用到較短的資料陣列上，導致資料錯位或 index-out-of-bounds。

3. **沒有 `unload_dataset` 命令。** Rust 端的 HashMap 單調增長。前端的 `removeFile()` 移除了 UI 參照，但將 DataFrame 留在 Rust 記憶體中。

4. **LTTB 使用單一通道進行索引選擇**（`data.rs:92`）。當代表性通道（第一個資料欄位）在某區域變異度低，但另一個通道有尖峰時，尖峰可能在降採樣輸出中被遺漏。這是單通道 LTTB 的已知限制，嚴格來說不算 bug，但值得記錄。多通道 LTTB（選擇使所有通道三角面積*總和*最大化的索引）可保留所有通道的特徵。

5. **`preview_csv` 讀取整個檔案**（`csv_reader.rs:13-17`）。它使用 `CsvReadOptions::default()` 且無列數限制，只為了取得標頭和列數就解析完整的 CSV。對於預覽，只讀取前幾列（或僅讀取標頭行）就足夠了。
