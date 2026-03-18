<script lang="ts">
	import { mode } from '$lib/stores/modeStore';
	import type { AppMode } from '$lib/stores/modeStore';
	import { precision, PRECISION_OPTIONS } from '$lib/stores/viewStore';
	import type { PrecisionLevel } from '$lib/stores/viewStore';
	import { mergeSeriesMode } from '$lib/stores/uiStore';

	interface Props {
		hasUnsaved?: boolean;
		hasProject?: boolean;
		multiFile?: boolean;
		onopenfile?: () => void;
		onsave?: () => void;
		onexport?: () => void;
		onexportviewport?: () => void;
		onclose?: () => void;
	}

	let { hasUnsaved = false, hasProject = false, multiFile = false, onopenfile, onsave, onexport, onexportviewport, onclose }: Props = $props();

	function setMode(newMode: AppMode) {
		mode.set(newMode);
	}

	function handlePrecisionChange(e: Event) {
		const value = (e.target as HTMLSelectElement).value as PrecisionLevel;
		precision.set(value);
	}
</script>

<div class="toolbar">
	<div class="toolbar-group">
		<button onclick={() => onopenfile?.()}>Open File</button>
		<button onclick={() => onsave?.()} class:unsaved={hasUnsaved}>
			{hasUnsaved ? 'Save *' : 'Save'}
		</button>
		{#if hasProject}
			<button onclick={() => onclose?.()}>Close</button>
		{/if}
	</div>

	<div class="toolbar-group mode-group">
		<button class:active={$mode === 'browse'} onclick={() => setMode('browse')}>Browse</button>
		<button class:active={$mode === 'annotate_point'} onclick={() => setMode('annotate_point')}>
			Mark Point
		</button>
		<button class:active={$mode === 'annotate_range'} onclick={() => setMode('annotate_range')}>
			Mark Range
		</button>
	</div>

	<div class="toolbar-group precision-group">
		<label class="precision-label" for="precision-select">精度</label>
		<select id="precision-select" value={$precision} onchange={handlePrecisionChange}>
			{#each PRECISION_OPTIONS as opt}
				<option value={opt.value}>{opt.label}</option>
			{/each}
		</select>
	</div>

	{#if multiFile}
		<div class="toolbar-group merge-group">
			<button
				class:active={$mergeSeriesMode}
				onclick={() => mergeSeriesMode.update((v) => !v)}
				title="合併同名通道為同一圖例項目"
			>
				{$mergeSeriesMode ? '分離通道' : '合併通道'}
			</button>
		</div>
	{/if}

	<div class="toolbar-group">
		<button onclick={() => onexportviewport?.()}>匯出視野</button>
		<button onclick={() => onexport?.()}>匯出全部</button>
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
		flex-wrap: wrap;
	}

	.toolbar-group {
		display: flex;
		gap: 0.25rem;
		align-items: center;
	}

	.mode-group {
		border-left: 1px solid var(--border, #e0e0e0);
		border-right: 1px solid var(--border, #e0e0e0);
		padding: 0 0.75rem;
	}

	.precision-group {
		border-left: 1px solid var(--border, #e0e0e0);
		padding: 0 0.75rem;
	}

	.merge-group {
		border-left: 1px solid var(--border, #e0e0e0);
		padding: 0 0.75rem;
	}

	.precision-label {
		font-size: 0.85rem;
		font-weight: 500;
		color: var(--text-secondary, #666);
	}

	select {
		padding: 0.35rem 0.5rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		font-size: 0.85rem;
		background: white;
		cursor: pointer;
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
