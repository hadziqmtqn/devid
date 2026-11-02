# Devid Documentation

> Scope: entry point for all Devid docs
> Verified against: 2026-11-02 (repository has no commits and no code yet)
> Status: current

Devid is a native local PHP development app for macOS (Herd-like, no Homebrew, no VM). The repository is at the planning stage: these docs record decisions and plans, not implemented behaviour.

## Start here

1. [Project context](structure-of-app/PROJECT_CONTEXT.md): why Devid exists, scope, constraints, phases, machine state.
2. [Architecture](structure-of-app/ARCHITECTURE.md): planned components and runtime flow (proposed, nothing built).
3. [Decisions](structure-of-app/DECISIONS.md): accepted and proposed decisions with rationale.

## External sources

- UI prototype (approved): `/Users/meowpush/Projects/devid-prototype/index.html`. Lives outside this repo.
- Original working notes: `/Users/meowpush/Projects/devid-prototype/CLAUDE.md`. These docs supersede it once they diverge.

## Conventions

- Label every claim as implemented, accepted, proposed or unknown. Until code lands, nothing is "implemented".
- Update `DECISIONS.md` when a decision is made or reversed. Never delete an entry; mark it superseded instead.
- When the first code lands, add `implementation-timeline/` for change records and update the "Verified against" headers.
