# Research: Mark Point 無法刷新顯示問題

## Executive Summary

Mark Point (點標註) 時常無法在 ECharts 上顯示的 **主因是 `buildMarkPoints` 中的 `axis` 過濾條件與實際 series 的 channel 名稱不一致**。Rust HashMap 序列化順序不確定，加上多檔案時 activeDataset 與 firstSeries 歸屬不同 dataset，導致過濾條件高機率不匹配，mark point 資料被濾除。

**信心水準:** 高 (95%) — 可直接由程式碼邏輯推導出。

---

## Bug 1 (PRIMARY): HashMap 序列化導致 axis 名稱不匹配

### 問題位置
- `chartOptions.ts:104-108` — `buildMarkPoints(annotations, channelName)`
- `chartOptions.ts:266-271` — 過濾: `a.annotation_type.axis === axis`

### 觸發路徑

```
createOverviewOption() {
  for (dsId of datasetOrder) {
    for (channelName of Object.keys(chunk.channels)) {  // ← HashMap 順序不確定
      if (firstSeries) {
        markPoint.data = buildMarkPoints(annotations, channelName)  // ← 用第一個 channel
        firstSeries = false
      }
    }
  }
}
```

建立 annotation 時 (`+page.svelte:168`):
```typescript
axis: $activeDataset?.column_mapping.data_columns[0] ?? 'x'
// ← 永遠用 data_columns[0]，這是 ColumnMapping 中使用者選定的順序
```

### 根本原因

Rust 的 `HashMap<String, Vec<f64>>` (TimeseriesChunk.channels) 序列化為 JSON 時 **不保證 key 順序**。前端 `Object.keys(chunk.channels)` 回傳的順序可能與 `column_mapping.data_columns` 不同。

**範例:**
1. 使用者選 data_columns = `["x", "y", "z"]`
2. Rust HashMap 序列化為 `{"z": [...], "x": [...], "y": [...]}`
3. `Object.keys()` → 第一個 key = `"z"`
4. `buildMarkPoints(annotations, "z")` — 只顯示 axis="z" 的 annotation
5. annotation 的 axis = `"x"` (data_columns[0]) → **被過濾掉！不顯示！**

### 嚴重度
**Critical** — 單檔案情境下就會發生。HashMap 順序每次載入可能不同，導致問題看似「時好時壞」。

---

## Bug 2 (SECONDARY): 多檔案時 activeDataset 與 firstSeries 不同 dataset

### 觸發路徑

1. 使用者載入 File A (channels: `accel_x`, `accel_y`) → 成為 datasetOrder[0] + activeDataset
2. 使用者載入 File B (channels: `velocity`) → 成為 activeDataset（`addFile` 中 `activeDatasetId.set(ds.id)`）
3. 使用者標註 point → `axis = "velocity"`（來自 activeDataset B 的 data_columns[0]）
4. 圖表渲染: firstSeries 是 File A 的 `accel_x` → `buildMarkPoints(annotations, "accel_x")`
5. 過濾: axis="velocity" ≠ "accel_x" → **不顯示**

### 嚴重度
**High** — 多檔案模式下必定觸發。

---

## Bug 3 (MINOR): Zoom-fetch 導致短暫消失

### 描述
zoom 事件觸發 `fetchAllChunks` → `chunks` store 更新 → chart `$effect` 重新執行。
fetch 過程中 `$chunks` 可能暫時處於中間狀態。

### 嚴重度
**Low** — 暫時性問題，fetch 完成後會恢復。

---

## 建議修復方案

### 方案 A: Overview chart 移除 axis 過濾 (推薦)

Overview chart 顯示所有 dataset 疊加，mark point 不需要按 channel 分類。直接顯示所有 Point annotations。

```typescript
// chartOptions.ts - buildMarkPoints
function buildMarkPoints(annotations: Annotation[]): any[] {
  return annotations
    .filter((a) => a.annotation_type.type === 'Point')
    .map((a) => {
      const pt = a.annotation_type as { type: 'Point'; time: number; value: number; axis: string };
      return {
        coord: [pt.time, pt.value],
        name: a.label,
        // ... 其餘不變
      };
    });
}
```

**優點:** 簡單、可靠，overview 本來就是全局視圖
**缺點:** 如果不同 channel 的 value 範圍差異大，mark point 可能顯示位置不理想

### 方案 B: 使用 datasetOrder[0] 的 data_columns 而非 Object.keys

```typescript
// 取得正確的 channelName
const firstDs = allDatasets[datasetOrder[0]];
const firstChannel = firstDs?.column_mapping.data_columns[0] ?? Object.keys(chunk.channels)[0];
```

**優點:** 保持 axis 過濾語意
**缺點:** 仍無法解決 Bug 2（多檔案時 activeDataset 與 first dataset 不同）

### 方案 C: 將 Rust HashMap 改為保序結構

使用 `IndexMap` 或在 TimeseriesChunk 中額外傳送 `channel_order: Vec<String>`。

**優點:** 根本解決 HashMap 順序問題
**缺點:** 需改 Rust 端，且仍無法完全解決 Bug 2

### 推薦: 方案 A

Overview chart 中移除 axis 過濾最簡單有效，一次解決 Bug 1 + Bug 2。axis 過濾僅保留給未來的 SingleAxisChart 使用。

---

## 影響的檔案

| 檔案 | 變更 |
|------|------|
| `src/lib/components/Chart/chartOptions.ts` | `buildMarkPoints` 移除 axis 參數和過濾 |
| `src/lib/components/Chart/chartOptions.ts:106` | 呼叫處移除 channelName 參數 |

---

## 驗證方法

1. 載入單一 CSV 檔案
2. 切換至 `annotate_point` 模式
3. 在圖表上點擊多個位置建立 mark point
4. 確認所有 mark point 立即顯示
5. Zoom in/out 後確認 mark point 仍在
6. 載入第二個 CSV 檔案，重複步驟 2-5
