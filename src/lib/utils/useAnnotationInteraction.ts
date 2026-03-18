import type * as echarts from 'echarts';
import { get } from 'svelte/store';
import { mode, rangeFirstClick } from '$lib/stores/modeStore';
import { annotations, selectedId } from '$lib/stores/annotationStore';

/** Pixel coordinate from a ZRender event (offsetX/offsetY). */
interface ZrEventParams {
	offsetX: number;
	offsetY: number;
}

/** Callback for point annotation click events. */
export type AnnotatePointCallback = (data: { time: number; value: number }) => void;

/** Callback for range annotation completion events. */
export type AnnotateRangeCallback = (data: { startTime: number; endTime: number }) => void;

/**
 * Set up a click handler on the chart's ZRender layer for point and range annotations.
 *
 * In `annotate_point` mode, a click converts pixel to data coordinates and fires onPoint.
 * In `annotate_range` mode, the first click sets `rangeFirstClick`; the second click
 * computes the range and fires onRange.
 */
export function setupAnnotationClickHandler(
	chart: echarts.ECharts,
	onPoint: AnnotatePointCallback,
	onRange: AnnotateRangeCallback
): void {
	chart.getZr().on('click', (params: ZrEventParams) => {
		const currentMode = get(mode);
		if (currentMode !== 'annotate_point' && currentMode !== 'annotate_range') return;

		const pointInPixel = [params.offsetX, params.offsetY];
		const pointInGrid = chart.convertFromPixel('grid', pointInPixel);
		if (!pointInGrid) return;

		const timeValue = pointInGrid[0];
		const yValue = pointInGrid[1];

		if (currentMode === 'annotate_point') {
			onPoint({ time: timeValue, value: yValue });
		} else if (currentMode === 'annotate_range') {
			const first = get(rangeFirstClick);
			if (first === null) {
				rangeFirstClick.set(timeValue);
			} else {
				const startTime = Math.min(first, timeValue);
				const endTime = Math.max(first, timeValue);
				rangeFirstClick.set(null);
				onRange({ startTime, endTime });
			}
		}
	});
}

/** Callback when a range annotation boundary is dragged to a new position. */
export type UpdateAnnotationCallback = (data: {
	id: string;
	updates: Record<string, unknown>;
}) => void;

/**
 * Set up drag handlers on the chart's ZRender layer for resizing selected
 * range-annotation boundaries.
 *
 * mousedown near a boundary edge starts the drag; mouseup ends it and
 * fires onUpdate with the new start/end time.
 */
export function setupAnnotationBrushHandler(
	chart: echarts.ECharts,
	container: HTMLElement,
	onUpdate: UpdateAnnotationCallback
): { isDragging: () => boolean } {
	let draggingBoundary: 'start' | 'end' | null = null;
	let draggingAnnotationId: string | null = null;

	chart.getZr().on('mousedown', (params: ZrEventParams) => {
		if (get(mode) !== 'browse') return;
		const selId = get(selectedId);
		if (!selId) return;

		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Range') return;

		const range = ann.annotation_type as {
			type: 'Range';
			start_time: number;
			end_time: number;
		};
		const startPx = chart.convertToPixel('grid', [range.start_time, 0]);
		const endPx = chart.convertToPixel('grid', [range.end_time, 0]);
		if (!startPx || !endPx) return;

		const threshold = 12;
		if (Math.abs(params.offsetX - startPx[0]) < threshold) {
			draggingBoundary = 'start';
			draggingAnnotationId = ann.id;
		} else if (Math.abs(params.offsetX - endPx[0]) < threshold) {
			draggingBoundary = 'end';
			draggingAnnotationId = ann.id;
		}
	});

	chart.getZr().on('mouseup', (params: ZrEventParams) => {
		if (!draggingBoundary || !draggingAnnotationId) {
			draggingBoundary = null;
			draggingAnnotationId = null;
			return;
		}

		const dataPos = chart.convertFromPixel('grid', [params.offsetX, params.offsetY]);
		if (dataPos) {
			const anns = get(annotations);
			const ann = anns.find((a) => a.id === draggingAnnotationId);
			if (ann && ann.annotation_type.type === 'Range') {
				const range = ann.annotation_type as {
					type: 'Range';
					start_time: number;
					end_time: number;
				};
				if (draggingBoundary === 'start') {
					const newStart = Math.min(dataPos[0], range.end_time - 0.001);
					onUpdate({
						id: ann.id,
						updates: {
							annotation_type: {
								type: 'Range',
								start_time: newStart,
								end_time: range.end_time
							}
						}
					});
				} else {
					const newEnd = Math.max(dataPos[0], range.start_time + 0.001);
					onUpdate({
						id: ann.id,
						updates: {
							annotation_type: {
								type: 'Range',
								start_time: range.start_time,
								end_time: newEnd
							}
						}
					});
				}
			}
		}

		draggingBoundary = null;
		draggingAnnotationId = null;
	});

	// Cursor feedback: show ew-resize when hovering near a selected range boundary
	chart.getZr().on('mousemove', (params: ZrEventParams) => {
		if (draggingBoundary) return;
		const selId = get(selectedId);
		if (!selId || get(mode) !== 'browse') {
			container.style.cursor = '';
			return;
		}

		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Range') {
			// Don't reset cursor — label drag handler manages Point annotations.
			// This works because ZRender fires listeners in registration order:
			// brush handler registers first, then label handler handles cursor for Points.
			return;
		}

		const range = ann.annotation_type as {
			type: 'Range';
			start_time: number;
			end_time: number;
		};
		const startPx = chart.convertToPixel('grid', [range.start_time, 0]);
		const endPx = chart.convertToPixel('grid', [range.end_time, 0]);

		if (startPx && endPx) {
			const threshold = 12;
			if (
				Math.abs(params.offsetX - startPx[0]) < threshold ||
				Math.abs(params.offsetX - endPx[0]) < threshold
			) {
				container.style.cursor = 'ew-resize';
			} else {
				container.style.cursor = '';
			}
		}
	});

	return {
		isDragging: () => draggingBoundary !== null
	};
}

