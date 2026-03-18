/**
 * 將 epoch seconds 格式化為 `MM/DD hh:mm:ss` 顯示字串。
 * 用於 ECharts tooltip、軸標籤及資料表格中的時間顯示。
 *
 * @param epochSeconds - Unix 時間戳（秒），例如 1679000000
 * @returns 格式化後的本地時間字串，例如 `"03/17 08:13:20"`
 */
export function formatTime(epochSeconds: number): string {
	const date = new Date(epochSeconds * 1000);
	const MM = String(date.getMonth() + 1).padStart(2, '0');
	const DD = String(date.getDate()).padStart(2, '0');
	const hh = String(date.getHours()).padStart(2, '0');
	const mm = String(date.getMinutes()).padStart(2, '0');
	const ss = String(date.getSeconds()).padStart(2, '0');
	return `${MM}/${DD} ${hh}:${mm}:${ss}`;
}
