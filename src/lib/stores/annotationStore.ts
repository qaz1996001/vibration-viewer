import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Annotation, AnnotationType } from '$lib/types/annotation';
import { DEFAULT_ANNOTATION_COLOR } from '$lib/constants/colors';

export const annotations = writable<Annotation[]>([]);
export const selectedId = writable<string | null>(null);
export const dirty = writable(false);

/** Per-device annotation map — keyed by device/dataset ID */
export const deviceAnnotations = writable<Record<string, Annotation[]>>({});

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

export function updateAnnotation(
	id: string,
	updates: Partial<Pick<Annotation, 'label' | 'color' | 'label_offset_x' | 'label_offset_y' | 'annotation_type'>>
): void {
	annotations.update((list) =>
		list.map((a) => (a.id === id ? { ...a, ...updates } : a))
	);
	dirty.set(true);
}

export function removeAnnotation(id: string): void {
	annotations.update((list) => list.filter((a) => a.id !== id));
	selectedId.update((sel) => (sel === id ? null : sel));
	dirty.set(true);
}

/** Derive annotation file path from a data file path */
export function annotationPath(filePath: string): string {
	return filePath + '.vibann.json';
}

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

/** Save annotations for a specific device, falling back to global annotations */
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

/** Load annotations for a specific device into the per-device map */
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
