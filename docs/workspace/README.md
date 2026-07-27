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

The five original `*-draft.md` design documents were promoted to `../design/` on 2026-07-26
and no longer live here.
