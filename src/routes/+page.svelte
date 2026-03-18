<script lang="ts">
	/**
	 * 主頁面元件 — 應用程式的唯一路由（SPA 模式）
	 *
	 * 職責：
	 * - 協調所有子元件（圖表、工具列、側邊欄、標註面板、統計表等）
	 * - 處理檔案開啟 / 專案管理（AIDPS / .vibproj）的 Tauri IPC 呼叫
	 * - 管理 dataZoom 事件 → 防抖 fetchAllChunks 的資料載入流程
	 * - 標註的建立、更新、儲存、匯出等使用者操作的 event handler
	 *
	 * 資料流：Toolbar/FileList 觸發操作 → 更新 store → 各元件透過 store 訂閱響應
	 */
	import { open, save } from '@tauri-apps/plugin-dialog';
	import { invoke } from '@tauri-apps/api/core';
	import TimeseriesChart from '$lib/components/Chart/TimeseriesChart.svelte';
	import SingleAxisChart from '$lib/components/Chart/SingleAxisChart.svelte';
	import AnnotationPanel from '$lib/components/Annotation/AnnotationPanel.svelte';
	import BasicStatsTable from '$lib/components/Statistics/BasicStatsTable.svelte';
	import ViewportDataTable from '$lib/components/DataTable/ViewportDataTable.svelte';
	import ColumnMappingDialog from '$lib/components/ColumnMapping/ColumnMappingDialog.svelte';
	import FileList from '$lib/components/Layout/FileList.svelte';
	import DeviceSelector from '$lib/components/Layout/DeviceSelector.svelte';
	import Toolbar from '$lib/components/Layout/Toolbar.svelte';
	import {
		datasetOrder,
		datasets,
		chunks,
		activeDataset,
		activeStatistics,
		globalTimeRange,
		activeDatasetId,
		loading,
		error,
		previewFile,
		addFile,
		fetchAllChunks,
		closeAll,
		syncDatasetsFromBackend
	} from '$lib/stores/dataStore';
	import {
		addAnnotation,
		updateAnnotation,
		saveAnnotations,
		loadAnnotations,
		dirty
	} from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick, precision, getMaxPoints } from '$lib/stores/uiStore';
	import { projectOpen, project, setProject, mapBackendProjectInfo, loadDeviceData } from '$lib/stores/projectStore';
	import { get } from 'svelte/store';
	import { debounce } from '$lib/utils/debounce';
	import type { ColumnMapping, CsvPreview } from '$lib/types/vibration';
	import type { Annotation } from '$lib/types/annotation';

	// ─── Svelte 5 Reactive State ($state) ───────────────────────────────

	/** 待確認的標註（使用者點擊圖表後、尚未輸入 label 前的暫存） */
	let pendingAnnotation: { type: 'point' | 'range'; data: any } | null = $state(null);
	/** 當前 dataZoom 的起始百分比（0-100），用於計算實際時間範圍 */
	let currentZoomStart = $state(0);
	/** 當前 dataZoom 的結束百分比（0-100） */
	let currentZoomEnd = $state(100);
	/** 驅動圖表跳轉至指定 zoom 範圍（由 handleJumpToAnnotation 設定） */
	let chartZoomTarget: { start: number; end: number } | null = $state(null);
	/** 是否顯示欄位映射對話框 */
	let showMappingDialog = $state(false);
	/** 當前預覽中的 CSV 資訊（供 ColumnMappingDialog 使用） */
	let currentPreview: CsvPreview | null = $state(null);
	/** 等待欄位映射確認的 CSV 檔案路徑列表（批次開啟時暫存） */
	let pendingFilePaths: string[] = $state([]);

	// ─── 檔案操作 Handlers ─────────────────────────────────────────────

	/** 開啟 CSV 檔案 — 彈出檔案對話框 → 預覽第一個檔案 → 顯示欄位映射 */
	async function handleOpenFile() {
		try {
			const selected = await open({
				multiple: true,
				filters: [{ name: 'CSV', extensions: ['csv'] }]
			});
			if (!selected) return;

			const paths = Array.isArray(selected) ? selected : [selected];
			if (paths.length === 0) return;

			// Preview first file for column mapping; same mapping applied to all
			const preview = await previewFile(paths[0]);
			currentPreview = preview;
			pendingFilePaths = paths;
			showMappingDialog = true;
		} catch (e) {
			console.error('Failed to open file:', e);
			error.set(String(e));
		}
	}

	/** 欄位映射確認 — 依序載入所有待處理檔案並嘗試讀取已有標註 */
	async function handleMappingConfirm(mapping: ColumnMapping) {
		showMappingDialog = false;
		const paths = pendingFilePaths;
		pendingFilePaths = [];
		currentPreview = null;

		for (const filePath of paths) {
			await addFile(filePath, mapping);
			try {
				await loadAnnotations(filePath);
			} catch (e) {
				console.error('Failed to load annotations:', e);
				// Non-fatal: file may have no annotations yet
			}
		}
	}

	function handleMappingCancel() {
		showMappingDialog = false;
		pendingFilePaths = [];
		currentPreview = null;
	}

	/** 關閉所有已載入的資料集並重置狀態 */
	async function handleCloseProject() {
		await closeAll();
	}

	/** AIDPS 模式下選擇裝置 — 若資料已載入則切換 active，否則從後端載入 */
	async function handleDeviceSelect(deviceId: string) {
		// Check if data is already loaded for this device
		const currentDatasets = get(datasets);
		if (currentDatasets[deviceId]) {
			// Data already loaded — just switch active
			activeDatasetId.set(deviceId);
			return;
		}

		// Load device data (shows loading state)
		loading.set(true);
		error.set(null);
		try {
			await loadDeviceData(deviceId);
		} catch (e) {
			error.set(String(e));
		} finally {
			loading.set(false);
		}
	}

	// ─── 專案操作 Handlers ─────────────────────────────────────────────

	/** 開啟 AIDPS 資料夾專案 — 選取資料夾 → IPC 掃描 → 自動載入首個裝置 */
	async function handleOpenProject() {
		try {
			const selected = await open({ directory: true });
			if (!selected) return;

			const folderPath = typeof selected === 'string' ? selected : selected[0];
			if (!folderPath) return;

			const result = await invoke<Record<string, unknown>>('open_aidps_project', { folderPath });
			const projectState = mapBackendProjectInfo(result);
			setProject(projectState);

			// Auto-load first device
			if (projectState.devices.length > 0) {
				await handleDeviceSelect(projectState.devices[0].id);
			}
		} catch (e) {
			console.error('Failed to open AIDPS project:', e);
			error.set(String(e));
		}
	}

	/** 儲存專案為 .vibproj 檔案 — 彈出儲存對話框 → IPC save_project_file */
	async function handleSaveProject() {
		try {
			const outputPath = await save({
				filters: [{ name: 'VibProj', extensions: ['vibproj'] }]
			});
			if (!outputPath) return;

			await invoke('save_project_file', { outputPath });
		} catch (e) {
			console.error('Failed to save project:', e);
			error.set(String(e));
		}
	}

	/** 載入 .vibproj 專案檔 — 開啟檔案 → IPC load_project_file → 同步後端資料集至前端 */
	async function handleLoadProject() {
		try {
			const selected = await open({
				multiple: false,
				filters: [{ name: 'VibProj', extensions: ['vibproj'] }]
			});
			if (!selected) return;

			const filePath = typeof selected === 'string' ? selected : selected[0];
			if (!filePath) return;

			const result = await invoke<Record<string, unknown>>('load_project_file', { filePath });
			const projectState = mapBackendProjectInfo(result);
			setProject(projectState);

			// Sync backend datasets to frontend stores
			loading.set(true);
			error.set(null);
			try {
				await syncDatasetsFromBackend();
			} finally {
				loading.set(false);
			}
		} catch (e) {
			console.error('Failed to load project:', e);
			error.set(String(e));
			loading.set(false);
		}
	}

	// ─── 標註 / 匯出 Handlers ──────────────────────────────────────────

	/** 儲存當前活躍資料集的標註至 .vibann.json 檔案 */
	async function handleSave() {
		const ds = $activeDataset;
		if (ds) {
			try {
				await saveAnnotations(ds.file_path);
			} catch (e) {
				console.error('Failed to save annotations:', e);
				error.set(`Save failed: ${e}`);
			}
		}
	}

	/** 匯出完整資料集為 CSV */
	async function handleExport() {
		const ds = $activeDataset;
		if (!ds) return;
		try {
			const outputPath = await save({
				filters: [{ name: 'CSV', extensions: ['csv'] }],
				defaultPath: 'export.csv'
			});
			if (outputPath) {
				await invoke('export_data', {
					datasetId: ds.id,
					outputPath
				});
			}
		} catch (e) {
			console.error('Failed to export data:', e);
			error.set(`Export failed: ${e}`);
		}
	}

	/** 匯出當前可視範圍（viewport）的資料為 CSV — 根據 zoom 百分比計算時間區間 */
	async function handleExportViewport() {
		const ds = $activeDataset;
		if (!ds) return;
		const range = $globalTimeRange;
		if (!range) return;
		const startTime = range[0] + (currentZoomStart / 100) * (range[1] - range[0]);
		const endTime = range[0] + (currentZoomEnd / 100) * (range[1] - range[0]);
		try {
			const outputPath = await save({
				filters: [{ name: 'CSV', extensions: ['csv'] }],
				defaultPath: 'export_viewport.csv'
			});
			if (outputPath) {
				await invoke('export_data', {
					datasetId: ds.id,
					outputPath,
					startTime,
					endTime
				});
			}
		} catch (e) {
			console.error('Failed to export viewport data:', e);
			error.set(`Export failed: ${e}`);
		}
	}

	// ─── 資料載入與 Zoom ────────────────────────────────────────────────

	/**
	 * 防抖版本的 chunk 載入 — dataZoom 事件觸發後 300ms 才實際發送 IPC 請求。
	 * 將百分比 zoom 範圍轉換為 epoch seconds 時間範圍後呼叫 fetchAllChunks。
	 */
	const debouncedFetchChunks = debounce((start: number, end: number) => {
		const range = $globalTimeRange;
		if (!range) return;
		currentZoomStart = start;
		currentZoomEnd = end;
		const startTime = range[0] + (start / 100) * (range[1] - range[0]);
		const endTime = range[0] + (end / 100) * (range[1] - range[0]);
		const maxPts = getMaxPoints($precision);
		fetchAllChunks(startTime, endTime, maxPts > 0 ? maxPts : Number.MAX_SAFE_INTEGER).catch(
			(e) => console.error('Failed to fetch chunks on zoom:', e)
		);
	}, 300);

	/** $effect: 當精度等級或已載入資料集變更時，重新載入當前 viewport 的 chunk */
	$effect(() => {
		const level = $precision;
		const range = $globalTimeRange;
		const order = $datasetOrder;
		if (!range || order.length === 0) return;
		const startTime = range[0] + (currentZoomStart / 100) * (range[1] - range[0]);
		const endTime = range[0] + (currentZoomEnd / 100) * (range[1] - range[0]);
		const maxPts = getMaxPoints(level);
		fetchAllChunks(startTime, endTime, maxPts > 0 ? maxPts : Number.MAX_SAFE_INTEGER).catch(
			(e) => console.error('Failed to fetch chunks on precision change:', e)
		);
	});

	// ─── 標註互動 Handlers ─────────────────────────────────────────────

	/** 收到圖表上的單點標註點擊 → 暫存至 pendingAnnotation 等待使用者確認 */
	function handleAnnotatePoint(data: { time: number; value: number }) {
		pendingAnnotation = { type: 'point', data };
	}

	/** 收到圖表上的區間標註完成 → 暫存至 pendingAnnotation 等待使用者確認 */
	function handleAnnotateRange(data: { startTime: number; endTime: number }) {
		pendingAnnotation = { type: 'range', data };
	}

	/** 使用者確認標註 — 將 pendingAnnotation 轉為正式 Annotation 加入 store */
	function handleAnnotationConfirm(detail: { label: string; color: string }) {
		if (!pendingAnnotation) return;

		const { label, color } = detail;
		if (pendingAnnotation.type === 'point') {
			addAnnotation(
				{
					type: 'Point',
					time: pendingAnnotation.data.time,
					value: pendingAnnotation.data.value,
					axis: $activeDataset?.column_mapping.data_columns[0] ?? 'x'
				},
				label,
				color
			);
		} else {
			addAnnotation(
				{
					type: 'Range',
					start_time: pendingAnnotation.data.startTime,
					end_time: pendingAnnotation.data.endTime
				},
				label,
				color
			);
		}

		pendingAnnotation = null;
	}

	/** 取消標註 — 清除暫存及 Range 模式的第一次點擊狀態 */
	function handleAnnotationCancel() {
		pendingAnnotation = null;
		rangeFirstClick.set(null);
	}

	/**
	 * 跳轉至標註位置 — 從 AnnotationPanel 點擊觸發。
	 * 計算標註周圍的 viewport（Point: 5% padding, Range: 10% padding），
	 * 設定 chartZoomTarget 驅動圖表 zoom，並載入對應範圍的 chunk。
	 */
	function handleJumpToAnnotation(ann: Annotation) {
		const range = $globalTimeRange;
		if (!range) return;

		const totalSpan = range[1] - range[0];
		let targetStart: number;
		let targetEnd: number;

		if (ann.annotation_type.type === 'Point') {
			const padding = totalSpan * 0.05;
			targetStart = ann.annotation_type.time - padding;
			targetEnd = ann.annotation_type.time + padding;
		} else {
			const rangeSpan = ann.annotation_type.end_time - ann.annotation_type.start_time;
			const padding = rangeSpan * 0.1;
			targetStart = ann.annotation_type.start_time - padding;
			targetEnd = ann.annotation_type.end_time + padding;
		}

		// Clamp to global range
		targetStart = Math.max(range[0], targetStart);
		targetEnd = Math.min(range[1], targetEnd);

		// Convert to percentage for dataZoom
		const zoomStart = ((targetStart - range[0]) / totalSpan) * 100;
		const zoomEnd = ((targetEnd - range[0]) / totalSpan) * 100;

		// Drive chart zoom via reactive prop
		chartZoomTarget = { start: zoomStart, end: zoomEnd };

		// Fetch data for the new viewport
		currentZoomStart = zoomStart;
		currentZoomEnd = zoomEnd;
		const startTime = range[0] + (zoomStart / 100) * totalSpan;
		const endTime = range[0] + (zoomEnd / 100) * totalSpan;
		const maxPts = getMaxPoints($precision);
		fetchAllChunks(startTime, endTime, maxPts > 0 ? maxPts : Number.MAX_SAFE_INTEGER).catch(
			(e) => console.error('Failed to fetch chunks on jump:', e)
		);
	}

	/** 標註更新 — 由拖曳 handler（邊界拖曳 / label 拖曳）觸發 */
	function handleUpdateAnnotation(data: { id: string; updates: Record<string, any> }) {
		updateAnnotation(data.id, data.updates);
	}

	// ─── Svelte 5 Derived State ($derived) ──────────────────────────────

	/** $derived: 當前活躍資料集的 TimeseriesChunk（供資料表及單通道圖表使用） */
	let activeChunk = $derived(
		$activeDatasetId ? ($chunks[$activeDatasetId] ?? null) : null
	);
	/** $derived: 當前活躍資料集的資料通道名稱列表（用於 SingleAxisChart 迴圈渲染） */
	let dataColumns = $derived($activeDataset?.column_mapping.data_columns ?? []);
	/** $derived: 是否有任何已載入的資料集（控制歡迎畫面 vs 主內容的顯示） */
	let hasData = $derived($datasetOrder.length > 0);
	/** $derived: 是否為 AIDPS 資料夾模式（決定側邊欄顯示 DeviceSelector 或 FileList） */
	let isAidpsMode = $derived(
		$projectOpen && $project?.project_type === 'aidps_folder'
	);
