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

## 方案總比較

| 比較項目 | A: React+ECharts | B: React+uPlot | C: Svelte+ECharts | D: Vue3+ECharts |
|---------|-------------------|-----------------|--------------------|-----------------|
| 繪圖效能 | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| 標注功能開發成本 | ⭐⭐ (低) | ⭐⭐⭐⭐⭐ (高) | ⭐⭐ (低) | ⭐⭐ (低) |
| 打包體積 | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| 中文社群/文件 | ⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| ECharts 整合度 | ⭐⭐⭐⭐ | N/A | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| 生態/元件庫 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| 學習曲線 | 低 | 高 | 中（若不熟） | 低 |

---

## 建議

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

請選擇你偏好的方案（A / B / C / D），我將據此初始化 Tauri 專案並開始實作。
