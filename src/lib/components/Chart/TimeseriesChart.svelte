<script lang="ts">
	import { onMount } from 'svelte';
	import * as echarts from 'echarts';
	import { chunks, datasets, datasetOrder, globalTimeRange } from '$lib/stores/dataStore';
	import { annotations } from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick } from '$lib/stores/modeStore';
	import { createOverviewOption } from './chartOptions';

	interface Props {
		ondatazoom?: (data: { start: number; end: number }) => void;
		onannotatepoint?: (data: { time: number; value: number }) => void;
		onannotaterange?: (data: { startTime: number; endTime: number }) => void;
	}

	let { ondatazoom, onannotatepoint, onannotaterange }: Props = $props();

	let chartContainer: HTMLDivElement;
	let chartInstance: echarts.ECharts | null = $state(null);

	// Non-reactive zoom state — plain variable so changes don't trigger $effect
	let currentZoom = { start: 0, end: 100 };

	onMount(() => {
		const instance = echarts.init(chartContainer);
		chartInstance = instance;

		instance.on('datazoom', (params: any) => {
			const start = params.start ?? params.batch?.[0]?.start;
			const end = params.end ?? params.batch?.[0]?.end;
			if (start === undefined || end === undefined) return;
			// Guard: skip if values match current state (prevents setOption feedback loop)
			if (Math.abs(start - currentZoom.start) < 0.01 && Math.abs(end - currentZoom.end) < 0.01) return;
			currentZoom = { start, end };
			ondatazoom?.({ start, end });
		});

		instance.getZr().on('click', (params: any) => {
			const currentMode = $mode;
			if (currentMode !== 'annotate_point' && currentMode !== 'annotate_range') return;

			const pointInPixel = [params.offsetX, params.offsetY];
			const pointInGrid = instance.convertFromPixel('grid', pointInPixel);
			if (!pointInGrid) return;

			const timeValue = pointInGrid[0];
			const yValue = pointInGrid[1];

			if (currentMode === 'annotate_point') {
				onannotatepoint?.({
					time: timeValue,
					value: yValue
				});
			} else if (currentMode === 'annotate_range') {
				const first = $rangeFirstClick;
				if (first === null) {
					rangeFirstClick.set(timeValue);
				} else {
					const startTime = Math.min(first, timeValue);
					const endTime = Math.max(first, timeValue);
					rangeFirstClick.set(null);
					onannotaterange?.({ startTime, endTime });
				}
			}
		});

		const resizeObserver = new ResizeObserver(() => {
			instance.resize();
		});
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
