export function debounce<T extends (...args: unknown[]) => void>(fn: T, delay: number): T {
	let timer: ReturnType<typeof setTimeout>;
	return ((...args: unknown[]) => {
		clearTimeout(timer);
		timer = setTimeout(() => fn(...args), delay);
	}) as T;
}
