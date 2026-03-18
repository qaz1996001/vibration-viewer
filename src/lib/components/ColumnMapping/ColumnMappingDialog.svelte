<script lang="ts">
	/**
	 * ColumnMappingDialog - CSV 欄位對應對話框
	 *
	 * 開檔流程的第二步：使用者從 CSV preview 結果中指定
	 * 哪一欄是時間軸（time column）、哪些欄是數據通道（data columns）。
	 *
	 * 自動偵測邏輯：
	 * - time column: 優先選取欄名含 "time" 的欄位
	 * - data columns: 若有 x/y/z 欄位則預選，否則全選（排除 time）
	 *
	 * 確認後回傳 ColumnMapping 給父層呼叫 load_vibration_data IPC。
	 */
	import type { CsvPreview, ColumnMapping } from '$lib/types/vibration';

	interface Props {
		/** CSV 預覽資料（欄位名稱、行數、檔案路徑），由 preview_csv_columns IPC 回傳 */
		preview: CsvPreview;
		/** 使用者確認對應後的 callback，回傳 ColumnMapping */
		onconfirm: (mapping: ColumnMapping) => void;
		/** 使用者取消對話框 */
		oncancel: () => void;
	}

	let { preview, onconfirm, oncancel }: Props = $props();

	// 快照 props：dialog 每次都是全新建立，不需要響應式追蹤 preview 變化
	// svelte-ignore state_referenced_locally
	const columns = preview.columns;
	// svelte-ignore state_referenced_locally
	const filePath = preview.file_path;
	// svelte-ignore state_referenced_locally
	const rowCount = preview.row_count;

	// 自動偵測 time column：優先匹配名稱含 "time" 的欄位，fallback 取第一欄
	const defaultTimeCol = columns.find((c) => /time/i.test(c)) ?? columns[0] ?? '';

	/** $state: 使用者選定的時間欄位 */
	let timeColumn = $state(defaultTimeCol);

	// 自動預選 data columns：若有 x/y/z 標準欄名則只選它們，否則全選（排除 time）
	const knownDataCols = ['x', 'y', 'z'];
	const autoSelected = columns.filter(
		(c) => c !== defaultTimeCol && knownDataCols.includes(c.toLowerCase())
	);
	/** $state: 被勾選的數據欄位集合 */
	let selectedColumns = $state<Set<string>>(
		new Set(
			autoSelected.length > 0 ? autoSelected : columns.filter((c) => c !== defaultTimeCol)
		)
	);

	/** $derived: 排除 time column 後可供選擇的數據欄位 */
	let availableDataColumns = $derived(columns.filter((c) => c !== timeColumn));

	/** 切換單一 data column 的勾選狀態（需重建 Set 觸發 reactivity） */
	function toggleColumn(col: string) {
		selectedColumns = new Set(selectedColumns);
		if (selectedColumns.has(col)) {
			selectedColumns.delete(col);
		} else {
			selectedColumns.add(col);
		}
	}

	/** 全選所有可用 data columns */
	function selectAll() {
		selectedColumns = new Set(availableDataColumns);
	}

	/** 清除所有 data columns 選取 */
	function selectNone() {
		selectedColumns = new Set();
	}

	/** 確認送出：過濾已選欄位、組成 ColumnMapping 後呼叫 onconfirm */
	function handleConfirm() {
		const dataColumns = availableDataColumns.filter((c) => selectedColumns.has(c));
		if (!timeColumn || dataColumns.length === 0) return;
		onconfirm({
			time_column: timeColumn,
			data_columns: dataColumns
		});
	}

	/** $derived: 是否可按確認（需有 time column 且至少選一個 data column） */
	let canConfirm = $derived(timeColumn !== '' && selectedColumns.size > 0);

	/** Dialog element reference for focus trap */
	let dialogEl: HTMLDivElement;

	import { onMount } from 'svelte';

	onMount(() => {
		// Focus first interactive element on open
		const firstInput = dialogEl?.querySelector('select, input, button') as HTMLElement | null;
		firstInput?.focus();
	});

	/** Handle Escape key to close dialog + focus trap */
	function handleOverlayKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			oncancel();
			return;
		}
		// Focus trap: Tab/Shift+Tab cycle within dialog
		if (e.key === 'Tab') {
			const focusable = dialogEl?.querySelectorAll(
				'button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
			) as NodeListOf<HTMLElement>;
			if (!focusable || focusable.length === 0) return;
			const first = focusable[0];
			const last = focusable[focusable.length - 1];
			if (e.shiftKey && document.activeElement === first) {
				e.preventDefault();
				last.focus();
			} else if (!e.shiftKey && document.activeElement === last) {
				e.preventDefault();
				first.focus();
			}
		}
	}
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="overlay" role="dialog" aria-modal="true" aria-labelledby="mapping-dialog-title" tabindex="-1" onkeydown={handleOverlayKeydown}>
	<div class="dialog" bind:this={dialogEl}>
		<h3 id="mapping-dialog-title">Column Mapping</h3>
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

		<div class="field" role="group" aria-labelledby="data-columns-label">
			<span class="field-label" id="data-columns-label">Data Columns</span>
			<div class="select-actions">
				<button type="button" class="link-btn" onclick={selectAll} aria-label="Select all data columns">All</button>
				<button type="button" class="link-btn" onclick={selectNone} aria-label="Deselect all data columns">None</button>
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
