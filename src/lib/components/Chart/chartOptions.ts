import type { EChartsOption } from 'echarts';
import type { TimeseriesChunk } from '$lib/types/vibration';
import type { Annotation } from '$lib/types/annotation';

export function createOverviewOption(
	chunk: TimeseriesChunk,
	annotations: Annotation[]
): EChartsOption {
	return {
		tooltip: {
			trigger: 'axis',
			axisPointer: { type: 'cross' }
		},
		legend: {
			data: ['X', 'Y', 'Z']
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
			type: 'category',
			data: chunk.time.map((t) => t.toFixed(4)),
			name: 'Time',
			boundaryGap: false
		},
		yAxis: {
			type: 'value',
			name: 'Vibration'
		},
		series: [
			{
				name: 'X',
				type: 'line',
				data: chunk.x,
				symbol: 'none',
				lineStyle: { width: 1 },
				markPoint: { data: buildMarkPoints(annotations, 'x') },
				markArea: { data: buildMarkAreas(annotations) as any }
			},
			{
				name: 'Y',
				type: 'line',
				data: chunk.y,
				symbol: 'none',
				lineStyle: { width: 1 }
			},
			{
				name: 'Z',
				type: 'line',
				data: chunk.z,
				symbol: 'none',
				lineStyle: { width: 1 }
			}
		]
	};
}

function buildMarkPoints(
	annotations: Annotation[],
	axis: string
): Array<{ coord: [number, number]; name: string }> {
	return annotations
		.filter((a) => a.annotation_type.type === 'Point' && a.annotation_type.axis === axis)
		.map((a) => {
			const pt = a.annotation_type as { type: 'Point'; time: number; value: number; axis: string };
			return {
				coord: [pt.time, pt.value] as [number, number],
				name: a.label,
				itemStyle: { color: a.color },
				label: {
					show: true,
					formatter: a.label,
					offset: [a.label_offset_x, a.label_offset_y]
				}
			};
		});
}

function buildMarkAreas(
	annotations: Annotation[]
): Array<[{ xAxis: number; name: string }, { xAxis: number }]> {
	return annotations
		.filter((a) => a.annotation_type.type === 'Range')
		.map((a) => {
			const range = a.annotation_type as {
				type: 'Range';
				start_time: number;
				end_time: number;
			};
			return [
				{
					xAxis: range.start_time,
					name: a.label,
					itemStyle: { color: a.color, opacity: 0.3 },
					label: { show: true, position: 'insideTop' }
				},
				{ xAxis: range.end_time }
			];
		});
}
