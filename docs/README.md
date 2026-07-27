# treepo — Documentation Map

`treepo` is a consumer desktop application that grows a living world-tree from a software
repository's real structure, size, age, churn, ownership, and activity.

This directory is the project's durable documentation set. Read this page first to know
which document answers which kind of question.

---

## Document hierarchy

| Layer | Document | Answers | Changes |
|-------|----------|---------|---------|
| **Enduring intent** | [`CONSTITUTION.md`](CONSTITUTION.md) — *ratified 2026-07-27* | Why this exists, what it must always be, what it must never become | Rarely, and deliberately |
| **Tactical intent** | [`PRD.md`](PRD.md) — *draft, under review* | What we build, in what order, and how we know it works | Every planning cycle |
| **Design** | [`design/`](design/) | How the thing actually works | Continuously, revised in place |

The Constitution and the PRD are complements, not substitutes. The Constitution holds
principles and boundaries; the PRD holds capabilities, acceptance criteria, and sequencing.
Neither document should contain the other's content.

**Precedence.** Where documents disagree on *intent*, the Constitution governs. Where they
disagree on *detail*, the relevant design document governs and the others are corrected to
match. Design documents are revised in place rather than superseded by new files.

---

## Design documents

All five are living documents, each authoritative for its own area.

- [`design/design-outline.md`](design/design-outline.md) — **start here.** The high-level
  living summary of the whole system, kept consistent with the four supplements below.
- [`design/feature-system.md`](design/feature-system.md) — the catalog of repository
  primitives, the lens model, and the Interaction Physics that turn measurements into
  appearance and behavior. The detailed source of truth for what data drives what.
- [`design/visual-construction.md`](design/visual-construction.md) — the hybrid trunk
  decision and the four-layer generative stack (Skeleton → Semantics → Enrichment → Thrive).
- [`design/engine-architecture.md`](design/engine-architecture.md) — the Grow / Thrive
  dual-phase contracts, triggers, cinematic diff behavior, State Sync, and Bevy notes.
- [`design/l-system-parameterization.md`](design/l-system-parameterization.md) — the
  structural skeleton's parameters, primitive→parameter mapping guidelines, and the
  decision menu for the first coherent parameter set.

### Reading order for a newcomer

1. [`CONSTITUTION.md`](CONSTITUTION.md) — what this is and what it refuses to be
2. [`design/design-outline.md`](design/design-outline.md) — the system end to end
3. [`PRD.md`](PRD.md) §§3–5 — what actually gets built, and to what standard
4. [`design/feature-system.md`](design/feature-system.md) §§1–3 — what gets measured
5. [`design/engine-architecture.md`](design/engine-architecture.md) §§1–4 — how it runs
6. Then the visual and L-system documents as needed

---

## Working conventions

- **Living, not versioned by filename.** Documents carry `Version:` and `Status:` in their
  own headers and are revised in place. Filenames stay stable so links do not rot.
- **Decisions land where they belong.** A durable decision about phase ownership goes in
  the engine document first; about primitives, the feature system; about identity or
  boundaries, the Constitution. The outline is then updated to stay consistent.
- **Resolved questions are kept, not deleted.** See `design-outline.md` §11 — recording
  what was decided and why is more valuable than a clean document.
- [`workspace/`](workspace/) is the staging area for raw material that has not yet been
  consolidated. Nothing there is authoritative.
