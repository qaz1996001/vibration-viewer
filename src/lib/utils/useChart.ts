/**
 * ECharts 生命周期工具函数 — 封装实例创建、销毁、自动 resize 和 Svelte action。
 * 所有 chart 组件共用此模块，避免重复的 init/dispose/resize 逻辑。
 */
import * as echarts from 'echarts';
import type { EChartsOption } from 'echarts';

/**
 * 在指定 DOM 容器上创建 ECharts 实例。
 * @param container - 挂载图表的 HTML 元素
 * @param theme - 可选的 ECharts 注册主题名称
 * @returns 新创建的 ECharts 实例
 */
export function createChartInstance(
	container: HTMLElement,
	theme?: string
): echarts.ECharts {
	return echarts.init(container, theme);
}

/**
 * 安全销毁 ECharts 实例 — 内部检查 isDisposed 防止重复销毁。
 * @param chart - 要销毁的 ECharts 实例
 */
export function disposeChart(chart: echarts.ECharts): void {
	if (!chart.isDisposed()) {
		chart.dispose();
	}
}

/**
 * 创建 ResizeObserver 监听容器尺寸变化，自动触发 chart.resize()。
 * @param chart - ECharts 实例
 * @param container - 被观察的 DOM 容器
 * @returns 清理函数 — 调用后断开 ResizeObserver
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
 * Svelte action — 将 ECharts 实例绑定到 DOM 节点。
 * 用法：`<div use:chart={options} />`
 *
 * - `update`: 当 options 响应式变化时，以 notMerge 模式重新渲染
 * - `destroy`: 组件卸载时清理 ResizeObserver 并销毁实例
 *
 * @param node - Svelte action 绑定的 DOM 节点
 * @param options - 初始 ECharts 配置
 * @returns Svelte action 接口 (update/destroy)
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
