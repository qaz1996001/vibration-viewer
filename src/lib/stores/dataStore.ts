import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { VibrationDataset, TimeseriesChunk } from '$lib/types/vibration';
import type { StatisticsReport } from '$lib/types/statistics';

export const dataset = writable<VibrationDataset | null>(null);
export const chunk = writable<TimeseriesChunk | null>(null);
export const statistics = writable<StatisticsReport | null>(null);
export const loading = writable(false);
export const error = writable<string | null>(null);

export async function loadFile(filePath: string): Promise<void> {
	loading.set(true);
	error.set(null);

	try {
		const ds = await invoke<VibrationDataset>('load_vibration_data', {
			filePath
		});
		dataset.set(ds);

		await Promise.all([
			fetchChunk(ds.id, ds.time_range[0], ds.time_range[1], 50000),
			fetchStatistics(ds.id)
		]);
	} catch (e) {
		error.set(String(e));
	} finally {
		loading.set(false);
	}
}

export async function fetchChunk(
	datasetId: string,
	startTime: number,
	endTime: number,
	maxPoints: number
): Promise<void> {
	try {
		const c = await invoke<TimeseriesChunk>('get_timeseries_chunk', {
			datasetId,
			startTime,
			endTime,
			maxPoints
		});
		chunk.set(c);
	} catch (e) {
		error.set(String(e));
	}
}

async function fetchStatistics(datasetId: string): Promise<void> {
	try {
		const stats = await invoke<StatisticsReport>('compute_statistics', {
			datasetId
		});
		statistics.set(stats);
	} catch (e) {
		error.set(String(e));
	}
}
