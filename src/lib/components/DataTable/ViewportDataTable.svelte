<script lang="ts">
	import type { TimeseriesChunk } from '$lib/types/vibration';

	interface Props {
		chunk: TimeseriesChunk;
		maxRows?: number;
	}

	let { chunk, maxRows = 200 }: Props = $props();

	function formatTime(epochSeconds: number): string {
		const date = new Date(epochSeconds * 1000);
		const MM = String(date.getMonth() + 1).padStart(2, '0');
		const DD = String(date.getDate()).padStart(2, '0');
		const hh = String(date.getHours()).padStart(2, '0');
		const mm = String(date.getMinutes()).padStart(2, '0');
		const ss = String(date.getSeconds()).padStart(2, '0');
		return `${MM}/${DD} ${hh}:${mm}:${ss}`;
	}

	let channelNames = $derived(Object.keys(chunk.channels));
	let totalRows = $derived(chunk.time.length);
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
