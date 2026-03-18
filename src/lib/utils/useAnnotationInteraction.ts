/**
 * 標註互動 hooks — ECharts ZRender 層級的滑鼠事件處理
 *
 * 本模組提供三個互動功能，皆透過 ZRender 底層事件實現（而非 ECharts 高階事件），
 * 以便在圖表繪製區域上精確攔截滑鼠操作：
 *
 * 1. {@link setupAnnotationClickHandler} — 點擊新增 Point / Range 標註
 * 2. {@link setupAnnotationBrushHandler} — 拖曳 Range 標註邊界以調整範圍
 * 3. {@link setupLabelDragHandler} — 拖曳 Point 標註的 label 位置
 *
 * 設計考量：
 * - 三個 handler 共享同一張圖表的 ZRender 實例，事件監聽器按註冊順序觸發
 * - 各 handler 根據 AppMode（browse / annotate_point / annotate_range）決定是否攔截
 * - 座標轉換使用 `chart.convertFromPixel('grid', ...)` 在像素與資料座標間轉換
 */
import type * as echarts from 'echarts';
import { get } from 'svelte/store';
import { mode, rangeFirstClick } from '$lib/stores/uiStore';
import { annotations, selectedId } from '$lib/stores/annotationStore';

/** ZRender 事件的像素座標（相對於 canvas 元素的 offsetX/offsetY） */
interface ZrEventParams {
	offsetX: number;
	offsetY: number;
}

/** 單點標註回呼 — 收到使用者點擊的資料座標（時間 + 數值） */
export type AnnotatePointCallback = (data: { time: number; value: number }) => void;

/** 區間標註回呼 — 收到使用者選取的時間範圍（兩次點擊確定起訖） */
export type AnnotateRangeCallback = (data: { startTime: number; endTime: number }) => void;

/**
 * 設定圖表上的標註點擊 handler。
 *
 * - `annotate_point` 模式：點擊一次即產生單點標註，將像素座標轉為資料座標後觸發 onPoint
 * - `annotate_range` 模式：第一次點擊記錄起點至 `rangeFirstClick` store；
 *   第二次點擊計算起訖時間後觸發 onRange，自動處理順序（取 min/max）
 * - `browse` 模式下不攔截任何點擊
 *
 * @param chart - ECharts 實例，用於存取 ZRender 層及座標轉換
 * @param onPoint - 單點標註完成時的回呼
 * @param onRange - 區間標註完成時的回呼
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

/**
 * 標註更新回呼 — 拖曳操作完成時觸發，攜帶標註 ID 及需更新的欄位。
 * updates 可包含 annotation_type（Range 邊界變更）或 label_offset_x/y（Label 位移變更）。
 */
export type UpdateAnnotationCallback = (data: {
	/** 被更新的標註 ID */
	id: string;
	/** 需要 patch 的欄位（部分更新） */
	updates: Record<string, unknown>;
}) => void;

/**
 * 設定 Range 標註邊界拖曳 handler — 在 browse 模式下拖曳調整選取中的 Range 標註的起/迄邊界。
 *
 * 互動流程：
 * 1. mousedown 時偵測滑鼠是否在選取標註的起始或結束邊界附近（threshold: 12px）
 * 2. 若命中邊界，記錄拖曳狀態（draggingBoundary: 'start' | 'end'）
 * 3. mouseup 時將新的像素位置轉換為資料座標，觸發 onUpdate
 * 4. mousemove 提供 `ew-resize` cursor 回饋
 *
 * 防護機制：新邊界與對側邊界保持至少 0.001 秒間距，避免區間反轉。
 *
 * @param chart - ECharts 實例
 * @param container - 圖表容器 DOM 元素，用於設定 cursor 樣式
 * @param onUpdate - 邊界更新完成時的回呼
 * @returns 包含 `isDragging()` 查詢函式的物件，供外部判斷是否正在拖曳中
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
 * 設定 Point 標註 label 拖曳 handler — 在 browse 模式下拖曳重新定位標註文字。
 *
 * 互動流程：
 * 1. mousedown 時對選取中的 Point 標註進行 pin hit-test（25px 寬 x 60px 高的範圍）
 * 2. 命中後記錄拖曳起始位置及初始 offset
 * 3. mousemove 以 ~60fps 節流即時更新 label_offset_x/y
 * 4. mouseup 持久化最終偏移量
 *
 * Cursor 回饋：hover 時顯示 `grab`，拖曳中顯示 `grabbing`。
 * 與 brush handler 的 cursor 管理互不干擾 — brush handler 只管 Range 標註的 cursor。
 *
 * @param chart - ECharts 實例
 * @param container - 圖表容器 DOM 元素，用於設定 cursor 樣式
 * @param onUpdate - label 偏移更新時的回呼（拖曳中即時觸發 + mouseup 最終觸發）
 * @returns 包含 `isDragging()` 查詢函式的物件，供外部判斷是否正在拖曳中
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

	/**
	 * Pin 命中測試 — 判斷滑鼠是否在選取中 Point 標註的圖釘圖示附近。
	 * 命中範圍：水平 25px 內，垂直從圖釘尖端上方 50px 到下方 10px。
	 */
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
