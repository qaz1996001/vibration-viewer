<script lang="ts">
	import { annotations, selectedId, removeAnnotation } from '$lib/stores/annotationStore';
	import type { Annotation } from '$lib/types/annotation';

	function handleSelect(id: string) {
		selectedId.set(id);
	}

	function handleDelete(id: string) {
		removeAnnotation(id);
	}

	function formatType(ann: Annotation): string {
		if (ann.annotation_type.type === 'Point') {
			return `Point (${ann.annotation_type.time.toFixed(2)})`;
		} else {
			return `Range (${ann.annotation_type.start_time.toFixed(2)} - ${ann.annotation_type.end_time.toFixed(2)})`;
		}
	}
</script>

<div class="annotation-panel">
	<h3>Annotations ({$annotations.length})</h3>

	{#each $annotations as ann (ann.id)}
		<div
			class="annotation-item"
			class:selected={$selectedId === ann.id}
			role="button"
			tabindex="0"
			on:click={() => handleSelect(ann.id)}
			on:keydown={(e) => e.key === 'Enter' && handleSelect(ann.id)}
		>
			<span class="color-dot" style="background: {ann.color}"></span>
			<div class="annotation-info">
				<span class="label">{ann.label}</span>
				<span class="type">{formatType(ann)}</span>
			</div>
			<button class="delete-btn" on:click|stopPropagation={() => handleDelete(ann.id)}>
				&times;
			</button>
		</div>
	{:else}
		<p class="empty">No annotations yet</p>
	{/each}
</div>

<style>
	.annotation-panel {
		padding: 0.5rem;
		overflow-y: auto;
	}

	h3 {
		margin: 0 0 0.5rem;
		padding: 0 0.5rem;
		font-size: 0.95rem;
	}

	.annotation-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem;
		border-radius: 4px;
		cursor: pointer;
	}

	.annotation-item:hover {
		background: var(--surface-hover, #f0f0f0);
	}

	.annotation-item.selected {
		background: var(--surface-active, #e0e0ff);
	}

	.color-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.annotation-info {
		flex: 1;
		display: flex;
		flex-direction: column;
	}

	.label {
		font-weight: 500;
		font-size: 0.9rem;
	}

	.type {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
	}

	.delete-btn {
		background: none;
		border: none;
		cursor: pointer;
		color: var(--text-secondary, #999);
		font-size: 1.1rem;
		padding: 0 0.25rem;
	}

	.delete-btn:hover {
		color: var(--error, #ff4444);
	}

	.empty {
		color: var(--text-secondary, #999);
		text-align: center;
		padding: 2rem 0;
		font-size: 0.85rem;
	}
</style>
