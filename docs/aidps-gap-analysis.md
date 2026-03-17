# AIDPS 資料整合差異分析

> 分析日期：2026-03-17
> 目標：讓 vibration-viewer 能開啟 AIDPS 專案資料夾，完成標註 + FFT 頻譜分析

---

## 1. AIDPS 資料結構

```
aidps/
├── history/                    # 時域 CSV（5 裝置，117 檔案，共 5.4 MB）
│   ├── device1_1/              # 23 CSV
│   ├── device2/                # 23 CSV
│   ├── device3/                # 24 CSV
│   ├── device4/                # 24 CSV
│   └── device5/                # 23 CSV
├── wav/                        # 原始波形 WAV（5 感測器，1266 檔案，共 1.6 GB）
│   ├── sensor1/                # 1 WAV
│   ├── sensor2/                # 1 WAV
│   ├── sensor3/                # 1262 WAV（每 10 秒一檔）
│   ├── sensor4/                # 1 WAV
│   └── sensor5/                # 1 WAV
├── db/                         # SQLite 時序資料庫（5 個，共 21 MB）
└── appserver/                  # 應用程式狀態 SQLite（92 KB）
```

### 1.1 History CSV 格式（13 欄位，每秒一筆）

| 欄位 | 類型 | 說明 |
|------|------|------|
| time | datetime | `YYYY-MM-DD HH:MM:SS` |
| x, y, z | float | 瞬時加速度值 |
| x_max, x_min | float | 區間極值 |
| y_max, y_min | float | 區間極值 |
| z_max, z_min | float | 區間極值 |
| x_vrms, y_vrms, z_vrms | float | RMS 速度 |

- 檔名規則：`{device_id}_{seq}_{YYYYMMDD}_{HHMMSS}.csv`
- 每檔約 1000-1300 筆（11-20 分鐘）
- 有空值（missing data）

### 1.2 WAV 檔案

- 檔名規則：`{sensor_id}-{YYYY-MM-DD}_{HH}-{MM}-{SS}.wav`
- 每檔約 1.3 MB
- sensor3 有密集的 10 秒間隔錄製，其餘 sensor 僅 1 檔
- 時間範圍：2026-03-09 11:19 ~ 15:05（約 3.75 小時）

### 1.3 裝置 ↔ 感測器對應

| 裝置（history） | 感測器（wav） |
|-----------------|---------------|
| device1_1 | sensor1 |
| device2 | sensor2 |
| device3 | sensor3 |
| device4 | sensor4 |
| device5 | sensor5 |

---

## 2. 現有 Vibration-Viewer 能力

| 功能 | 狀態 | 說明 |
|------|------|------|
| CSV 開檔 | ✅ | 單檔開啟，手動選欄位對應 |
| 多檔疊加 | ✅ | 多 CSV 同一圖表，時間軸對齊 |
| 時域圖表 | ✅ | ECharts zoom/pan/dataZoom |
| 單通道分析 | ✅ | SingleAxisChart 單軸放大 |
| 點標註 | ✅ | annotate_point 模式 |
| 範圍標註 | ✅ | annotate_range 模式 |
| 標註存取 | ✅ | JSON sidecar 格式 |
| 基本統計 | ✅ | mean/std/quartiles/skewness/kurtosis |
| LTTB 降採樣 | ✅ | 50K 點上限，index-based |
| 資料匯出 | ✅ | 全量或視窗範圍 CSV |
| **資料夾批次開啟** | ❌ | 僅支援單檔 |
| **裝置分群顯示** | ❌ | 無裝置概念 |
| **CSV 自動串接** | ❌ | 無法將同裝置多檔合併為連續時序 |
| **WAV 檔案讀取** | ❌ | 僅支援 CSV |
| **FFT 頻譜分析** | ❌ | 無頻域分析功能 |
| **標註→頻譜連動** | ❌ | 無法從標註點觸發 WAV 查找 |
| **AIDPS 專案格式辨識** | ❌ | 無結構化專案概念 |

---

