import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type {
	VibrationDataset,
	TimeseriesChunk,
	CsvPreview,
	ColumnMapping
} from '$lib/types/vibration';
import type { StatisticsReport } from '$lib/types/statistics';
import { COLOR_PALETTE } from '$lib/constants/colors';
import { setProject, closeProject as closeProjectStore } from './projectStore';
import type { ProjectState } from './projectStore';

// Multi-file stores
export const datasetOrder = writable<string[]>([]);
export const datasets = writable<Record<string, VibrationDataset>>({});
export const chunks = writable<Record<string, TimeseriesChunk>>({});
export const statistics = writable<Record<string, StatisticsReport>>({});

// Active dataset (for stats display, single-axis charts)
export const activeDatasetId = writable<string | null>(null);

// CSV preview for column mapping dialog
export const csvPreview = writable<CsvPreview | null>(null);

// Per-file color overrides
export const fileColors = writable<Record<string, string>>({});

export function setFileColor(id: string, color: string): void {
	fileColors.update((fc) => ({ ...fc, [id]: color }));
}

// UI state
export const loading = writable(false);
export const error = writable<string | null>(null);

// Derived: global time range across all datasets
export const globalTimeRange = derived(datasets, ($datasets) => {
	const ids = Object.keys($datasets);
	if (ids.length === 0) return null;
	let min = Infinity;
	let max = -Infinity;
	for (const id of ids) {
		const ds = $datasets[id];
		if (ds.time_range[0] < min) min = ds.time_range[0];
		if (ds.time_range[1] > max) max = ds.time_range[1];
	}
	return [min, max] as [number, number];
});

// Derived: active dataset
export const activeDataset = derived(
	[activeDatasetId, datasets],
	([$activeId, $datasets]) => ($activeId ? $datasets[$activeId] ?? null : null)
);

// Derived: active statistics
export const activeStatistics = derived(
	[activeDatasetId, statistics],
	([$activeId, $statistics]) => ($activeId ? $statistics[$activeId] ?? null : null)
);

export async function previewFile(filePath: string): Promise<CsvPreview> {
	const preview = await invoke<CsvPreview>('preview_csv_columns', { filePath });
	csvPreview.set(preview);
	return preview;
}

export async function addFile(filePath: string, columnMapping: ColumnMapping): Promise<void> {
	loading.set(true);
	error.set(null);

	try {
		const ds = await invoke<VibrationDataset>('load_vibration_data', {
			filePath,
			columnMapping
		});

		datasets.update((d) => ({ ...d, [ds.id]: ds }));
		const colorIdx = get(datasetOrder).length;
		fileColors.update((fc) => ({
			...fc,
			[ds.id]: COLOR_PALETTE[colorIdx % COLOR_PALETTE.length]
		}));
		datasetOrder.update((order) => [...order, ds.id]);
		activeDatasetId.set(ds.id);

		// Fetch chunk and stats for new dataset
		await Promise.all([
			fetchChunkForDataset(ds.id, ds.time_range[0], ds.time_range[1], 50000),
			fetchStatisticsForDataset(ds.id)
		]);

		// Sync project state
		const allDatasets = get(datasets);
		const projectState: ProjectState = {
			project_type: 'single_file',
			devices: Object.values(allDatasets).map((d) => ({
				id: d.id,
				name: d.file_name,
				sources: [
					{
						file_path: d.file_path,
						file_name: d.file_name,
						source_type: 'csv' as const
					}
				],
				channel_schema: { groups: {} }
			})),
			sensor_mapping: {},
			metadata: {
				name: Object.values(allDatasets)[0]?.file_name ?? 'Untitled',
				created_at: new Date().toISOString()
			}
		};
		setProject(projectState);
	} catch (e) {
		error.set(String(e));
	} finally {
		loading.set(false);
	}
}

export function removeFile(id: string): void {
	datasets.update((d) => {
		const copy = { ...d };
		delete copy[id];
		return copy;
	});
	chunks.update((c) => {
		const copy = { ...c };
		delete copy[id];
		return copy;
	});
	statistics.update((s) => {
		const copy = { ...s };
		delete copy[id];
		return copy;
	});
	fileColors.update((fc) => {
		const copy = { ...fc };
		delete copy[id];
		return copy;
	});
	datasetOrder.update((order) => order.filter((oid) => oid !== id));

	// Read current values synchronously — avoids nested subscribe anti-pattern
	const currentActive = get(activeDatasetId);
	if (currentActive === id) {
		const remaining = get(datasetOrder);
		activeDatasetId.set(remaining.length > 0 ? remaining[0] : null);
	}

	// Close project when all files removed
	if (get(datasetOrder).length === 0) {
		closeProjectStore();
	}
}

export async function closeAll(): Promise<void> {
	try {
		await invoke('close_project');
	} catch (e) {
		console.error('Failed to close project:', e);
	}
	datasets.set({});
	chunks.set({});
	statistics.set({});
	fileColors.set({});
	datasetOrder.set([]);
	activeDatasetId.set(null);
	closeProjectStore();
}

export async function fetchAllChunks(
	startTime: number,
	endTime: number,
	maxPoints: number
): Promise<void> {
	const order = get(datasetOrder);

	const promises = order.map((id) => fetchChunkForDataset(id, startTime, endTime, maxPoints));
	await Promise.all(promises);
}

async function fetchChunkForDataset(
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
		chunks.update((ch) => ({ ...ch, [datasetId]: c }));
	} catch (e) {
		error.set(String(e));
	}
}

async function fetchStatisticsForDataset(datasetId: string): Promise<void> {
	try {
		const stats = await invoke<StatisticsReport>('compute_statistics', {
			datasetId
		});
		statistics.update((s) => ({ ...s, [datasetId]: stats }));
	} catch (e) {
		error.set(String(e));
	}
}
