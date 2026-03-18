/**
 * 数据集/通道的颜色调色板 — 依序分配给多文件叠加图中的各系列。
 * 顺序来自 ECharts 默认主题，确保视觉区分度。
 */
export const COLOR_PALETTE = [
	'#5470c6',
	'#91cc75',
	'#fac858',
	'#ee6666',
	'#73c0de',
	'#3ba272',
	'#fc8452',
	'#9a60b4',
	'#ea7ccc'
];

/**
 * 标注专用颜色选择 — 用于 AnnotationPanel 的颜色选取器。
 * 与数据系列颜色有意区分，避免视觉混淆。
 */
export const ANNOTATION_COLORS = [
	'#ff6b6b',
	'#4ecdc4',
	'#45b7d1',
	'#f9ca24',
	'#6c5ce7',
	'#a29bfe'
];

/** 新建标注时的默认颜色（红色调，醒目） */
export const DEFAULT_ANNOTATION_COLOR = '#ff6b6b';

/**
 * 根据索引取得通道颜色 — 索引超过调色板长度时自动循环。
 * @param index - 通道在数据集中的序号
 * @returns 十六进制颜色字符串
 */
export function getChannelColor(index: number): string {
	return COLOR_PALETTE[index % COLOR_PALETTE.length];
}