/**
 * Set up drag handlers on the chart's ZRender layer for repositioning
 * Point annotation labels by dragging.
 *
 * In browse mode with a selected Point annotation, mousedown near the pin
 * starts a drag. mousemove updates label_offset_x/y in real-time.
 * mouseup ends the drag and persists the final offset.
 */
export function setupLabelDragHandler(
	chart: echarts.ECharts,
	container: HTMLElement,
	onUpdate: UpdateAnnotationCallback
): { isDragging: () => boolean } {
	let dragging = false;
	let dragAnnotationId: string | null = null;
	let dragStartX = 0;
	let dragStartY = 0;
	let initialOffsetX = 0;
	let initialOffsetY = 0;
	let lastUpdateTime = 0;

	/** Check if mouse position is near a selected Point annotation's pin. */
	function hitTestPin(offsetX: number, offsetY: number): boolean {
		const selId = get(selectedId);
		if (!selId) return false;
		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Point') return false;

		const pt = ann.annotation_type;
		const pinPixel = chart.convertToPixel('grid', [pt.time, pt.value]);
		if (!pinPixel) return false;

		const dx = Math.abs(offsetX - pinPixel[0]);
		const dy = offsetY - pinPixel[1]; // negative = above pin tip
		// Accept clicks within 25px horizontally, from 50px above to 10px below pin tip
		return dx <= 25 && dy <= 10 && dy >= -50;
	}

	chart.getZr().on('mousedown', (params: ZrEventParams) => {
		if (get(mode) !== 'browse') return;
		const selId = get(selectedId);
		if (!selId) return;

		const anns = get(annotations);
		const ann = anns.find((a) => a.id === selId);
		if (!ann || ann.annotation_type.type !== 'Point') return;

		// hitTestPin re-reads stores (minor redundancy for code clarity)
		if (!hitTestPin(params.offsetX, params.offsetY)) return;

		dragging = true;
		dragAnnotationId = ann.id;
		dragStartX = params.offsetX;
		dragStartY = params.offsetY;
		initialOffsetX = ann.label_offset_x;
		initialOffsetY = ann.label_offset_y;
		lastUpdateTime = 0;
		container.style.cursor = 'grabbing';
	});

	chart.getZr().on('mousemove', (params: ZrEventParams) => {
		if (!dragging || !dragAnnotationId) {
			// Cursor feedback: show grab when hovering near a selected Point annotation's pin
			if (get(mode) !== 'browse') return;
			if (!get(selectedId)) return;
			if (hitTestPin(params.offsetX, params.offsetY)) {
				container.style.cursor = 'grab';
			} else {
				// Only reset if a Point annotation is selected (we own cursor management)
				const anns = get(annotations);
				const ann = anns.find((a) => a.id === get(selectedId));
				if (ann && ann.annotation_type.type === 'Point') {
					container.style.cursor = '';
				}
			}
			return;
		}

		// Throttle updates to ~60fps during drag
		const now = Date.now();
		if (now - lastUpdateTime < 16) return;
		lastUpdateTime = now;

		const deltaX = params.offsetX - dragStartX;
		const deltaY = params.offsetY - dragStartY;

		onUpdate({
			id: dragAnnotationId,
			updates: {
				label_offset_x: Math.round(initialOffsetX + deltaX),
				label_offset_y: Math.round(initialOffsetY + deltaY)
			}
		});

		container.style.cursor = 'grabbing';
	});

	chart.getZr().on('mouseup', (params: ZrEventParams) => {
		if (dragging && dragAnnotationId) {
			const deltaX = params.offsetX - dragStartX;
			const deltaY = params.offsetY - dragStartY;
			onUpdate({
				id: dragAnnotationId,
				updates: {
					label_offset_x: Math.round(initialOffsetX + deltaX),
					label_offset_y: Math.round(initialOffsetY + deltaY)
				}
			});
			container.style.cursor = '';
			dragging = false;
			dragAnnotationId = null;
		}
	});

	return {
		isDragging: () => dragging
	};
}
