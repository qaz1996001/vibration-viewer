import { writable } from 'svelte/store';

export type AppMode = 'browse' | 'annotate_point' | 'annotate_range';

export const mode = writable<AppMode>('browse');
