# Devid Documentation

> Scope: entry point for all Devid docs
> Verified against: 2026-11-25 (v0.20.1)
> Status: current

Devid is a native local PHP development app for macOS and Linux (Herd-like, no Homebrew, no VM). These docs record the plans, the decisions and how the code in `crates/` works. To contribute, start with [CONTRIBUTING.md](../CONTRIBUTING.md).

## Start here

1. [Project context](structure-of-app/PROJECT_CONTEXT.md): why Devid exists, scope, constraints, phases, machine state.
2. [Architecture](structure-of-app/ARCHITECTURE.md): components and runtime flow.
3. [Decisions](structure-of-app/DECISIONS.md): accepted and proposed decisions with rationale.

## Plans and records

- [Phase 2 plan](roadmap/phase-2.md): milestones M1–M7 and technical choices (done).
- [Phase 5 plan](roadmap/phase-5.md): build pipeline, releases and updates (R1–R5, done).
- [Phase 4 plan](roadmap/phase-4.md): the TUI (T1–T5, done).
- [Phase 6 plan](roadmap/phase-6.md): extra PHP extensions from a trusted catalog (D-021; done 2026-11-05).
- [Phase 7 plan](roadmap/phase-7.md): the desktop app, Tauri v2 on the daemon API (D-023; G1–G5, done 2026-11-06).
- [Phase 8 plan](roadmap/phase-8.md): app types, user commands, a local port per site and Share (D-035 to D-037; accepted 2026-11-08, done 2026-11-09 in v0.12.1).
- [Phase 9 plan](roadmap/phase-9.md): Mail, a mail catcher that runs Mailpit, in two stages (D-049; accepted 2026-11-16, done in 0.15.0).
- [Phase 10 plan](roadmap/phase-10.md): dev servers on demand, started by a site's first request and stopped when idle (D-054, accepted 2026-11-21; O2 in 0.16.1; O3 to O5 in 0.17.0, released 2026-11-22).
- [Phase 11 plan](roadmap/phase-11.md): Laravel + Vite over HTTPS, so a Laravel site's Vite dev server works in Safari too (D-056, accepted 2026-11-22; done in 0.18.0, released 2026-11-22).
- [Phase 12 plan](roadmap/phase-12.md): Linux, starting with Ubuntu 24.04 and 26.04 LTS on x86_64 with the CLI and the TUI (D-057, accepted 2026-11-23; done in 0.19.0, released 2026-11-24).
- [Phase 13 plan](roadmap/phase-13.md): Arch-based Linux, extending 0.19's Linux support from Ubuntu to Arch and its derivatives (D-059, accepted 2026-11-24; done in 0.20.0, released 2026-11-24).
- [Phase 14 plan](roadmap/phase-14.md): databases, with Devid running D-058's MySQL, PostgreSQL and MongoDB builds (D-060, D-061, accepted 2026-11-25; D2 and D3 shipped in 0.20.1, off by default; done in 0.21.0, released 2026-11-26).
- [Phase 15 plan](roadmap/phase-15.md): creating projects with `devid new`, from each framework's official tool, served at `https://<name>.test` with their database, Mail and dev-server commands (D-063; accepted 2026-11-25; N1 to N4 built 2026-11-26: the spike, the daemon and the CLI, the Node.js templates and the TUI's wizard, the app's New project sheet; N5's weekly check of every template and the guide's page 2026-11-26; rc.1 passed on the M1 and the Ubuntu, Arch and CachyOS VMs and rc.2, with the owner's changes to the sheet, on the M1 on 2026-11-27; done in 0.22.0).
- [PHP 8.6](implementation-timeline/daemon/2026-11-25-php-8.6.md): 8.6 joins 8.3, 8.4 and 8.5 in 0.20.1, built before static-php-cli supports it, with 8.5 still the default (D-062, accepted 2026-11-25; done in 0.20.1, released 2026-11-25).
- [Implementation timeline](implementation-timeline/README.md): one record per merged milestone.

## Research

- [Competitors, 2026-11](research/competitors-2026-11.md): pricing, limits and complaints of Herd, Valet, ServBay, MAMP, Local, Laragon, FlyEnv, Yerd, EnvKit, DDEV, Lando and others, with sources; the opportunities it suggests for Devid are proposed, not accepted.

## External sources

- User guide (public, D-034): [devid.bekenweb.com/docs](https://devid.bekenweb.com/docs), from the public `devid-site` repository (Next.js + Fumadocs on Vercel), with the landing page at `/`. User-facing changes update it there; this repository's README links to it.
- UI prototype (approved, D-026): the Claude Design board "Devid — macOS style", source copied to `/Users/meowpush/Projects/devid-prototype/redesign/`. The logo canvas is copied to `/Users/meowpush/Projects/devid-prototype/logo/`. Both live outside this repo. The first prototype, `index.html` there, is history.
- Original working notes: `/Users/meowpush/Projects/devid-prototype/CLAUDE.md`. These docs supersede it once they diverge.

## Conventions

- Label every claim as implemented, accepted, proposed or unknown.
- Update `DECISIONS.md` when a decision is made or reversed. Never delete an entry; mark it superseded instead.
- Add a record under `implementation-timeline/` for each merged milestone, and update the "Verified against" headers that the change touches.