## 3. 差異分析（需新增功能）

### 3.1 🔴 P0：AIDPS 專案開啟 + 裝置分群

**現狀：** 使用者逐一開 CSV，手動選欄位。
**需求：** 選擇 AIDPS 資料夾 → 自動掃描 `history/` → 按裝置分群 → 自動識別 13 欄位 schema。

**需要的變更：**

| 層 | 變更內容 |
|----|---------|
| **Rust commands** | 新增 `open_aidps_project` 指令：掃描目錄、辨識結構、回傳裝置清單 |
| **Rust models** | 新增 `AidpsProject` 結構（devices, sensor_mapping, wav_path 等） |
| **Rust csv_reader** | 支援多 CSV 串接（同裝置的檔案依時間排序合併） |
| **Frontend stores** | 新增 `projectStore.ts`（專案狀態、裝置清單、啟用裝置） |
| **Frontend UI** | 新增「開啟專案」流程，取代手動開檔 + 欄位對應 |
| **FileList 元件** | 改為按裝置分群顯示，點擊裝置載入對應時域資料 |

**工作量估計：** 中（主要是目錄掃描 + 多檔合併邏輯）

### 3.2 🔴 P0：時域圖按裝置顯示

**現狀：** 多檔疊加在同一圖表，無裝置概念。
**需求：** 選擇裝置 → 載入該裝置下所有 CSV → 合併為連續時序 → 顯示 13 通道。

**需要的變更：**

| 層 | 變更內容 |
|----|---------|
| **Rust csv_reader** | `read_csvs_concat(paths: Vec<String>, mapping)` — 多檔讀取 + Polars concat + 時間排序 |
| **Rust commands** | `load_device_data(project_id, device_id)` — 載入裝置全部 CSV |
| **Frontend** | 切換裝置時觸發載入；圖表顯示該裝置完整時域 |
| **TimeseriesChart** | 支援 13 通道分群顯示（加速度 / 極值 / VRMS 分組） |

**工作量估計：** 中

### 3.3 🔴 P0：WAV 讀取 + FFT 頻譜

**現狀：** 完全不支援 WAV 和頻域分析。
**需求：** 對標註的時間點 → 找到最近的 WAV 檔 → 讀取波形 → FFT → 顯示頻譜圖。

**需要的變更：**

| 層 | 變更內容 |
|----|---------|
| **Rust 依賴** | 新增 `hound`（WAV 讀取）+ `rustfft`（FFT 計算） |
| **Rust services** | 新增 `wav_reader.rs`：讀 WAV → f64 samples |
| **Rust services** | 新增 `fft_engine.rs`：FFT → 頻率 + 振幅陣列 |
| **Rust commands** | 新增 `get_spectrum(project_id, device_id, timestamp)` 指令 |
| **WAV 查找邏輯** | 根據 device→sensor 對應 + timestamp 找到最近的 WAV 檔案 |
| **Frontend types** | 新增 `SpectrumData { frequencies: number[], amplitudes: number[] }` |
| **Frontend 元件** | 新增 `SpectrumChart.svelte`（ECharts 頻譜圖，x=Hz, y=amplitude） |
| **Annotation 連動** | 點擊標註 → 自動呼叫 `get_spectrum` → 顯示頻譜 |

**工作量估計：** 大（WAV 解析 + FFT + 新圖表 + 互動流程）

### 3.4 🟡 P1：標註→頻譜 互動流程

**現狀：** 標註只記錄時間/值/標籤。
**需求：** 點擊已有標註（或新建標註） → 面板顯示對應頻譜 → 可比較多個標註點的頻譜。

**互動設計：**

```
[時域圖] ─── 點擊標註點 ───→ [AnnotationPanel 選中]
                                    │
                                    ▼
                          [呼叫 get_spectrum]
                                    │
                                    ▼
                          [SpectrumChart 顯示頻譜]
```

**需要的變更：**

