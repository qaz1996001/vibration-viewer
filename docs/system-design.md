# 振動時序標注工具 — 系統設計

> **技術棧：方案 C — Tauri + Svelte + ECharts**
>
> 設計原則：
> - 「好的資料結構讓演算法變得顯而易見」— Ken Thompson
> - 「乾淨的介面應小到可記住」— Ken Thompson
> - 「架構是那些既重要又難以改變的決策」— Martin Fowler
> - 「先正確，再清晰，最後才快」— Donald Knuth

---

## 1. 目錄結構

```
vibration-viewer/
├── src-tauri/                    # Rust 後端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   ├── commands/              # IPC 命令（每個檔案做一件事）
│   │   │   ├── mod.rs
│   │   │   ├── data.rs            # load_vibration_data, get_timeseries_chunk
│   │   │   ├── statistics.rs      # compute_statistics
│   │   │   ├── annotation.rs      # save/load_annotations
│   │   │   └── export.rs          # export_data
│   │   ├── models/                # 資料結構
│   │   │   ├── mod.rs
│   │   │   ├── vibration.rs       # VibrationDataset, TimeseriesChunk
│   │   │   ├── annotation.rs      # Annotation, AnnotationType
│   │   │   └── statistics.rs      # StatisticsReport
│   │   ├── services/              # 業務邏輯
│   │   │   ├── mod.rs
│   │   │   ├── csv_reader.rs      # CSV 解析
│   │   │   ├── downsampling.rs    # LTTB 演算法
│   │   │   └── stats_engine.rs    # 統計計算
│   │   └── state.rs               # Tauri 全域狀態（AppState）
│   └── tests/
│       ├── data_test.rs
│       ├── downsampling_test.rs
│       └── annotation_test.rs
│
├── src/                           # Svelte 前端
│   ├── app.html
│   ├── app.css
│   ├── lib/
│   │   ├── components/            # UI 元件
│   │   │   ├── Chart/
│   │   │   │   ├── TimeseriesChart.svelte    # 三軸時序圖
│   │   │   │   ├── SingleAxisChart.svelte    # 單軸分析
│   │   │   │   └── chartOptions.ts           # ECharts 配置工廠
│   │   │   ├── Annotation/
│   │   │   │   ├── AnnotationPanel.svelte    # 標注列表面板
│   │   │   │   ├── AnnotationDialog.svelte   # 新增/編輯標注 dialog
│   │   │   │   └── AnnotationMarker.svelte   # 單一標注渲染
│   │   │   ├── Statistics/
│   │   │   │   ├── BasicStatsTable.svelte    # 基本統計
│   │   │   │   ├── DistributionTable.svelte  # 分佈統計
│   │   │   │   └── ShapeStatsTable.svelte    # 形狀統計
│   │   │   ├── DataTable/
│   │   │   │   └── ViewportTable.svelte      # 視野資料表
│   │   │   └── Layout/
│   │   │       ├── Toolbar.svelte            # 工具列
│   │   │       └── Sidebar.svelte            # 側邊欄
│   │   ├── stores/                # Svelte stores（狀態管理）
│   │   │   ├── dataStore.ts       # 振動資料狀態
│   │   │   ├── annotationStore.ts # 標注狀態
│   │   │   ├── viewStore.ts       # UI 視圖狀態（zoom 範圍等）
│   │   │   └── modeStore.ts       # 操作模式（瀏覽/標注）
│   │   ├── services/              # Tauri IPC 封裝
│   │   │   ├── dataService.ts     # 資料相關 invoke
│   │   │   ├── annotationService.ts # 標注相關 invoke
│   │   │   └── exportService.ts   # 匯出相關 invoke
│   │   └── types/                 # TypeScript 型別定義
│   │       ├── vibration.ts       # 振動資料型別
│   │       ├── annotation.ts      # 標注型別
│   │       └── statistics.ts      # 統計型別
│   └── routes/
│       └── +page.svelte           # 主頁面（SvelteKit）
│
├── static/                        # 靜態資源
├── docs/                          # 文件
│   ├── tech-selection.md
│   ├── project-plan.md
│   ├── system-design.md           # 本文件
│   └── program-design.md
└── res/                           # 參考資源
    └── device3_vibration_dashboard.html
```

