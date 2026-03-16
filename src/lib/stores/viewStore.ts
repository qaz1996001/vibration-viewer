import { writable } from 'svelte/store';

export const zoomRange = writable<[number, number]>([0, 100]);
export const maxPoints = writable(50000);
