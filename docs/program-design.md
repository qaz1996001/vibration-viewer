# 振動時序標注工具 — 程式設計

> **技術棧：方案 C — Tauri + Svelte + ECharts**
>
> 設計原則：
> - 「方法應該做一件事」— Martin Fowler
> - 「先讓程式運作，再確保正確性，最後才優化」— Ken Thompson
> - 「過早優化是萬惡之源」— Donald Knuth
> - 「系統應小到一個人能理解全貌」— Ken Thompson

---

## 1. Rust 後端程式設計

### 1.1 進入點 — main.rs

```rust
// src-tauri/src/main.rs

mod commands;
mod models;
mod services;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::data::load_vibration_data,
            commands::data::get_timeseries_chunk,
            commands::statistics::compute_statistics,
            commands::annotation::save_annotations,
            commands::annotation::load_annotations,
            commands::export::export_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 1.2 全域狀態 — state.rs

> **原則：資料結構清晰，不藏暗處（Linus Torvalds）**

```rust
// src-tauri/src/state.rs

use std::collections::HashMap;
use std::sync::Mutex;
use polars::prelude::DataFrame;

/// 應用程式全域狀態
/// 使用 Mutex 保護，Tauri manage() 自動注入
pub struct AppState {
    /// 已載入的資料集（id → DataFrame）
    pub datasets: Mutex<HashMap<String, DatasetEntry>>,
}

pub struct DatasetEntry {
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            datasets: Mutex::new(HashMap::new()),
        }
    }
}
```

### 1.3 CSV 讀取服務 — services/csv_reader.rs

```rust
// src-tauri/src/services/csv_reader.rs

use polars::prelude::*;
use std::path::Path;

/// 讀取振動 CSV 檔案為 Polars DataFrame
///
/// 預期欄位：time, x, y, z（或類似命名）
/// 自動計算 amplitude = sqrt(x² + y² + z²)
pub fn read_vibration_csv(file_path: &str) -> Result<DataFrame, String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("檔案不存在: {}", file_path));
    }

    let df = CsvReadOptions::default()
        .try_into_reader_with_file_path(Some(path.into()))
        .map_err(|e| format!("無法建立 CSV reader: {}", e))?
        .finish()
        .map_err(|e| format!("CSV 解析失敗: {}", e))?;

    // 驗證必要欄位存在
    let required = ["time", "x", "y", "z"];
    for col in &required {
        if df.column(col).is_err() {
            return Err(format!("缺少必要欄位: {}", col));
        }
    }

    // 新增 amplitude 欄位
    let df = df
        .lazy()
        .with_column(
            (col("x").pow(2) + col("y").pow(2) + col("z").pow(2))
                .sqrt()
                .alias("amplitude"),
        )
        .collect()
        .map_err(|e| format!("計算 amplitude 失敗: {}", e))?;

    Ok(df)
}
```

### 1.4 LTTB Downsampling — services/downsampling.rs

> **原則：懷疑時，用暴力法。正確比聰明更重要（Ken Thompson）**

```rust
// src-tauri/src/services/downsampling.rs

