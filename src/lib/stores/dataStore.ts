/**
 * 多文件振动数据 Store — 核心数据层。
 *
 * 管理多个 CSV 数据集的完整生命周期：加载、降采样 chunk 获取、统计、移除、全部关闭。
 * 所有数据集以 ID 为 key 存储在 Record 映射中，支持多文件叠加显示。
 *
 * 数据流：previewFile -> addFile (IPC: load_vibration_data) -> fetchChunk (IPC: get_timeseries_chunk)
 */
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

// ---------------------------------------------------------------------------
// 多文件数据 Stores — 以数据集 ID 为 key 的 Record 映射
// ---------------------------------------------------------------------------

/** 数据集加载顺序 — 决定图表中系列的渲染顺序和颜色分配 */
export const datasetOrder = writable<string[]>([]);

/** 所有已加载的数据集元信息（文件名、时间范围、列映射等） */
export const datasets = writable<Record<string, VibrationDataset>>({});

/** 各数据集的当前可视 chunk — 经 LTTB 降采样后的时间序列片段 */
export const chunks = writable<Record<string, TimeseriesChunk>>({});

/** 各数据集的统计报告 */
export const statistics = writable<Record<string, StatisticsReport>>({});

/** 当前活跃数据集 ID — 用于统计表、单轴图等需要聚焦单一数据集的视图 */
export const activeDatasetId = writable<string | null>(null);

/** CSV 预览结果 — 由 ColumnMappingDialog 在用户选择列映射前显示 */
export const csvPreview = writable<CsvPreview | null>(null);

/** 每个数据集的颜色覆盖 — 自动分配，也可由用户手动修改 */
export const fileColors = writable<Record<string, string>>({});

/**
 * 手动设置某数据集的显示颜色。
 * @param id - 数据集 ID
 * @param color - 十六进制颜色值
 */
export function setFileColor(id: string, color: string): void {
	fileColors.update((fc) => ({ ...fc, [id]: color }));
}

/** 是否正在执行加载操作 */
export const loading = writable(false);

/** 最近一次错误信息（null 表示无错误） */
export const error = writable<string | null>(null);

// ---------------------------------------------------------------------------
// 派生 Stores
// ---------------------------------------------------------------------------

/**
 * 全局时间范围 — 所有数据集时间范围的并集 (min, max)。
 * 用于 ECharts xAxis 的 min/max 设定，确保多文件时间对齐。
 */
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

/** 派生：当前活跃数据集的完整信息 */
export const activeDataset = derived(
	[activeDatasetId, datasets],
	([$activeId, $datasets]) => ($activeId ? $datasets[$activeId] ?? null : null)
);

/** 派生：当前活跃数据集的统计报告 */
export const activeStatistics = derived(
	[activeDatasetId, statistics],
	([$activeId, $statistics]) => ($activeId ? $statistics[$activeId] ?? null : null)
);

// ---------------------------------------------------------------------------
// Actions — 文件操作
// ---------------------------------------------------------------------------

/**
 * 预览 CSV 文件列信息 — 两步开文件流程的第一步。
 * 调用后端 `preview_csv_columns`，结果存入 csvPreview 供 ColumnMappingDialog 使用。
 * @param filePath - CSV 文件的完整路径
 * @returns CSV 预览信息（列名、行数等）
 */
export async function previewFile(filePath: string): Promise<CsvPreview> {
	const preview = await invoke<CsvPreview>('preview_csv_columns', { filePath });
	csvPreview.set(preview);
	return preview;
}

/**
 * 加载 CSV 文件为数据集 — 两步开文件流程的第二步。
 * 流程：IPC 加载 -> 注册数据集 -> 分配颜色 -> 并行获取 chunk 和统计 -> 同步项目状态。
 * @param filePath - CSV 文件路径
 * @param columnMapping - 用户在 ColumnMappingDialog 中选择的列映射
 */
