<script lang="ts">
	/**
	 * ViewportDataTable - 視窗範圍資料表格
	 *
	 * 以表格形式顯示當前 chart viewport 中的 TimeseriesChunk 資料。
	 * 欄位由 chunk.channels 動態決定（支援任意數量的 data columns）。
	 * 預設僅顯示前 200 筆，避免大量 DOM 節點影響效能。
	 * 以 <details> 包裹，預設收合。
	 */
	import type { TimeseriesChunk } from '$lib/types/vibration';
	import { formatTime } from '$lib/utils/formatTime';

	interface Props {
		/** 當前 viewport 的時序資料切片（含 time[] 與 channels Record） */
		chunk: TimeseriesChunk;
		/** 表格最大顯示行數，超過則截斷並顯示提示。預設 200 */
		maxRows?: number;
	}

	let { chunk, maxRows = 200 }: Props = $props();

	/** $derived: 從 chunk.channels 取得所有通道名稱作為表頭 */
	let channelNames = $derived(Object.keys(chunk.channels));
	/** $derived: chunk 中的總資料點數 */
	let totalRows = $derived(chunk.time.length);
	/** $derived: 實際顯示行數（取 totalRows 與 maxRows 的較小值） */
	let displayedRows = $derived(Math.min(totalRows, maxRows));
</script>

<div class="data-table-section">
	<details>
		<summary>Data Points ({totalRows.toLocaleString()})</summary>
		<div class="table-scroll">
			<table>
				<thead>
					<tr>
						<th>Time</th>
						{#each channelNames as name}
							<th>{name}</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each { length: displayedRows } as _, i}
						<tr>
							<td>{formatTime(chunk.time[i])}</td>
							{#each channelNames as name}
								<td>{chunk.channels[name][i].toFixed(6)}</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<div class="row-info">
			{displayedRows.toLocaleString()} / {totalRows.toLocaleString()} rows
			{#if totalRows > maxRows}
				<span class="truncated-note">(showing first {maxRows})</span>
			{/if}
		</div>
	</details>
</div>

<style>
	.data-table-section {
		margin-top: 1rem;
	}

	details {
		border: 1px solid var(--border, #e0e0e0);
		border-radius: 4px;
		overflow: hidden;
	}

	summary {
		padding: 0.5rem 0.75rem;
		background: var(--surface, #fafafa);
		cursor: pointer;
		font-weight: 600;
		font-size: 0.9rem;
	}

	.table-scroll {
		max-height: 300px;
		overflow-y: auto;
	}

	table {
		width: 100%;
		border-collapse: collapse;
		font-size: 0.85rem;
	}

	th,
	td {
		padding: 0.4rem 0.6rem;
		text-align: right;
		border-bottom: 1px solid var(--border, #eee);
	}

	th {
		background: var(--surface, #f5f5f5);
		font-weight: 600;
		position: sticky;
		top: 0;
		z-index: 1;
	}

	td:first-child,
	th:first-child {
		text-align: left;
	}

	.row-info {
		padding: 0.4rem 0.75rem;
		font-size: 0.8rem;
		color: var(--text-secondary, #666);
		border-top: 1px solid var(--border, #eee);
	}

	.truncated-note {
		color: var(--text-tertiary, #999);
	}
</style>
