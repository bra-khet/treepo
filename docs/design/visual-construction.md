# Core Visual Construction
**Design Document — Supplemental Living Section**  
**Version:** 2.1 (originated as a context compaction of the Core Visual Construction + Architecture thread)  
**Status:** Active draft — authoritative for the hybrid trunk decision and the layered generative architecture  
**Last updated:** 2026-07-28  

**v2.1 — the hybrid trunk's second construction.** The hybrid decision stands; how mass becomes
a column does not. The first implementation was *co-origin* — one minimal basal segment, every
primary leaving its tip, trunk mass purely their overlap — and the first silhouettes
(`tools/m0-silhouette`) showed it failing: a wide fan leaves the same fraction of a stem-width
of overlap however many limbs there are, so the base read as an oversized seed with rays coming
out of it. A point is not a support column. Replaced by a **pipe column grown from primary
internodes**; see the trunk entries below. Promoted from
[`../workspace/trunk-pipe-rework.md`](../workspace/trunk-pipe-rework.md).

Companion documents: [`design-outline.md`](design-outline.md) (high-level summary), [`feature-system.md`](feature-system.md) (primitives + Interaction Physics), [`engine-architecture.md`](engine-architecture.md) (Grow / Thrive), [`l-system-parameterization.md`](l-system-parameterization.md) (skeleton parameters). Enduring intent and constraints live in [`../CONSTITUTION.md`](../CONSTITUTION.md).

---

