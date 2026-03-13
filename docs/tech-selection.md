# Tauri 振動時序標注工具 - 技術選型方案

## 目前 Dashboard 功能分析

根據 `res/device3_vibration_dashboard.html`（Bokeh 3.9.0 生成），現有功能包括：

| 功能模組 | 說明 |
|---------|------|
| 三軸振動總覽圖 | X/Y/Z 三軸時序折線圖，約 12,581 點 |
| 單軸分析圖 | X-axis / Y-axis / Z-axis 獨立圖表 |
| 資料精度切換 | Select 選擇器，目前「中等 (50K)」|
| 視野資料表 | 時間、X、Y、Z、幅度 欄位的 DataTable |
| 基本統計表 | 軸向、數量、平均值、標準差、CV% |
| 分佈統計表 | Min、Q1、Med、Q3、Max、IQR |
| 形狀統計表 | 偏度、峰度 |
| 匯出功能 | 匯出視野資料 / 匯出全部資料 |
| 互動工具 | Pan、BoxZoom、WheelZoom、Reset、Save、Hover |

## 需要新增的功能

根據需求描述，除了重現以上功能外，還需加入：

1. **標記點或區間** — 在時序圖上標記特定時間點/區間，顯示時間與值
2. **框選區間標籤** — 畫方格框選區間，賦予標籤分類
3. **Label 顯示與拖拉** — 標注 Label 顯示在折線圖上，可調整位置
4. **標注資料持久化** — 儲存 / 載入標注結果

---

## 方案 A：Tauri + React + ECharts

### 架構

```
Tauri (Rust backend)
├── 資料讀取：csv / polars crate
├── Downsampling：LTTB 演算法（Rust 實作）
├── 標注儲存：serde_json（JSON 檔案）或 rusqlite（SQLite）
└── IPC：tauri::command

Frontend (React + TypeScript)
├── 繪圖：ECharts 5.x
├── 標注 UI：ECharts markPoint / markArea + brush 組件
├── 狀態管理：zustand 或 React Context
└── UI 框架：Ant Design 或 Shadcn/ui
```

### 優點

- ECharts **原生支援 markPoint、markArea、brush 框選**，與標注需求高度吻合
- dataZoom 內建，拖拉縮放零成本
- tooltip formatter 可自定義標注資訊顯示
- 社群大、中文文件齊全，遇到問題容易找到解答
- React 生態成熟，UI 組件庫豐富

### 缺點

- 資料量超過 10 萬點效能開始下降，需依賴 Rust 端 downsampling
- Label 拖拉調整位置需要自訂 ECharts plugin 或 DOM overlay
- 打包體積比 Svelte 方案略大

### 標注功能實作方式

| 需求 | ECharts 方案 |
|------|-------------|
| 標記點 | `markPoint` + 自定義 `symbolSize` / `label` |
| 標記區間 | `markArea` + 半透明填色 + label |
| 框選標籤 | `brush` 組件 + `brushSelected` 事件回調 |
| Label 拖拉 | `graphic` 組件（可拖拉）或 DOM overlay |

### 預估開發難度：⭐⭐⭐ (中等)

---

## 方案 B：Tauri + React + uPlot

### 架構

```
Tauri (Rust backend)
├── 資料讀取：csv / polars crate
├── Downsampling：LTTB（可選，uPlot 本身效能很強）
├── 標注儲存：serde_json / rusqlite
└── IPC：tauri::command

Frontend (React + TypeScript)
├── 繪圖：uPlot
├── 標注 UI：自定義 uPlot plugin + Canvas overlay
├── 狀態管理：zustand
└── UI 框架：Ant Design 或 Shadcn/ui
```

### 優點

- **效能最強**，百萬點渲染流暢，Canvas 直繪
- 極輕量（~45KB min+gzip）
- 適合大資料量振動數據，前端可直接處理較多數據
- 減少 Rust 端 downsampling 壓力

### 缺點

- **標注功能全部需要自己寫**（uPlot 無內建 annotation）
- API 較底層，學習曲線較陡
- 框選、拖拉 Label 需要自己處理 Canvas 座標換算
- 社群較小，遇到問題需要自己 debug

### 標注功能實作方式

