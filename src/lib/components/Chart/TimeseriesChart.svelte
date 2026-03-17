<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import * as echarts from 'echarts';
	import { chunks, datasets, datasetOrder, globalTimeRange, fileColors } from '$lib/stores/dataStore';
	import { annotations, selectedId } from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick } from '$lib/stores/modeStore';
	import { createOverviewOption } from './chartOptions';

	interface Props {
		ondatazoom?: (data: { start: number; end: number }) => void;
		onannotatepoint?: (data: { time: number; value: number }) => void;
		onannotaterange?: (data: { startTime: number; endTime: number }) => void;
		onupdateannotation?: (data: { id: string; updates: Record<string, any> }) => void;
	}

	let { ondatazoom, onannotatepoint, onannotaterange, onupdateannotation }: Props = $props();

	let chartContainer: HTMLDivElement;
	let chartInstance: echarts.ECharts | null = $state(null);

	// Non-reactive zoom state — plain variable so changes don't trigger $effect
	let currentZoom = { start: 0, end: 100 };

	// Legend selection state — preserved across re-renders
	let legendSelected: Record<string, boolean> = {};

	// Range boundary drag state
	let draggingBoundary: 'start' | 'end' | null = null;
	let draggingAnnotationId: string | null = null;

	function handleDataZoom(instance: echarts.ECharts, params: any) {
		if (draggingBoundary) return;
		const start = params.start ?? params.batch?.[0]?.start;
		const end = params.end ?? params.batch?.[0]?.end;
		if (start === undefined || end === undefined) return;
		if (Math.abs(start - currentZoom.start) < 0.01 && Math.abs(end - currentZoom.end) < 0.01) return;
		currentZoom = { start, end };
		ondatazoom?.({ start, end });
	}

	function handleLegendChange(params: any) {
		legendSelected = { ...params.selected };
	}

	function handleChartClick(instance: echarts.ECharts, params: any) {
		const currentMode = get(mode);
		if (currentMode !== 'annotate_point' && currentMode !== 'annotate_range') return;

		const pointInPixel = [params.offsetX, params.offsetY];
		const pointInGrid = instance.convertFromPixel('grid', pointInPixel);
		if (!pointInGrid) return;

		const timeValue = pointInGrid[0];
		const yValue = pointInGrid[1];

		if (currentMode === 'annotate_point') {
			onannotatepoint?.({ time: timeValue, value: yValue });
		} else if (currentMode === 'annotate_range') {
			const first = get(rangeFirstClick);
			if (first === null) {
				rangeFirstClick.set(timeValue);
			} else {
				const startTime = Math.min(first, timeValue);
				const endTime = Math.max(first, timeValue);
				rangeFirstClick.set(null);
				onannotaterange?.({ startTime, endTime });
			}
		}
	}

	function handleBoundaryDragStart(instance: echarts.ECharts, params: any) {
		if (get(mode) !== 'browse') return;
		const selId = get(selectedId);
		if (!selId) return;

		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Range') return;

		const range = ann.annotation_type as { type: 'Range'; start_time: number; end_time: number };
		const startPx = instance.convertToPixel('grid', [range.start_time, 0]);
		const endPx = instance.convertToPixel('grid', [range.end_time, 0]);
		if (!startPx || !endPx) return;

		const threshold = 12;
		if (Math.abs(params.offsetX - startPx[0]) < threshold) {
			draggingBoundary = 'start';
			draggingAnnotationId = ann.id;
		} else if (Math.abs(params.offsetX - endPx[0]) < threshold) {
			draggingBoundary = 'end';
			draggingAnnotationId = ann.id;
		}
	}

	function handleBoundaryDragEnd(instance: echarts.ECharts, params: any) {
		if (!draggingBoundary || !draggingAnnotationId) {
			draggingBoundary = null;
			draggingAnnotationId = null;
			return;
		}

		const dataPos = instance.convertFromPixel('grid', [params.offsetX, params.offsetY]);
		if (dataPos) {
			const anns = get(annotations);
			const ann = anns.find((a) => a.id === draggingAnnotationId);
			if (ann && ann.annotation_type.type === 'Range') {
				const range = ann.annotation_type as { type: 'Range'; start_time: number; end_time: number };
				if (draggingBoundary === 'start') {
					const newStart = Math.min(dataPos[0], range.end_time - 0.001);
					onupdateannotation?.({
						id: ann.id,
						updates: { annotation_type: { type: 'Range', start_time: newStart, end_time: range.end_time } }
					});
				} else {
					const newEnd = Math.max(dataPos[0], range.start_time + 0.001);
					onupdateannotation?.({
						id: ann.id,
						updates: { annotation_type: { type: 'Range', start_time: range.start_time, end_time: newEnd } }
					});
				}
			}
		}

		draggingBoundary = null;
		draggingAnnotationId = null;
	}

	function handleCursorUpdate(instance: echarts.ECharts, params: any) {
		if (draggingBoundary) return;
		const selId = get(selectedId);
		if (!selId || get(mode) !== 'browse') {
			chartContainer.style.cursor = '';
			return;
		}

		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Range') {
			chartContainer.style.cursor = '';
			return;
		}

		const range = ann.annotation_type as { type: 'Range'; start_time: number; end_time: number };
		const startPx = instance.convertToPixel('grid', [range.start_time, 0]);
		const endPx = instance.convertToPixel('grid', [range.end_time, 0]);

		if (startPx && endPx) {
			const threshold = 12;
			if (
				Math.abs(params.offsetX - startPx[0]) < threshold ||
				Math.abs(params.offsetX - endPx[0]) < threshold
			) {
				chartContainer.style.cursor = 'ew-resize';
			} else {
				chartContainer.style.cursor = '';
			}
		}
	}

	onMount(() => {
		const instance = echarts.init(chartContainer);
		chartInstance = instance;

		instance.on('datazoom', (params: any) => handleDataZoom(instance, params));
		instance.on('legendselectchanged', (params: any) => handleLegendChange(params));
		instance.getZr().on('click', (params: any) => handleChartClick(instance, params));
		instance.getZr().on('mousedown', (params: any) => handleBoundaryDragStart(instance, params));
		instance.getZr().on('mouseup', (params: any) => handleBoundaryDragEnd(instance, params));
		instance.getZr().on('mousemove', (params: any) => handleCursorUpdate(instance, params));

		const resizeObserver = new ResizeObserver(() => instance.resize());
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			instance.dispose();
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
				selectedAnnotationId: currentSelectedId
			}
		);
		chart.setOption(option, { notMerge: true });
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
