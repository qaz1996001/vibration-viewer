/**
 * ECharts option 构建模块 — 将数据集、标注、交互状态转换为 ECharts 配置对象。
 *
 * 架构：
 * - createOverviewOption: 多文件叠加总览图（主入口）
 * - createSingleAxisOption: 单通道独立图
 * - createSeriesConfig / createAxisConfig / createToolboxConfig / createDataZoomConfig: 子构建器
 * - buildMarkPoints / buildMarkAreas / buildSelectedRangeHandles: 标注标记转换
 *
 * 所有函数为纯函数（无副作用），便于测试。
 */
import type {
	EChartsOption,
	LineSeriesOption,
	DataZoomComponentOption,
	LegendComponentOption,
	ToolboxComponentOption,
	TooltipOption,
	XAXisOption,
	YAXisOption,
	GridOption,
	MarkPointOption,
	MarkLineOption,
	MarkAreaOption
} from 'echarts/types/dist/shared';
import type { TimeseriesChunk, VibrationDataset } from '$lib/types/vibration';
import type { Annotation } from '$lib/types/annotation';
import { formatTime } from '$lib/utils/formatTime';
import { getChannelColor } from '$lib/constants/colors';

// ---------------------------------------------------------------------------
// 共享参数类型
// ---------------------------------------------------------------------------

/** 缩放控制参数 — 传递给 createOverviewOption 控制 dataZoom 和 xAxis 范围 */
export interface ZoomOptions {
	/** 全局时间范围（所有数据集的并集），设置 xAxis min/max */
	globalTimeRange?: [number, number] | null;
	/** dataZoom 起始百分比 (0-100) */
	zoomStart?: number;
	/** dataZoom 结束百分比 (0-100) */
	zoomEnd?: number;
}

/** 交互状态参数 — 控制 legend 选中、文件颜色、标注高亮、系列合并 */
export interface InteractionState {
	/** legend 各系列的选中/隐藏状态 */
	legendSelected?: Record<string, boolean>;
	/** 每个数据集的颜色覆盖 */
	fileColors?: Record<string, string>;
	/** 当前选中的标注 ID — 选中的范围标注会加粗边界线 */
	selectedAnnotationId?: string | null;
	/** 多文件合并模式 — 启用时系列名不含文件名前缀 */
	mergeSeriesMode?: boolean;
}

// ---------------------------------------------------------------------------
// 子构建器 — 从 createOverviewOption 中抽取的独立构建函数
// ---------------------------------------------------------------------------

/**
 * 构建 ECharts series 数组 — 每个「数据集 x 通道」生成一条折线系列。
 *
 * 命名规则：
 * - 多文件模式（非合并）：`{fileName}:{channelName}`
 * - 单文件或合并模式：`{channelName}`
 *
 * 标注标记（markPoint/markArea/markLine）只挂载在第一个 series 上，
 * 避免标注在多个 series 上重复渲染。
 *
 * @param allChunks - 所有数据集的 chunk 映射
 * @param allDatasets - 所有数据集的元信息映射
 * @param datasetOrder - 数据集渲染顺序
 * @param annotationMarks - 标注标记配置
 * @param interactionState - 交互状态（颜色、合并模式等）
 * @returns series 数组和 legend 数据名列表
 */
