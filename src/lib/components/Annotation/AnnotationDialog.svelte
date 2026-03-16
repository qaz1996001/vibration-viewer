<script lang="ts">
	import { createEventDispatcher } from 'svelte';

	const dispatch = createEventDispatcher<{
		confirm: { label: string; color: string };
		cancel: void;
	}>();

	let label = '';
	let color = '#ff6b6b';

	const presetColors = ['#ff6b6b', '#4ecdc4', '#45b7d1', '#f9ca24', '#6c5ce7', '#a29bfe'];

	function handleSubmit() {
		if (label.trim()) {
			dispatch('confirm', { label: label.trim(), color });
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			dispatch('cancel');
		}
	}
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="overlay" on:click={() => dispatch('cancel')} role="presentation">
	<div class="dialog" on:click|stopPropagation role="presentation">
		<h3>New Annotation</h3>

		<form on:submit|preventDefault={handleSubmit}>
			<div class="field">
				<label for="ann-label">Label</label>
				<input id="ann-label" type="text" bind:value={label} placeholder="Annotation label..." />
			</div>

			<div class="field">
				<label>Color</label>
				<div class="color-picker">
					{#each presetColors as c}
						<button
							type="button"
							class="color-swatch"
							class:selected={color === c}
							style="background: {c}"
							on:click={() => (color = c)}
						></button>
					{/each}
					<input type="color" bind:value={color} class="color-input" />
				</div>
			</div>

			<div class="actions">
				<button type="button" class="btn-cancel" on:click={() => dispatch('cancel')}>
					Cancel
				</button>
				<button type="submit" class="btn-confirm" disabled={!label.trim()}>Confirm</button>
			</div>
		</form>
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
		z-index: 1000;
	}

	.dialog {
		background: white;
		border-radius: 8px;
		padding: 1.5rem;
		width: 360px;
		box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
	}

	h3 {
		margin: 0 0 1rem;
		font-size: 1.1rem;
	}

	.field {
		margin-bottom: 1rem;
	}

	.field label {
		display: block;
		font-size: 0.85rem;
		font-weight: 500;
		margin-bottom: 0.3rem;
	}

	input[type='text'] {
		width: 100%;
		padding: 0.5rem;
		border: 1px solid var(--border, #ccc);
		border-radius: 4px;
		font-size: 0.9rem;
		box-sizing: border-box;
	}

	.color-picker {
		display: flex;
		gap: 0.4rem;
		align-items: center;
	}

	.color-swatch {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		border: 2px solid transparent;
		cursor: pointer;
	}

	.color-swatch.selected {
		border-color: #333;
	}

	.color-input {
		width: 28px;
		height: 28px;
		padding: 0;
		border: none;
		cursor: pointer;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
		margin-top: 1.5rem;
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
