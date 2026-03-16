<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import { mode } from '$lib/stores/modeStore';
	import type { AppMode } from '$lib/stores/modeStore';

	export let hasUnsaved: boolean = false;

	const dispatch = createEventDispatcher<{
		'open-file': void;
		save: void;
		export: void;
	}>();

	function setMode(newMode: AppMode) {
		mode.set(newMode);
	}
</script>

<div class="toolbar">
	<div class="toolbar-group">
		<button on:click={() => dispatch('open-file')}>Open File</button>
		<button on:click={() => dispatch('save')} class:unsaved={hasUnsaved}>
			{hasUnsaved ? 'Save *' : 'Save'}
		</button>
	</div>

	<div class="toolbar-group mode-group">
		<button class:active={$mode === 'browse'} on:click={() => setMode('browse')}>Browse</button>
		<button class:active={$mode === 'annotate_point'} on:click={() => setMode('annotate_point')}>
			Mark Point
		</button>
		<button class:active={$mode === 'annotate_range'} on:click={() => setMode('annotate_range')}>
			Mark Range
		</button>
	</div>

	<div class="toolbar-group">
		<button on:click={() => dispatch('export')}>Export</button>
	</div>
</div>

<style>
	.toolbar {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 0.5rem 1rem;
		border-bottom: 1px solid var(--border, #e0e0e0);
		background: var(--surface, #fafafa);
	}

	.toolbar-group {
		display: flex;
		gap: 0.25rem;
	}

	.mode-group {
		border-left: 1px solid var(--border, #e0e0e0);
		border-right: 1px solid var(--border, #e0e0e0);
		padding: 0 0.75rem;
	}

	button {
		padding: 0.4rem 0.8rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		background: white;
		cursor: pointer;
		font-size: 0.85rem;
	}

	button:hover {
		background: var(--surface-hover, #f0f0f0);
	}

	button.active {
		background: var(--primary, #4a90d9);
		color: white;
		border-color: var(--primary, #4a90d9);
	}

	button.unsaved {
		color: var(--warning, #e67e22);
		font-weight: 600;
	}
</style>
