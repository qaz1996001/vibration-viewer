<!--
	TimeseriesChart — 多文件叠加总览图表组件（核心视图）。

	职责：
	- 将所有已加载数据集的降采样 chunk 叠加渲染为折线图
	- 显示标注标记（markPoint/markArea/markLine）
	- 转发 dataZoom、annotation 创建/更新事件给父组件
	- 响应外部 zoomTo 指令（如点击标注面板跳转）

	初始化步骤（编号对应下方函数）：
	1. initChart — 创建 ECharts 实例
	2. setupDataZoomHandler — 监听缩放并通知父组件
	3. setupClickHandler — 标注创建（点击）
	4. setupBrushHandler — 标注拖拽（范围边界 + 标签偏移）
	5. setupResizeHandler — 容器尺寸自适应
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { get } from 'svelte/store';
	import * as echarts from 'echarts';
	import { chunks, datasets, datasetOrder, globalTimeRange, fileColors } from '$lib/stores/dataStore';
	import { annotations, selectedId } from '$lib/stores/annotationStore';
	import { mode, rangeFirstClick, mergeSeriesMode } from '$lib/stores/uiStore';
	import { createOverviewOption } from './chartOptions';
	import { createChartInstance, disposeChart, setupResizeObserver } from '$lib/utils/useChart';
	import {
		setupAnnotationClickHandler,
		setupAnnotationBrushHandler,
		setupLabelDragHandler
	} from '$lib/utils/useAnnotationInteraction';

	/** 组件 Props — 全部为可选的事件回调和外部控制 */
	interface Props {
		/** dataZoom 变化时触发，百分比范围 */
		ondatazoom?: (data: { start: number; end: number }) => void;
		/** 点标注创建时触发 */
		onannotatepoint?: (data: { time: number; value: number }) => void;
		/** 范围标注创建时触发 */
		onannotaterange?: (data: { startTime: number; endTime: number }) => void;
		/** 标注拖拽更新时触发 */
		onupdateannotation?: (data: { id: string; updates: Record<string, unknown> }) => void;
		/** 外部缩放指令（如标注面板的「跳转」功能） */
		zoomTo?: { start: number; end: number } | null;
	}

	let { ondatazoom, onannotatepoint, onannotaterange, onupdateannotation, zoomTo = null }: Props = $props();

	let chartContainer: HTMLDivElement;
	/** $state: ECharts 实例引用 */
	let chartInstance: echarts.ECharts | null = $state(null);

	// 非响应式缩放状态 — 用普通变量避免每次缩放都触发 $effect 重渲染
	let currentZoom = { start: 0, end: 100 };

	// Legend 选中状态 — 跨重渲染保留，避免用户切换 legend 后被覆盖
	let legendSelected: Record<string, boolean> = {};

	// 判断标注拖拽是否正在进行 — 拖拽期间忽略 dataZoom 事件防止冲突
	let isDragging: () => boolean = () => false;

	/** 1. 创建 ECharts 实例并保存引用 */
	function initChart(): echarts.ECharts {
		const instance = createChartInstance(chartContainer);
		chartInstance = instance;
		return instance;
	}

	/** 2. 监听 dataZoom 事件并转发给父组件；同时捕获 legend 选中状态变化 */
	function setupDataZoomHandler(chart: echarts.ECharts): void {
		chart.on('datazoom', (...args: unknown[]) => {
			const params = args[0] as { start?: number; end?: number; batch?: Array<{ start: number; end: number }> };
			// 拖拽标注时忽略 dataZoom，避免拖拽过程中触发数据重载
			if (isDragging()) return;
			const start = params.start ?? params.batch?.[0]?.start;
			const end = params.end ?? params.batch?.[0]?.end;
			if (start === undefined || end === undefined) return;
			// 小数精度过滤，防止浮点抖动引起不必要的回调
			if (Math.abs(start - currentZoom.start) < 0.01 && Math.abs(end - currentZoom.end) < 0.01) return;
			currentZoom = { start, end };
			ondatazoom?.({ start, end });
		});

		chart.on('legendselectchanged', (...args: unknown[]) => {
			const params = args[0] as { selected: Record<string, boolean> };
			legendSelected = { ...params.selected };
		});
	}

	/** 3. 设置点击处理：根据当前 mode 创建点标注或范围标注 */
	function setupClickHandler(chart: echarts.ECharts): void {
		setupAnnotationClickHandler(
			chart,
			(data) => onannotatepoint?.(data),
			(data) => onannotaterange?.(data)
		);
	}

	/** 4. 设置拖拽处理：范围标注边界拖拽 + 点标注标签位移 */
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

	/** 5. 容器 resize 自适应。返回清理函数。 */
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

	/** Escape 键取消范围标注的第一次点击 */
	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape' && $rangeFirstClick !== null) {
			rangeFirstClick.set(null);
		}
	}

	// $effect: 核心渲染 — 当任何 store 数据变化时重新构建 ECharts option 并应用。
	// 读取所有相关 store 值以建立依赖追踪，任一变化都会触发重渲染。
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

	// $effect: 响应外部缩放指令（如标注面板点击跳转到标注位置）
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

<div bind:this={chartContainer} class="chart-container" role="img" aria-label="Vibration timeseries overview chart"></div>

<style>
	.chart-container {
		width: 100%;
		height: 400px;
		min-height: 400px;
	}
</style>
