**World Tree Repo Visualization — Full Design Outline (Living Document v0.3)**

**Project name:** treepo  
*Locked 2026-07-27 — see [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5. The name is final and no longer provisional in any document. Earlier candidate names (Worldtree, Repo Arbor, The Living Manifest, Grove of the Code, “the Tree”, srcerer) are retained only as historical reference.*

**Last updated:** 2026-07-26  
**Status:** Active living summary. Detailed supplemental documents exist for the Feature System, Engine Architecture (Grow vs Thrive), and Core Visual Construction. This outline is the high-level single source of truth and is kept consistent with those documents.

---

### 1. Vision & Experience Goals

This is a desktop application that turns any software repository into a single, living, magical world-tree menagerie. The tree is not a decorative metaphor placed on top of the code. It is *grown* from the actual structure, size, age, churn, ownership, and activity of the repository using low-level primitives and organic procedural rules.

The primary goal is aesthetic and cognitive: the user should be able to form an intuitive, memorable mental model of their codebase by looking at and interacting with the tree. Clicking a branch, hovering a cluster of leaves, or watching a little worker move should immediately evoke “that is the audio encoding path” or “that is the old UI layer that hasn’t been touched in months.” Precision for static analysis or refactoring advice is secondary. Beauty, legibility, and a sense of a living system come first.

The scene is deliberately restricted to one core type at the beginning: a large, lively, somewhat alien world-tree with supporting elements around its base and in its canopy. Later themes can still be added, but the first version goes deep on this single, rich menagerie rather than spreading across many metaphors.

The tree should feel simultaneously organic and data-driven. Large or chaotic repositories produce trees that look strange, heavy, or exuberantly overgrown rather than “pretty but generic.” Clean, well-factored repositories produce clearer, more harmonious forms. Mess is allowed to look like mess; that is a feature.

Liveliness, progress, and the passage of time (age + churn) are thematic pillars. Fresh activity draws the eye; age and stability create distinctive contrast. The system is biased toward making these signals readable and emotionally resonant.

---

### 2. Core Visual Construction

**Trunk is hybrid and emergent.**  
There is a minimal basal segment that serves as the L-system axiom (length and radius driven by total root mass / primary limb count). The true visual trunk mass, however, is created by the overlapping and tight clustering of the primary limbs near the origin. Primary limbs rise from a cluster of root-boulder forms. Where they heavily interpenetrate, the eye reads a thick, coherent trunk. This hybrid approach preserves L-system compatibility and redraw stability while still allowing the bulk of the trunk to emerge from real structural data.

**Generation tools**
- **L-systems** (or L-system-inspired recursive rules) drive the primary branching topology, growth angles, and recursion. Rules are parameterized by the structural and size primitives of the corresponding directory subtree.
- **Herringbone Wang tiles** (or a closely related constraint-tile system) provide the organic surface and internal patterning of branches, platforms, and foliage masses. The herringbone offset hides seams better than square Wang tiles and still allows controlled randomness and material variation.
- Hierarchical deterministic seeding: every path in the repository is hashed to produce a local PRNG seed. Identical structure always produces identical visual output. Subtrees remain independent yet coherent with their parents.

**Layered generative architecture** (canonical order)
1. Structural Skeleton (L-system / hybrid trunk)
2. Semantic Role / Annotation (tags on limbs and subtrees)
3. Enrichment / Decoration (platforms, treehouses, foliage variation, scars, etc.)
4. Thrive / Live layer (inhabitants, particles, continuous animation, event reactions)

**Overall feel**  
A magical, slightly otherworldly world-tree. It can support platforms, hanging elements, small structures, ambient creatures, and activity at its base. It should never look like a stock “oak with leaves.” It should look like the specific repository made flesh — organic, sometimes alien, always data-driven.

---

### 3. Comprehensive Abstract Feature System

Every visual and behavioral property of the tree is driven by a defined set of primitives extracted from the repository. These primitives are the only source of truth. The full catalog, lenses, and Interaction Physics live in [`feature-system.md`](feature-system.md) (currently v0.5). This section summarizes the categories.

**Structural primitives**  
Directory depth, branching factor, hierarchy balance/skew, conventional folder signals, monorepo/workspace presence.

**Size & composition primitives**  
Bytes, LOC (total/code/comment/blank), average/median/max file size, language distribution, code/asset/config/docs ratios, generated/binary proportion.

**Temporal primitives (first-class and heavily biased)**  
Path age, last-modification age, churn rate across sliding windows, activity heat (recency-weighted).

**Ownership & social primitives**  
Unique authors, dominant author, contribution concentration, bus-factor proxies.

**Derived / quality signals**  
Test-to-source ratio, documentation presence and freshness, TODO/FIXME density, naming regularity heuristics.

**Interaction rules (summary)**  
Age and churn are major drivers of both appearance and behavior. Fresh / high-recency regions are visually emphasized (bright tips, energetic material, higher particle rates) and blend via proximity and gradient rules into the surrounding bulk. Old / low-churn regions are rendered with distinctive contrast (patina, mass, slower motion, different material families). The system deliberately emphasizes contrast while applying smoothing rules so that transitions remain organic and cohesive rather than harsh. Size controls volume and visual “weight.” Material families and ownership accents further differentiate regions. Full mapping tables and phase-aware physics (Grow vs Thrive) are maintained in the Feature System document.

---

### 4. Engine Architecture: Grow vs Thrive

A dual-phase system keeps expensive analysis rare and the living feel constant. Detailed contracts, triggers, cinematic behavior, and Bevy implementation notes live in [`engine-architecture.md`](engine-architecture.md) (v0.1). High-level summary:

**Grow Phase (long tick / structural + cinematic update)**  
Owns complete topology rebuilds and diff-driven transitions. Performs full or incremental repository scan, primitive extraction, L-system regeneration, enrichment placement, and constrained cellular material passes on changed regions. Classification threshold crossings are rendered as explicit, dramatic transformations. Grow is event-driven, off the main thread, and designed to be watched (cinematic, exportable). First-time association of a repository plays the entire history as one continuous Grow sequence.

**Thrive Phase (short tick / main loop)**  
Keeps the world alive. Continuous ambient animation, worker behaviors, dirtiness visualization, lightweight local particles/CA on dirty rectangles only, all player interaction, and reactions to non-structural signals. Structure is frozen between Grows. A narrowly scoped State Sync may run inside Thrive for lightweight status (ahead/behind, open issues, etc.) without triggering topology change.

**Performance separation is strict.** Full topology and heavy CA belong exclusively to Grow. Thrive stays cheap enough to hold a smooth interactive frame rate with chunking, dirty rectangles, and static-vs-dynamic separation. Authoritative budgets live in [`../PRD.md`](../PRD.md) §7 — 30 fps floor, designed to sit comfortably above it, with 60 fps explicitly *not* a hard requirement (PRD §11 Q5, decided 2026-07-27).

**Current tech direction**  
Implementation is beginning in **Bevy (Rust ECS)**. This choice supports the dual-phase architecture, hierarchical seeds, pixel-level control, and eventual Steam packaging. The choice is the active direction rather than an irrevocable lock; the design documents remain engine-agnostic where possible.

---

### 5. Procedural Generation Pipeline (Multi-scale)

1. Walk the repository tree and assign semantic roles + hierarchical seeds (path hash).
2. Coarse topology via parameterized L-system rules driven by structural and size primitives. Primary limbs correspond to major directories or logical groupings. Minimal basal axiom + overlapping primary limbs produce the hybrid trunk.
3. Surface and internal detail via herringbone tile constraints + material rules from the Feature System.
4. Optional lightweight local cellular-automaton or reaction pass for secondary organic texture (moss, scars, growth rings, small debris) — modular and intensity-controlled.
5. Connectivity / cleanup pass so the result remains topologically coherent and readable as a tree.
6. Placement of supporting elements (root boulders, base platforms, notice boards, ambient objects) according to global and local primitives and semantic tags.
7. Inhabitant and activity seeding for the Thrive phase.

All stages respect hierarchical seeds so the same repo always produces the same base form.

---

### 6. Multi-scale Navigation & Mental Model

The tree supports continuous or stepped zoom with level-of-detail changes:

- **Far**: overall silhouette, major limb masses, global activity heat, root structure.
- **Medium**: individual major branches, platforms, clusters of secondary growth, ownership coloring.
- **Near**: finer branch structure, leaf or detail density, small workers, and abstracted file/directory representations.

**Detail quantization rule**  
Branching and nesting are not infinite. There is a practical depth threshold beyond which further subdivision is aggregated. Representations become quantized:

- Deep subdirectory trees may collapse into bookshelf / Fibonacci-spiral shelf metaphors or similar proportional containers.
- At the limit, an object can simply represent “this directory and all its contents.”
- Extreme near-pixel resolution is appropriate only for certain characters (e.g., junk piles).
- Structured containers (vaults, asset rooms, libraries) prefer game-like modal / inventory interactions when the user requests deeper inspection rather than forcing infinite geometric subdivision.

This preserves readability and performance while still allowing the user to form a coherent mental model. Clicking or focusing a region surfaces just enough identity (path, key traits, dominant visual signals) that the mapping back to real code feels intuitive and emergent.

---

### 7. Live Elements & Future Interactions (Thrive)

Workers / small creatures are theme-consistent inhabitants whose presence and behavior are driven by activity and ownership signals. They follow simple, stable equilibria that create clear but organically varied associations with the regions and materials they belong to. Exact vocabulary and pathing rules will evolve with concrete manifestations; the governing principle is constrained, readable behavior rather than complex global pathfinding.

Future event examples (post-MVP but architecturally anticipated):
- Pull request → small vehicle or creature arrives at the base and deposits a crate or package.
- Issue opened → figure posts a notice on a board or hangs a tag on a branch.
- Agent tool use (later) → themed reactions (bird flies off for web search, etc.).
- Commit → pulse of growth or light along the affected limb (or full Grow if topology changes).

These are driven by the Thrive loop watching for file-system or git events (and later agent traces). Structural consequences still require a Grow.

---

### 8. Performance & Technical Patterns (Selective Inspiration)

From the Noita / Powder Toy reference we deliberately keep only what serves a desktop visualization tool:

- Chunked representation + dirty rectangles.
- Clear static vs dynamic separation (Grow updates static structure; Thrive animates dynamic layers).
- Hierarchical deterministic seeds.
- Constraint-based rather than pure-noise generation (L-systems + herringbone tiles).
- Optional, modular, intensity-controlled local CA for secondary detail only. Future user-facing performance settings can expose or stack additional layers.
- No full free-form falling-sand physics or destructive rigid-body simulation unless a later mode specifically wants it. The tree must remain a coherent, navigable structure.

Target platforms: desktop (Windows / macOS / Linux). Current implementation direction is Bevy.

*Updated 2026-07-27 — storefront distribution is no longer a "longer-term goal." Per [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R1, treepo is positioned and built as a consumer desktop product distributed through a games storefront. This does not change build order (the tree must feel alive before it is packaged), but it does raise the polish bar, make first-run experience a primary design surface rather than a finishing task, and forbid requiring a terminal for any essential interaction.*

---

### 9. Metadata & Agent Path

The cached primitive vectors, classification scores, and any agent-written annotations live in **application data**, keyed by a stable repository identity — the primary remote URL where one exists, falling back to a content-derived local identity. Opening a repository writes nothing into the working tree. The Grow phase can regenerate the store at any time. Agent skills that maintain or enrich the manifest target this store, which keeps that path open without depositing anything into the repository being observed.

An in-repository `.treepo/` directory remains available as an explicit opt-in for users who want the manifest co-located with the working tree, and as the basis of a shareable package.

*Decided 2026-07-27 — see [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R7, which supersedes R3 and in turn the original `.repo-viz/` proposal. R3 had placed the manifest inside the repository; R7 reverses that in favor of app-data-primary storage, on the grounds that it is non-intrusive by default, gives a cleaner privacy story, handles repositories the user cannot write to as an ordinary case, and provides a natural home for future interactive state. Constitution N1 was restored to its absolute form as a result: treepo writes nothing to a repository unless explicitly asked. Store layout, identity resolution, and the opt-in mechanics are specified in [`../PRD.md`](../PRD.md) §5.3.*

---

### 10. MVP Scope (Recommended)

*This section predates the Constitution and will be superseded by the PRD, which owns scope, prioritization, and sequencing. It is retained as the design set's own recommendation. Note that §10 R1 (consumer positioning) and R6 (contributor identity in exports) both reshape it — onboarding, first-run, and the identity model in the export path all move earlier than this list implies.*

- Single world-tree scene with hybrid emergent trunk (basal axiom + overlapping primary limbs).
- Full primitive extraction + L-system + herringbone pipeline driven by the Feature System.
- Grow / Thrive dual system with cinematic Grow transitions and basic automatic + manual triggers.
- Multi-scale zoom with quantized depth abstraction and basic click-to-identify.
- Simple ambient animation and a few worker types under stable-equilibria rules.
- Lightweight manifest support.
- Desktop window + optional always-on-top widget mode.
- Export of Grow sequences (GIF / image sequence as minimum).

Everything else (rich event system, agent speech bubbles, additional scene types, deep analytics overlays, full Steam polish, user-exposed CA intensity controls) is deliberately sequenced after the core tree feels alive and meaningful.

---

### 11. Resolved Design Decisions (formerly Open Questions)

The following items were open in v0.2 and have now been clarified by design discussion. They are recorded here for continuity and to keep the outline internally consistent with the supplemental documents.

**Exact parameterization tables for L-system rules from primitives**  
Deferred. Concrete tables will be derived later from the Feature System, the project Constitution, and the PRD once those documents mature. The outline and Feature System already define the input primitives and the required output parameters (thickness, angle ranges, recursion depth, etc.). No contradiction exists; this remains an implementation / data-authoring task rather than an unresolved design principle. The parameter set, primitive→parameter mapping guidelines, and the v0.1 decision menu now live in [`l-system-parameterization.md`](l-system-parameterization.md); only the concrete numeric tables remain outstanding.

**How aggressively age and churn should bias positive vs negative readings**  
Heavily. Age and churn are major thematic and visual drivers (liveliness, progress, history). Fresh / high-recency material is eye-catching (bright tips and energetic regions) and blends via proximity and gradient rules into the surrounding bulk. Old / low-churn material is rendered with clear contrast (mass, patina, slower motion, different material families). Rules emphasize contrast while applying smoothing so transitions feel organic rather than harsh. Both positive readings (vitality, wisdom, sturdy foundation) and negative readings (scarring, brittleness, neglect) are supported and context-dependent. This bias is intentional and central to the project’s identity.

**Degree of cellular-automaton secondary detail that remains performant**  
Determined case-by-case and kept modular. Core simulation logic stays inside the Grow / Thrive separation and dirty-rectangle discipline. Additional CA intensity or optional layers should be stackable and, in the future, user-configurable. No single hard performance number is locked beyond the existing architectural constraints.

**Visual language for transformations when a major classification shift occurs**  
Organic, striking, stylized, and sometimes rough. Dramatic rather than subtle. The transition should carry a sense of inertia and mass (superfluous particle churn, material flow, visible growth or withering). Transformations are celebrated cinematic events owned by the Grow phase, not silent swaps.

**How much file-level vs directory-level detail is shown at the closest zoom**  
Abstracted and quantized. Branching depth and nesting have practical limits. Beyond a threshold, representations aggregate (bookshelf / spiral metaphors, “this directory and all its contents” as a single object, etc.). Near-pixel resolution is reserved for appropriate characters such as junk piles. Structured containers prefer modal / inventory-style inspection when deeper detail is requested. This preserves both readability and the “everything is an object” mental model without infinite geometric subdivision.

**Exact worker vocabulary and pathing rules for the first Thrive inhabitants**  
Will evolve with concrete visual design. Governing principle is already set: simple, stable equilibria that produce clear but organically varied associations between workers and the regions or materials they belong to. No complex global pathfinding in the first version.

---

### 12. Relationship to Other Living Documents

- [`feature-system.md`](feature-system.md) — authoritative catalog of primitives and the full Interaction Physics / rules layer (including age/churn gradients, material flow, and phase-aware behavior).
- [`engine-architecture.md`](engine-architecture.md) — authoritative dual-phase contracts, triggers, cinematic Grow behavior, State Sync, and Bevy notes.
- [`visual-construction.md`](visual-construction.md) — hybrid trunk decision, layered generative architecture, and L-system foundation details.
- [`l-system-parameterization.md`](l-system-parameterization.md) — authoritative parameter set, primitive→parameter mapping guidelines, and the decision menu for the structural skeleton.

Above these sits [`../CONSTITUTION.md`](../CONSTITUTION.md), which holds the enduring product vision, principles, and non-negotiable constraints. Where this outline and the Constitution disagree on intent, the Constitution governs; where they disagree on detail, this outline and its supplements are corrected to match.

This outline is revised in place. When a decision hardens, it is reflected here and in the relevant supplemental document so the set remains consistent.

---

*End of living outline v0.3. Next concrete work continues in the Feature System mapping tables, L-system parameter derivation, and first Bevy experiments with the hybrid skeleton.*