---

## 2. 資料模型

> **原則：資料優先 — 好的資料結構讓演算法變得顯而易見（Ken Thompson）**

### 2.1 Rust 端資料結構

```rust
// === models/vibration.rs ===

/// 振動資料集元資料
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    pub id: String,              // UUID
    pub file_path: String,       // 原始檔案路徑
    pub total_points: usize,     // 總資料點數
    pub time_range: (f64, f64),  // (start, end) 時間範圍
    pub columns: Vec<String>,    // 可用欄位名稱
}

/// 時序資料分段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    pub time: Vec<f64>,          // 時間戳
    pub x: Vec<f64>,             // X 軸振動值
    pub y: Vec<f64>,             // Y 軸振動值
    pub z: Vec<f64>,             // Z 軸振動值
    pub amplitude: Vec<f64>,     // 幅度 sqrt(x² + y² + z²)
    pub is_downsampled: bool,    // 是否經過 downsampling
    pub original_count: usize,   // 原始資料點數
}
```

```rust
// === models/annotation.rs ===

/// 標注類型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnnotationType {
    /// 標記點：單一時間點
    Point {
        time: f64,
        value: f64,
        axis: String,   // "x" | "y" | "z" | "amplitude"
    },
    /// 標記區間：時間範圍
    Range {
        start_time: f64,
        end_time: f64,
    },
}

/// 標注
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,                // UUID
    pub annotation_type: AnnotationType,
    pub label: String,             // 使用者定義標籤
    pub color: String,             // 顯示顏色 (hex)
    pub label_offset_x: f64,      // Label 位置偏移（拖拉後）
    pub label_offset_y: f64,
    pub created_at: String,        // ISO 8601
}

/// 標注檔案格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationFile {
    pub version: u32,              // 檔案格式版本 = 1
    pub dataset_id: String,
    pub annotations: Vec<Annotation>,
}
```

```rust
// === models/statistics.rs ===

/// 統計報表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsReport {
    pub basic: Vec<AxisBasicStats>,
    pub distribution: Vec<AxisDistributionStats>,
    pub shape: Vec<AxisShapeStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisBasicStats {
    pub axis: String,
    pub count: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub cv_percent: f64,         // 變異係數 = std_dev / mean * 100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisDistributionStats {
    pub axis: String,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub iqr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisShapeStats {
    pub axis: String,
    pub skewness: f64,
    pub kurtosis: f64,
}
```

### 2.2 TypeScript 前端型別

```typescript
// === types/vibration.ts ===

export interface VibrationDataset {
  id: string
  file_path: string
  total_points: number
  time_range: [number, number]
  columns: string[]
}

export interface TimeseriesChunk {
  time: number[]
  x: number[]
  y: number[]
  z: number[]
  amplitude: number[]
  is_downsampled: boolean
  original_count: number
}
```

```typescript
// === types/annotation.ts ===

export type AnnotationType =
  | { type: "Point"; time: number; value: number; axis: string }
  | { type: "Range"; start_time: number; end_time: number }

export interface Annotation {
  id: string
  annotation_type: AnnotationType
  label: string
  color: string
  label_offset_x: number
  label_offset_y: number
  created_at: string
}
```

---

## 3. 系統介面

> **原則：乾淨的介面應小到可記住，使用通用動詞（Ken Thompson）**

### 3.1 Tauri IPC 命令（Rust ↔ Frontend）

全部介面只有 **6 個命令**，每個命令職責單一：

