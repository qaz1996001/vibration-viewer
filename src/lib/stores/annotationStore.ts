/**
 * 标注 CRUD Store — 管理振动数据的点标注和范围标注。
 *
 * 职责：
 * - 内存中维护当前活跃的标注列表 (`annotations`)
 * - 通过 Tauri IPC 与后端交互，持久化到 `.vibann.json` 文件
 * - 追踪 dirty 状态，提示用户保存
 * - 支持按设备分组的标注 (`deviceAnnotations`)
 */
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Annotation, AnnotationType } from '$lib/types/annotation';
import { DEFAULT_ANNOTATION_COLOR } from '$lib/constants/colors';

/** 当前活跃数据集的标注列表 */
export const annotations = writable<Annotation[]>([]);

/** 当前选中的标注 ID — 用于高亮显示和编辑操作 */
export const selectedId = writable<string | null>(null);

/** 标注是否有未保存的修改 */
export const dirty = writable(false);

/** 按设备/数据集 ID 分组的标注映射 — 用于 AIDPS 多设备项目 */
export const deviceAnnotations = writable<Record<string, Annotation[]>>({});

/**
 * 新增一条标注到列表。自动生成 UUID 和时间戳。
 * @param annotationType - 标注类型（Point 或 Range），含坐标信息
 * @param label - 用户可见的标注名称
 * @param color - 标注颜色（默认红色）
 */
export function addAnnotation(
	annotationType: AnnotationType,
	label: string,
	color: string = DEFAULT_ANNOTATION_COLOR
): void {
	const newAnnotation: Annotation = {
		id: crypto.randomUUID(),
		annotation_type: annotationType,
		label,
		color,
		label_offset_x: 0,
		label_offset_y: 0,
		created_at: new Date().toISOString()
	};

	annotations.update((list) => [...list, newAnnotation]);
	dirty.set(true);
}

/**
 * 更新指定标注的部分字段（标签、颜色、偏移量或类型）。
 * @param id - 要更新的标注 ID
 * @param updates - 要覆盖的字段
 */
export function updateAnnotation(
	id: string,
	updates: Partial<Pick<Annotation, 'label' | 'color' | 'label_offset_x' | 'label_offset_y' | 'annotation_type'>>
): void {
	annotations.update((list) =>
		list.map((a) => (a.id === id ? { ...a, ...updates } : a))
	);
	dirty.set(true);
}

/**
 * 删除指定标注。若删除的是当前选中项，同时清除选中状态。
 * @param id - 要删除的标注 ID
 */
export function removeAnnotation(id: string): void {
	annotations.update((list) => list.filter((a) => a.id !== id));
	selectedId.update((sel) => (sel === id ? null : sel));
	dirty.set(true);
}

/**
 * 根据数据文件路径推导标注文件路径。
 * 约定：标注文件 = 原文件路径 + `.vibann.json` 后缀。
 * @param filePath - 数据文件的完整路径
 * @returns 对应的标注文件路径
 */
export function annotationPath(filePath: string): string {
	return filePath + '.vibann.json';
}

/**
 * 将当前标注列表保存到磁盘（通过 Tauri IPC）。
 * 成功后清除 dirty 标记。
 * @param filePath - 数据文件路径（用于推导标注文件位置）
 */
export async function saveAnnotations(filePath: string): Promise<void> {
	try {
		const current = get(annotations);
		await invoke('save_annotations', {
			annotationPath: annotationPath(filePath),
			annotations: current
		});
		dirty.set(false);
	} catch (e) {
		console.error('Failed to save annotations:', e);
		throw e;
	}
}

/**
 * 从磁盘加载标注文件并替换当前列表。
 * @param filePath - 数据文件路径（用于推导标注文件位置）
 */
export async function loadAnnotations(filePath: string): Promise<void> {
	try {
		const loaded = await invoke<Annotation[]>('load_annotations', {
			annotationPath: annotationPath(filePath)
		});
		annotations.set(loaded);
		dirty.set(false);
	} catch (e) {
		console.error('Failed to load annotations:', e);
		throw e;
	}
}

/**
 * 保存指定设备的标注。若设备无专属标注，回退到全局标注列表。
 * @param deviceId - 设备/数据集 ID
 * @param filePath - 该设备对应的数据文件路径
 */
export async function saveDeviceAnnotations(deviceId: string, filePath: string): Promise<void> {
	try {
		const current = get(deviceAnnotations)[deviceId] ?? get(annotations);
		await invoke('save_annotations', {
			annotationPath: annotationPath(filePath),
			annotations: current
		});
	} catch (e) {
		console.error('Failed to save device annotations:', e);
		throw e;
	}
}

/**
 * 从磁盘加载指定设备的标注，写入 deviceAnnotations 映射。
 * @param deviceId - 设备/数据集 ID
 * @param filePath - 该设备对应的数据文件路径
 */
export async function loadDeviceAnnotations(deviceId: string, filePath: string): Promise<void> {
	try {
		const loaded = await invoke<Annotation[]>('load_annotations', {
			annotationPath: annotationPath(filePath)
		});
		deviceAnnotations.update((map) => ({ ...map, [deviceId]: loaded }));
	} catch (e) {
		console.error('Failed to load device annotations:', e);
		throw e;
	}
}
