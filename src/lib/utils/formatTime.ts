export function formatTime(epochSeconds: number): string {
	const date = new Date(epochSeconds * 1000);
	const MM = String(date.getMonth() + 1).padStart(2, '0');
	const DD = String(date.getDate()).padStart(2, '0');
	const hh = String(date.getHours()).padStart(2, '0');
	const mm = String(date.getMinutes()).padStart(2, '0');
	const ss = String(date.getSeconds()).padStart(2, '0');
	return `${MM}/${DD} ${hh}:${mm}:${ss}`;
}
