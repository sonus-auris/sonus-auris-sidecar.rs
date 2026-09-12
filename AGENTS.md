# sonus-auris — sonus-auris-sidecar.rs

Canonical `sonus-auris-sidecar.rs` for [`sonus-auris`](https://github.com/sonus-auris).

Inherits shared runtime from [`ores-otel/ores-otel-sidecar.rs`](https://github.com/ores-otel/ores-otel-sidecar.rs)
via zed-pkg (`ores-otel/ores-otel-sidecar`) and Cargo git.

- GitHub organization: https://github.com/sonus-auris
- This repository: https://github.com/sonus-auris/sonus-auris-sidecar.rs
- Linear project: https://linear.app/denman/project/githubcomsonus-auris-a557165528ef
- GitHub org project: https://github.com/orgs/sonus-auris/projects/1
- Package / service name: `sonus-auris-sidecar`
- Auth: github.com/shared-auth
- Sync: github.com/opto-sync
- Telemetry: github.com/ores-otel
- Flags: github.com/flags-2-env
- Packages: github.com/zed-pkg
- Never use React/JSX or webviews.
- Resolve git conflicts semantically; never rebase, stash, or reset.

## Repository-local Git worktrees

- Create or use a Git worktree only when the human operator explicitly authorizes it for the current task. Concurrency or a dirty checkout is not permission by itself.
- Put every authorized worktree at `<repository-root>/tmp/worktrees/<name>`; from the repository root, use `./tmp/worktrees/<name>`. Never place worktrees beside repositories or organization directories.
- Keep `tmp`, `temp`, `tmp/worktrees`, and `temp/worktrees` ignored in the repository-root `.gitignore`. Do not commit files from those directories.
- Relocate or remove a worktree only when the operator explicitly requests it. Before removal, preserve and publish intended changes, verify its commit is represented on the target branch, and confirm there are no tracked, untracked, ignored-sensitive, or in-use files that must survive. Remove it with `git worktree remove <path>` without `--force`; never delete a worktree directory with `rm`.
