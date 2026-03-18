<!--
	SingleAxisChart — 单通道独立图表组件。
	在「单轴视图」下为每个通道渲染一个独立的 ECharts 折线图，
	带有各自的 dataZoom 和坐标轴，不与其他通道叠加。
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import * as echarts from 'echarts';
	import type { TimeseriesChunk } from '$lib/types/vibration';
	import { createSingleAxisOption } from './chartOptions';

	/** @see Props.channelName 通道名称，@see Props.chunk 降采样数据 */
	interface Props {
		channelName: string;
		chunk: TimeseriesChunk;
	}

	let { channelName, chunk }: Props = $props();

	let chartContainer: HTMLDivElement;
	/** $state: ECharts 实例引用，onMount 后初始化 */
	let chartInstance: echarts.ECharts | null = $state(null);

	onMount(() => {
		const instance = echarts.init(chartContainer);
		chartInstance = instance;

		// 监听容器尺寸变化自动 resize
		const resizeObserver = new ResizeObserver(() => {
			instance.resize();
		});
		resizeObserver.observe(chartContainer);

		return () => {
			resizeObserver.disconnect();
			instance.dispose();
		};
	});

	// $effect: 当 chunk 或 channelName 变化时重新渲染图表
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