```
┌──────────────────────────────────────────────────────────┐
│                    IPC Commands                          │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  資料讀取:                                                │
│    load_vibration_data(file_path) → VibrationDataset     │
│    get_timeseries_chunk(id, start, end, max) → Chunk     │
│                                                          │
│  統計:                                                    │
│    compute_statistics(id) → StatisticsReport             │
│                                                          │
│  標注:                                                    │
│    save_annotations(id, annotations) → ()                │
│    load_annotations(id) → Vec<Annotation>                │
│                                                          │
│  匯出:                                                    │
│    export_data(id, range?, format) → String(file_path)   │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### 3.2 前端 Store 架構

```
┌──────────────────────────────────────────────┐
│              Svelte Stores                   │
├──────────────────────────────────────────────┤
│                                              │
│  dataStore                                   │
│  ├── dataset: VibrationDataset | null        │
│  ├── chunk: TimeseriesChunk | null           │
│  ├── statistics: StatisticsReport | null     │
│  ├── loading: boolean                        │
│  └── error: string | null                    │
│                                              │
│  annotationStore                             │
│  ├── annotations: Annotation[]               │
│  ├── selectedId: string | null               │
│  ├── dirty: boolean (有未存檔的變更)           │
│  └── actions: add / update / remove / save   │
│                                              │
│  viewStore                                   │
│  ├── zoomRange: [number, number]             │
│  ├── activeAxes: string[] (顯示中的軸)        │
│  └── precision: number (max_points)          │
│                                              │
│  modeStore                                   │
│  ├── mode: "browse" | "annotate_point"       │
│  │         | "annotate_range"                │
│  └── (控制 ECharts 的互動行為)                 │
│                                              │
└──────────────────────────────────────────────┘
```

---

## 4. 資料流

### 4.1 載入資料流程

```
使用者選擇 CSV 檔案
    │
    ▼
Toolbar: open file dialog (Tauri dialog API)
    │
    ▼
invoke("load_vibration_data", { file_path })
    │
    ▼
Rust: csv_reader 解析 → 建立 VibrationDataset → 存入 AppState
    │
    ▼
返回 VibrationDataset 元資料
    │
    ▼
dataStore.set(dataset) → 觸發 UI 更新
    │
    ├──► invoke("get_timeseries_chunk", { id, 0, max, 50000 })
    │        → Rust: LTTB downsampling → 返回 TimeseriesChunk
    │        → dataStore.chunk → ECharts 渲染
    │
    └──► invoke("compute_statistics", { id })
             → Rust: polars 計算 → 返回 StatisticsReport
             → dataStore.statistics → 統計表格渲染
```

### 4.2 標注互動流程

```
使用者切換到標注模式（modeStore.mode = "annotate_range"）
    │
    ▼
ECharts brush 組件啟用
    │
    ▼
使用者在圖上畫框選區間
    │
    ▼
ECharts brushSelected 事件 → 取得 [start_time, end_time]
    │
    ▼
彈出 AnnotationDialog → 使用者輸入標籤
    │
    ▼
annotationStore.add(new Annotation)
    │
    ├──► ECharts markArea 更新（即時顯示）
    │
    └──► annotationStore.dirty = true（提示有未存檔變更）
```

### 4.3 縮放連動流程

```
使用者拖拉 dataZoom
    │
    ▼
ECharts datazoom 事件 → 取得新的 [start, end] 範圍
    │
    ▼
viewStore.zoomRange 更新
    │
    ▼
debounce 300ms → invoke("get_timeseries_chunk", { id, start, end, max_points })
    │
    ▼
Rust: LTTB downsampling on 新範圍
    │
    ▼
