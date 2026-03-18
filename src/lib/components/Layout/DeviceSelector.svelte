<script lang="ts">
	/**
	 * DeviceSelector - AIDPS 裝置選擇器
	 *
	 * 當開啟 AIDPS 資料夾專案時，顯示掃描到的裝置清單。
	 * 每個裝置顯示名稱、CSV 來源檔案數量、channel group 摘要。
	 * 點擊裝置會切換 activeDeviceId，並通知父層觸發對應裝置的資料載入。
	 */
	import {
		project,
		activeDeviceId,
		selectDevice,
		deviceCount
	} from '$lib/stores/projectStore';
	import type { DeviceInfo } from '$lib/stores/projectStore';

	interface Props {
		/** 裝置被選取時的 callback，回傳 deviceId 供父層載入裝置資料 */
		onselect?: (deviceId: string) => void;
	}
	let { onselect }: Props = $props();

	/** 切換選取的裝置：更新 activeDeviceId store 並通知父層 */
	function handleSelect(deviceId: string) {
		selectDevice(deviceId);
		onselect?.(deviceId);
	}

	/** 產生裝置的 CSV 來源檔案數量標籤（含英文複數處理） */
	function sourceCountLabel(device: DeviceInfo): string {
		const count = device.sources.length;
		return `${count} CSV file${count !== 1 ? 's' : ''}`;
	}

	/** 產生裝置的 channel group 名稱摘要（例如 "acceleration, velocity"） */
	function channelGroupSummary(device: DeviceInfo): string {
		const groups = Object.keys(device.channel_schema.groups);
		if (groups.length === 0) return '';
		return groups.join(', ');
	}
</script>

{#if $project && $project.devices.length > 0}
	<div class="device-selector">
		<h3>裝置 ({$deviceCount})</h3>
		{#each $project.devices as device (device.id)}
			<div
				class="device-item"
				class:active={$activeDeviceId === device.id}
				role="button"
				tabindex="0"
				onclick={() => handleSelect(device.id)}
				onkeydown={(e) => e.key === 'Enter' && handleSelect(device.id)}
			>
				<div class="device-icon">
					<span class="device-dot"></span>
				</div>
				<div class="device-info">
					<span class="device-name">{device.name}</span>
					<span class="device-meta">{sourceCountLabel(device)}</span>
					{#if channelGroupSummary(device)}
						<span class="device-channels">{channelGroupSummary(device)}</span>
					{/if}
				</div>
			</div>
		{/each}
	</div>
{/if}

<style>
	.device-selector {
		padding: 0.5rem;
		border-bottom: 1px solid var(--border, #e0e0e0);
	}

	h3 {
		margin: 0 0 0.5rem;
		padding: 0 0.5rem;
		font-size: 0.95rem;
	}

	.device-item {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
		padding: 0.5rem;
		border-radius: 4px;
		cursor: pointer;
	}

	.device-item:hover {
		background: var(--surface-hover, #f0f0f0);
	}

	.device-item.active {
		background: var(--surface-active, #e0e0ff);
	}

	.device-icon {
		flex-shrink: 0;
		padding-top: 0.15rem;
	}

	.device-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		display: block;
		background: var(--primary, #4a90d9);
		border: 2px solid rgba(0, 0, 0, 0.15);
	}

	.device-item.active .device-dot {
		background: var(--primary, #4a90d9);
		border-color: var(--primary, #4a90d9);
		box-shadow: 0 0 0 2px rgba(74, 144, 217, 0.3);
	}

	.device-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-width: 0;
	}

	.device-name {
		font-size: 0.85rem;
		font-weight: 500;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.device-meta {
		font-size: 0.75rem;
		color: var(--text-secondary, #666);
	}

	.device-channels {
		font-size: 0.7rem;
		color: var(--text-secondary, #999);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