/// LTTB (Largest-Triangle-Three-Buckets) downsampling
///
/// 將 N 點降至 threshold 點，保留視覺特徵。
/// 參考：Sveinn Steinarsson, "Downsampling Time Series for Visual Representation"
pub fn lttb(time: &[f64], values: &[f64], threshold: usize) -> (Vec<f64>, Vec<f64>) {
    let n = time.len();

    // 不需要 downsampling
    if threshold >= n || threshold < 3 {
        return (time.to_vec(), values.to_vec());
    }

    let mut sampled_time = Vec::with_capacity(threshold);
    let mut sampled_values = Vec::with_capacity(threshold);

    // 永遠保留第一點和最後一點
    sampled_time.push(time[0]);
    sampled_values.push(values[0]);

    let bucket_size = (n - 2) as f64 / (threshold - 2) as f64;

    let mut prev_index = 0usize;

    for i in 1..(threshold - 1) {
        // 目前 bucket 的範圍
        let bucket_start = ((i - 1) as f64 * bucket_size).floor() as usize + 1;
        let bucket_end = (i as f64 * bucket_size).floor() as usize + 1;
        let bucket_end = bucket_end.min(n - 1);

        // 下一個 bucket 的平均值（作為三角形的第三個頂點）
        let next_start = bucket_end;
        let next_end = ((i + 1) as f64 * bucket_size).floor() as usize + 1;
        let next_end = next_end.min(n);

        let avg_time: f64 = time[next_start..next_end].iter().sum::<f64>()
            / (next_end - next_start) as f64;
        let avg_value: f64 = values[next_start..next_end].iter().sum::<f64>()
            / (next_end - next_start) as f64;

        // 在目前 bucket 中找最大三角形面積的點
        let mut max_area = -1.0f64;
        let mut max_index = bucket_start;

        let prev_time = time[prev_index];
        let prev_value = values[prev_index];

        for j in bucket_start..bucket_end {
            let area = ((prev_time - avg_time) * (values[j] - prev_value)
                - (prev_time - time[j]) * (avg_value - prev_value))
                .abs()
                * 0.5;

            if area > max_area {
                max_area = area;
                max_index = j;
            }
        }

        sampled_time.push(time[max_index]);
        sampled_values.push(values[max_index]);
        prev_index = max_index;
    }

    sampled_time.push(time[n - 1]);
    sampled_values.push(values[n - 1]);

    (sampled_time, sampled_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lttb_preserves_length_when_below_threshold() {
        let time = vec![1.0, 2.0, 3.0];
        let values = vec![10.0, 20.0, 30.0];
        let (t, v) = lttb(&time, &values, 10);
        assert_eq!(t.len(), 3);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn test_lttb_reduces_to_threshold() {
        let time: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.1).sin()).collect();
        let (t, v) = lttb(&time, &values, 100);
        assert_eq!(t.len(), 100);
        assert_eq!(v.len(), 100);
        // 保留第一點和最後一點
        assert_eq!(t[0], 0.0);
        assert_eq!(t[99], 999.0);
    }

    #[test]
    fn test_lttb_preserves_first_and_last() {
        let time: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| t * t).collect();
        let (t, _) = lttb(&time, &values, 20);
        assert_eq!(t[0], time[0]);
        assert_eq!(*t.last().unwrap(), *time.last().unwrap());
    }
}
```

### 1.5 統計引擎 — services/stats_engine.rs

```rust
// src-tauri/src/services/stats_engine.rs

use polars::prelude::*;
use crate::models::statistics::*;

/// 計算單軸基本統計
pub fn compute_basic_stats(series: &Series, axis_name: &str) -> AxisBasicStats {
    let count = series.len();
    let mean = series.mean().unwrap_or(0.0);
    let std_dev = series.std(1).unwrap_or(0.0); // ddof=1
    let cv_percent = if mean.abs() > f64::EPSILON {
        (std_dev / mean.abs()) * 100.0
    } else {
        0.0
    };

    AxisBasicStats {
        axis: axis_name.to_string(),
        count,
        mean,
        std_dev,
        cv_percent,
    }
}

/// 計算單軸分佈統計
pub fn compute_distribution_stats(series: &Series, axis_name: &str) -> AxisDistributionStats {
    let sorted = series.sort(false).unwrap();
    let n = sorted.len();

    let min = sorted.min::<f64>().unwrap_or(0.0);
    let max = sorted.max::<f64>().unwrap_or(0.0);
    let median = sorted.median().unwrap_or(0.0);

    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);

    AxisDistributionStats {
        axis: axis_name.to_string(),
        min,
        q1,
        median,
        q3,
        max,
        iqr: q3 - q1,
    }
}

/// 計算單軸形狀統計（偏度、峰度）
pub fn compute_shape_stats(series: &Series, axis_name: &str) -> AxisShapeStats {
    let mean = series.mean().unwrap_or(0.0);
    let std_dev = series.std(1).unwrap_or(1.0);
    let n = series.len() as f64;

    let ca = series.f64().unwrap();
    let mut sum3 = 0.0;
    let mut sum4 = 0.0;

    for opt_val in ca.into_iter() {
        if let Some(val) = opt_val {
            let z = (val - mean) / std_dev;
            sum3 += z.powi(3);
            sum4 += z.powi(4);
        }
    }

    let skewness = sum3 / n;
    let kurtosis = (sum4 / n) - 3.0; // excess kurtosis

    AxisShapeStats {
        axis: axis_name.to_string(),
        skewness,
        kurtosis,
    }
}

