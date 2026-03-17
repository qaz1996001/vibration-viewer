import { writable } from 'svelte/store';

export type PrecisionLevel = 'ultra_fast' | 'fast' | 'medium' | 'detailed' | 'full';

export const PRECISION_OPTIONS: { value: PrecisionLevel; label: string; maxPoints: number }[] = [
	{ value: 'ultra_fast', label: '超快速 (5K)', maxPoints: 5000 },
	{ value: 'fast', label: '快速 (15K)', maxPoints: 15000 },
	{ value: 'medium', label: '中等 (50K)', maxPoints: 50000 },
	{ value: 'detailed', label: '詳細 (150K)', maxPoints: 150000 },
	{ value: 'full', label: '完整資料', maxPoints: -1 }
];

export const precision = writable<PrecisionLevel>('medium');

export function getMaxPoints(level: PrecisionLevel): number {
	const opt = PRECISION_OPTIONS.find((o) => o.value === level);
	return opt ? opt.maxPoints : 50000;
}
