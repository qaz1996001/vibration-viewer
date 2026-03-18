<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import * as echarts from 'echarts';
	import { chunks, datasets, datasetOrder, globalTimeRange, fileColors } from '$lib/stores/dataStore';
	import { annotations, selectedId } from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick } from '$lib/stores/modeStore';
	import { mergeSeriesMode } from '$lib/stores/uiStore';
	import { createOverviewOption } from './chartOptions';
	import { createChartInstance, disposeChart, setupResizeObserver } from '$lib/utils/useChart';
	import {
		setupAnnotationClickHandler,
		setupAnnotationBrushHandler,
		setupLabelDragHandler
	} from '$lib/utils/useAnnotationInteraction';

	interface Props {
		ondatazoom?: (data: { start: number; end: number }) => void;
		onannotatepoint?: (data: { time: number; value: number }) => void;
		onannotaterange?: (data: { startTime: number; endTime: number }) => void;
		onupdateannotation?: (data: { id: string; updates: Record<string, unknown> }) => void;
		zoomTo?: { start: number; end: number } | null;
	}

	let { ondatazoom, onannotatepoint, onannotaterange, onupdateannotation, zoomTo = null }: Props = $props();

	let chartContainer: HTMLDivElement;
	let chartInstance: echarts.ECharts | null = $state(null);

	// Non-reactive zoom state — plain variable so changes don't trigger $effect
	let currentZoom = { start: 0, end: 100 };

	// Legend selection state — preserved across re-renders
	let legendSelected: Record<string, boolean> = {};

	// Tracks whether the annotation brush handler is in a drag
	let isDragging: () => boolean = () => false;

	/** 1. Create the ECharts instance and store it. */
	function initChart(): echarts.ECharts {
		const instance = createChartInstance(chartContainer);
		chartInstance = instance;
		return instance;
	}

	/** 2. Listen for dataZoom events and forward to parent. */
	function setupDataZoomHandler(chart: echarts.ECharts): void {
		chart.on('datazoom', (...args: unknown[]) => {
			const params = args[0] as { start?: number; end?: number; batch?: Array<{ start: number; end: number }> };
			if (isDragging()) return;
			const start = params.start ?? params.batch?.[0]?.start;
			const end = params.end ?? params.batch?.[0]?.end;
			if (start === undefined || end === undefined) return;
			if (Math.abs(start - currentZoom.start) < 0.01 && Math.abs(end - currentZoom.end) < 0.01) return;
			currentZoom = { start, end };
			ondatazoom?.({ start, end });
		});

		chart.on('legendselectchanged', (...args: unknown[]) => {
			const params = args[0] as { selected: Record<string, boolean> };
			legendSelected = { ...params.selected };
		});
	}

	/** 3. Set up click handler for point/range annotation creation. */
	function setupClickHandler(chart: echarts.ECharts): void {
		setupAnnotationClickHandler(
			chart,
			(data) => onannotatepoint?.(data),
			(data) => onannotaterange?.(data)
		);
	}

	/** 4. Set up drag handlers for annotation interaction (range boundaries + point label repositioning). */
	function setupBrushHandler(chart: echarts.ECharts): void {
		const brushHandle = setupAnnotationBrushHandler(
			chart,
			chartContainer,
			(data) => onupdateannotation?.(data)
		);
		const labelHandle = setupLabelDragHandler(
			chart,
			chartContainer,
			(data) => onupdateannotation?.(data)
		);
		isDragging = () => brushHandle.isDragging() || labelHandle.isDragging();
	}

	/** 5. Observe container resize and auto-resize the chart. Returns cleanup fn. */
	function setupResizeHandler(chart: echarts.ECharts): () => void {
		return setupResizeObserver(chart, chartContainer);
	}

	onMount(() => {
		const chart = initChart();
		setupDataZoomHandler(chart);
		setupClickHandler(chart);
		setupBrushHandler(chart);
		const cleanup = setupResizeHandler(chart);

		return () => {
			cleanup();
			disposeChart(chart);
		};
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && $rangeFirstClick !== null) {
			rangeFirstClick.set(null);
		}
	}

	$effect(() => {
		const currentChunks = $chunks;
		const currentDatasets = $datasets;
		const currentOrder = $datasetOrder;
		const currentAnnotations = $annotations;
		const currentPendingLine = $rangeFirstClick;
		const currentGlobalRange = $globalTimeRange;
		const currentSelectedId = $selectedId;
		const currentFileColors = $fileColors;
		const currentMergeMode = $mergeSeriesMode;
		const chart = chartInstance;

		if (!chart || currentOrder.length === 0) return;

		const option = createOverviewOption(
			currentChunks,
			currentDatasets,
			currentOrder,
			currentAnnotations,
			currentPendingLine,
			{
				globalTimeRange: currentGlobalRange,
				zoomStart: currentZoom.start,
				zoomEnd: currentZoom.end
			},
			{
				legendSelected,
				fileColors: currentFileColors,
				selectedAnnotationId: currentSelectedId,
				mergeSeriesMode: currentMergeMode
			}
		);
		chart.setOption(option, { notMerge: true });
	});

	// Respond to external zoom commands (e.g., jump-to-annotation)
	$effect(() => {
		if (zoomTo && chartInstance) {
			currentZoom = { start: zoomTo.start, end: zoomTo.end };
			chartInstance.dispatchAction({
				type: 'dataZoom',
				start: zoomTo.start,
				end: zoomTo.end
			});
		}
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<div bind:this={chartContainer} class="chart-container"></div>

<style>
	.chart-container {
		width: 100%;
		height: 400px;
		min-height: 400px;
	}
</style>
