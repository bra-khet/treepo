# Workspace — Raw Material

Staging area for design notes, session compactions, research, and drafts that have not yet
been consolidated into the durable documentation set.

**Nothing in this directory is authoritative.** Documents here may be incomplete,
contradictory, or superseded. Do not cite a workspace file as a source of truth, and do not
implement from one.

## Promotion path

1. Raw material lands here under any name.
2. When it stabilizes, it is folded into the relevant document in [`../design/`](../design/)
   — or promoted into its own design document with a stable filename and a
   `Version:` / `Status:` header.
3. Cross-references in the affected documents are updated, and
   [`../design/design-outline.md`](../design/design-outline.md) is reconciled so the set
   stays internally consistent.
4. The workspace copy is removed rather than left to drift.

## Promotion log

| Workspace draft | Promoted | Into |
|-----------------|----------|------|
| Five original `*-draft.md` design documents | 2026-07-26 | `../design/` |
| `engine-architecture-grow-staging-supplement.md` | 2026-07-27 | `../design/engine-architecture.md` v0.3; outline §4; `../PRD.md` v1.2 (`F-ASSOC-6`, `F-GROW-2`/`4`/`6`–`7`/`11`–`13`); `.planning` D11 + Phases 6–7/11–12 |
| `LICENSE-THIRD-PARTY.md` | 2026-07-27 | `/LICENSE-THIRD-PARTY.md` (repo root; MPL-2.0 attribution for `uluru`, verified against `Cargo.lock`) |
| `trunk-pipe-rework.md` | 2026-07-28 | `../design/visual-construction.md` v2.1 (hybrid trunk → pipe column via primary internodes). Draft retained in place, marked superseded, with the implementation's departures from it recorded — the reasoning is the record of why the first construction was replaced. |
