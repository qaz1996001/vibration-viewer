# 振動時序標注工具 — 專案規劃

> **技術棧：方案 C — Tauri + Svelte + ECharts**
>
> 本文依循 Ken Thompson「簡單至上」、Linus Torvalds「資料結構優先」、
> Martin Fowler「YAGNI / 演化式設計」、Donald Knuth「先正確再優化」原則撰寫。

---

## 1. 專案目標

將現有 Bokeh Python Dashboard（`res/device3_vibration_dashboard.html`）重構為
**Tauri 桌面應用程式**，並新增「互動標注」功能。

### 1.1 核心交付物

| # | 交付物 | 說明 |
|---|--------|------|
| 1 | 三軸振動時序圖 | X/Y/Z 三軸折線圖 + 單軸分析圖 |
| 2 | 統計儀表板 | 基本統計 / 分佈統計 / 形狀統計表 |
| 3 | 互動標注系統 | 標記點、標記區間、框選賦標籤、拖拉調整 |
| 4 | 資料持久化 | 標注 JSON 存檔 / 讀檔，匯出 CSV |
| 5 | 桌面應用 | Tauri 跨平台打包（Windows / macOS / Linux）|

### 1.2 非目標（YAGNI）

以下功能**不在**本次開發範圍，避免過度設計：

- 多使用者協作 / 雲端同步
- 即時串流振動資料（僅離線分析）
- FFT / 頻域分析（未來可擴充）
- 多國語系 i18n
- 自動異常偵測 / ML 模型

---

## 2. 架構概覽

```
┌─────────────────────────────────────────────┐
│                Tauri Shell                   │
├────────────────────┬────────────────────────┤
│   Rust Backend     │   Svelte Frontend      │
│                    │                        │
│  ┌──────────────┐  │  ┌──────────────────┐  │
│  │ CSV/Data I/O │  │  │ ECharts 時序圖   │  │
│  │ (polars)     │  │  │ (markPoint/Area) │  │
│  ├──────────────┤  │  ├──────────────────┤  │
│  │ LTTB Down-   │◄─┼─►│ 標注管理器      │  │
│  │ sampling     │  │  │ (Svelte store)   │  │
│  ├──────────────┤  │  ├──────────────────┤  │
│  │ Statistics   │  │  │ 統計表格         │  │
│  │ Engine       │  │  │                  │  │
│  ├──────────────┤  │  ├──────────────────┤  │
│  │ Annotation   │  │  │ UI Controls      │  │
│  │ Persistence  │  │  │ (Skeleton UI)    │  │
│  └──────────────┘  │  └──────────────────┘  │
│                    │                        │
│  IPC: tauri::command (JSON over Rust↔JS)    │
└────────────────────┴────────────────────────┘
```

---

## 3. 開發階段

### 階段 0：專案初始化

| 任務 | 說明 |
|------|------|
| 0.1 | `create-tauri-app` 建立 Svelte + TypeScript 專案 |
| 0.2 | 安裝前端依賴：`echarts`、UI 框架 |
| 0.3 | 安裝 Rust 依賴：`polars`、`csv`、`serde`、`serde_json` |
| 0.4 | 建立目錄結構（見系統設計文件） |
| 0.5 | 設定 Tauri 基本窗口配置 |

### 階段 1：資料管線（Rust → Frontend）

> **原則：由下而上，資料優先（Ken Thompson）**
>
> 先建立穩固的資料結構和介面，再建 UI。

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 1.1 | Rust 端 CSV 讀取（`load_vibration_data`）| 能讀取 device3 格式 CSV |
| 1.2 | 資料模型定義（`VibrationDataset`, `TimeseriesChunk`）| serde 序列化/反序列化正確 |
| 1.3 | LTTB downsampling 實作 | 10 萬點 → 5 萬點，波形特徵保留 |
| 1.4 | `get_timeseries_chunk` IPC 命令 | 前端可分段取得資料 |
| 1.5 | 基本統計計算（`compute_statistics`）| 平均、標準差、CV%、四分位數 |

### 階段 2：時序圖渲染

> **原則：做一件事，做到完美（Ken Thompson）**
>
> 先只做折線圖渲染，確保效能和正確性。

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 2.1 | ECharts 折線圖元件（三軸總覽）| 5 萬點渲染 < 1 秒 |
| 2.2 | dataZoom 互動（縮放/平移）| 拖拉平滑，無卡頓 |
| 2.3 | 單軸分析圖（X/Y/Z 獨立）| 點擊切換或同時顯示 |
| 2.4 | 圖表 ↔ Rust 連動（縮放時重取資料）| dataZoom 事件觸發新 chunk |
| 2.5 | tooltip / hover 顯示值 | 顯示時間、X、Y、Z、幅度 |

