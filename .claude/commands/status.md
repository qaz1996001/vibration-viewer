Show the current project status:

1. Run `git status` to show modified/untracked files
2. Run `git log --oneline -5` to show recent commits
3. Run `cd src-tauri && cargo clippy -- -D warnings 2>&1 | tail -5` for Rust health
4. Summarize what's changed since last commit and any outstanding issues
