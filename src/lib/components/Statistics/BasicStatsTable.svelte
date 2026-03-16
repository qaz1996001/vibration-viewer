<script lang="ts">
	import type { StatisticsReport } from '$lib/types/statistics';

	export let stats: StatisticsReport;
</script>

<div class="stats-tables">
	<details open>
		<summary>Basic Statistics</summary>
		<table>
			<thead>
				<tr>
					<th>Axis</th>
					<th>Count</th>
					<th>Mean</th>
					<th>Std Dev</th>
					<th>CV%</th>
				</tr>
			</thead>
			<tbody>
				{#each stats.basic as row}
					<tr>
						<td>{row.axis}</td>
						<td>{row.count.toLocaleString()}</td>
						<td>{row.mean.toFixed(6)}</td>
						<td>{row.std_dev.toFixed(6)}</td>
						<td>{row.cv_percent.toFixed(2)}%</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</details>

	<details>
		<summary>Distribution Statistics</summary>
		<table>
			<thead>
				<tr>
					<th>Axis</th>
					<th>Min</th>
					<th>Q1</th>
					<th>Median</th>
					<th>Q3</th>
					<th>Max</th>
					<th>IQR</th>
				</tr>
			</thead>
			<tbody>
				{#each stats.distribution as row}
					<tr>
						<td>{row.axis}</td>
						<td>{row.min.toFixed(6)}</td>
						<td>{row.q1.toFixed(6)}</td>
						<td>{row.median.toFixed(6)}</td>
						<td>{row.q3.toFixed(6)}</td>
						<td>{row.max.toFixed(6)}</td>
						<td>{row.iqr.toFixed(6)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</details>

	<details>
		<summary>Shape Statistics</summary>
		<table>
			<thead>
				<tr>
					<th>Axis</th>
					<th>Skewness</th>
					<th>Kurtosis</th>
				</tr>
			</thead>
			<tbody>
				{#each stats.shape as row}
					<tr>
						<td>{row.axis}</td>
						<td>{row.skewness.toFixed(6)}</td>
						<td>{row.kurtosis.toFixed(6)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</details>
</div>

<style>
	.stats-tables {
		margin-top: 1rem;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
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
		text-align: right;
	}

	td:first-child,
	th:first-child {
		text-align: left;
		font-weight: 500;
	}
</style>
