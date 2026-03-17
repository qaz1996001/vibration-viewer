<script lang="ts">
	import { annotations, selectedId, removeAnnotation, updateAnnotation } from '$lib/stores/annotationStore';
	import type { Annotation } from '$lib/types/annotation';
	import { formatTime } from '$lib/components/Chart/chartOptions';

	interface PendingAnnotation {
		type: 'point' | 'range';
		data: { time?: number; value?: number; startTime?: number; endTime?: number };
	}

	interface Props {
		pendingAnnotation?: PendingAnnotation | null;
		onconfirm?: (data: { label: string; color: string }) => void;
		oncancel?: () => void;
	}

	let { pendingAnnotation = null, onconfirm, oncancel }: Props = $props();

	let label = $state('');
	let color = $state('#ff6b6b');

	// Edit state
	let editingId = $state<string | null>(null);
	let editLabel = $state('');
	let editColor = $state('');
	let editOffsetX = $state(0);
	let editOffsetY = $state(0);

	const presetColors = ['#ff6b6b', '#4ecdc4', '#45b7d1', '#f9ca24', '#6c5ce7', '#a29bfe'];

	// Reset form when a new pending annotation arrives
	$effect(() => {
		if (pendingAnnotation) {
			label = '';
			color = '#ff6b6b';
		}
	});

	function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (label.trim()) {
			onconfirm?.({ label: label.trim(), color });
		}
	}

	function pendingDescription(p: PendingAnnotation): string {
		if (p.type === 'point' && p.data.time !== undefined) {
			return `Point at ${formatTime(p.data.time)}`;
		}
		if (p.type === 'range' && p.data.startTime !== undefined && p.data.endTime !== undefined) {
			return `Range ${formatTime(p.data.startTime)} — ${formatTime(p.data.endTime)}`;
		}
		return '';
	}

	function handleSelect(id: string) {
		if (editingId === id) return;
		selectedId.set(id);
	}

	function handleDelete(e: MouseEvent, id: string) {
		e.stopPropagation();
		if (editingId === id) editingId = null;
		removeAnnotation(id);
	}

	function startEdit(e: MouseEvent, ann: Annotation) {
		e.stopPropagation();
		editingId = ann.id;
		editLabel = ann.label;
		editColor = ann.color;
		editOffsetX = ann.label_offset_x;
		editOffsetY = ann.label_offset_y;
		selectedId.set(ann.id);
	}

	function saveEdit() {
		if (!editingId || !editLabel.trim()) return;
		updateAnnotation(editingId, {
			label: editLabel.trim(),
			color: editColor,
			label_offset_x: editOffsetX,
			label_offset_y: editOffsetY
		});
		editingId = null;
	}

	function cancelEdit() {
		editingId = null;
	}

	function formatType(ann: Annotation): string {
		if (ann.annotation_type.type === 'Point') {
			return `Point (${formatTime(ann.annotation_type.time)})`;
		} else {
			return `Range (${formatTime(ann.annotation_type.start_time)} - ${formatTime(ann.annotation_type.end_time)})`;
		}
	}
</script>