/// 線性插值百分位數
fn percentile(sorted_series: &Series, pct: f64) -> f64 {
    let ca = sorted_series.f64().unwrap();
    let n = ca.len();
    if n == 0 {
        return 0.0;
    }
    let rank = (pct / 100.0) * (n - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let frac = rank - lower as f64;

    let lower_val = ca.get(lower).unwrap_or(0.0);
    let upper_val = ca.get(upper).unwrap_or(0.0);

    lower_val + frac * (upper_val - lower_val)
}
```

### 1.6 IPC 命令 — commands/data.rs

> **原則：函數做一件事（Martin Fowler）**

```rust
// src-tauri/src/commands/data.rs

use tauri::State;
use uuid::Uuid;
use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling};
use crate::state::AppState;

/// 載入振動資料檔案
#[tauri::command]
pub fn load_vibration_data(
    file_path: String,
    state: State<AppState>,
) -> Result<VibrationDataset, String> {
    let df = csv_reader::read_vibration_csv(&file_path)?;

    let time_col = df.column("time").unwrap().f64().unwrap();
    let total_points = df.height();
    let time_range = (
        time_col.min().unwrap_or(0.0),
        time_col.max().unwrap_or(0.0),
    );

    let id = Uuid::new_v4().to_string();
    let metadata = VibrationDataset {
        id: id.clone(),
        file_path: file_path.clone(),
        total_points,
        time_range,
        columns: df.get_column_names().iter().map(|s| s.to_string()).collect(),
    };

    let mut datasets = state.datasets.lock().unwrap();
    datasets.insert(
        id.clone(),
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );

    Ok(metadata)
}

/// 取得時序資料分段（含 downsampling）
#[tauri::command]
pub fn get_timeseries_chunk(
    dataset_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    state: State<AppState>,
) -> Result<TimeseriesChunk, String> {
    let datasets = state.datasets.lock().unwrap();
    let entry = datasets
        .get(&dataset_id)
        .ok_or("資料集不存在")?;

    let df = &entry.dataframe;

    // 篩選時間範圍
    let mask = df
        .column("time")
        .unwrap()
        .f64()
        .unwrap()
        .into_iter()
        .map(|opt| opt.map_or(false, |t| t >= start_time && t <= end_time))
        .collect::<BooleanChunked>();

    let filtered = df.filter(&mask).map_err(|e| e.to_string())?;
    let original_count = filtered.height();

    // 提取各欄位
    let time_raw = extract_f64_vec(&filtered, "time");
    let x_raw = extract_f64_vec(&filtered, "x");
    let y_raw = extract_f64_vec(&filtered, "y");
    let z_raw = extract_f64_vec(&filtered, "z");
    let amp_raw = extract_f64_vec(&filtered, "amplitude");

    let is_downsampled = original_count > max_points;

    if is_downsampled {
        // 對每軸分別做 LTTB（共用同一組時間軸）
        let (time, x) = downsampling::lttb(&time_raw, &x_raw, max_points);
        let (_, y) = downsampling::lttb(&time_raw, &y_raw, max_points);
        let (_, z) = downsampling::lttb(&time_raw, &z_raw, max_points);
        let (_, amplitude) = downsampling::lttb(&time_raw, &amp_raw, max_points);

        Ok(TimeseriesChunk {
            time,
            x,
            y,
            z,
            amplitude,
            is_downsampled: true,
            original_count,
        })
    } else {
        Ok(TimeseriesChunk {
            time: time_raw,
            x: x_raw,
            y: y_raw,
            z: z_raw,
            amplitude: amp_raw,
            is_downsampled: false,
            original_count,
        })
    }
}

fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name)
        .unwrap()
        .f64()
        .unwrap()
        .into_no_null_iter()
        .collect()
}
```

### 1.7 IPC 命令 — commands/annotation.rs

```rust
// src-tauri/src/commands/annotation.rs

