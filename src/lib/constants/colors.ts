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

export function getChannelColor(index: number): string {
	return COLOR_PALETTE[index % COLOR_PALETTE.length];
}
