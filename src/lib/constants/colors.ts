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

export const ANNOTATION_COLORS = [
	'#ff6b6b',
	'#4ecdc4',
	'#45b7d1',
	'#f9ca24',
	'#6c5ce7',
	'#a29bfe'
];

export const DEFAULT_ANNOTATION_COLOR = '#ff6b6b';

export function getChannelColor(index: number): string {
	return COLOR_PALETTE[index % COLOR_PALETTE.length];
}