| 層 | 變更內容 |
|----|---------|
| **AnnotationPanel** | 選中標註時觸發頻譜載入 |
| **Frontend store** | 新增 `spectrumStore.ts`（快取已查詢的頻譜） |
| **UI 布局** | 頁面下方或右側新增頻譜區域 |
| **比較功能** | 支援勾選多個標註疊加頻譜（未來） |

**工作量估計：** 中

### 3.5 🟢 P2：進階功能（未來）

| 功能 | 說明 |
|------|------|
| SQLite DB 讀取 | 直接讀取 `db/` 下的時序資料庫 |
| WAV 波形預覽 | 顯示原始波形圖（不只 FFT） |
| 頻譜參數調整 | 窗函數選擇（Hanning/Hamming）、FFT size、overlap |
| 裝置比較模式 | 多裝置頻譜並排比較 |
| 批次標註匯出 | 所有標註 + 頻譜資料匯出報告 |
| 自動異常偵測 | 基於頻譜特徵標記異常時段 |

---

## 4. 技術方案摘要

### 4.1 新增 Rust 依賴

```toml
# Cargo.toml 新增
hound = "3.5"          # WAV 檔案讀取
rustfft = "6.2"        # FFT 計算
```

### 4.2 新增 Tauri 指令

| 指令 | 參數 | 回傳 |
|------|------|------|
| `open_aidps_project` | `folder_path: String` | `AidpsProject { devices, sensors, wav_path, ... }` |
| `load_device_data` | `project_id, device_id` | `DatasetEntry`（合併後的完整時序） |
| `get_spectrum` | `project_id, device_id, timestamp, fft_size?` | `SpectrumData { frequencies, amplitudes }` |

### 4.3 新增前端元件

| 元件 | 用途 |
|------|------|
| `ProjectOpenDialog.svelte` | 選擇 AIDPS 資料夾 + 預覽結構 |
| `DeviceSelector.svelte` | 裝置切換面板（取代手動多檔開啟） |
| `SpectrumChart.svelte` | 頻譜圖（ECharts bar/line） |

### 4.4 新增 Store

| Store | 用途 |
|-------|------|
| `projectStore.ts` | AIDPS 專案狀態、裝置清單、sensor mapping |
| `spectrumStore.ts` | 頻譜資料快取、當前顯示的頻譜 |

---

## 5. 實作優先順序

```
Phase 1 ─ 專案開啟 + 裝置分群
  ├─ open_aidps_project 指令
  ├─ 多 CSV 串接（同裝置合併）
  ├─ 裝置清單 UI
  └─ 自動 schema 辨識（跳過 ColumnMappingDialog）

Phase 2 ─ WAV + FFT 頻譜
  ├─ hound WAV 讀取
  ├─ rustfft FFT 計算
  ├─ get_spectrum 指令
  ├─ SpectrumChart 元件
  └─ WAV 檔案時間匹配邏輯

Phase 3 ─ 標註↔頻譜 連動
  ├─ 點擊標註觸發頻譜
  ├─ 頻譜面板 UI 布局
  └─ 頻譜資料快取

Phase 4 ─ 進階功能
  ├─ 窗函數選擇
  ├─ 多標註頻譜比較
  └─ 波形預覽
```

---

## 6. 風險與注意事項

| 風險 | 說明 | 緩解 |
|------|------|------|
| WAV 檔案大小 | sensor3 有 1262 個 WAV（共 1.6GB），需避免全部載入記憶體 | 按需讀取單檔，不預載 |
| FFT 計算時間 | 大型 WAV 的 FFT 可能耗時 | Rust 端非同步計算，前端顯示 loading |
| Device-Sensor 對應 | 目前靠編號推斷，不一定正確 | 提供 UI 讓使用者確認/修改對應 |
| 空值處理 | CSV 有 missing data | 沿用現有 fill(0.0) 策略，或改為 NaN + 圖表跳過 |
| 多檔排序 | CSV 檔案間時間可能重疊或有間隔 | 合併時按 time 排序 + 去重 |
| WAV 取樣率 | 未知，需從 WAV header 讀取 | hound 自動讀取 spec.sample_rate |
