# Devid

Native local PHP development app for macOS (Herd-like, no Homebrew, no VM). Rust workspace (`daemon`, `cli`, `helper`). v1 has a CLI and a TUI, with no GUI (D-012).

## Read first

- `docs/README.md` is the entry point. Start with `docs/structure-of-app/PROJECT_CONTEXT.md`, then `ARCHITECTURE.md` and `DECISIONS.md`.
- The docs are the source of truth for plans and decisions. Update them instead of this file.
- The approved UI prototype lives outside this repo at `/Users/meowpush/Projects/devid-prototype/index.html`.

## Current stage

Planning. No code exists yet. The next step is the Phase 1 spike; its risks and pass criteria are in PROJECT_CONTEXT. Put spike work in `spike/`.

## Hard constraints

- Target machine: Intel Mac, 2 cores, 16 GB RAM, macOS 14.8. Idle RAM, CPU and battery cost matter in every design choice.
- No Homebrew and no VM dependency at runtime.
- PHP 8.3, 8.4 and 8.5 (see D-009), static builds, php-fpm with `pm = ondemand`.
- One global web server: nginx by default, Caddy or Apache as alternatives (D-015).
- Databases are out of scope (the owner uses DBngin).

## Working rules

- Reply to the owner in Bahasa Indonesia. Code, comments and docs are in English.
- Work in a git worktree per task, so every change is auditable on its own branch and PR. Use `git worktree add ../devid-worktrees/<branch> -b <branch>`, open a PR to `main` and merge from there. Do not commit directly to `main`; the only exception was the initial commit. Gitignored build caches (for example `spike/php/source`) stay in the main checkout; point to them instead of rebuilding them in a worktree.
- Commit and push only on request. Do not add a "Generated with Claude Code" line to PR or issue text.
- Ask before any installer or system-level change: sudo, LaunchDaemons, `/etc/resolver`, CA trust, or stopping Valet.
- Do not break the running cloudflared tunnel (`com.cloudflare.cloudflared`) or the 9 Valet sites until migration is planned.
- When a decision is made or reversed, append it to `docs/structure-of-app/DECISIONS.md`. Never delete an entry; mark it superseded.
