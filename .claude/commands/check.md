Run all project checks and fix any errors found. Run these in sequence:

1. `cd src-tauri && cargo clippy -- -D warnings` — Rust lint
2. `cd src-tauri && cargo test` — Rust unit tests
3. `npx svelte-check` — Svelte/TypeScript checks

For each step:
- If errors are found, fix them before proceeding to the next step
- Ignore the known pre-existing TS error in `src/routes/+page.svelte:60` (debounce type mismatch)
- Report results concisely
