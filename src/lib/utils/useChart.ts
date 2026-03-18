import * as echarts from 'echarts';
import type { EChartsOption } from 'echarts';

/**
 * Create an ECharts instance on the given container element.
 * Optionally applies a registered theme.
 */
export function createChartInstance(
	container: HTMLElement,
	theme?: string
): echarts.ECharts {
	return echarts.init(container, theme);
}

/**
 * Safely dispose an ECharts instance, checking it hasn't already been disposed.
 */
export function disposeChart(chart: echarts.ECharts): void {
	if (!chart.isDisposed()) {
		chart.dispose();
	}
}

/**
 * Create a ResizeObserver that auto-resizes the chart when its container changes size.
 * Returns a cleanup function that disconnects the observer.
 */
export function setupResizeObserver(
	chart: echarts.ECharts,
	container: HTMLElement
): () => void {
	const resizeObserver = new ResizeObserver(() => {
		if (!chart.isDisposed()) {
			chart.resize();
		}
	});
	resizeObserver.observe(container);
	return () => resizeObserver.disconnect();
}

/**
 * Svelte action for mounting an ECharts instance on a DOM node.
 * Usage: <div use:chart={options} />
 *
 * Returns an update function so reactive option changes re-render the chart,
 * and a destroy function that disposes the chart on unmount.
 */
export function chart(
	node: HTMLElement,
	options: EChartsOption
): { update: (opts: EChartsOption) => void; destroy: () => void } {
	const instance = createChartInstance(node);
	instance.setOption(options);

	const cleanupResize = setupResizeObserver(instance, node);

	return {
		update(opts: EChartsOption) {
			if (!instance.isDisposed()) {
				instance.setOption(opts, { notMerge: true });
			}
		},
		destroy() {
			cleanupResize();
			disposeChart(instance);
		}
	};
}
