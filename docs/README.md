# Devid Documentation

> Scope: entry point for all Devid docs
> Verified against: 2026-11-02 (repository has no commits and no code yet)
> Status: current

Devid is a native local PHP development app for macOS (Herd-like, no Homebrew, no VM). The repository is at the planning stage: these docs record decisions and plans, not implemented behaviour.

## Start here

1. [Project context](structure-of-app/PROJECT_CONTEXT.md): why Devid exists, scope, constraints, phases, machine state.
2. [Architecture](structure-of-app/ARCHITECTURE.md): planned components and runtime flow (proposed, nothing built).
3. [Decisions](structure-of-app/DECISIONS.md): accepted and proposed decisions with rationale.

## Plans and records

- [Phase 2 plan](roadmap/phase-2.md): milestones M1–M7 and technical choices (done).
- [Phase 5 plan](roadmap/phase-5.md): build pipeline, releases and updates (R1–R5, done).
- [Phase 4 plan](roadmap/phase-4.md): the TUI (T1–T5, done).
- [Phase 6 plan](roadmap/phase-6.md): extra PHP extensions from a trusted catalog (D-021; done 2026-11-05).
- [Phase 7 plan](roadmap/phase-7.md): the desktop app, Tauri v2 on the daemon API (D-023; G1–G5, done 2026-11-06).
- [Implementation timeline](implementation-timeline/README.md): one record per merged milestone.

## External sources

- User guide (public, D-033): [`GUIDE.md` in devid-binaries](https://github.com/hadziqmtqn/devid-binaries/blob/main/GUIDE.md). User-facing changes update it there; this repository's README links to it.
- UI prototype (approved, D-026): the Claude Design board "Devid — macOS style", source copied to `/Users/meowpush/Projects/devid-prototype/redesign/`. The logo canvas is copied to `/Users/meowpush/Projects/devid-prototype/logo/`. Both live outside this repo. The first prototype, `index.html` there, is history.
- Original working notes: `/Users/meowpush/Projects/devid-prototype/CLAUDE.md`. These docs supersede it once they diverge.

## Conventions

- Label every claim as implemented, accepted, proposed or unknown. Until code lands, nothing is "implemented".
- Update `DECISIONS.md` when a decision is made or reversed. Never delete an entry; mark it superseded instead.
- Add a record under `implementation-timeline/` for each merged milestone, and update the "Verified against" headers that the change touches.
