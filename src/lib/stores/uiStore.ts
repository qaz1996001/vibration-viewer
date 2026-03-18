/**
 * UI 状态管理 — 操作模式、精度选择、缩放状态、面板开关。
 * 不含业务数据，仅控制界面交互行为。
 */
import { writable, derived } from 'svelte/store';

// ---------------------------------------------------------------------------
// 操作模式 (Mode) — 决定图表点击行为
// ---------------------------------------------------------------------------

/**
 * 应用操作模式：
 * - `browse`: 浏览模式，支持 pan/zoom
 * - `annotate_point`: 单击图表创建点标注
 * - `annotate_range`: 两次点击创建范围标注
 */
export type AppMode = 'browse' | 'annotate_point' | 'annotate_range';

/** 当前操作模式 */
export const mode = writable<AppMode>('browse');

/** 范围标注模式下第一次点击的时间戳（epoch seconds），等待第二次点击完成范围 */
export const rangeFirstClick = writable<number | null>(null);

// ---------------------------------------------------------------------------
// 显示精度 (Precision) — 控制 LTTB 降采样的目标点数
// ---------------------------------------------------------------------------

/**
 * 精度等级 — 对应不同的 LTTB 降采样目标点数。
 * `full` 表示不降采样，返回全量数据。
 */
export type PrecisionLevel = 'ultra_fast' | 'fast' | 'medium' | 'detailed' | 'full';

/** 精度选项列表 — 用于 Toolbar 下拉选择器，maxPoints=-1 表示不限制 */
export const PRECISION_OPTIONS: { value: PrecisionLevel; label: string; maxPoints: number }[] = [
  { value: 'ultra_fast', label: '超快速 (5K)', maxPoints: 5000 },
  { value: 'fast', label: '快速 (15K)', maxPoints: 15000 },
  { value: 'medium', label: '中等 (50K)', maxPoints: 50000 },
  { value: 'detailed', label: '詳細 (150K)', maxPoints: 150000 },
  { value: 'full', label: '完整資料', maxPoints: -1 },
];

/** 当前选中的精度等级 */
export const precision = writable<PrecisionLevel>('medium');

/**
 * 将精度等级转为 LTTB maxPoints 数值。
 * @param level - 精度等级
 * @returns 对应的最大点数（-1 表示不降采样）
 */
export function getMaxPoints(level: PrecisionLevel): number {
  const opt = PRECISION_OPTIONS.find((o) => o.value === level);
  return opt ? opt.maxPoints : 50000;
}

// ---------------------------------------------------------------------------
// 缩放状态 (Zoom) — ECharts dataZoom 百分比
// ---------------------------------------------------------------------------

/** dataZoom 起始百分比 (0-100) */
export const zoomStart = writable<number>(0);

/** dataZoom 结束百分比 (0-100) */
export const zoomEnd = writable<number>(100);

// ---------------------------------------------------------------------------
// UI 面板开关
// ---------------------------------------------------------------------------

/** 左侧文件列表侧边栏是否展开 */
export const sidebarOpen = writable<boolean>(true);

/** 标注面板是否展开 */
export const isAnnotationPanelOpen = writable<boolean>(true);

/** 多文件系列合并模式 — 启用时 legend 仅显示通道名，不含文件名前缀 */
export const mergeSeriesMode = writable<boolean>(false);