use std::fs;
use std::path::PathBuf;
use crate::models::annotation::*;

/// 儲存標注到 JSON 檔案
#[tauri::command]
pub fn save_annotations(
    dataset_id: String,
    file_path: String,
    annotations: Vec<Annotation>,
) -> Result<(), String> {
    let ann_path = annotation_file_path(&file_path);

    let ann_file = AnnotationFile {
        version: 1,
        dataset_id,
        annotations,
    };

    let json = serde_json::to_string_pretty(&ann_file)
        .map_err(|e| format!("序列化失敗: {}", e))?;

    fs::write(&ann_path, json)
        .map_err(|e| format!("寫入檔案失敗: {}", e))?;

    Ok(())
}

/// 載入標注從 JSON 檔案
#[tauri::command]
pub fn load_annotations(file_path: String) -> Result<Vec<Annotation>, String> {
    let ann_path = annotation_file_path(&file_path);

    if !ann_path.exists() {
        return Ok(Vec::new()); // 沒有標注檔案，返回空
    }

    let json = fs::read_to_string(&ann_path)
        .map_err(|e| format!("讀取檔案失敗: {}", e))?;

    let ann_file: AnnotationFile = serde_json::from_str(&json)
        .map_err(|e| format!("解析 JSON 失敗: {}", e))?;

    Ok(ann_file.annotations)
}

/// 標注檔案路徑：{原始檔名}.vibann.json
fn annotation_file_path(data_file_path: &str) -> PathBuf {
    let mut path = PathBuf::from(data_file_path);
    let filename = path.file_name().unwrap().to_string_lossy().to_string();
    path.set_file_name(format!("{}.vibann.json", filename));
    path
}
```

---

## 2. Svelte 前端程式設計

### 2.1 Stores — stores/dataStore.ts

> **原則：用資料驅動行為（Ken Thompson）**

```typescript
// src/lib/stores/dataStore.ts

import { writable, derived } from "svelte/store"
import { invoke } from "@tauri-apps/api/core"
import type { VibrationDataset, TimeseriesChunk } from "$lib/types/vibration"
import type { StatisticsReport } from "$lib/types/statistics"

// --- State ---

export const dataset = writable<VibrationDataset | null>(null)
export const chunk = writable<TimeseriesChunk | null>(null)
export const statistics = writable<StatisticsReport | null>(null)
export const loading = writable(false)
export const error = writable<string | null>(null)

// --- Actions ---

export async function loadFile(filePath: string): Promise<void> {
  loading.set(true)
  error.set(null)

  try {
    const ds = await invoke<VibrationDataset>("load_vibration_data", {
      filePath,
    })
    dataset.set(ds)

    // 載入後同時取得初始 chunk 和統計
    await Promise.all([
      fetchChunk(ds.id, ds.time_range[0], ds.time_range[1], 50000),
      fetchStatistics(ds.id),
    ])
  } catch (e) {
    error.set(String(e))
  } finally {
    loading.set(false)
  }
}

export async function fetchChunk(
  datasetId: string,
  startTime: number,
  endTime: number,
  maxPoints: number,
): Promise<void> {
  try {
    const c = await invoke<TimeseriesChunk>("get_timeseries_chunk", {
      datasetId,
      startTime,
      endTime,
      maxPoints,
    })
    chunk.set(c)
  } catch (e) {
    error.set(String(e))
  }
}

async function fetchStatistics(datasetId: string): Promise<void> {
  try {
    const stats = await invoke<StatisticsReport>("compute_statistics", {
      datasetId,
    })
    statistics.set(stats)
  } catch (e) {
    error.set(String(e))
  }
}
```

### 2.2 Stores — stores/annotationStore.ts

```typescript
// src/lib/stores/annotationStore.ts

import { writable, get } from "svelte/store"
import { invoke } from "@tauri-apps/api/core"
import type { Annotation, AnnotationType } from "$lib/types/annotation"

// --- State ---

export const annotations = writable<Annotation[]>([])
export const selectedId = writable<string | null>(null)
export const dirty = writable(false)