export async function addFile(filePath: string, columnMapping: ColumnMapping): Promise<void> {
	loading.set(true);
	error.set(null);

	try {
		const ds = await invoke<VibrationDataset>('load_vibration_data', {
			filePath,
			columnMapping
		});

		datasets.update((d) => ({ ...d, [ds.id]: ds }));
		// 按加载顺序从 COLOR_PALETTE 循环分配颜色
		const colorIdx = get(datasetOrder).length;
		fileColors.update((fc) => ({
			...fc,
			[ds.id]: COLOR_PALETTE[colorIdx % COLOR_PALETTE.length]
		}));
		datasetOrder.update((order) => [...order, ds.id]);
		activeDatasetId.set(ds.id);

		// 并行获取降采样 chunk 和统计数据
		await Promise.all([
			fetchChunkForDataset(ds.id, ds.time_range[0], ds.time_range[1], 50000),
			fetchStatisticsForDataset(ds.id)
		]);

		// 将当前所有数据集同步为 projectStore 的 single_file 项目
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

/**
 * 移除单个数据集 — 同时清理 chunks、statistics、fileColors。
 * 若移除的是活跃数据集，自动切换到剩余的第一个；全部移除后关闭项目。
 * @param id - 要移除的数据集 ID
 */
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

	// 用 get() 同步读取，避免嵌套 subscribe 反模式
	const currentActive = get(activeDatasetId);
	if (currentActive === id) {
		const remaining = get(datasetOrder);
		activeDatasetId.set(remaining.length > 0 ? remaining[0] : null);
	}

	if (get(datasetOrder).length === 0) {
		closeProjectStore();
	}
}

/**
 * 关闭所有数据集并重置全部 store。
 * 同时通知后端释放内存（IPC: close_project）。
 */
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

// ---------------------------------------------------------------------------
// Actions — 数据获取
// ---------------------------------------------------------------------------

/**
 * 为所有已加载数据集并行获取指定时间范围的 chunk。
 * 由 dataZoom 事件（debounced 300ms）触发。
 * @param startTime - 可视范围起始时间（epoch seconds）
 * @param endTime - 可视范围结束时间（epoch seconds）
 * @param maxPoints - LTTB 降采样目标点数
 */
export async function fetchAllChunks(
	startTime: number,
	endTime: number,
	maxPoints: number
): Promise<void> {
	const order = get(datasetOrder);

	const promises = order.map((id) => fetchChunkForDataset(id, startTime, endTime, maxPoints));
	await Promise.all(promises);
}

/**
 * 获取单个数据集的降采样 chunk 并更新 store。
 * @param datasetId - 数据集 ID
 * @param startTime - 时间范围起点
 * @param endTime - 时间范围终点
 * @param maxPoints - LTTB 目标点数
 */
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

/**
 * 获取单个数据集的全量统计报告并更新 store。
 * @param datasetId - 数据集 ID
 */
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

/**
 * 将后端返回的单个数据集（如 load_device_data）注册到所有前端 store，
 * 并获取初始 chunk 和统计数据。用于 AIDPS 设备加载流程。
 * @param ds - 后端返回的 VibrationDataset
 */
export async function addDeviceDataset(ds: VibrationDataset): Promise<void> {
	datasets.update((d) => ({ ...d, [ds.id]: ds }));

	const colorIdx = get(datasetOrder).length;
	fileColors.update((fc) => ({
		...fc,
		[ds.id]: COLOR_PALETTE[colorIdx % COLOR_PALETTE.length]
	}));
	datasetOrder.update((order) => [...order, ds.id]);
	activeDatasetId.set(ds.id);

	await Promise.all([
		fetchChunkForDataset(ds.id, ds.time_range[0], ds.time_range[1], 50000),
		fetchStatisticsForDataset(ds.id)
	]);
}

/**
 * 从后端同步所有已持有的数据集到前端 store。
 * 跳过 datasetOrder 中已存在的数据集。用于加载 .vibproj 项目文件后的恢复。
 */
export async function syncDatasetsFromBackend(): Promise<void> {
	const allDs = await invoke<VibrationDataset[]>('list_datasets');
	const currentOrder = get(datasetOrder);

	for (const ds of allDs) {
		if (!currentOrder.includes(ds.id)) {
			await addDeviceDataset(ds);
		}
	}
}