export function createSeriesConfig(
	allChunks: Record<string, TimeseriesChunk>,
	allDatasets: Record<string, VibrationDataset>,
	datasetOrder: string[],
	annotationMarks: {
		markPoint: MarkPointOption;
		markArea: MarkAreaOption;
		markLine: MarkLineOption | null;
	},
	interactionState?: InteractionState
): { series: LineSeriesOption[]; legendData: string[] } {
	const isMultiFile = datasetOrder.length > 1;
	const mergeMode = interactionState?.mergeSeriesMode ?? false;
	const fColors = interactionState?.fileColors;

	const series: LineSeriesOption[] = [];
	const legendData: string[] = [];
	let colorIdx = 0;
	let firstSeries = true;

	for (const dsId of datasetOrder) {
		const chunk = allChunks[dsId];
		const ds = allDatasets[dsId];
		if (!chunk || !ds) continue;

		const fileColor = fColors?.[dsId];
		const channelNames = ds.column_mapping.data_columns.filter(
			(col) => col in chunk.channels
		);

		for (const channelName of channelNames) {
			const values = chunk.channels[channelName];
			const pairedData: [number, number][] = chunk.time.map((t, i) => [t, values[i]]);
			const seriesName = (isMultiFile && !mergeMode)
				? `${ds.file_name}:${channelName}`
				: channelName;

			if (!legendData.includes(seriesName)) {
				legendData.push(seriesName);
			}

			const channelColor = fileColor ?? getChannelColor(colorIdx);

			const baseSeries: LineSeriesOption = {
				name: seriesName,
				type: 'line',
				data: pairedData,
				symbol: 'none',
				lineStyle: { width: 1 },
				itemStyle: { color: channelColor }
			};

			// First series gets annotations
			if (firstSeries) {
				baseSeries.markPoint = annotationMarks.markPoint;
				baseSeries.markArea = annotationMarks.markArea;
				if (annotationMarks.markLine) {
					baseSeries.markLine = annotationMarks.markLine;
				}
				firstSeries = false;
			}

			series.push(baseSeries);
			colorIdx++;
		}
	}

	return { series, legendData };
}

/**
 * 构建 xAxis / yAxis 配置。
 * xAxis 使用 value 类型（epoch seconds）而非 time 类型，以支持多文件时间对齐。
 * @param globalTimeRange - 全局时间范围，null 时使用 'dataMin'/'dataMax' 自适应
 * @returns xAxis 和 yAxis 配置对象
 */
export function createAxisConfig(
	globalTimeRange?: [number, number] | null
): { xAxis: XAXisOption; yAxis: YAXisOption } {
	return {
		xAxis: {
			type: 'value',
			name: 'Time',
			axisLabel: {
				formatter: (v: number) => formatTime(v),
				rotate: 30,
				fontSize: 10
			},
			min: globalTimeRange ? globalTimeRange[0] : 'dataMin',
			max: globalTimeRange ? globalTimeRange[1] : 'dataMax'
		},
		yAxis: {
			type: 'value',
			name: 'Vibration'
		}
	};
}

/**
 * 构建 tooltip、legend、toolbox、grid 四项 UI 配置。
 * - tooltip: 自定义 formatter 区分标注项和普通数据轴
 * - legend: 超过 6 项时自动切换为 scroll 类型
 * - toolbox: 提供框选缩放和重置功能
 * @param legendData - legend 显示的系列名列表
 * @param legendSelected - 可选的 legend 选中状态（保留用户切换）
 * @returns tooltip、legend、toolbox、grid 配置对象
 */
export function createToolboxConfig(
	legendData: string[],
	legendSelected?: Record<string, boolean>
): {
	tooltip: TooltipOption;
	legend: LegendComponentOption;
	toolbox: ToolboxComponentOption;
	grid: GridOption;
} {
	return {
		tooltip: {
			trigger: 'axis',
			axisPointer: { type: 'cross' },
			formatter: (params: unknown) => {
				// 非数组 params 表示 hover 在 markPoint/markArea 上，需特殊处理
				if (!Array.isArray(params)) {
					const p = params as {
						componentType?: string;
						name?: string;
						value?: [number, number] | number;
						data?: {
							coord?: [number, number];
							xAxis?: number;
							name?: string;
						};
					};
					if (p.componentType === 'markPoint') {
						const coord = p.data?.coord;
						const timeStr = coord ? formatTime(coord[0]) : '';
						const valStr = coord ? Number(coord[1]).toFixed(6) : '';
						return `<strong>${p.name ?? ''}</strong><br/>Time: ${timeStr}<br/>Value: ${valStr}<br/><em>Point annotation</em>`;
					}
					if (p.componentType === 'markArea') {
						return `<strong>${p.name ?? ''}</strong><br/><em>Range annotation</em>`;
					}
					return '';
				}
				const items = params as Array<{
					value?: [number, number];
					marker?: string;
					seriesName?: string;
				}>;
				if (items.length === 0) return '';
				const time = items[0].value?.[0];
				const timeStr = time !== undefined ? formatTime(time) : '';
				let html = `<strong>${timeStr}</strong><br/>`;
				for (const p of items) {
					const val = p.value?.[1];
					html += `${p.marker ?? ''} ${p.seriesName ?? ''}: ${val !== undefined ? Number(val).toFixed(6) : 'N/A'}<br/>`;
				}
				return html;
			}
		},
		legend: {
			data: legendData,
			type: legendData.length > 6 ? 'scroll' : 'plain',
			bottom: 0,
			...(legendSelected ? { selected: legendSelected } : {})
		},
		toolbox: {
			feature: {
				dataZoom: { title: { zoom: 'Box Zoom', back: 'Undo Zoom' } },
				restore: { title: 'Reset' }
			},
			right: 20
		},
		grid: {
			left: '3%',
			right: '4%',
			top: 60,
			bottom: '15%',
			containLabel: true
		}
	};
}