// --- Actions ---

export function addAnnotation(
  annotationType: AnnotationType,
  label: string,
  color: string = "#ff6b6b",
): void {
  const newAnnotation: Annotation = {
    id: crypto.randomUUID(),
    annotation_type: annotationType,
    label,
    color,
    label_offset_x: 0,
    label_offset_y: 0,
    created_at: new Date().toISOString(),
  }

  annotations.update((list) => [...list, newAnnotation])
  dirty.set(true)
}

export function removeAnnotation(id: string): void {
  annotations.update((list) => list.filter((a) => a.id !== id))
  selectedId.update((sel) => (sel === id ? null : sel))
  dirty.set(true)
}

export function updateAnnotation(
  id: string,
  updates: Partial<Annotation>,
): void {
  annotations.update((list) =>
    list.map((a) => (a.id === id ? { ...a, ...updates } : a)),
  )
  dirty.set(true)
}

export async function saveAnnotations(
  datasetId: string,
  filePath: string,
): Promise<void> {
  const current = get(annotations)
  await invoke("save_annotations", {
    datasetId,
    filePath,
    annotations: current,
  })
  dirty.set(false)
}

export async function loadAnnotations(filePath: string): Promise<void> {
  const loaded = await invoke<Annotation[]>("load_annotations", { filePath })
  annotations.set(loaded)
  dirty.set(false)
}
```

### 2.3 Stores — stores/modeStore.ts

```typescript
// src/lib/stores/modeStore.ts

import { writable } from "svelte/store"

export type AppMode = "browse" | "annotate_point" | "annotate_range"

export const mode = writable<AppMode>("browse")
```

### 2.4 ECharts 配置工廠 — components/Chart/chartOptions.ts

> **原則：分離資料與呈現（Ken Thompson）— 配置是資料，元件是呈現**

```typescript
// src/lib/components/Chart/chartOptions.ts

import type { EChartsOption } from "echarts"
import type { TimeseriesChunk } from "$lib/types/vibration"
import type { Annotation } from "$lib/types/annotation"

/** 建立三軸總覽圖的 ECharts 配置 */
export function createOverviewOption(
  chunk: TimeseriesChunk,
  annotations: Annotation[],
): EChartsOption {
  return {
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "cross" },
    },
    legend: {
      data: ["X", "Y", "Z"],
    },
    dataZoom: [
      { type: "slider", xAxisIndex: 0, start: 0, end: 100 },
      { type: "inside", xAxisIndex: 0 },
    ],
    xAxis: {
      type: "category",
      data: chunk.time.map((t) => t.toFixed(4)),
      name: "Time",
    },
    yAxis: {
      type: "value",
      name: "Vibration",
    },
    series: [
      {
        name: "X",
        type: "line",
        data: chunk.x,
        symbol: "none",
        lineStyle: { width: 1 },
        markPoint: { data: buildMarkPoints(annotations, "x") },
        markArea: { data: buildMarkAreas(annotations) },
      },
      {
        name: "Y",
        type: "line",
        data: chunk.y,
        symbol: "none",
        lineStyle: { width: 1 },
      },
      {
        name: "Z",
        type: "line",
        data: chunk.z,
        symbol: "none",
        lineStyle: { width: 1 },
      },
    ],
  }
}

/** 將 Point 標注轉為 ECharts markPoint 格式 */
function buildMarkPoints(
  annotations: Annotation[],
  axis: string,
): Array<{ coord: [number, number]; name: string }> {
  return annotations
    .filter(
      (a) => a.annotation_type.type === "Point" && a.annotation_type.axis === axis,
    )
    .map((a) => {
      const pt = a.annotation_type as { type: "Point"; time: number; value: number; axis: string }
      return {
        coord: [pt.time, pt.value],
        name: a.label,
        itemStyle: { color: a.color },
        label: {
          show: true,
          formatter: a.label,
          offset: [a.label_offset_x, a.label_offset_y],
        },
      }
    })
}

