<script lang="ts">
	import {
		datasets,
		datasetOrder,
		activeDatasetId,
		removeFile
	} from '$lib/stores/dataStore';

	function handleSelect(id: string) {
		activeDatasetId.set(id);
	}

	function handleRemove(e: MouseEvent, id: string) {
		e.stopPropagation();
		removeFile(id);
	}
</script>

{#if $datasetOrder.length > 0}
	<div class="file-list">
		<h3>Files ({$datasetOrder.length})</h3>
		{#each $datasetOrder as id (id)}
			{@const ds = $datasets[id]}
			{#if ds}
				<div
					class="file-item"
					class:active={$activeDatasetId === id}
					role="button"
					tabindex="0"
					onclick={() => handleSelect(id)}
					onkeydown={(e) => e.key === 'Enter' && handleSelect(id)}
				>
					<div class="file-info">
						<span class="file-name">{ds.file_name}</span>
						<span class="file-meta">{ds.total_points.toLocaleString()} pts</span>
					</div>
					<button class="remove-btn" onclick={(e) => handleRemove(e, id)}>&times;</button>
				</div>
			{/if}
		{/each}
	</div>
{/if}

<style>
	.file-list {
		padding: 0.5rem;
		border-bottom: 1px solid var(--border, #e0e0e0);
	}

	h3 {
		margin: 0 0 0.5rem;
		padding: 0 0.5rem;
		font-size: 0.95rem;
	}

	.file-item {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.4rem 0.5rem;
		border-radius: 4px;
		cursor: pointer;
	}

	.file-item:hover {
		background: var(--surface-hover, #f0f0f0);
	}

	.file-item.active {
		background: var(--surface-active, #e0e0ff);
	}

	.file-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.file-name {
		font-size: 0.85rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.file-meta {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
	}

	.remove-btn {
		background: none;
		border: none;
		cursor: pointer;
		color: var(--text-secondary, #999);
		font-size: 1.1rem;
		padding: 0 0.25rem;
		flex-shrink: 0;
	}

	.remove-btn:hover {
		color: var(--error, #ff4444);
	}
</style>