| 需求 | uPlot 方案 |
|------|-----------|
| 標記點 | 自定義 plugin，在 draw hook 繪製標記 |
| 標記區間 | 自定義 plugin，drawRect + fillStyle 半透明 |
| 框選標籤 | 監聽 mousedown/mousemove/mouseup，手動計算 |
| Label 拖拉 | DOM overlay div + drag event |

### 預估開發難度：⭐⭐⭐⭐⭐ (高)

---

## 方案 C：Tauri + Svelte + ECharts

### 架構

```
Tauri (Rust backend)
├── 資料讀取：csv / polars crate
├── Downsampling：LTTB
├── 標注儲存：serde_json / rusqlite
└── IPC：tauri::command

Frontend (Svelte + TypeScript)
├── 繪圖：ECharts 5.x
├── 標注 UI：ECharts markPoint / markArea + brush
├── 狀態管理：Svelte store（內建）
└── UI 框架：Skeleton UI 或手寫
```

### 優點

- Svelte 編譯出的 JS 最小，Tauri 打包體積最佳
- Svelte reactivity 天然適合圖表資料綁定
- Tauri 官方 create-tauri-app 預設支援 Svelte 模板
- ECharts 標注優勢同方案 A

### 缺點

- Svelte 生態比 React 小，UI 元件庫較少
- 如果團隊不熟 Svelte，額外學習成本
- ECharts 的 Svelte wrapper 不如 React 成熟

### 預估開發難度：⭐⭐⭐ (中等，但前提是熟悉 Svelte)

---

## 方案 D：Tauri + Vue 3 + ECharts

### 架構

```
Tauri (Rust backend)
├── 資料讀取：csv / polars crate
├── Downsampling：LTTB
├── 標注儲存：serde_json / rusqlite
└── IPC：tauri::command

Frontend (Vue 3 + TypeScript)
├── 繪圖：ECharts 5.x（vue-echarts 官方封裝）
├── 標注 UI：ECharts markPoint / markArea + brush
├── 狀態管理：Pinia
└── UI 框架：Element Plus 或 Naive UI
```

### 優點

- `vue-echarts` 是 ECharts 官方維護的 Vue 封裝，整合度最高
- Vue 3 Composition API + TypeScript 開發體驗好
- Element Plus / Naive UI 中文 UI 元件庫完善
- 中文社群最大，遇到問題最容易找到解答

### 缺點

- 打包體積比 Svelte 大
- Vue 生態在海外相對小，英文資源較少

### 預估開發難度：⭐⭐⭐ (中等)

---

## 方案 E：Tauri + React/Vue + BokehJS（純 JavaScript，不需要 Python）

### 什麼是 BokehJS？

