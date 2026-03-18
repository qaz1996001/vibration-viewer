import type { EChartsOption } from 'echarts';
import type { TimeseriesChunk, VibrationDataset } from '$lib/types/vibration';
import type { Annotation } from '$lib/types/annotation';
import { formatTime } from '$lib/utils/formatTime';
import { getChannelColor } from '$lib/constants/colors';

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
	zoomOptions?: {
		globalTimeRange?: [number, number] | null;
		zoomStart?: number;
		zoomEnd?: number;
	},
	interactionState?: {
		legendSelected?: Record<string, boolean>;
		fileColors?: Record<string, string>;
		selectedAnnotationId?: string | null;
	}
): EChartsOption {
	const isMultiFile = datasetOrder.length > 1;
	const legendSel = interactionState?.legendSelected;
	const fColors = interactionState?.fileColors;
	const selAnnId = interactionState?.selectedAnnotationId;

	// Build markLine data: pending range first-click line
	const markLineData: any[] = [];
	if (pendingRangeLine !== null) {
		markLineData.push({
			xAxis: pendingRangeLine,
			lineStyle: { color: '#ff4444', type: 'dashed', width: 2 },
			label: { show: true, formatter: 'Start', position: 'insideStartTop' }
		});
	}

	// Add drag-handle lines for selected range annotation
	const handleLines = buildSelectedRangeHandles(annotations, selAnnId);
	const allMarkLineData = [...markLineData, ...handleLines];

	const series: any[] = [];
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
			const pairedData = chunk.time.map((t, i) => [t, values[i]]);
			const seriesName = isMultiFile ? `${ds.file_name}:${channelName}` : channelName;

			legendData.push(seriesName);

			const channelColor = fileColor ?? getChannelColor(colorIdx);

			const baseSeries: any = {
				name: seriesName,
				type: 'line',
				data: pairedData,
				symbol: 'none',
				lineStyle: { width: 1 },
				itemStyle: { color: channelColor }
			};

			// First series gets annotations
			if (firstSeries) {
				baseSeries.markPoint = {
					data: buildMarkPoints(annotations),
					animation: false
				};
				baseSeries.markArea = {
					data: buildMarkAreas(annotations, selAnnId) as any
				};
				if (allMarkLineData.length > 0) {
					baseSeries.markLine = {
						data: allMarkLineData,
						silent: true,
						symbol: 'none',
						animation: false
					};
				}
				firstSeries = false;
			}

			series.push(baseSeries);
			colorIdx++;
		}
	}

	return {
		tooltip: {
			trigger: 'axis',
			axisPointer: { type: 'cross' },
			formatter: (params: any) => {
				if (!Array.isArray(params) || params.length === 0) return '';
				const time = params[0].value?.[0];
				const timeStr = time !== undefined ? formatTime(time) : '';
				let html = `<strong>${timeStr}</strong><br/>`;
				for (const p of params) {
					const val = p.value?.[1];
					html += `${p.marker} ${p.seriesName}: ${val !== undefined ? Number(val).toFixed(6) : 'N/A'}<br/>`;
				}
				return html;
			}
		},
		legend: {
			data: legendData,
			type: legendData.length > 6 ? 'scroll' : 'plain',
			bottom: 0,
			...(legendSel ? { selected: legendSel } : {})
		},
		toolbox: {
			feature: {
				dataZoom: { title: { zoom: 'Box Zoom', back: 'Undo Zoom' } },
				restore: { title: 'Reset' }
			},
			right: 20,

		},
		grid: {
			left: '3%',
			right: '4%',
			top: 60,
			bottom: '15%',
			containLabel: true
		},
		dataZoom: [
			{
				type: 'slider',
				xAxisIndex: 0,
				start: zoomOptions?.zoomStart ?? 0,
				end: zoomOptions?.zoomEnd ?? 100,
				bottom: '8%',
			},
			{ type: 'inside', xAxisIndex: 0 }
		],
		xAxis: {

			type: 'value',
			name: 'Time',
			axisLabel: {
				formatter: (v: number) => formatTime(v),
				rotate: 30,
				fontSize: 10
			},
			min: zoomOptions?.globalTimeRange ? zoomOptions.globalTimeRange[0] : 'dataMin',
			max: zoomOptions?.globalTimeRange ? zoomOptions.globalTimeRange[1] : 'dataMax'
		},
		yAxis: {
			type: 'value',
			name: 'Vibration'
		},
		series
	};
}

export function createSingleAxisOption(
	chunk: TimeseriesChunk,
	channelName: string
): EChartsOption {
	const data = chunk.channels[channelName];
	const channelNames = Object.keys(chunk.channels);
	const colorIdx = channelNames.indexOf(channelName);
	const color = getChannelColor(colorIdx >= 0 ? colorIdx : 0);
	const pairedData = chunk.time.map((t, i) => [t, data[i]]);

	return {
		title: {
			text: `${channelName} channel`,
			textStyle: { fontSize: 14 }
		},
		tooltip: {
			trigger: 'axis',
			formatter: (params: any) => {
				const p = Array.isArray(params) ? params[0] : params;
				if (!p) return '';
				const time = p.value?.[0];
				const val = p.value?.[1];
				const timeStr = time !== undefined ? formatTime(time) : '';
				return `<strong>${timeStr}</strong><br/>${p.marker} ${channelName}: ${val !== undefined ? Number(val).toFixed(6) : 'N/A'}`;
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

/** Map a single Point annotation to ECharts markPoint data format. */
function mapPointToMarkData(a: Annotation) {
	const pt = a.annotation_type as { type: 'Point'; time: number; value: number; axis: string };
	return {
		coord: [pt.time, pt.value] as [number, number],
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
function buildMarkPoints(annotations: Annotation[]): any[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point')
		.map(mapPointToMarkData);
}

/** For SingleAxisChart: filter by specific channel axis. */
export function buildMarkPointsForAxis(annotations: Annotation[], axis: string): any[] {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point' && a.annotation_type.axis === axis)
		.map(mapPointToMarkData);
}

function buildMarkAreas(
	annotations: Annotation[],
	selectedId?: string | null
): Array<[any, any]> {
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
			];
		});
}

function buildSelectedRangeHandles(
	annotations: Annotation[],
	selectedId: string | null | undefined
): any[] {
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
			label: { show: true, formatter: '◂', position: 'start', color: ann.color, fontSize: 14 }
		},
		{
			xAxis: range.end_time,
			lineStyle: { color: ann.color, width: 4, type: 'solid' },
			label: { show: true, formatter: '▸', position: 'start', color: ann.color, fontSize: 14 }
		}
	];
}