dataStore.chunk 更新 → ECharts 重繪
```

---

## 5. 操作模式設計

> **原則：最少驚訝（Ken Thompson）— 使用者操作的行為應與預期一致**

為避免 brush 框選與圖表互動衝突，設計**工具模式切換**：

| 模式 | ECharts 行為 | 滑鼠行為 |
|------|-------------|---------|
| `browse` | pan + zoom 啟用 | 拖拉=平移，滾輪=縮放，hover=tooltip |
| `annotate_point` | brush 停用 | 點擊=新增標記點 |
| `annotate_range` | brush 啟用 | 拖拉=框選區間 |

工具列按鈕組切換模式，當前模式高亮顯示。

---

## 6. UI 佈局

```
┌──────────────────────────────────────────────────────┐
│  Toolbar                                             │
│  [開啟檔案] [存檔] | [瀏覽] [標記點] [標記區間] | [匯出] │
├────────────────────────────────────┬─────────────────┤
│                                    │                 │
│  三軸振動總覽圖                      │  標注列表面板    │
│  (TimeseriesChart)                 │  (Annotation    │
│  ┌──────────────────────────────┐  │   Panel)        │
│  │  X ─── Y ─── Z ───          │  │                 │
│  │                              │  │  ┌───────────┐ │
│  │       markPoint / markArea   │  │  │ 標注 1    │ │
│  │                              │  │  │ 標注 2    │ │
│  │  ════════ dataZoom ════════  │  │  │ 標注 3    │ │
│  └──────────────────────────────┘  │  └───────────┘ │
│                                    │                 │
├────────────────────────────────────┤  [精度: 50K ▾]  │
│  單軸分析圖 (可摺疊)                 │                 │
│  ┌────────┐┌────────┐┌────────┐   │                 │
│  │ X-axis ││ Y-axis ││ Z-axis │   │                 │
│  └────────┘└────────┘└────────┘   │                 │
├────────────────────────────────────┼─────────────────┤
│  統計表格 (可摺疊 tabs)             │  視野資料表      │
│  [基本統計] [分佈統計] [形狀統計]     │  (ViewportTable)│
└────────────────────────────────────┴─────────────────┘
```

---

## 7. 錯誤處理策略

> **原則：務實主義（Ken Thompson）— 只在系統邊界做驗證**

| 邊界 | 驗證項目 | 處理方式 |
|------|---------|---------|
| 檔案讀取 | 檔案不存在、格式不正確 | Rust Result → 前端 toast 提示 |
| IPC 通訊 | invoke 失敗 | catch → dataStore.error → UI 顯示 |
| 使用者輸入 | 標籤為空、重複 ID | dialog 表單驗證 |
| 資料範圍 | zoom 超出資料邊界 | clamp 到有效範圍 |

內部模組之間（Rust service ↔ command、store ↔ component）**不做防禦性驗證**，
信任型別系統和內部介面契約。

---

## 8. 效能設計

| 瓶頸 | 策略 |
|------|------|
| 大檔案讀取 | Rust polars lazy scan，不一次載入全部到記憶體 |
| 前端渲染 | LTTB downsampling，前端最多 5 萬點 |
| 縮放時重取資料 | debounce 300ms，避免連續觸發 IPC |
| 標注更新 | 只更新 ECharts markPoint/markArea series，不重繪整張圖 |

---

## 9. 持久化格式

### 9.1 標注檔案 (.vibann.json)

```json
{
  "version": 1,
  "dataset_id": "abc-123",
  "annotations": [
    {
      "id": "ann-001",
      "annotation_type": {
        "type": "Point",
        "time": 1234.56,
        "value": 0.032,
        "axis": "x"
      },
      "label": "異常振動峰值",
      "color": "#ff6b6b",
      "label_offset_x": 10,
      "label_offset_y": -20,
      "created_at": "2026-03-13T10:30:00Z"
    },
    {
      "id": "ann-002",
      "annotation_type": {
        "type": "Range",
        "start_time": 2000.0,
        "end_time": 2500.0
      },
      "label": "設備啟動期",
      "color": "#4ecdc4",
      "label_offset_x": 0,
      "label_offset_y": 0,
      "created_at": "2026-03-13T10:32:00Z"
    }
  ]
}
```

### 9.2 檔案命名慣例

標注檔案與資料檔案同目錄，命名為 `{原始檔名}.vibann.json`。

例如：`device3_vibration.csv` → `device3_vibration.csv.vibann.json`