[BokehJS](https://github.com/bokeh/bokeh/tree/branch-3.10/bokehjs) 是 Bokeh 的**純 JavaScript/TypeScript 前端核心**，npm 套件名為 `@bokeh/bokehjs`。它可以完全脫離 Python 獨立運行，提供與 Python Bokeh 相同的繪圖模型和互動能力。

- 原生 TypeScript 撰寫，型別安全
- npm 安裝：`npm install @bokeh/bokehjs`
- 官方提供 React / Vue / Vite / Webpack 整合範例（[bokehjs-examples](https://github.com/bokeh/bokehjs-examples)）
- **不需要嵌入 Python runtime**，純前端套件

### 架構

```
Tauri (Rust backend)
├── 資料讀取：csv / polars crate
├── Downsampling：LTTB
├── 標注儲存：serde_json / rusqlite
└── IPC：tauri::command

Frontend (React 或 Vue + TypeScript)
├── 繪圖：@bokeh/bokehjs（BokehJS 獨立套件）
├── 標注 UI：BoxAnnotation + Label + BoxEditTool + PointDrawTool
├── 狀態管理：zustand / Pinia
└── UI 框架：Ant Design / Element Plus
```

### 優點

- **與現有 Dashboard 模型一致** — 你目前的 Bokeh HTML 使用的所有模型（Figure、ColumnDataSource、DataTable、HoverTool 等）在 BokehJS 中是完全相同的 class，遷移概念成本最低
- **原生 TypeScript** — BokehJS 本身就是 TypeScript 寫的，型別定義完整，IDE 提示良好
- **內建編輯工具豐富** — BokehJS 提供一整套 Edit Tools：
  - `BoxEditTool` — 可互動繪製、拖拉、刪除矩形區域（Rect/Block/Quad/HBar/VBar/HStrip/VStrip）
  - `PointDrawTool` — 可互動新增、拖拉、刪除標記點
  - `PolyDrawTool` / `PolyEditTool` — 可互動繪製/編輯多邊形
  - `FreehandDrawTool` — 手繪工具
  - `LineEditTool` — 編輯線段
- **BoxEditTool 支援拖拉** — 原生支援 pan（拖拉移動）、draw（繪製新方塊）、tap（選取）、Backspace（刪除），這是其他方案中最難實作的功能
- **Annotation 種類完整** — Label、LabelSet、BoxAnnotation、Span、Arrow、Band、Whisker、PolyAnnotation 等全部可用
- **事件系統完整** — SelectionGeometry 事件支援 5 種選取模式（Replace / Append / Intersect / Subtract / XOR）
- **DataTable 內建** — 你現有的統計表可直接沿用
- **不需要 Python** — 純 JS 套件，Tauri 打包體積不受影響

### 缺點

- **BokehJS 獨立使用的文件較少** — 官方文件主要面向 Python 使用者，純 JS API 文件需要參考 TypeScript 原始碼和 [bokehjs-examples](https://github.com/bokeh/bokehjs-examples)
- **社群規模小** — npm 下載量遠低於 ECharts/uPlot，遇到問題 StackOverflow 答案少，可能需要直接看原始碼或開 GitHub Issue
- **Bundle 體積較大** — BokehJS 完整套件包含 WebGL (Regl)、MathJax、地圖等模組，體積預估 ~800KB-1.5MB (gzip)，遠大於 ECharts (~300KB gzip) 和 uPlot (~45KB gzip)
- **API 風格偏 Python** — BokehJS 的 API 是映射自 Python Bokeh（如 `figure()` → `Bokeh.Plotting.figure()`），不是典型的 JS/React 風格，需要適應
- **缺乏 dataZoom 等便利元件** — Bokeh 的縮放是靠 WheelZoomTool + BoxZoomTool + RangeTool，沒有 ECharts 那種拖拉式 dataZoom 滑軌
- **BokehJS 4.0 潛在破壞性變更** — 官方提到部分 standalone 功能要到 BokehJS 4.0 才完整，目前 3.x 版可能有些 edge case
- **框架整合不如 ECharts 成熟** — 沒有官方的 `react-bokeh` 或 `vue-bokeh` wrapper，需要手動管理生命週期

### 標注功能實作方式

| 需求 | BokehJS 方案 |
|------|-------------|
| 標記點（顯示時間與值） | `PointDrawTool` + `ColumnDataSource` + `Label` / `HoverTool` — **原生互動，可拖拉** |
| 標記區間（半透明方塊） | `BoxEditTool` + `Rect` glyph + `ColumnDataSource` — **原生繪製/拖拉/刪除** |
| 框選賦予標籤 | `BoxSelectTool` + `SelectionGeometry` 事件 → 回調中寫入標籤到 DataSource |
| Label 拖拉調整位置 | `PointDrawTool` 搭配 `LabelSet`（Label 位置綁定到可拖拉的 DataSource） |
| 標注持久化 | `ColumnDataSource.data` 直接 JSON 序列化 → Tauri IPC 存檔 |

### 關鍵優勢：Edit Tools 原生解決最難的問題

其他方案（ECharts / uPlot）中最困難的「**拖拉標注位置**」和「**互動繪製區間**」，在 BokehJS 中是**內建功能**：

```typescript
import * as Bokeh from "@bokeh/bokehjs"

// 建立可編輯的矩形區間（標注用）
const annotation_source = new Bokeh.ColumnDataSource({
  data: { x: [], y: [], width: [], height: [], label: [] }
})

const rects = fig.rect({
  source: annotation_source,
  x: { field: "x" }, y: { field: "y" },
  width: { field: "width" }, height: { field: "height" },
  fill_alpha: 0.3, fill_color: "#ff6b6b",
})

// BoxEditTool：使用者可以直接在圖上畫方塊、拖拉移動、按 Backspace 刪除
const box_edit = new Bokeh.BoxEditTool({ renderers: [rects] })
fig.add_tools(box_edit)

// 當 source 資料變更時，自動同步到 Rust 後端存檔
annotation_source.change.connect(() => {
  // tauri invoke save_annotations...
})
```

### 預估開發難度：⭐⭐ (低，若熟悉 Bokeh)  /  ⭐⭐⭐⭐ (高，若不熟悉 Bokeh)

---

## 方案總比較

| 比較項目 | A: React+ECharts | B: React+uPlot | C: Svelte+ECharts | D: Vue3+ECharts | **E: React/Vue+BokehJS** |
|---------|-------------------|-----------------|--------------------|-----------------|-----------------------|
| 繪圖效能 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ (Canvas+WebGL) |
| 標注功能開發成本 | ⭐⭐ (低) | ⭐⭐⭐⭐⭐ (高) | ⭐⭐ (低) | ⭐⭐ (低) | **⭐ (最低)** |
| 拖拉標注/繪製區間 | ⭐⭐⭐ (需 hack) | ⭐⭐⭐⭐⭐ (全自寫) | ⭐⭐⭐ (需 hack) | ⭐⭐⭐ (需 hack) | **⭐ (原生內建)** |
| 打包體積 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ (最大) |
| 中文社群/文件 | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ (文件偏 Python) |
| 整合度/Wrapper | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐ (需手動管理) |
| 生態/元件庫 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| 與現有 Dashboard 相容 | ⭐ (全部重寫) | ⭐ (全部重寫) | ⭐ (全部重寫) | ⭐ (全部重寫) | **⭐⭐⭐⭐⭐ (模型一致)** |
| 學習曲線 | 低 | 高 | 中（若不熟） | 低 | 低（若熟 Bokeh）/ 高（若不熟）|

---

## 建議

### 如果你已經熟悉 Bokeh 且標注互動是核心需求 → 選 **方案 E (React/Vue + BokehJS)**
- BoxEditTool / PointDrawTool **原生解決最困難的拖拉標注問題**
- 與現有 Dashboard 模型完全一致，遷移概念成本最低
- 代價是 bundle 較大、獨立使用文件較少

### 如果你熟悉 Vue → 選 **方案 D (Vue 3 + ECharts)**
- vue-echarts 官方封裝最省事，中文資源最豐富

### 如果你熟悉 React → 選 **方案 A (React + ECharts)**
- 生態最大，第三方資源最多

### 如果你追求極致效能（百萬點以上）→ 選 **方案 B (React + uPlot)**
- 但標注功能要全部自己寫，開發週期最長

### 如果你追求最小打包體積 → 選 **方案 C (Svelte + ECharts)**
- Tauri + Svelte 是最輕量的組合

---

## Rust 後端共通設計（各方案通用）

### 核心 Tauri Commands

```rust
// 1. 讀取振動資料（支援 CSV / 二進位格式）
#[tauri::command]
fn load_vibration_data(file_path: String) -> Result<VibrationDataset, String>

// 2. 分段取得時序資料（含 downsampling）
#[tauri::command]
fn get_timeseries_chunk(
    dataset_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
) -> Result<TimeseriesChunk, String>

// 3. 計算統計資訊
#[tauri::command]
fn compute_statistics(dataset_id: String) -> Result<StatisticsReport, String>

// 4. 儲存標注
#[tauri::command]
fn save_annotations(
    dataset_id: String,
    annotations: Vec<Annotation>,
) -> Result<(), String>

// 5. 載入標注
#[tauri::command]
fn load_annotations(dataset_id: String) -> Result<Vec<Annotation>, String>

// 6. 匯出資料（CSV）
#[tauri::command]
fn export_data(
    dataset_id: String,
    range: Option<TimeRange>,
    format: ExportFormat,
) -> Result<String, String>
```

### 建議 Rust Crates

| 用途 | Crate |
|------|-------|
| CSV 讀取 | `csv` 或 `polars` |
| 資料處理 | `polars` / `ndarray` |
| Downsampling | 自行實作 LTTB 或使用 `lttb` crate |
| 標注儲存 | `serde_json`（JSON 檔案）或 `rusqlite`（SQLite）|
| 序列化 | `serde` + `serde_json` |

---

## 下一步

請選擇你偏好的方案（A / B / C / D / E），我將據此初始化 Tauri 專案並開始實作。