/**
 * 构建 dataZoom 组件 — slider（底部滑块）+ inside（鼠标滚轮/拖拽缩放）。
 * @param start - 起始百分比 (0-100)
 * @param end - 结束百分比 (0-100)
 * @returns dataZoom 配置数组
 */
export function createDataZoomConfig(
	start: number,
	end: number
): DataZoomComponentOption[] {
	return [
		{
			type: 'slider',
			xAxisIndex: 0,
			start,
			end,
			bottom: '8%'
		},
		{ type: 'inside', xAxisIndex: 0 }
	];
}

// ---------------------------------------------------------------------------
// 主入口 — 组合子构建器输出完整 EChartsOption
// ---------------------------------------------------------------------------

/**
 * 多文件叠加总览图的完整 ECharts 配置构建器。
 *
 * 将数据集 chunk、标注、缩放状态、交互状态组合为一个完整的 EChartsOption。
 * 这是 TimeseriesChart 组件 $effect 中调用的核心函数。
 *
 * @param allChunks - 所有数据集的降采样 chunk
 * @param allDatasets - 所有数据集元信息
 * @param datasetOrder - 渲染顺序
 * @param annotations - 当前标注列表
 * @param pendingRangeLine - 范围标注模式下第一次点击的时间戳（显示虚线）
 * @param zoomOptions - 缩放控制参数
 * @param interactionState - 交互状态参数
 * @returns 完整的 EChartsOption
 */
export function createOverviewOption(
	allChunks: Record<string, TimeseriesChunk>,
	allDatasets: Record<string, VibrationDataset>,
	datasetOrder: string[],
	annotations: Annotation[],
	pendingRangeLine: number | null = null,
	zoomOptions?: ZoomOptions,
	interactionState?: InteractionState
): EChartsOption {
	const selAnnId = interactionState?.selectedAnnotationId;

	// 构建 markLine: 合并「范围标注待确认虚线」和「选中范围的边界手柄线」
	const markLineData: MarkLineOption['data'] = [];
	if (pendingRangeLine !== null) {
		// 范围标注第一次点击后显示红色虚线，提示用户第二次点击位置
		(markLineData as unknown[]).push({
			xAxis: pendingRangeLine,
			lineStyle: { color: '#ff4444', type: 'dashed' as const, width: 2 },
			label: { show: true, formatter: 'Start', position: 'insideStartTop' as const }
		});
	}
	const handleLines = buildSelectedRangeHandles(annotations, selAnnId);
	const allMarkLineData = [...(markLineData as unknown[]), ...handleLines];

	const markLine: MarkLineOption | null =
		allMarkLineData.length > 0
			? {
					data: allMarkLineData as MarkLineOption['data'],
					silent: true,
					symbol: 'none',
					animation: false
				}
			: null;

	const annotationMarks = {
		markPoint: {
			data: buildMarkPoints(annotations),
			animation: false
		} as MarkPointOption,
		markArea: {
			data: buildMarkAreas(annotations, selAnnId) as MarkAreaOption['data']
		} as MarkAreaOption,
		markLine
	};

	const { series, legendData } = createSeriesConfig(
		allChunks,
		allDatasets,
		datasetOrder,
		annotationMarks,
		interactionState
	);

	const { xAxis, yAxis } = createAxisConfig(zoomOptions?.globalTimeRange);
	const { tooltip, legend, toolbox, grid } = createToolboxConfig(
		legendData,
		interactionState?.legendSelected
	);
	const dataZoom = createDataZoomConfig(
		zoomOptions?.zoomStart ?? 0,
		zoomOptions?.zoomEnd ?? 100
	);

	return {
		tooltip,
		legend,
		toolbox,
		grid,
		dataZoom,
		xAxis,
		yAxis,
		series
	};
}

