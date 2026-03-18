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
// Shared types for option-building parameters
// ---------------------------------------------------------------------------

export interface ZoomOptions {
	globalTimeRange?: [number, number] | null;
	zoomStart?: number;
	zoomEnd?: number;
}

export interface InteractionState {
	legendSelected?: Record<string, boolean>;
	fileColors?: Record<string, string>;
	selectedAnnotationId?: string | null;
}

// ---------------------------------------------------------------------------
// 1.6 — Sub-functions extracted from createOverviewOption
// ---------------------------------------------------------------------------

/**
 * Build the ECharts series array from loaded datasets and their chunks.
 * Each dataset + channel = one line series.
 * The first series in the list carries annotation markPoint/markArea/markLine.
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
			const seriesName = isMultiFile ? `${ds.file_name}:${channelName}` : channelName;

			legendData.push(seriesName);

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
 * Build xAxis and yAxis configuration for the overview chart.
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
 * Build toolbox, legend, grid, and tooltip configuration.
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
				const items = params as Array<{
					value?: [number, number];
					marker?: string;
					seriesName?: string;
				}>;
				if (!Array.isArray(items) || items.length === 0) return '';
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
 * Build dataZoom components (slider + inside scroll).
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
// Main entry — thin wrapper calling sub-functions
// ---------------------------------------------------------------------------

/**
 * Multi-file overview chart option.
 * Each dataset + channel = one ECharts series.
 * Series name: "fileName:channelName" for multi-file, or just "channelName" for single file.
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

	// Build annotation marks
	const markLineData: MarkLineOption['data'] = [];
	if (pendingRangeLine !== null) {
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
// Single-axis chart option (unchanged interface)
// ---------------------------------------------------------------------------

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
// Annotation mark helpers
// ---------------------------------------------------------------------------

/** Coordinate pair for mark data. */
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
}

/** Map a single Point annotation to ECharts markPoint data format. */
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
		}
	};
}

/** Overview chart: show ALL point annotations (no axis filter). */
function buildMarkPoints(annotations: Annotation[]): MarkPointData[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point')
		.map(mapPointToMarkData);
}

/** For SingleAxisChart: filter by specific channel axis. */
export function buildMarkPointsForAxis(annotations: Annotation[], axis: string): MarkPointData[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point' && a.annotation_type.axis === axis)
		.map(mapPointToMarkData);
}

/** Mark area pair type: [start-item, end-item]. */
type MarkAreaPair = [
	{
		xAxis: number;
		name: string;
		itemStyle: { color: string; opacity: number };
		label: { show: boolean; position: string };
	},
	{ xAxis: number }
];

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
					label: { show: true, position: 'insideTop' }
				},
				{ xAxis: range.end_time }
			] as MarkAreaPair;
		});
}

/** Vertical handle lines shown at the edges of the selected range annotation. */
interface HandleLineData {
	xAxis: number;
	lineStyle: { color: string; width: number; type: 'solid' };
	label: { show: boolean; formatter: string; position: string; color: string; fontSize: number };
}

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
