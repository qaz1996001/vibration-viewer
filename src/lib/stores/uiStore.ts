import { writable, derived } from 'svelte/store';

// --- Mode (from modeStore.ts) ---

export type AppMode = 'browse' | 'annotate_point' | 'annotate_range';

export const mode = writable<AppMode>('browse');
export const rangeFirstClick = writable<number | null>(null);

// --- Precision (from viewStore.ts) ---

export type PrecisionLevel = 'ultra_fast' | 'fast' | 'medium' | 'detailed' | 'full';

export const PRECISION_OPTIONS: { value: PrecisionLevel; label: string; maxPoints: number }[] = [
  { value: 'ultra_fast', label: '超快速 (5K)', maxPoints: 5000 },
  { value: 'fast', label: '快速 (15K)', maxPoints: 15000 },
  { value: 'medium', label: '中等 (50K)', maxPoints: 50000 },
  { value: 'detailed', label: '詳細 (150K)', maxPoints: 150000 },
  { value: 'full', label: '完整資料', maxPoints: -1 },
];

export const precision = writable<PrecisionLevel>('medium');

export function getMaxPoints(level: PrecisionLevel): number {
  const opt = PRECISION_OPTIONS.find((o) => o.value === level);
  return opt ? opt.maxPoints : 50000;
}

// --- Zoom state ---

export const zoomStart = writable<number>(0);
export const zoomEnd = writable<number>(100);

// --- UI flags ---

export const sidebarOpen = writable<boolean>(true);
export const isAnnotationPanelOpen = writable<boolean>(true);

/** When true, multi-file series use channel name only (merged legend) */
export const mergeSeriesMode = writable<boolean>(false);
