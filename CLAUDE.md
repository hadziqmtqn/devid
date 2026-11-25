# Devid

Native local PHP development app for macOS (Herd-like, no Homebrew, no VM). Rust workspace (`daemon`, `cli`, `helper`). v1 has a CLI and a TUI, with no GUI (D-012).

## Read first

- `docs/README.md` is the entry point. Start with `docs/structure-of-app/PROJECT_CONTEXT.md`, then `ARCHITECTURE.md` and `DECISIONS.md`.
- The docs are the source of truth for plans and decisions. Update them instead of this file.
- The approved UI prototype lives outside this repo at `/Users/meowpush/Projects/devid-prototype/redesign/` (D-026).

## Current stage

Phase 1 (spike) is done. Phase 2 (daemon + CLI) is in progress; the milestones are in `docs/roadmap/phase-2.md`. Rust code lives in `crates/`. Before pushing, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test`; CI (`.github/workflows/ci.yml`, Ubuntu) runs the same checks on every PR. Spike scripts stay in `spike/`.

## Hard constraints

- Target machine (D-052): Apple M1, 8 cores, 8 GB RAM, macOS 26. Idle RAM, CPU and battery cost matter in every design choice, and RAM is the tightest. Intel Macs stay supported, so work that is heavy on 2 cores still needs a reason.
- No Homebrew and no VM dependency at runtime.
- PHP 8.3, 8.4, 8.5 and 8.6 (see D-009 and D-062), static builds, php-fpm with `pm = ondemand`.
- One global web server: nginx by default, Caddy or Apache as alternatives (D-015).
- Databases are out of scope (the owner uses DBngin).

## Working rules

- Reply to the owner in Bahasa Indonesia. Code, comments and docs are in English.
- Work in a git worktree per task, so every change is auditable on its own branch and PR. Use `git worktree add ../devid-worktrees/<branch> -b <branch>`, open a PR to `main` and merge from there. Do not commit directly to `main`; the only exception was the initial commit. Gitignored build caches (for example `spike/php/source`) stay in the main checkout; point to them instead of rebuilding them in a worktree.
- Commit and push only on request. Do not add a "Generated with Claude Code" line to PR or issue text.
- Ask before any installer or system-level change: sudo, LaunchDaemons, `/etc/resolver`, CA trust.
- Do not break the running cloudflared tunnel (`com.cloudflare.cloudflared`). It targets `php artisan serve` on `127.0.0.1:8000` and ports 3xxx. Valet was removed on 2026-11-03, and Devid serves every `.test` site.
- When a decision is made or reversed, append it to `docs/structure-of-app/DECISIONS.md`. Never delete an entry; mark it superseded.
