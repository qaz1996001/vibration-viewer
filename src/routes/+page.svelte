<script lang="ts">
	import { open, save } from '@tauri-apps/plugin-dialog';
	import { invoke } from '@tauri-apps/api/core';
	import TimeseriesChart from '$lib/components/Chart/TimeseriesChart.svelte';
	import SingleAxisChart from '$lib/components/Chart/SingleAxisChart.svelte';
	import AnnotationPanel from '$lib/components/Annotation/AnnotationPanel.svelte';
	import BasicStatsTable from '$lib/components/Statistics/BasicStatsTable.svelte';
	import ViewportDataTable from '$lib/components/DataTable/ViewportDataTable.svelte';
	import ColumnMappingDialog from '$lib/components/ColumnMapping/ColumnMappingDialog.svelte';
	import FileList from '$lib/components/Layout/FileList.svelte';
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
		closeAll
	} from '$lib/stores/dataStore';
	import {
		addAnnotation,
		updateAnnotation,
		saveAnnotations,
		loadAnnotations,
		dirty
	} from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick } from '$lib/stores/modeStore';
	import { precision, getMaxPoints } from '$lib/stores/viewStore';
	import { projectOpen } from '$lib/stores/projectStore';
	import { debounce } from '$lib/utils/debounce';
	import type { ColumnMapping, CsvPreview } from '$lib/types/vibration';

	let pendingAnnotation: { type: 'point' | 'range'; data: any } | null = $state(null);
	let currentZoomStart = $state(0);
	let currentZoomEnd = $state(100);
	let showMappingDialog = $state(false);
	let currentPreview: CsvPreview | null = $state(null);
	let pendingFilePaths: string[] = $state([]);

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

	async function handleCloseProject() {
		await closeAll();
	}

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

	// Re-fetch when precision changes
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

	function handleAnnotatePoint(data: { time: number; value: number }) {
		pendingAnnotation = { type: 'point', data };
	}

	function handleAnnotateRange(data: { startTime: number; endTime: number }) {
		pendingAnnotation = { type: 'range', data };
	}

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

	function handleAnnotationCancel() {
		pendingAnnotation = null;
		rangeFirstClick.set(null);
	}

	function handleUpdateAnnotation(data: { id: string; updates: Record<string, any> }) {
		updateAnnotation(data.id, data.updates);
	}

	let activeChunk = $derived(
		$activeDatasetId ? ($chunks[$activeDatasetId] ?? null) : null
	);
	let dataColumns = $derived($activeDataset?.column_mapping.data_columns ?? []);
	let hasData = $derived($datasetOrder.length > 0);
</script>

{#if showMappingDialog && currentPreview}
	<ColumnMappingDialog
		preview={currentPreview}
		onconfirm={handleMappingConfirm}
		oncancel={handleMappingCancel}
	/>
{/if}

<main class="app-layout">
	<Toolbar
		onopenfile={handleOpenFile}
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
				<div class="status-message">Loading...</div>
			{:else if $error}
				<div class="status-message error">{$error}</div>
			{:else if hasData}
				<section class="section">
					<h3>Overview</h3>
					<TimeseriesChart
						ondatazoom={(data) => debouncedFetchChunks(data.start, data.end)}
						onannotatepoint={handleAnnotatePoint}
						onannotaterange={handleAnnotateRange}
						onupdateannotation={handleUpdateAnnotation}
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
			<FileList />
			<AnnotationPanel
				{pendingAnnotation}
				onconfirm={handleAnnotationConfirm}
				oncancel={handleAnnotationCancel}
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
