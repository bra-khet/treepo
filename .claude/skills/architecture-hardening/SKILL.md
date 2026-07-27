---
name: architecture-hardening
description: Map, audit, and harden project architecture and keep living versioned diagram-rich docs in docs/architecture/ that survive context loss between sessions. Use WHENEVER the user wants to understand or document how the project fits together; trace data message or state flow; review maintainability tech debt coupling or key boundaries; plan how a NEW feature will integrate BEFORE building it; or asks for an architecture diagram ADR extension-point map or a get-me-back-up-to-speed summary. Also trigger on architecture map harden tech-debt audit how does X flow where does state live message contracts extension points or right before a large refactor. Prefer this skill over ad-hoc exploration so hard-won context is captured durably.
---

# Architecture Analysis & Hardening

A repeatable four-phase workflow for understanding, documenting, and incrementally hardening a project's architecture as it grows.

The whole point of this skill is **carry-forward**. Every artifact is written for a future reader (human or AI) who has no memory of the current conversation. Prefer extending existing living docs over re-deriving the map from scratch.

## Prime directive: don't re-derive, extend

Before exploring code, read what already exists:

1. `references/repo-map.md` (or the project’s equivalent orientation seed) — the durable skeleton.
2. `docs/architecture/architecture-map.md` — the living, versioned map this skill maintains (Phase 1 creates it on first run).
3. Any canonical subsystem or design docs that already own a topic. Those docs win on their subject; do not duplicate them.

If code contradicts existing docs, that is a **finding**, not a fact to silently absorb. Surface it in Phase 2.

## Four phases (run all, or name one)

Each phase has a Goal, Steps, Artifacts, and Success criteria. Detailed methodology lives in `references/phase-playbooks.md`. Mermaid patterns live in `references/diagram-cookbook.md`.

### Phase 1 — Architecture Mapping & Visualization

**Goal:** A referenceable, visual, current map that a newcomer can use to locate any major subsystem quickly.

**Steps:**
1. Re-orient from the repo-map / orientation seed and any existing living map.
2. Inventory major components, execution or runtime boundaries, data stores, and communication surfaces.
3. Draw or refresh core diagram families with Mermaid (context / component map, data flow, key state machine, representative sequence).
4. Document the project’s first-class architectural concerns (the load-bearing boundaries and invariants that define the system). These become the spine of the map.
5. Maintain or create an **extension-points registry** listing the intentional seams where new capabilities should plug in.
6. Bump the map version and add a dated changelog entry.

**Artifacts:** `docs/architecture/architecture-map.md` (versioned, Mermaid embedded) · `docs/architecture/extension-points.md` (versioned).

**Success criteria:** Major components, boundaries, and data/ownership surfaces appear in the map. First-class concerns have current sections. Diagrams are spot-checked against code. Version bumped; changelog dated; a short “Resume in a new chat” block is present.

### Phase 2 — Deep Understanding & Self-Critique

**Goal:** Move past description to load-bearing invariants and be honest about where understanding is thin.

**Steps:**
1. For each first-class concern, state the invariant in one falsifiable sentence and cite the enforcing code or doc.
2. Trace at least two important end-to-end paths (“money paths”) and confirm the map predicts the code.
3. Self-critique explicitly: unverified assumptions, places docs disagree, surprising coupling, “what breaks if I change X.” Rate confidence per major area High / Med / Low with evidence.
4. Record results as Invariants + Confidence ledger sections in the map. Open ADR stubs for anything that needs a decision.

**Artifacts:** Invariants and Confidence ledger in the map · ADR stubs in `docs/architecture/adr/`.

**Success criteria:** Every first-class concern has a one-line invariant + reference. ≥2 documented money-path traces. An explicit Low-confidence list exists — no silent guesses.

### Phase 3 — Targeted Hardening (scoped, not blanket)

**Goal:** The smallest changes that remove a class of future bugs or make the next feature cheaper. Anticipatory, high-ROI hardening — not a refactor crusade.

**Steps:**
1. Generate candidates from Phase-2 Low-confidence items and any recurring problem patterns. Each candidate cites evidence.
2. Score roughly by (impact on upcoming work × likelihood) ÷ cost. Keep only the high-ROI few.
3. For each kept item, scope a surgical change: the invariant it protects, blast radius, a verification approach, and an explicit **Out of scope / Non-goals** line that names the over-engineering being rejected.
4. If a feature is planned, show how each item makes that integration cleaner.
5. Produce or update a ranked Hardening Backlog.

**Artifacts:** `docs/architecture/hardening-backlog.md` · ADRs for structural choices.

**Success criteria:** Every item cites evidence. Every item has explicit scope and non-goals. Nothing is a multi-sprint rewrite; at least one tempting-but-rejected over-engineering is named with its reason.

### Phase 4 — Living-Documentation Standards & Update Process

**Goal:** Guarantee artifacts stay current and survive context loss.

**Steps:**
1. Place new architecture docs under `docs/architecture/`. Link them into an index. Give each a clear “this file wins on its topic” header.
2. Version living docs (`vMAJOR.MINOR` + dated changelog). Registries may version independently.
3. Enumerate update triggers (new major component, new communication surface, new data store, recurring bug class, etc.).
4. End major docs with a short “Resume in a new chat” block that re-seeds a cold session.
5. Update-don’t-duplicate: before creating any doc, check existing docs; extend the canonical owner; only create a new file for a genuinely new concern.

**Artifacts:** `docs/architecture/README.md` (index + standards) · carry-forward blocks on living docs.

**Success criteria:** Index links resolve; every living doc has version + changelog + carry-forward block. Update triggers are listed. No parallel/duplicate docs created.

## Feature-integration mode (cross-phase)

When the user is about to add a feature, run focused analysis instead of a full sweep:
- Locate the extension point it should use.
- Check it against the first-class concerns and invariants.
- Name the data, message, or state surfaces it touches.
- Emit only the Phase-3 hardening that de-risks *this* addition.
- Output an ADR proposing the integration.

## Update-don’t-duplicate rules

- Canonical docs win on their topic. Cross-link; never restate their content inside the architecture map.
- One topic → one home. Bump version inside the existing file; do not create `architecture-map-v2.md`.
- When superseding older notes, state clearly which file is current and which remains historical.
- Keep an outbound index in `docs/architecture/README.md`.

## Invocation templates

- Full pass: “Run architecture-hardening — refresh the map and give me the confidence ledger + hardening backlog.”
- Phase 1 only: “architecture-hardening Phase 1 — refresh the architecture map and diagrams.”
- Phase 3 only: “architecture-hardening Phase 3 — I’m about to add <feature>; give me scoped evidence-backed hardening with non-goals.”
- Feature integration: “architecture-hardening feature-integration — analyze how <feature> fits and propose an ADR.”
- Cold resume: “architecture-hardening resume — read docs/architecture/ and summarize current architecture state and open questions in ~15 lines.”

## Files in this skill

| Path | Read when |
|------|-----------|
| `references/repo-map.md` | Always first — how to build / maintain the durable orientation seed. |
| `references/phase-playbooks.md` | Before running any phase — full methodology and heuristics. |
| `references/diagram-cookbook.md` | When drawing or refreshing Mermaid diagrams. |
| `assets/architecture-map.template.md` | First Phase-1 run — copy to `docs/architecture/architecture-map.md`. |
| `assets/extension-points.template.md` | First extension-points registry run. |
| `assets/adr.template.md` | Every ADR. |