**Project name:** treepo  
*Locked 2026-07-27 — see [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5.*

Active goal: Design and validate the core visual construction and high-level architecture for a single magical world-tree menagerie that procedurally represents a Git repository, with L-systems as foundation, layered enrichment for character, and dual Grow/Thrive engine.

## Working State
Design phase focused exclusively on core visual construction and supporting architecture. Trunk approach settled on hybrid, and as of v2.1 built as a data-driven pipe column grown from primary internodes (see the trunk entries below; the original co-origin reading failed its first silhouettes). Layered generative architecture defined and accepted as the way to add meaningful character (structures, activity, inhabitants) without overloading the L-system. Next concrete work is defining the first semantic tags, enrichment rules, and L-system parameter mappings. Broader multi-theme, agent-monitoring, and Steam packaging topics are out of scope for this thread.

## Decisions & Constraints
- Single scene type for MVP and near-term: large lively magical world-tree menagerie with possible base/canopy additions (source: user narrowing, mid-thread).
- Trunk construction: hybrid. Nothing draws an *arbitrary* trunk; the trunk is a **support column induced by primary internodes**, with width from the remaining pipe support plus a base flare, and primaries inserting as **knots** along that column. Pure trunkless “just stack independent branches” rejected for L-system compatibility, redraw stability, and readable silhouettes (source: SWOT + user confirmation that axiom is required); a constant dedicated trunk rejected because every repository would then share the silhouette a viewer reads first.
- Trunk width is a **projection, not a measurement** (`P6`). Support past a knee counts only in part, so a forty-directory monorepo does not draw a telephone pole. The ordering is kept strictly — a bigger repository always draws a wider base — and only the rate is bounded.
- L-systems retained as foundation for structural skeleton because they deliver recursive organic complexity from simple parameterized rules; user prefers systems that “act interestingly” over exhaustive predefinition.
- Generative architecture is strictly layered:
  1. Structural Skeleton (L-system / hybrid) — topology, limb geometry, thickness, angles from hierarchy + size/depth/mass primitives.
  2. Semantic Role / Annotation — tags on limbs/subtrees (docs, tests, high-churn, core, owned-by-X, etc.).
  3. Enrichment / Decoration — rule- or secondary-generator-driven placement of treehouses, platforms, foliage variation, fruits, landing pads, material details, scars, etc., driven by tags + local primitives.
  4. Thrive / Live — inhabitants, particles, continuous animation, event reactions.
- Herringbone Wang-style tiles (or equivalent constraint tiles) for organic surface/internal patterning of limbs and structures; hierarchical path-hash seeds for determinism.
- Age and churn are first-class primitives that can read positively (patina, vitality) or negatively (brittleness, scarring) according to context.
- Git branches (VCS feature branches) deferred; first version visualizes only the current working tree / default branch. Side-shoots or dramatic parallel growth possible later.
- Local working tree is the primary target (with sensible ignores for artifacts/node_modules). Remote/clean history as optional later twin view / multi-context.
- Aesthetic + intuitive mental-model formation is the primary goal; academic precision or immediate actionability is secondary.
- Grow phase (long tick): full/incremental scan, primitive extraction, skeleton + tags + enrichment rebuild, classification-shift transformations. Thrive phase (short tick): cheap animation, workers, live events.
- Empty/sparse repo correctly shows minimal or no structure (seed / small root cluster), not a lonely trunk.

## Open Tasks & Blockers
1. Define the first concrete set of semantic tags and the corresponding enrichment rules (e.g., docs → library treehouse probability + visual parameters; high-churn → activity particles / restless foliage).
2. Specify L-system parameters and how primitives map into them (basal segment length/radius, primary limb count/angles/thickness, recursion depth, etc.).
3. Detail the exact data contracts and triggers for Grow vs Thrive phases.
4. Decide initial material / tile vocabulary and how thickness accumulation + draw order produce clean overlapping trunk mass.
5. Produce a minimal end-to-end generative walk-through (directory tree → skeleton → tags → one enrichment) for a realistic small repo.
6. Clarify filtering rules for local working-tree noise (build artifacts, untracked files, etc.).

## Key Artifacts
Layered architecture (canonical):
1. Structural Skeleton (L-system/hybrid)
2. Semantic Role/Annotation
3. Enrichment/Decoration
4. Thrive/Live

Trunk decision (hybrid, v2.1 — pipe column via primary internodes):
- Primary limbs generated from major top-level directories/groupings, fully data-driven.
- Each primary claims an **internode** on the axis — the vertical room it needs to leave as volume rather than as a ray. The chain of internodes *is* the grown axiom; there is no pre-sized basal stick.
- **Width at any height = the support still carried there.** Below the first departure that is every primary; each departure drops its own share; above the last, nothing.
- A **flared collar** at the foot, below the first departure, widening into the roots.
- Primaries insert as **knots**: they leave closer to the axis than their fan angle and open out over their first segments, so part of that stretch reads as trunk.
- Overlap and thickness of the limbs near the origin still supply the trunk's *surface*; what the column adds is something for them to fuse onto.
- `trunk.fan` is **lateral character only** — it no longer doubles as the trunk's height budget.
- Root-boulder cluster at base carries global signals, reaching past the collar's flare.
- Empty repository: no primaries, so no internodes, so no column — a seed in its roots, by construction rather than by special case (`AC-SKEL-2`).

Primitive categories retained for this thread:
- Structural (depth, branching factor, hierarchy balance, conventional folders)
- Size & composition (LOC, bytes, language mix, code/asset/config ratios)
- Temporal (age, last-modified, churn windows, activity heat)
- Ownership (authors, concentration, bus-factor proxies)
- Derived signals (test ratio, docs presence, TODO density)

## Session Timeline (Most Recent First)
- [Most recent] User confirmed understanding of L-system axiom, elected to keep L-systems, requested clarification on character enrichment, Git-branch representation, local vs remote, and overall architectural choices; layered architecture and hybrid trunk accepted.
- [Prior] Full SWOT on pure emergent overlapping trunk vs dedicated trunk; hybrid recommended and discussed.
- [Earlier in thread] User narrowed from multi-metaphor to single world-tree scene; introduced herringbone + L-systems, overlapping-trunk idea, Grow/Thrive dual system, and desire for emergent character (treehouse libraries, landing pads, etc.).
- [Origin of this focus] Redraft of overall outline restricted to one tree scene with organic procedural rules driven by primitives.