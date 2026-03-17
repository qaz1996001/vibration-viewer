# 程式碼品質審查：Vibration Viewer

> 審查日期：2026-03-17
> 分支：`claude/tauri-vibration-dashboard-d6Qx2`
> 哲學依據：Martin Fowler（code smell（程式碼異味）、YAGNI）、Ken Thompson（簡潔性）、Linus Torvalds（最多 3 層縮排、函式少於 24 行）、Donald Knuth（程式即文學）

---

## 1. 程式碼異味清單

| # | 檔案 | 異味描述 | 類型（Fowler 分類） | 嚴重度 |
|---|------|---------|-------------------|--------|
| 1 | `state.rs:9` | `metadata` 欄位上的 `#[allow(dead_code)]` | Dead Code（死碼） | Minor（輕微） |
| 2 | `commands/data.rs:71-78` | 手動迭代器遮罩，未使用 Polars 表達式 | Primitive Obsession（基本型別偏執） | Major（重大） |
| 3 | `commands/export.rs:20-27` | 與 `data.rs` 重複的時間過濾程式碼 | Duplicate Code（重複程式碼） | Major（重大） |
| 4 | `commands/data.rs:73-74` | `column("time")` 與 `f64()` 使用 `unwrap()` — 出現 2 次 | Missing Error Handling（缺少錯誤處理） | Critical（嚴重） |
| 5 | `commands/export.rs:22-24` | `column("time")` 與 `f64()` 使用 `unwrap()` — 出現 2 次 | Missing Error Handling（缺少錯誤處理） | Critical（嚴重） |
| 6 | `commands/data.rs:123-130` | `extract_f64_vec` 在 `column()` 和 `f64()` 上使用 `unwrap()` | Missing Error Handling（缺少錯誤處理） | Critical（嚴重） |
| 7 | `commands/data.rs:45` | `state.datasets.lock().unwrap()` — mutex 中毒時會 panic | 缺少錯誤處理 | Major（重大） |
| 8 | `data.rs:65`, `statistics.rs:12`, `export.rs:14` | 每個命令中相同的 `lock().unwrap()` 模式 | 重複程式碼 | Major（重大） |
| 9 | `commands/export.rs:37` | `export_df.clone()` — 對大型 DataFrame 的不必要克隆 | Performance（效能） | Minor（輕微） |
| 10 | `stats_engine.rs:51` | `series.f64().unwrap()` 在非 f64 型別時 panic | 缺少錯誤處理 | Major（重大） |
| 11 | `stats_engine.rs:72` | 百分位數計算中的 `sorted_series.f64().unwrap()` | 缺少錯誤處理 | Major（重大） |
| 12 | `stores/dataStore.ts:128-130` | 使用 `subscribe(...)()` 反模式，而非 `get()` | Inappropriate Intimacy（不當親密） | Major（重大） |
| 13 | `stores/dataStore.ts:142-143` | 再次出現相同的訂閱後立即取消模式 | 重複程式碼 | Major（重大） |
| 14 | `stores/dataStore.ts:102-135` | `removeFile` 以相同模式更新 5 個 store | Shotgun Surgery（散彈式修改） | Major（重大） |
| 15 | `chartOptions.ts:59,72,94` | ECharts 選項中普遍使用 `any[]` 和 `any` 型別 | 基本型別偏執 | Major（重大） |
| 16 | `TimeseriesChart.svelte:14` | 回呼 prop 中使用 `Record<string, any>` | Type Safety（型別安全） | Minor（輕微） |
| 17 | `TimeseriesChart.svelte:36,48,52,82,107,145` | 所有 ECharts 事件處理器使用 `params: any` | 型別安全 | Major（重大） |
| 18 | `TimeseriesChart.svelte:91,119,160,337` | 重複的 `as { type: 'Range'; ... }` 型別斷言 — 出現 4 次 | 重複程式碼 | Major（重大） |
| 19 | `+page.svelte:38` | `pendingAnnotation.data` 型別為 `any` | 型別安全 | Minor（輕微） |
| 20 | `ViewportDataTable.svelte:11-18` | `formatTime` 與 `chartOptions.ts:21-29` 重複 | 重複程式碼 | Major（重大） |
| 21 | `chartOptions.ts:5-15` | `COLOR_PALETTE` 與 `dataStore.ts:26-29` 重複 | 重複程式碼 | Minor（輕微） |
| 22 | `+page.svelte:168` | Point annotation 軸向硬編碼為 `'x'` 的備援值 | Hardcoded Value（硬編碼值） | Minor（輕微） |
| 23 | `csv_reader.rs:71` | 日期時間格式硬編碼為單一格式 | Rigidity（僵固性） | Minor（輕微） |
| 24 | `TimeseriesChart.svelte:32-186` | `onMount` 回呼長達 154 行，包含 5 個事件處理器 | Long Method（過長方法） | Critical（嚴重） |
| 25 | `chartOptions.ts:36-193` | `createOverviewOption` 長達 157 行 | 過長方法 | Major（重大） |
| 26 | `+page.svelte` | 14 個函式加狀態 — 逼近 God Component（上帝元件） | Large Class（過大類別） | Major（重大） |
| 27 | `commands/data.rs:58-121` | `get_timeseries_chunk` 長達 63 行 | 過長方法 | Major（重大） |
| 28 | `stores/dataStore.ts` | 單一模組中有 9 個 store、6 個函式、3 個衍生 store | 過大類別 | Minor（輕微） |

