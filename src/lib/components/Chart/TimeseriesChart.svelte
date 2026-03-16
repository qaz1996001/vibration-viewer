<script lang="ts">
	import { onMount } from 'svelte';
	import * as echarts from 'echarts';
	import { chunk } from '$lib/stores/dataStore';
	import { annotations } from '$lib/stores/annotationStore';
	import { mode } from '$lib/stores/modeStore';
	import { createOverviewOption } from './chartOptions';
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher<{
		datazoom: { start: number; end: number };
		'annotate-point': { time: number; value: number };
		'annotate-range': { startTime: number; endTime: number };
	}>();

	let chartContainer: HTMLDivElement;
	let chart: echarts.ECharts | null = null;

	onMount(() => {
		chart = echarts.init(chartContainer);

		chart.on('datazoom', handleDataZoom);
		chart.getZr().on('click', handleChartClick);
		chart.on('brushSelected', handleBrushSelected);

		const resizeObserver = new ResizeObserver(() => {
			chart?.resize();
		});
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			chart?.dispose();
		};
	});

	$: if (chart && $chunk) {
		const option = createOverviewOption($chunk, $annotations);
		configureBrush(option, $mode);
		chart.setOption(option, { notMerge: false });
	}

	function configureBrush(option: echarts.EChartsOption, currentMode: string) {
		if (currentMode === 'annotate_range') {
			(option as any).brush = {
				toolbox: ['rect'],
				xAxisIndex: 0,
				brushStyle: { borderWidth: 1, color: 'rgba(255,107,107,0.2)' }
			};
		} else {
			(option as any).brush = { toolbox: [] };
		}
	}

	function handleDataZoom(params: any) {
		dispatch('datazoom', {
			start: params.start ?? params.batch?.[0]?.start ?? 0,
			end: params.end ?? params.batch?.[0]?.end ?? 100
		});
	}

	function handleChartClick(params: any) {
		if ($mode !== 'annotate_point') return;
		if (!chart) return;

		const pointInPixel = [params.offsetX, params.offsetY];
		const pointInGrid = chart.convertFromPixel('grid', pointInPixel);
		if (!pointInGrid) return;

		dispatch('annotate-point', {
			time: pointInGrid[0],
			value: pointInGrid[1]
		});
	}

	function handleBrushSelected(params: any) {
		if ($mode !== 'annotate_range') return;

		const areas = params.batch?.[0]?.areas;
		if (!areas || areas.length === 0) return;

		const range = areas[0].coordRange;
		dispatch('annotate-range', {
			startTime: range[0],
			endTime: range[1]
		});
	}
</script>

<div bind:this={chartContainer} class="chart-container"></div>

<style>
	.chart-container {
		width: 100%;
		height: 400px;
	}
</style>
