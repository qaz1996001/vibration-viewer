<script lang="ts">
	import { open, save } from '@tauri-apps/plugin-dialog';
	import { invoke } from '@tauri-apps/api/core';
	import TimeseriesChart from '$lib/components/Chart/TimeseriesChart.svelte';
	import AnnotationPanel from '$lib/components/Annotation/AnnotationPanel.svelte';
	import AnnotationDialog from '$lib/components/Annotation/AnnotationDialog.svelte';
	import BasicStatsTable from '$lib/components/Statistics/BasicStatsTable.svelte';
	import Toolbar from '$lib/components/Layout/Toolbar.svelte';
	import {
		dataset,
		chunk,
		statistics,
		loading,
		error,
		loadFile,
		fetchChunk
	} from '$lib/stores/dataStore';
	import {
		addAnnotation,
		saveAnnotations,
		loadAnnotations,
		dirty
	} from '$lib/stores/annotationStore';
	import { mode } from '$lib/stores/modeStore';
	import { debounce } from '$lib/utils/debounce';

	let showAnnotationDialog = false;
	let pendingAnnotation: { type: 'point' | 'range'; data: any } | null = null;

	async function handleOpenFile() {
		const filePath = await open({
			filters: [{ name: 'CSV', extensions: ['csv'] }]
		});
		if (filePath) {
			await loadFile(filePath as string);
			await loadAnnotations(filePath as string);
		}
	}

	async function handleSave() {
		if ($dataset) {
			await saveAnnotations($dataset.id, $dataset.file_path);
		}
	}

	async function handleExport() {
		if (!$dataset) return;
		const outputPath = await save({
			filters: [{ name: 'CSV', extensions: ['csv'] }],
			defaultPath: 'export.csv'
		});
		if (outputPath) {
			await invoke('export_data', {
				datasetId: $dataset.id,
				outputPath
			});
		}
	}

	const debouncedFetchChunk = debounce((start: number, end: number) => {
		if (!$dataset) return;
		const range = $dataset.time_range;
		const startTime = range[0] + (start / 100) * (range[1] - range[0]);
		const endTime = range[0] + (end / 100) * (range[1] - range[0]);
		fetchChunk($dataset.id, startTime, endTime, 50000);
	}, 300);

	function handleDataZoom(event: CustomEvent<{ start: number; end: number }>) {
		debouncedFetchChunk(event.detail.start, event.detail.end);
	}

	function handleAnnotatePoint(event: CustomEvent<{ time: number; value: number }>) {
		pendingAnnotation = { type: 'point', data: event.detail };
		showAnnotationDialog = true;
	}

	function handleAnnotateRange(event: CustomEvent<{ startTime: number; endTime: number }>) {
		pendingAnnotation = { type: 'range', data: event.detail };
		showAnnotationDialog = true;
	}

	function handleAnnotationConfirm(event: CustomEvent<{ label: string; color: string }>) {
		if (!pendingAnnotation) return;

		const { label, color } = event.detail;
		if (pendingAnnotation.type === 'point') {
			addAnnotation(
				{
					type: 'Point',
					time: pendingAnnotation.data.time,
					value: pendingAnnotation.data.value,
					axis: 'x'
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

		showAnnotationDialog = false;
		pendingAnnotation = null;
	}
</script>

<main class="app-layout">
	<Toolbar on:open-file={handleOpenFile} on:save={handleSave} on:export={handleExport} hasUnsaved={$dirty} />

	<div class="content">
		<div class="main-area">
			{#if $loading}
				<div class="status-message">Loading...</div>
			{:else if $error}
				<div class="status-message error">{$error}</div>
			{:else if $chunk}
				<TimeseriesChart
					on:datazoom={handleDataZoom}
					on:annotate-point={handleAnnotatePoint}
					on:annotate-range={handleAnnotateRange}
				/>

				{#if $statistics}
					<BasicStatsTable stats={$statistics} />
				{/if}

				{#if $chunk.is_downsampled}
					<div class="info-bar">
						Showing {$chunk.time.length.toLocaleString()} of {$chunk.original_count.toLocaleString()} points (downsampled)
					</div>
				{/if}
			{:else}
				<div class="welcome">
					<p>Open a CSV file to start analyzing vibration data</p>
					<p class="hint">File should contain columns: time, x, y, z</p>
				</div>
			{/if}
		</div>

		<aside class="sidebar">
			<AnnotationPanel />
		</aside>
	</div>

	{#if showAnnotationDialog}
		<AnnotationDialog
			on:confirm={handleAnnotationConfirm}
			on:cancel={() => (showAnnotationDialog = false)}
		/>
	{/if}
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