**合計：4 項嚴重、17 項重大、9 項輕微 = 30 個問題**

---

## 2. 函式長度稽核（> 20 行）

| 函式 | 檔案 | 行數 | 判定 |
|------|------|------|------|
| `onMount` 回呼 | `TimeseriesChart.svelte` | ~154 | 嚴重超標。應分解為 5 個具名處理器。 |
| `createOverviewOption` | `chartOptions.ts` | ~157 | 嚴重超標。抽取 tooltip、axes、dataZoom、series 子函式。 |
| `read_csv_with_mapping` | `csv_reader.rs` | 78 | 抽取時間轉換邏輯。 |
| `createSingleAxisOption` | `chartOptions.ts` | 69 | 大部分為宣告式設定。勉強可接受。 |
| `get_timeseries_chunk` | `commands/data.rs` | 63 | 拆分為：過濾、降取樣、回應建構。 |
| `lttb_indices` | `downsampling.rs` | 56 | 可接受 — 單一數值演算法。 |
| `load_vibration_data` | `commands/data.rs` | 44 | 勉強可接受。CSV 讀取 + 中繼資料 + 狀態插入。 |
| `removeFile` | `dataStore.ts` | 33 | 重複的 store 清理操作。 |
| `handleAnnotationConfirm` | `+page.svelte` | 28 | 勉強可接受。 |
| `compute_shape_stats` | `stats_engine.rs` | 23 | 恰在門檻上。 |
| `compute_distribution_stats` | `stats_engine.rs` | 21 | 恰在門檻上。 |

---

## 3. 複雜度熱點

1. **`TimeseriesChart.svelte` 的 onMount** — 5 個巢狀事件處理器，`mouseup` 達到 5 層縮排。違反 Torvalds 的「最多 3 層」規則。

2. **`get_timeseries_chunk`** — `if original_count > max_points` 重複了通道迭代，`for` 迴圈內有 3 層巢狀。

3. **`createOverviewOption`** — 對資料集和通道進行巢狀 `for` 迭代，`tooltip.formatter` 回呼又增加一層。

4. **`read_csv_with_mapping`** — `match &time_dtype` 包含 3 個非平凡分支。可抽取為 `convert_time_column` 輔助函式。

5. **`dataStore.ts` 中的 `removeFile`** — `activeDatasetId.update` 內包含巢狀 `subscribe` 呼叫 — 控制流程不尋常。

---

## 4. 型別安全問題

| # | 位置 | 問題 |
|---|------|------|
| 1 | `chartOptions.ts:59` | `markLineData: any[]` |
| 2 | `chartOptions.ts:72` | `series: any[]`（應為 `LineSeriesOption[]`） |
| 3 | `chartOptions.ts:94` | `baseSeries: any` |
| 4 | `chartOptions.ts:132` | `formatter: (params: any)` |
| 5 | `TimeseriesChart.svelte:36-145` | 6 個 ECharts 事件回呼型別標註為 `(params: any)` |
| 6 | `TimeseriesChart.svelte:91,119,160` | 重複使用 `as { type: 'Range' }` 而非 type guard（型別守衛） |
| 7 | `+page.svelte:38` | `pendingAnnotation` 中的 `data: any` |
| 8 | `Toolbar.svelte:22` | 雙重型別轉換 `as HTMLSelectElement` + `as PrecisionLevel` |

---

## 5. 錯誤處理審查

### Rust 端 — 嚴重

