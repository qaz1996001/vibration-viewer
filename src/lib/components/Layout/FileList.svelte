<script lang="ts">
	/**
	 * FileList - 已載入檔案列表（側欄）
	 *
	 * 顯示所有已載入的 CSV 檔案，支援：
	 * - 點擊選取（設定 activeDatasetId，影響 statistics/data table 顯示）
	 * - 色點按鈕：開啟隱藏的 color picker 更改檔案在 chart 上的系列顏色
	 * - 刪除按鈕：從 dataStore 移除檔案
	 *
	 * 檔案順序由 datasetOrder store 控制（按載入順序）。
	 */
	import {
		datasets,
		datasetOrder,
		activeDatasetId,
		removeFile,
		fileColors,
		setFileColor
	} from '$lib/stores/dataStore';

	/** 選取檔案為 active dataset（影響 statistics、data table 等單檔案視圖） */
	function handleSelect(id: string) {
		activeDatasetId.set(id);
	}

	/** 移除檔案，stopPropagation 避免觸發父層的 select */
	function handleRemove(e: MouseEvent, id: string) {
		e.stopPropagation();
		removeFile(id);
	}

	/** 處理 color picker 變更，更新 fileColors store 中該檔案的顏色 */
	function handleColorChange(id: string, e: Event) {
		const value = (e.target as HTMLInputElement).value;
		setFileColor(id, value);
	}
</script>

{#if $datasetOrder.length > 0}
	<div class="file-list">
		<h3>Files ({$datasetOrder.length})</h3>
		{#each $datasetOrder as id (id)}
			{@const ds = $datasets[id]}
			{@const fileColor = $fileColors[id] ?? '#5470c6'}
			{#if ds}
				<div
					class="file-item"
					class:active={$activeDatasetId === id}
					role="button"
					tabindex="0"
					onclick={() => handleSelect(id)}
					onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), handleSelect(id))}
					aria-label="Select file {ds.file_name}"
				>
					<button
						type="button"
						class="file-color-btn"
						onclick={(e) => {
							e.stopPropagation();
							const input = e.currentTarget.querySelector('input');
							input?.click();
						}}
						aria-label="Change file color"
					>
						<span class="file-color-dot" style="background: {fileColor}"></span>
						<input
							type="color"
							value={fileColor}
							onchange={(e) => handleColorChange(id, e)}
							class="file-color-hidden"
							tabindex="-1"
						/>
					</button>
					<div class="file-info">
						<span class="file-name">{ds.file_name}</span>
						<span class="file-meta">{ds.total_points.toLocaleString()} pts</span>
					</div>
					<button class="remove-btn" onclick={(e) => handleRemove(e, id)} aria-label="Remove file {ds.file_name}">&times;</button>
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

	.file-color-btn {
		cursor: pointer;
		position: relative;
		flex-shrink: 0;
		background: none;
		border: none;
		padding: 0;
	}

	.file-color-dot {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		display: block;
		border: 2px solid rgba(0, 0, 0, 0.15);
		transition: border-color 0.15s;
	}

	.file-color-btn:hover .file-color-dot {
		border-color: rgba(0, 0, 0, 0.4);
	}

	.file-color-hidden {
		position: absolute;
		opacity: 0;
		width: 0;
		height: 0;
		overflow: hidden;
		pointer-events: none;
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
