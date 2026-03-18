/**
 * 建立防抖函式 — 在最後一次呼叫後延遲 `delay` 毫秒才執行。
 * 主要用於 dataZoom 事件節流（300ms），避免頻繁的 IPC 請求。
 *
 * @param fn - 需要防抖的目標函式
 * @param delay - 延遲毫秒數
 * @returns 具有相同簽章的防抖版本函式
 */
export function debounce<T extends (...args: never[]) => void>(fn: T, delay: number): T {
	let timer: ReturnType<typeof setTimeout>;
	return ((...args: Parameters<T>) => {
		clearTimeout(timer);
		timer = setTimeout(() => fn(...args), delay);
	}) as T;
}