- **8 個以上的 `unwrap()` 呼叫**位於使用者操作可觸及的路徑上。任何 panic 都會導致應用程式崩潰。
- `extract_f64_vec` 對使用者提供的欄位名稱使用 `unwrap()`。
- 全部 3 個命令檔案使用 `state.datasets.lock().unwrap()`。Mutex 中毒會擴散至所有命令。
- `stats_engine.rs:48` — `std_dev = 0` 時會產生偏度/峰度的 NaN/Inf（參見 data-pipeline-review）。

### 前端

- `saveAnnotations` / `loadAnnotations` — 未使用 try/catch，錯誤會變成未處理的 promise rejection。
- `handleSave`、`handleExport`、`handleExportViewport` — `invoke()` 周圍無錯誤處理。
- `previewFile` 向呼叫者拋出例外，而其他 dataStore 函式則在內部捕獲 — 行為不一致。

---

## 6. 測試覆蓋率缺口

| 優先順序 | 模組 | 原因 |
|----------|------|------|
| P0 | `csv_reader::read_csv_with_mapping` | 3 個時間格式分支、使用者提供的檔案 |
| P0 | `stats_engine`（所有函式） | 數值正確性、邊界情況：空序列/單值/常數序列 |
| P1 | `commands::data::get_timeseries_chunk` | 過濾 + 降取樣整合測試 |
| P1 | `commands::annotation` | 儲存/載入往返測試、檔案不存在、格式錯誤的 JSON |
| P2 | `annotationStore.ts` | 新增/更新/移除/dirty 狀態行為 |
| P2 | `chartOptions.ts` — `formatTime`、`buildMarkPoints`、`buildMarkAreas` | 純函式 |

**目前：5 個測試（僅 LTTB）。目標：關鍵路徑約 30 個以上測試。**

---

## 7. 重複分析

| # | 模式 | 出現位置 | 修正方式 |
|---|------|---------|---------|
| 1 | `formatTime` | `chartOptions.ts:21-29`、`ViewportDataTable.svelte:11-18` | 抽取至 `utils/formatTime.ts` |
| 2 | 色彩調色盤 | `chartOptions.ts:5-15`、`dataStore.ts:26-29` | 抽取至 `constants/colors.ts` |
| 3 | 時間範圍過濾 | `data.rs:71-78`、`export.rs:20-27` | 抽取至 `services/time_filter.rs` |
| 4 | Range 型別斷言 | `TimeseriesChart.svelte`（4 次）、`chartOptions.ts`（2 次） | 建立型別守衛函式 |
| 5 | Mutex 鎖定 + 查詢 | `data.rs`、`statistics.rs`、`export.rs` | 建立輔助函式 |
| 6 | Store 清理 | `dataStore.ts removeFile`（5 個 store） | 組合式 store 或泛用輔助函式 |

---

## 8. 前 10 項建議（依影響程度排序）

1. **[嚴重] 消除 Tauri 命令處理器中的 `unwrap()`** — 8 個以上的 panic 點可由使用者操作觸發。替換為 `?` 和 `map_err()`。

2. **[嚴重] 分解 `TimeseriesChart.svelte` 的 onMount** — 154 行、5 層縮排。抽取 5 個具名事件處理函式。

3. **[重大] 為前端非同步操作加入錯誤處理** — `handleSave`、`handleExport`、`saveAnnotations` 完全沒有錯誤回饋。

4. **[重大] 抽取重複的時間範圍過濾** — `data.rs` 和 `export.rs` 中相同的 Polars 遮罩程式碼。建立共用服務函式。

5. **[重大] 以 ECharts 型別減少 `any` 的使用** — 使用 `LineSeriesOption`、`ECElementEvent`、`CallbackDataParams`。

6. **[重大] 為 `csv_reader` 和 `stats_engine` 加入單元測試** — 最容易出錯的兩個模組目前零測試。

7. **[重大] 修正「訂閱後立即取消」反模式** — `dataStore.ts:128,142` 應使用 `get()`（已匯入但未使用）。

8. **[重大] 消除 `formatTime` 的重複** — 移至共用的 `utils/formatTime.ts`。

9. **[輕微] 處理 `compute_shape_stats` 中標準差為零的情況** — 當 `std_dev < EPSILON` 時，偏度/峰度回傳 0.0。

10. **[輕微] 將 `createOverviewOption` 拆分為子函式** — 抽取 `buildTooltipConfig`、`buildAxisConfig`、`buildDataZoomConfig`。