// ---------------------------------------------------------------------------
// 单通道独立图配置
// ---------------------------------------------------------------------------

/**
 * 单通道独立图的 ECharts 配置构建器。
 * 为指定通道渲染独立折线图，带 area 半透明填充和独立 dataZoom。
 * @param chunk - 该数据集的降采样 chunk（含多通道，仅取 channelName 对应的）
 * @param channelName - 要渲染的通道名
 * @returns 完整的 EChartsOption
 */
export function createSingleAxisOption(
	chunk: TimeseriesChunk,
	channelName: string
): EChartsOption {
	const data = chunk.channels[channelName];
	const channelNames = Object.keys(chunk.channels);
	const colorIdx = channelNames.indexOf(channelName);
	const color = getChannelColor(colorIdx >= 0 ? colorIdx : 0);
	const pairedData: [number, number][] = chunk.time.map((t, i) => [t, data[i]]);

	return {
		title: {
			text: `${channelName} channel`,
			textStyle: { fontSize: 14 }
		},
		tooltip: {
			trigger: 'axis',
			formatter: (params: unknown) => {
				const p = Array.isArray(params)
					? (params[0] as { value?: [number, number]; marker?: string })
					: (params as { value?: [number, number]; marker?: string });
				if (!p) return '';
				const time = p.value?.[0];
				const val = p.value?.[1];
				const timeStr = time !== undefined ? formatTime(time) : '';
				return `<strong>${timeStr}</strong><br/>${p.marker ?? ''} ${channelName}: ${val !== undefined ? Number(val).toFixed(6) : 'N/A'}`;
			}
		},
		toolbox: {
			feature: {
				dataZoom: { title: { zoom: 'Box Zoom', back: 'Undo Zoom' } },
				restore: { title: 'Reset' }
			},
			right: 20
		},
		grid: {
			left: '3%',
			right: '4%',
			bottom: '15%',
			containLabel: true
		},
		dataZoom: [
			{ type: 'slider', xAxisIndex: 0, start: 0, end: 100 },
			{ type: 'inside', xAxisIndex: 0 }
		],
		xAxis: {
			type: 'value',
			axisLabel: {
				formatter: (v: number) => formatTime(v),
				rotate: 30,
				fontSize: 10
			},
			min: 'dataMin',
			max: 'dataMax'
		},
		yAxis: {
			type: 'value',
			name: channelName
		},
		series: [
			{
				name: channelName,
				type: 'line',
				data: pairedData,
				symbol: 'none',
				lineStyle: { width: 1 },
				itemStyle: { color },
				areaStyle: { color, opacity: 0.05 }
			}
		]
	};
}

// ---------------------------------------------------------------------------
// 标注标记转换 — Annotation -> ECharts markPoint/markArea/markLine 数据
// ---------------------------------------------------------------------------

/** 点标注的 ECharts markPoint data 项类型 */
interface MarkPointData {
	coord: [number, number];
	name: string;
	symbol: string;
	symbolSize: number;
	itemStyle: {
		color: string;
		borderColor: string;
		borderWidth: number;
		shadowBlur: number;
		shadowColor: string;
	};
	label: {
		show: boolean;
		formatter: string;
		offset: [number, number];
		backgroundColor: string;
		borderColor: string;
		borderWidth: number;
		borderRadius: number;
		padding: [number, number];
		color: string;
		fontSize: number;
	};
	tooltip: {
		formatter: string;
	};
}

/**
 * 将单个 Point 标注转为 ECharts markPoint data 格式。
 * 包含 pin 图标、标签偏移、背景样式和自定义 tooltip。
 * @param a - Point 类型的标注
 * @returns markPoint 数据项
 */
function mapPointToMarkData(a: Annotation): MarkPointData {
	const pt = a.annotation_type as { type: 'Point'; time: number; value: number; axis: string };
	return {
		coord: [pt.time, pt.value],
		name: a.label,
		symbol: 'pin',
		symbolSize: 30,
		itemStyle: {
			color: a.color,
			borderColor: '#fff',
			borderWidth: 2,
			shadowBlur: 4,
			shadowColor: 'rgba(0,0,0,0.3)'
		},
		label: {
			show: true,
			formatter: a.label,
			offset: [a.label_offset_x, a.label_offset_y],
			backgroundColor: 'rgba(255,255,255,0.9)',
			borderColor: a.color,
			borderWidth: 1,
			borderRadius: 4,
			padding: [4, 8],
			color: '#333',
			fontSize: 12
		},
		tooltip: {
			formatter: `<strong>${a.label}</strong><br/>Time: ${formatTime(pt.time)}<br/>Value: ${Number(pt.value).toFixed(6)}<br/><em>Point annotation</em>`
		}
	};
}

