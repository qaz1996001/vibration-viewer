import { writable } from 'svelte/store';

export type AppMode = 'browse' | 'annotate_point' | 'annotate_range';

export const mode = writable<AppMode>('browse');

/** First click X value (epoch seconds) during two-click range annotation, null when idle */
export const rangeFirstClick = writable<number | null>(null);