### 階段 3：統計儀表板

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 3.1 | 基本統計表元件 | 軸向、數量、平均值、標準差、CV% |
| 3.2 | 分佈統計表元件 | Min、Q1、Med、Q3、Max、IQR |
| 3.3 | 形狀統計表元件 | 偏度、峰度 |
| 3.4 | 視野資料表（當前 zoom 範圍內的資料）| 表格可排序 |

### 階段 4：標注系統（核心功能）

> **原則：先讓程式運作，再確保正確性，最後才優化（Ken Thompson）**
>
> 先實作最簡單的標記點，再逐步增加區間和拖拉。

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 4.1 | 標注資料模型（Annotation struct）| 支援 Point / Range 兩種類型 |
| 4.2 | 標記點功能 | 點擊圖表 → markPoint 出現 → 顯示時間與值 |
| 4.3 | 標記區間功能 | ECharts brush → markArea 半透明方塊 |
| 4.4 | 框選賦予標籤 | brush 框選後彈出 dialog 輸入標籤名稱 |
| 4.5 | 標注列表面板 | 側邊欄顯示所有標注，可點擊跳轉 |
| 4.6 | 標注刪除 | 選取標注 → 刪除按鈕或 Delete 鍵 |
| 4.7 | Label 拖拉調整位置 | ECharts graphic 組件 draggable |

### 階段 5：持久化與匯出

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 5.1 | 標注存檔（`save_annotations` → JSON）| 標注資料寫入檔案 |
| 5.2 | 標注讀檔（`load_annotations`）| 重開程式後標注恢復 |
| 5.3 | 視野資料匯出 CSV | 匯出當前 zoom 範圍內的資料 |
| 5.4 | 全部資料匯出 CSV | 匯出完整資料集 |
| 5.5 | 檔案對話框整合 | Tauri dialog API 選取存檔路徑 |

### 階段 6：收尾與打包

| 任務 | 說明 | 驗收標準 |
|------|------|----------|
| 6.1 | 資料精度切換（50K / 100K / 全量）| Select 元件控制 max_points |
| 6.2 | 整體 UI 佈局調整 | 響應式佈局，視窗縮放不破版 |
| 6.3 | 錯誤處理與使用者回饋 | 檔案不存在、格式錯誤等提示 |
| 6.4 | Tauri 打包（Windows / macOS / Linux）| 產出安裝包 |

---

## 4. 技術決策紀錄

> **原則：架構是那些既重要又難以改變的決策（Martin Fowler）**

| 決策 | 選擇 | 理由 |
|------|------|------|
| 前端框架 | Svelte + TypeScript | 最小 bundle、天然 reactivity、Tauri 官方支援 |
| 圖表庫 | ECharts 5.x | markPoint/markArea/brush 原生支援標注、dataZoom 內建 |
| 後端語言 | Rust (Tauri) | 效能、記憶體安全、跨平台 |
| 資料處理 | polars | 高效能 DataFrame、原生 Rust |
| Downsampling | LTTB 自行實作 | 演算法簡單，避免不必要的外部依賴 |
| 標注儲存 | JSON 檔案 (serde_json) | 簡單、可讀、足夠用（YAGNI: 不需要 SQLite）|
| 狀態管理 | Svelte store | 內建、不需額外依賴 |
| UI 框架 | Skeleton UI 或手寫 | Svelte 原生 UI 框架，輕量 |

---

## 5. 風險與緩解

| 風險 | 影響 | 緩解策略 |
|------|------|----------|
| ECharts 大資料量效能不足 | 圖表卡頓 | Rust LTTB downsampling 確保前端 ≤ 5 萬點 |
| ECharts Label 拖拉限制 | 功能受限 | 使用 `graphic` 組件 draggable，必要時加 DOM overlay |
| Svelte 生態 UI 元件不足 | 開發時間增加 | 優先用 Skeleton UI，不足時手寫（保持簡單）|
| brush 框選與標注衝突 | 操作體驗差 | 設計工具模式切換（瀏覽模式 / 標注模式）|

---

## 6. 品質標準

> **原則：程式碼首先是寫給人類讀的（Martin Fowler）**

### 6.1 程式碼規範

- **Rust**：`cargo fmt` + `cargo clippy`，所有 warning 視為 error
- **Svelte/TS**：`eslint` + `prettier`，嚴格 TypeScript（`strict: true`）
- **命名**：使用能表達意圖的名稱，避免縮寫

### 6.2 測試策略

- **Rust 單元測試**：每個 Tauri command 至少一個正常 + 一個異常測試
- **資料管線整合測試**：使用 device3 樣本資料驗證完整流程
- **前端元件測試**：關鍵互動行為（標注 CRUD）使用 Svelte Testing Library

### 6.3 重構原則

- 遵循 Martin Fowler 重構決策樹：小步驟、測試保護、不改變行為
- 發現 code smell（方法 > 20 行、參數 > 3 個、巢狀 > 3 層）立即標記
- 先讓功能正確運作（Ken Thompson），再重構改善設計（Martin Fowler）
