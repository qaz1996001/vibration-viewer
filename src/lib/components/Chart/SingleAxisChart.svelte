<script lang="ts">
	import { onMount } from 'svelte';
	import * as echarts from 'echarts';
	import type { TimeseriesChunk } from '$lib/types/vibration';
	import { createSingleAxisOption } from './chartOptions';

	interface Props {
		channelName: string;
		chunk: TimeseriesChunk;
	}

	let { channelName, chunk }: Props = $props();

	let chartContainer: HTMLDivElement;
	let chartInstance: echarts.ECharts | null = $state(null);

	onMount(() => {
		const instance = echarts.init(chartContainer);
		chartInstance = instance;

		const resizeObserver = new ResizeObserver(() => {
			instance.resize();
		});
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			instance.dispose();
		};
	});

	$effect(() => {
		const chart = chartInstance;
		if (!chart || !chunk) return;

		const option = createSingleAxisOption(chunk, channelName);
		chart.setOption(option, { notMerge: true });
	});
</script>

<div bind:this={chartContainer} class="chart-container"></div>

<style>
	.chart-container {
		width: 100%;
		height: 300px;
		min-height: 300px;
	}
</style>