</script>

{#if showMappingDialog && currentPreview}
	<ColumnMappingDialog
		preview={currentPreview}
		onconfirm={handleMappingConfirm}
		oncancel={handleMappingCancel}
	/>
{/if}

<main class="app-layout" id="main-content">
	<Toolbar
		onopenfile={handleOpenFile}
		onopenproject={handleOpenProject}
		onsaveproject={handleSaveProject}
		onloadproject={handleLoadProject}
		onsave={handleSave}
		onexport={handleExport}
		onexportviewport={handleExportViewport}
		onclose={handleCloseProject}
		hasUnsaved={$dirty}
		hasProject={$projectOpen}
		multiFile={$datasetOrder.length > 1}
	/>

	<div class="content">
		<div class="main-area">
			{#if $loading}
				<div class="status-message" role="status" aria-live="polite">Loading...</div>
			{:else if $error}
				<div class="status-message error" role="alert" aria-live="assertive">{$error}</div>
			{:else if hasData}
				<section class="section">
					<h3>Overview</h3>
					<TimeseriesChart
						ondatazoom={(data) => debouncedFetchChunks(data.start, data.end)}
						onannotatepoint={handleAnnotatePoint}
						onannotaterange={handleAnnotateRange}
						onupdateannotation={handleUpdateAnnotation}
						zoomTo={chartZoomTarget}
					/>
				</section>

				{#if activeChunk}
					<section class="section">
						<ViewportDataTable chunk={activeChunk} />
					</section>

					<section class="section">
						<h3>Channel Analysis ({$activeDataset?.file_name ?? ''})</h3>
						<div class="axis-charts">
							{#each dataColumns as channelName (channelName)}
								<SingleAxisChart {channelName} chunk={activeChunk} />
							{/each}
						</div>
					</section>
				{/if}

				{#if $activeStatistics}
					<section class="section">
						<BasicStatsTable stats={$activeStatistics} />
					</section>
				{/if}

				{#if activeChunk?.is_downsampled}
					<div class="info-bar">
						Showing {activeChunk.time.length.toLocaleString()} / {activeChunk.original_count.toLocaleString()}
						points (downsampled)
					</div>
				{/if}
			{:else}
				<div class="welcome">
					<p>Open a CSV file to start analyzing data</p>
					<p class="hint">Any CSV with a time column and numeric data columns</p>
				</div>
			{/if}
		</div>

		<aside class="sidebar">
			{#if isAidpsMode}
				<DeviceSelector onselect={handleDeviceSelect} />
			{:else}
				<FileList />
			{/if}
			<AnnotationPanel
				{pendingAnnotation}
				onconfirm={handleAnnotationConfirm}
				oncancel={handleAnnotationCancel}
				onjumpto={handleJumpToAnnotation}
			/>
		</aside>
	</div>
</main>

<style>
	.app-layout {
		display: flex;
		flex-direction: column;
		height: 100vh;
	}

	.content {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.main-area {
		flex: 1;
		overflow-y: auto;
		padding: 1rem;
	}

	.sidebar {
		width: 280px;
		border-left: 1px solid var(--border, #e0e0e0);
		overflow-y: auto;
	}

	.section {
		margin-bottom: 1.5rem;
	}

	.section h3 {
		margin: 0 0 0.5rem;
		font-size: 1rem;
		color: var(--text-primary, #333);
	}

	.axis-charts {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.status-message,
	.welcome {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		height: 300px;
		color: var(--text-secondary, #666);
	}

	.status-message.error {
		color: var(--error, #ff4444);
	}

	.welcome .hint {
		font-size: 0.85rem;
		margin-top: 0.5rem;
		opacity: 0.7;
	}

	.info-bar {
		text-align: center;
		font-size: 0.8rem;
		color: var(--text-secondary, #999);
		padding: 0.5rem;
	}
</style>
