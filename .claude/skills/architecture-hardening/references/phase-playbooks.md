# Phase playbooks — methodology & heuristics

Detailed execution guidance for each phase of `architecture-hardening`. Read the matching section before running a phase. `SKILL.md` has the short version and success criteria; this file has the how and the judgment calls.

Throughout: **cite evidence** (file path, doc section, or prior finding). An assertion without a pointer is a Phase-2 finding waiting to happen.

---

## Phase 1 — Mapping & Visualization

### Order of operations
1. Read the orientation seed (`references/repo-map.md` or project equivalent), then any existing `docs/architecture/architecture-map.md`. You are updating, not regenerating.
2. Inventory major components, runtime boundaries, data stores, and communication surfaces from the code and existing docs.
3. Identify the project’s first-class architectural concerns (the load-bearing boundaries and invariants). These become the spine of the map.
4. Draw or refresh the core diagrams (see `diagram-cookbook.md`).
5. Create or refresh the extension-points registry.
6. Bump version and changelog.

### Diagrams to maintain
- **Component / context map** — major pieces and who talks to whom.
- **Data flow** — how primary data moves through the system.
- **State machine** — one important lifecycle (pick the most load-bearing one).
- **Sequence** — one representative end-to-end interaction that reveals the important hops or contracts.

### Heuristics
- Prefer stable identifiers (type names, module names, message constants, store names) over line numbers.
- One altitude per diagram. If it needs more than ~15 nodes, split it.
- The architecture map is an index and cross-cutting view, not a re-host of subsystem internals. Link to canonical docs; do not restate them.
- Render-check every Mermaid diagram.

### Done when
You can answer “where would a new X live?” for the kinds of extension the project actually supports, using only the map + extension-points registry.

---

## Phase 2 — Deep Understanding & Self-Critique

The value here is honesty about uncertainty. A confident-sounding map that is subtly wrong costs more than an explicit “I’m not sure about Y.”

### Invariant extraction
For each first-class concern, write the invariant as a falsifiable sentence and cite its enforcer (code or doc). If the invariant is only informally enforced, say so — that is itself a finding.

### Money-path traces (do ≥2)
Walk a real important outcome end-to-end and verify the code matches the map. Choose paths that cross the most interesting boundaries.

### Self-critique checklist (write the answers down)
- Assumptions I could not verify.
- Places two docs (or a doc and the code) disagree.
- Coupling that surprised me.
- What breaks if I change X (for each major seam).
- Confidence rating (High / Med / Low) per major area, with evidence.

Open ADR stubs for any decision that is needed but not yet made.

---

## Phase 3 — Targeted Hardening

### Candidate generation
Start from Phase-2 Low-confidence items and any recurring problem patterns visible in history or code. Every candidate must cite evidence.

### Scoring
Rough ROI: (impact on upcoming work × likelihood of pain) ÷ cost. Keep only the high-ROI few. Explicitly name at least one tempting larger change you are rejecting and why.

### Scoping a hardening item
Each kept item needs:
- The invariant it protects.
- Blast radius.
- A concrete way to verify it later.
- Explicit **Out of scope / Non-goals**.

If a feature is about to be added, show how the hardening makes that integration safer or cheaper.

---

## Phase 4 — Living Documentation

### Placement
New architecture docs live under `docs/architecture/`. Maintain a README index. Each living doc should declare that it wins on its topic.

### Versioning
`vMAJOR.MINOR` + dated changelog. Bump MINOR for additive refreshes; MAJOR when a fundamental boundary or concern is added or removed.

### Carry-forward block
End major docs with a short, paste-ready “Resume in a new chat” section (≤20 lines) that re-seeds a cold session with the current state and open questions.

### Update triggers
List the events that should force a re-run of one or more phases (new major component, new communication surface, new data ownership, recurring class of bugs, large refactor, etc.).

---

## Feature-integration mode

When the user is about to add a feature:
1. Find the intended extension point.
2. Check the feature against every first-class concern / invariant.
3. Name the data, message, and state surfaces it will touch.
4. Propose only the minimal hardening that de-risks *this* addition.
5. Emit an ADR for the integration decision.