/**
 * 构建总览图的点标注 markPoint 数据 — 显示所有 Point 标注（不按通道过滤）。
 * @param annotations - 标注列表
 * @returns markPoint data 数组
 */
function buildMarkPoints(annotations: Annotation[]): MarkPointData[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point')
		.map(mapPointToMarkData);
}

/**
 * 构建单轴图的点标注 markPoint 数据 — 仅包含属于指定通道的 Point 标注。
 * @param annotations - 标注列表
 * @param axis - 通道名（过滤条件）
 * @returns 过滤后的 markPoint data 数组
 */
export function buildMarkPointsForAxis(annotations: Annotation[], axis: string): MarkPointData[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point' && a.annotation_type.axis === axis)
		.map(mapPointToMarkData);
}

/** markArea 数据对类型：[起点配置, 终点配置]，定义一个高亮区间 */
type MarkAreaPair = [
	{
		xAxis: number;
		name: string;
		itemStyle: { color: string; opacity: number };
		label: { show: boolean; position: string; formatter: string; color: string; fontSize: number };
		tooltip: { formatter: string };
	},
	{ xAxis: number }
];

/**
 * 构建范围标注的 markArea 数据。
 * 选中的标注 opacity 加深（0.4 vs 0.2）以提供视觉反馈。
 * @param annotations - 标注列表
 * @param selectedId - 当前选中的标注 ID
 * @returns markArea data 数组
 */
function buildMarkAreas(
	annotations: Annotation[],
	selectedId?: string | null
): MarkAreaPair[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Range')
		.map((a) => {
			const range = a.annotation_type as {
				type: 'Range';
				start_time: number;
				end_time: number;
			};
			const isSelected = a.id === selectedId;
			return [
				{
					xAxis: range.start_time,
					name: a.label,
					itemStyle: {
						color: a.color,
						opacity: isSelected ? 0.4 : 0.2
					},
					label: {
						show: true,
						position: 'insideTop',
						formatter: a.label,
						color: '#333',
						fontSize: 11
					},
					tooltip: {
						formatter: `<strong>${a.label}</strong><br/>From: ${formatTime(range.start_time)}<br/>To: ${formatTime(range.end_time)}<br/><em>Range annotation</em>`
					}
				},
				{ xAxis: range.end_time }
			] as MarkAreaPair;
		});
}

/** 选中范围标注边界的拖拽手柄线数据类型 */
interface HandleLineData {
	xAxis: number;
	lineStyle: { color: string; width: number; type: 'solid' };
	label: { show: boolean; formatter: string; position: string; color: string; fontSize: number };
}

/**
 * 为选中的范围标注构建边界手柄线（竖直粗线 + 三角箭头）。
 * 仅当有选中的 Range 标注时返回两条线（起点 + 终点），否则返回空数组。
 * @param annotations - 标注列表
 * @param selectedId - 当前选中的标注 ID
 * @returns 手柄线 markLine data 数组（0 或 2 项）
 */
function buildSelectedRangeHandles(
	annotations: Annotation[],
	selectedId: string | null | undefined
): HandleLineData[] {
	if (!selectedId) return [];
	const ann = annotations.find((a) => a.id === selectedId);
	if (!ann || ann.annotation_type.type !== 'Range') return [];
	const range = ann.annotation_type as {
		type: 'Range';
		start_time: number;
		end_time: number;
	};
	return [
		{
			xAxis: range.start_time,
			lineStyle: { color: ann.color, width: 4, type: 'solid' },
			label: { show: true, formatter: '\u25C2', position: 'start', color: ann.color, fontSize: 14 }
		},
		{
			xAxis: range.end_time,
			lineStyle: { color: ann.color, width: 4, type: 'solid' },
			label: { show: true, formatter: '\u25B8', position: 'start', color: ann.color, fontSize: 14 }
		}
	];
}