/** 將 Range 標注轉為 ECharts markArea 格式 */
function buildMarkAreas(
  annotations: Annotation[],
): Array<[{ xAxis: number; name: string }, { xAxis: number }]> {
  return annotations
    .filter((a) => a.annotation_type.type === "Range")
    .map((a) => {
      const range = a.annotation_type as { type: "Range"; start_time: number; end_time: number }
      return [
        {
          xAxis: range.start_time,
          name: a.label,
          itemStyle: { color: a.color, opacity: 0.3 },
          label: { show: true, position: "insideTop" },
        },
        { xAxis: range.end_time },
      ]
    })
}
```

### 2.5 主圖表元件 — components/Chart/TimeseriesChart.svelte

```svelte
<!-- src/lib/components/Chart/TimeseriesChart.svelte -->
<script lang="ts">
  import { onMount, onDestroy } from "svelte"
  import * as echarts from "echarts"
  import { chunk } from "$lib/stores/dataStore"
  import { annotations } from "$lib/stores/annotationStore"
  import { mode } from "$lib/stores/modeStore"
  import { createOverviewOption } from "./chartOptions"
  import type { AppMode } from "$lib/stores/modeStore"

  let chartContainer: HTMLDivElement
  let chart: echarts.ECharts | null = null

  onMount(() => {
    chart = echarts.init(chartContainer)

    // 監聽 dataZoom 事件
    chart.on("datazoom", handleDataZoom)

    // 監聯點擊事件（標記點模式）
    chart.getZr().on("click", handleChartClick)

    // 監聽 brush 事件（標記區間模式）
    chart.on("brushSelected", handleBrushSelected)

    return () => {
      chart?.dispose()
    }
  })

  // 當資料或標注變更時更新圖表
  $: if (chart && $chunk) {
    const option = createOverviewOption($chunk, $annotations)
    configureBrush(option, $mode)
    chart.setOption(option, { notMerge: false })
  }

  function configureBrush(option: echarts.EChartsOption, currentMode: AppMode) {
    if (currentMode === "annotate_range") {
      option.brush = {
        toolbox: ["rect"],
        xAxisIndex: 0,
        brushStyle: { borderWidth: 1, color: "rgba(255,107,107,0.2)" },
      }
    } else {
      option.brush = { toolbox: [] } // 停用 brush
    }
  }

  function handleDataZoom(params: any) {
    // debounce 後觸發 fetchChunk — 由父元件處理
    dispatch("datazoom", {
      start: params.start,
      end: params.end,
    })
  }

  function handleChartClick(params: any) {
    if ($mode !== "annotate_point") return

    // 將像素座標轉為資料座標
    const pointInPixel = [params.offsetX, params.offsetY]
    if (!chart) return

    const pointInGrid = chart.convertFromPixel("grid", pointInPixel)
    if (!pointInGrid) return

    dispatch("annotate-point", {
      time: pointInGrid[0],
      value: pointInGrid[1],
    })
  }

  function handleBrushSelected(params: any) {
    if ($mode !== "annotate_range") return

    const areas = params.batch?.[0]?.areas
    if (!areas || areas.length === 0) return

    const range = areas[0].coordRange
    dispatch("annotate-range", {
      startTime: range[0],
      endTime: range[1],
    })
  }

  // Svelte event dispatching
  import { createEventDispatcher } from "svelte"
  const dispatch = createEventDispatcher()
</script>

<div bind:this={chartContainer} class="chart-container" />

<style>
  .chart-container {
    width: 100%;
    height: 400px;
  }
</style>
```

### 2.6 標注面板 — components/Annotation/AnnotationPanel.svelte

```svelte
<!-- src/lib/components/Annotation/AnnotationPanel.svelte -->
<script lang="ts">
  import {
    annotations,
    selectedId,
    removeAnnotation,
  } from "$lib/stores/annotationStore"
  import type { Annotation } from "$lib/types/annotation"

  function handleSelect(id: string) {
    selectedId.set(id)
  }

  function handleDelete(id: string) {
    removeAnnotation(id)
  }

  function formatType(ann: Annotation): string {
    if (ann.annotation_type.type === "Point") {
      return `點 (${ann.annotation_type.time.toFixed(2)})`
    } else {
      return `區間 (${ann.annotation_type.start_time.toFixed(2)} - ${ann.annotation_type.end_time.toFixed(2)})`
    }
  }
