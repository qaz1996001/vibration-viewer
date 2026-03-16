import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type { Annotation, AnnotationType } from '$lib/types/annotation';

export const annotations = writable<Annotation[]>([]);
export const selectedId = writable<string | null>(null);
export const dirty = writable(false);

export function addAnnotation(
	annotationType: AnnotationType,
	label: string,
	color: string = '#ff6b6b'
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

export function removeAnnotation(id: string): void {
	annotations.update((list) => list.filter((a) => a.id !== id));
	selectedId.update((sel) => (sel === id ? null : sel));
	dirty.set(true);
}

export function updateAnnotation(id: string, updates: Partial<Annotation>): void {
	annotations.update((list) => list.map((a) => (a.id === id ? { ...a, ...updates } : a)));
	dirty.set(true);
}

export async function saveAnnotations(datasetId: string, filePath: string): Promise<void> {
	const current = get(annotations);
	await invoke('save_annotations', {
		datasetId,
		filePath,
		annotations: current
	});
	dirty.set(false);
}

export async function loadAnnotations(filePath: string): Promise<void> {
	const loaded = await invoke<Annotation[]>('load_annotations', { filePath });
	annotations.set(loaded);
	dirty.set(false);
}