<div class="annotation-panel">
	<h3>Annotations ({$annotations.length})</h3>

	{#if pendingAnnotation}
		<div class="inline-form">
			<div class="pending-desc">{pendingDescription(pendingAnnotation)}</div>
			<form onsubmit={handleSubmit}>
				<input
					type="text"
					bind:value={label}
					placeholder="Label..."
					class="label-input"
				/>
				<div class="color-picker">
					{#each presetColors as c}
						<button
							type="button"
							class="color-swatch"
							class:selected={color === c}
							style="background: {c}"
							onclick={() => (color = c)}
							aria-label="Select color {c}"
						></button>
					{/each}
					<input type="color" bind:value={color} class="color-input" />
				</div>
				<div class="form-actions">
					<button type="button" class="btn-cancel" onclick={() => oncancel?.()}>Cancel</button>
					<button type="submit" class="btn-confirm" disabled={!label.trim()}>Confirm</button>
				</div>
			</form>
		</div>
	{/if}

	{#each $annotations as ann (ann.id)}
		{#if editingId === ann.id}
			<div class="inline-form edit-form">
				<div class="pending-desc">{formatType(ann)}</div>
				<input
					type="text"
					bind:value={editLabel}
					placeholder="Label..."
					class="label-input"
				/>
				<div class="color-picker">
					{#each presetColors as c}
						<button
							type="button"
							class="color-swatch"
							class:selected={editColor === c}
							style="background: {c}"
							onclick={() => (editColor = c)}
							aria-label="Select color {c}"
						></button>
					{/each}
					<input type="color" bind:value={editColor} class="color-input" />
				</div>
				{#if ann.annotation_type.type === 'Point'}
					<div class="offset-controls">
						<span class="offset-label">Label Offset</span>
						<div class="offset-row">
							<span class="offset-axis">X</span>
							<button type="button" class="offset-btn" onclick={() => (editOffsetX -= 5)}>&#8592;</button>
							<input type="number" bind:value={editOffsetX} step="5" class="offset-input" />
							<button type="button" class="offset-btn" onclick={() => (editOffsetX += 5)}>&#8594;</button>
							<span class="offset-axis">Y</span>
							<button type="button" class="offset-btn" onclick={() => (editOffsetY -= 5)}>&#8593;</button>
							<input type="number" bind:value={editOffsetY} step="5" class="offset-input" />
							<button type="button" class="offset-btn" onclick={() => (editOffsetY += 5)}>&#8595;</button>
						</div>
					</div>
				{/if}
				<div class="form-actions">
					<button type="button" class="btn-cancel" onclick={cancelEdit}>Cancel</button>
					<button type="button" class="btn-confirm" disabled={!editLabel.trim()} onclick={saveEdit}>Save</button>
				</div>
			</div>
		{:else}
			<div
				class="annotation-item"
				class:selected={$selectedId === ann.id}
				role="button"
				tabindex="0"
				onclick={() => handleSelect(ann.id)}
				onkeydown={(e) => e.key === 'Enter' && handleSelect(ann.id)}
			>
				<span class="color-dot" style="background: {ann.color}"></span>
				<div class="annotation-info">
					<span class="ann-label">{ann.label}</span>
					<span class="type">{formatType(ann)}</span>
				</div>
				<button class="edit-btn" onclick={(e) => startEdit(e, ann)} aria-label="Edit">&#9998;</button>
				<button class="delete-btn" onclick={(e) => handleDelete(e, ann.id)}>&times;</button>
			</div>
		{/if}
	{:else}
		{#if !pendingAnnotation}
			<p class="empty">No annotations yet</p>
		{/if}
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

	/* Inline form (create + edit) */
	.inline-form {
		background: var(--surface-active, #f0f4ff);
		border: 1px solid var(--primary, #4a90d9);
		border-radius: 6px;
		padding: 0.75rem;
		margin-bottom: 0.5rem;
	}

	.pending-desc {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		margin-bottom: 0.5rem;
	}

	.label-input {
		width: 100%;
		padding: 0.4rem 0.5rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		font-size: 0.85rem;
		box-sizing: border-box;
		margin-bottom: 0.5rem;
	}

	.color-picker {
		display: flex;
		gap: 0.3rem;
		align-items: center;
		margin-bottom: 0.5rem;
		flex-wrap: wrap;
	}

	.color-swatch {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		border: 2px solid transparent;
		cursor: pointer;
		padding: 0;
	}

	.color-swatch.selected {
		border-color: #333;
	}

	.color-input {
		width: 24px;
		height: 24px;
		padding: 0;
		border: none;
		cursor: pointer;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.4rem;
	}

	.btn-cancel,
	.btn-confirm {
		padding: 0.3rem 0.75rem;
		border-radius: 4px;
		font-size: 0.8rem;
		cursor: pointer;
	}

	.btn-cancel {
		background: none;
		border: 1px solid var(--border, #ccc);
	}

	.btn-confirm {
		background: var(--primary, #4a90d9);
		color: white;
		border: none;
	}

	.btn-confirm:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	/* Offset controls */
	.offset-controls {
		margin-bottom: 0.5rem;
	}

	.offset-label {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		display: block;
		margin-bottom: 0.3rem;
	}

	.offset-row {
		display: flex;
		align-items: center;
		gap: 0.2rem;
		flex-wrap: wrap;
	}

	.offset-axis {
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--text-secondary, #666);
		min-width: 14px;
		text-align: center;
	}

	.offset-btn {
		width: 24px;
		height: 24px;
		padding: 0;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		background: white;
		cursor: pointer;
		font-size: 0.8rem;
		line-height: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.offset-btn:hover {
		background: var(--surface-hover, #f0f0f0);
	}

	.offset-input {
		width: 48px;
		padding: 0.2rem 0.3rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		font-size: 0.8rem;
		text-align: center;
	}

	/* Annotation list */
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
		min-width: 0;
	}

	.annotation-info .ann-label {
		font-weight: 500;
		font-size: 0.9rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.type {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
	}

	.edit-btn,
	.delete-btn {
		background: none;
		border: none;
		cursor: pointer;
		color: var(--text-secondary, #999);
		font-size: 1.1rem;
		padding: 0 0.25rem;
		flex-shrink: 0;
	}

	.edit-btn:hover {
		color: var(--primary, #4a90d9);
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