</script>

<div class="annotation-panel">
  <h3>標注列表 ({$annotations.length})</h3>

  {#each $annotations as ann (ann.id)}
    <div
      class="annotation-item"
      class:selected={$selectedId === ann.id}
      on:click={() => handleSelect(ann.id)}
      on:keydown={(e) => e.key === "Enter" && handleSelect(ann.id)}
      role="button"
      tabindex="0"
    >
      <span class="color-dot" style="background: {ann.color}" />
      <div class="annotation-info">
        <span class="label">{ann.label}</span>
        <span class="type">{formatType(ann)}</span>
      </div>
      <button
        class="delete-btn"
        on:click|stopPropagation={() => handleDelete(ann.id)}
      >
        ✕
      </button>
    </div>
  {:else}
    <p class="empty">尚無標注</p>
  {/each}
</div>

<style>
  .annotation-panel {
    padding: 0.5rem;
    overflow-y: auto;
  }
  .annotation-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    border-radius: 4px;
    cursor: pointer;
  }
  .annotation-item:hover {
    background: var(--surface-hover, #f0f0f0);
  }
  .annotation-item.selected {
    background: var(--surface-active, #e0e0ff);
  }
  .color-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .annotation-info {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .label {
    font-weight: 500;
  }
  .type {
    font-size: 0.8em;
    color: var(--text-secondary, #666);
  }
  .delete-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-secondary, #999);
    font-size: 1rem;
  }
  .delete-btn:hover {
    color: var(--error, #ff4444);
  }
  .empty {
    color: var(--text-secondary, #999);
    text-align: center;
    padding: 2rem 0;
  }
</style>
```

### 2.7 主頁面 — routes/+page.svelte

```svelte
<!-- src/routes/+page.svelte -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog"
  import TimeseriesChart from "$lib/components/Chart/TimeseriesChart.svelte"
  import AnnotationPanel from "$lib/components/Annotation/AnnotationPanel.svelte"
  import AnnotationDialog from "$lib/components/Annotation/AnnotationDialog.svelte"
  import BasicStatsTable from "$lib/components/Statistics/BasicStatsTable.svelte"
  import Toolbar from "$lib/components/Layout/Toolbar.svelte"
  import {
    dataset,
    chunk,
    statistics,
    loading,
    error,
    loadFile,
    fetchChunk,
  } from "$lib/stores/dataStore"
  import {
    addAnnotation,
    saveAnnotations,
    loadAnnotations,
    dirty,
  } from "$lib/stores/annotationStore"
  import { mode } from "$lib/stores/modeStore"

  let showAnnotationDialog = false
  let pendingAnnotation: { type: "point" | "range"; data: any } | null = null

  async function handleOpenFile() {
    const filePath = await open({
      filters: [{ name: "CSV", extensions: ["csv"] }],
    })
    if (filePath) {
      await loadFile(filePath)
      await loadAnnotations(filePath)
    }
  }

  async function handleSave() {
    if ($dataset) {
      await saveAnnotations($dataset.id, $dataset.file_path)
    }
  }

  function handleAnnotatePoint(event: CustomEvent<{ time: number; value: number }>) {
    pendingAnnotation = { type: "point", data: event.detail }
    showAnnotationDialog = true
  }

  function handleAnnotateRange(event: CustomEvent<{ startTime: number; endTime: number }>) {
    pendingAnnotation = { type: "range", data: event.detail }
    showAnnotationDialog = true
  }

  function handleAnnotationConfirm(event: CustomEvent<{ label: string; color: string }>) {
    if (!pendingAnnotation) return

    const { label, color } = event.detail
    if (pendingAnnotation.type === "point") {
      addAnnotation(
        {
          type: "Point",
          time: pendingAnnotation.data.time,
          value: pendingAnnotation.data.value,
          axis: "x", // TODO: 根據實際點擊的軸判斷
        },
        label,
        color,
      )
    } else {
      addAnnotation(
        {
          type: "Range",
          start_time: pendingAnnotation.data.startTime,
          end_time: pendingAnnotation.data.endTime,
        },
        label,
        color,
      )
    }

    showAnnotationDialog = false
    pendingAnnotation = null
  }
</script>

<main class="app-layout">
  <Toolbar
    on:open-file={handleOpenFile}
    on:save={handleSave}
    hasUnsaved={$dirty}
  />

  <div class="content">
    <div class="main-area">
      {#if $loading}
        <div class="loading">載入中...</div>
      {:else if $error}
        <div class="error">{$error}</div>
      {:else if $chunk}
        <TimeseriesChart
          on:annotate-point={handleAnnotatePoint}
          on:annotate-range={handleAnnotateRange}
        />

        {#if $statistics}
          <BasicStatsTable stats={$statistics} />
        {/if}
      {:else}
        <div class="welcome">
          <p>請開啟 CSV 檔案開始分析</p>
        </div>
      {/if}
    </div>

    <aside class="sidebar">
      <AnnotationPanel />
    </aside>
  </div>

  {#if showAnnotationDialog}
    <AnnotationDialog
      on:confirm={handleAnnotationConfirm}
      on:cancel={() => (showAnnotationDialog = false)}
    />
  {/if}
</main>

<style>
  .app-layout {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  .content {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .main-area {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }
  .sidebar {
    width: 280px;
    border-left: 1px solid var(--border, #e0e0e0);
    overflow-y: auto;
  }
  .loading,
  .error,
  .welcome {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 300px;
  }
  .error {
    color: var(--error, #ff4444);
  }
</style>
```

---

## 3. 關鍵演算法

### 3.1 LTTB Downsampling

見 §1.4，完整實作含測試。

### 3.2 座標轉換（像素 ↔ 資料座標）

ECharts 提供 `convertFromPixel` / `convertToPixel`，直接使用：

```typescript
// 像素 → 資料座標（用於標記點）
const dataCoord = chart.convertFromPixel("grid", [pixelX, pixelY])

// 資料座標 → 像素（用於 Label 拖拉定位）
const pixelCoord = chart.convertToPixel("grid", [dataX, dataY])
```

### 3.3 Debounce（縮放事件節流）

```typescript
// src/lib/utils/debounce.ts

export function debounce<T extends (...args: any[]) => void>(
  fn: T,
  delay: number,
): T {
  let timer: ReturnType<typeof setTimeout>
  return ((...args: any[]) => {
    clearTimeout(timer)
    timer = setTimeout(() => fn(...args), delay)
  }) as T
}
```

---

## 4. 測試策略

> **原則：沒有測試就沒有信心（Martin Fowler）**

### 4.1 Rust 單元測試

```rust
// src-tauri/tests/data_test.rs

#[cfg(test)]
mod tests {
    use crate::services::csv_reader::read_vibration_csv;

    #[test]
    fn test_read_valid_csv() {
        let df = read_vibration_csv("../res/test_data.csv").unwrap();
        assert!(df.height() > 0);
        assert!(df.column("amplitude").is_ok());
    }

    #[test]
    fn test_read_missing_file() {
        let result = read_vibration_csv("nonexistent.csv");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_invalid_format() {
        let result = read_vibration_csv("../res/invalid.csv");
        assert!(result.is_err());
    }
}
```

### 4.2 前端測試重點

| 測試目標 | 方法 |
|---------|------|
| annotationStore CRUD | Svelte store 單元測試 |
| chartOptions 輸出 | 純函數單元測試 |
| 標注 Dialog 互動 | Svelte Testing Library |
| IPC 整合 | mock `invoke`，驗證參數 |

---

## 5. 程式碼品質檢查清單

> 根據 Martin Fowler 指引，每次 commit 前檢查：

- [ ] 每個函數只做一件事？
- [ ] 命名是否表達意圖（不用縮寫）？
- [ ] 方法 < 20 行？
- [ ] 參數 < 4 個？
- [ ] 巢狀 < 3 層？
- [ ] 無重複程式碼？
- [ ] Rust: `cargo clippy` 無 warning？
- [ ] TypeScript: `strict: true` 無 error？
- [ ] 關鍵路徑有測試覆蓋？
