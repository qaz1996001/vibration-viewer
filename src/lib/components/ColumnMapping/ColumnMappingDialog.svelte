<script lang="ts">
	import type { CsvPreview, ColumnMapping } from '$lib/types/vibration';

	interface Props {
		preview: CsvPreview;
		onconfirm: (mapping: ColumnMapping) => void;
		oncancel: () => void;
	}

	let { preview, onconfirm, oncancel }: Props = $props();

	// Snapshot props at creation time (dialog is created fresh each time)
	// svelte-ignore state_referenced_locally
	const columns = preview.columns;
	// svelte-ignore state_referenced_locally
	const filePath = preview.file_path;
	// svelte-ignore state_referenced_locally
	const rowCount = preview.row_count;

	// Auto-detect: pick first column with "time" in name, or first column
	const defaultTimeCol = columns.find((c) => /time/i.test(c)) ?? columns[0] ?? '';

	let timeColumn = $state(defaultTimeCol);

	// Auto-select known columns (x, y, z) or all non-time columns
	const knownDataCols = ['x', 'y', 'z'];
	const autoSelected = columns.filter(
		(c) => c !== defaultTimeCol && knownDataCols.includes(c.toLowerCase())
	);
	let selectedColumns = $state<Set<string>>(
		new Set(
			autoSelected.length > 0 ? autoSelected : columns.filter((c) => c !== defaultTimeCol)
		)
	);

	let availableDataColumns = $derived(columns.filter((c) => c !== timeColumn));

	function toggleColumn(col: string) {
		selectedColumns = new Set(selectedColumns);
		if (selectedColumns.has(col)) {
			selectedColumns.delete(col);
		} else {
			selectedColumns.add(col);
		}
	}

	function selectAll() {
		selectedColumns = new Set(availableDataColumns);
	}

	function selectNone() {
		selectedColumns = new Set();
	}

	function handleConfirm() {
		const dataColumns = availableDataColumns.filter((c) => selectedColumns.has(c));
		if (!timeColumn || dataColumns.length === 0) return;
		onconfirm({
			time_column: timeColumn,
			data_columns: dataColumns
		});
	}

	let canConfirm = $derived(timeColumn !== '' && selectedColumns.size > 0);
</script>

<div class="overlay" role="dialog" aria-modal="true">
	<div class="dialog">
		<h3>Column Mapping</h3>
		<p class="file-info">
			{filePath.split(/[\\/]/).pop()} — {rowCount.toLocaleString()} rows,
			{columns.length} columns
		</p>

		<div class="field">
			<label for="time-col">Time Column</label>
			<select id="time-col" bind:value={timeColumn}>
				{#each columns as col}
					<option value={col}>{col}</option>
				{/each}
			</select>
		</div>

		<div class="field">
			<span class="field-label">Data Columns</span>
			<div class="select-actions">
				<button type="button" class="link-btn" onclick={selectAll}>All</button>
				<button type="button" class="link-btn" onclick={selectNone}>None</button>
			</div>
			<div class="column-list">
				{#each availableDataColumns as col}
					<label class="checkbox-label">
						<input
							type="checkbox"
							checked={selectedColumns.has(col)}
							onchange={() => toggleColumn(col)}
						/>
						{col}
					</label>
				{/each}
			</div>
		</div>

		<div class="actions">
			<button type="button" class="btn-cancel" onclick={oncancel}>Cancel</button>
			<button type="button" class="btn-confirm" disabled={!canConfirm} onclick={handleConfirm}>
				Load Data
			</button>
		</div>
	</div>
</div>

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.4);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.dialog {
		background: white;
		border-radius: 8px;
		padding: 1.5rem;
		width: 400px;
		max-height: 80vh;
		overflow-y: auto;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
	}

	h3 {
		margin: 0 0 0.25rem;
	}

	.file-info {
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		margin: 0 0 1rem;
	}

	.field {
		margin-bottom: 1rem;
	}

	.field > label,
	.field-label {
		display: block;
		font-weight: 600;
		font-size: 0.85rem;
		margin-bottom: 0.25rem;
	}

	select {
		width: 100%;
		padding: 0.4rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		font-size: 0.85rem;
	}

	.select-actions {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.25rem;
	}

	.link-btn {
		background: none;
		border: none;
		color: var(--primary, #4a90d9);
		cursor: pointer;
		font-size: 0.8rem;
		padding: 0;
		text-decoration: underline;
	}

	.column-list {
		max-height: 200px;
		overflow-y: auto;
		border: 1px solid var(--border, #e0e0e0);
		border-radius: 4px;
		padding: 0.5rem;
	}

	.checkbox-label {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.2rem 0;
		font-size: 0.85rem;
		cursor: pointer;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 1rem;
	}

	.btn-cancel,
	.btn-confirm {
		padding: 0.4rem 1rem;
		border-radius: 4px;
		font-size: 0.85rem;
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
</style>
