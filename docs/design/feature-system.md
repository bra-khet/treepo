# Comprehensive Abstract Feature System
**Design Document Draft — Living Section**  
**Version:** 0.5  
**Status:** Active draft — primitives + full interaction / physics rules layer  
**Last updated:** 2026-07-26

**Project name:** treepo  
*Locked 2026-07-27 — see [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5.*

---

## 1. Purpose

The Feature System is the single source of truth that every visual, structural, and behavioral decision in the World Tree draws from. It is deliberately over-complete. Unused primitives cost almost nothing at extraction time and give us classification power, future-proofing, and richer emergent behavior later. Presence, magnitude, distribution, and relationships among primitives are themselves classification signals.

The system answers one fundamental question for every path (file or directory) in the repository:

> “What measurable properties does this path possess, and under which lenses / contexts?”

Everything the Grow phase builds and the Thrive phase animates is derived from these properties (plus hierarchical seeds and the interpretation / interaction rules that act upon them).

---

## 2. Design Principles

- **More is better.** Prefer an extra cheap primitive over a missing one. Unused primitives do not hurt the system.
- **Preserve dimensionality.** When a concept is naturally multi-faceted, store it as a small vector or structured record rather than collapsing it early into a single scalar.
- **Context is first-class**, but responsibility is carefully partitioned (see Section 4).
- **Determinism.** Extraction must be reproducible. Hierarchical path hashing remains the primary seed source.
- **Extraction cost awareness.** Cheap primitives (filesystem + basic git) run on every Grow. More expensive or heuristic primitives can be marked optional or agent-assisted.
- **Classification by presence and shape.** The mere existence of certain primitives or particular shapes in their distributions is already a strong classifier.
- **Primitives stay close to measurable facts.** Interpretation, valence (positive/negative reading), and visual mapping live in a separate, contextual layer of rules (the Interaction Physics defined in Section 8).

### 2.1 Deep Treatment of Context

There is real tension in how context should act:

**Option A — Contextual primitives**  
The same path can hold different primitive *values* depending on the active lens (remote vs local working tree, etc.).  
Example: a file that is untracked only exists (or has size/churn) under the working-tree lens.

**Option B — Static / global primitives + contextual interactions**  
Primitives are extracted once as objective as possible. Context then selects or modifies which *interpretation rules* and *interaction mappings* are applied to those values.

**Chosen position: Hybrid with clear separation of concerns (recommended)**

1. **Lenses produce different primitive sets** where the underlying data genuinely differs.  
   - Remote / published lens  
   - Local committed (HEAD) lens  
   - Working-tree lens (includes untracked, modified, staged)  
   - (Optional) Index / staging lens  

   This is necessary and correct. An untracked file simply does not exist in the remote lens; its size, age, and churn are therefore different (or absent).

2. **Once a primitive set has been extracted for a given lens, the interpretation and interaction rules may themselves be contextual.**  
   The same numeric churn value can be read more positively (“vital growth”) under the working-tree lens and more cautiously (“instability”) under the remote lens. Mapping rules, material choices, animation weights, and valence can all carry contextual modifiers or entirely different rule tables per lens or per high-level tree character.

3. **Why this hybrid is preferable**
   - Keeps the Feature System honest: primitives remain measurements, not already-interpreted meanings.
   - Gives maximum expressive power: both the data *and* the meaning can respond to context.
   - Stays extensible: new lenses can be added without rewriting every mapping rule; new contextual rule variants can be added without changing the primitive schema.
   - Avoids the trap of baking every possible interpretation into the data layer (which becomes brittle).

In short:  
**Context first acts on which primitives exist and what values they hold (data).  
Context then acts again on how those values are interpreted and turned into visuals/behavior (rules).**

Both layers remain first-class. Neither is asked to carry the entire burden of contextual emergence.

---

## 3. Primitive Categories

### 3.1 Structural Primitives

These describe topology and organization.

**Atomic / scalar**
- `depth` — distance from repository root
- `child_count` — immediate children
- `descendant_file_count`, `descendant_dir_count`
- `max_subtree_depth`
- `is_leaf` (boolean)

**Multi-dimensional / profile**
- `branching_histogram` — distribution of children-per-node across the subtree (bushiness vs spindliness)
- `depth_profile` — how mass is distributed across depth levels (top-heavy, bottom-heavy, even)
- `balance_score` — quantitative measure of how evenly the tree is balanced (can itself be a small vector: size-balance, depth-balance, type-balance)
- `hierarchy_skew` — tendency toward deep chains vs wide fans

**Folder-signal primitives (context-sensitive records)**  
These are never simple booleans. A folder named `public`, `src`, `lib`, `internal`, `vendor`, `assets`, `docs`, `scripts`, `test(s)`, `examples`, `pkg`, `cmd`, `crates`, etc. receives a structured signal:

```text
signal_name: string
default_semantic_weight: float          # from a conventional dictionary
content_modulation: {
  language_distribution,
  size_ratio,
  binary_ratio,
  test_like_ratio,
  ...
}
effective_weight: computed
position_in_hierarchy: (depth, parent_signals)
```

A `public` folder full of static assets means something different from a `public` folder full of binaries or from a `public` that is actually the root of a separate package. The multi-attribute record preserves the information needed for correct later interpretation.

**On multi-dimensionality**  
For structural primitives, multi-dimensionality (explicit vectors or structured records) is the correct model. Complex numbers are an elegant metaphor but add little practical value over named components and make debugging harder. Nested dimensionality is achieved naturally by allowing a primitive to contain other primitives (the signal vector above is already an example).

### 3.2 Size & Composition Primitives

- Absolute and relative bytes
- Lines of code breakdown: total, code, comment, blank
- Average / median / max / percentile file sizes
- Language distribution (by bytes and by LOC)
- Code : assets : config : documentation : generated : binary ratios
- Generated-file proportion
- Binary-file proportion
- Large-file outliers (files above configurable thresholds)

These primitives are extracted per lens. The working-tree lens will commonly show higher size, more binaries, and more “mess” than the remote lens.

### 3.3 Temporal Primitives

Core set:
- `first_commit_age` (path age)
- `last_commit_age`
- `commit_count` (total and in windows)
- `churn_rate` — lines or commits changed per time window (30 / 90 / 365 days + lifetime)
- `recency_heat` — exponentially weighted recent activity
- `modification_burstiness` — how concentrated changes are in time
- `stability_score` — inverse of recent churn relative to size

These values differ meaningfully across lenses (a file may have high recent churn only in the working tree). The Grow phase is the natural home for heavier simulations that use temporal primitives (material flowing toward distal tips, heat-driven liveliness, etc.).

### 3.4 Ownership & Social Primitives

- `author_count`
- `author_distribution` (map or histogram of contribution share)
- `dominant_author`
- `bus_factor_proxy` (how many authors account for ~80 % of the work)
- `blame_segments` — spatial or sequential breakdown of who last touched which portions (summarized)
- `contribution_recency_per_author`

**Blame summarization (tentative decision)**  
Line-range aggregation is the preferred strategy. Full per-line blame for an entire large monorepo is too expensive; aggregating into meaningful ranges (or sampling) keeps the mosaic effect possible while remaining practical.

Ownership data supports mosaic / segmented materials (different authors receiving stable, seeded material or color families on the same limb). Even a single pixel of a contributor’s color is considered high-value for emotional resonance.

**Identity display policy (decided 2026-07-27)**

Ownership primitives are extracted in full — real author identity is needed to compute stable per-author seeds, blame segments, and distributions. What is *displayed* is separately constrained by [`../CONSTITUTION.md`](../CONSTITUTION.md) N4 and N9:

- **Depiction, not ranking (N4).** Contribution share may size a mosaic, allocate material, or seed an accent. It may never be surfaced as a figure, rank, or ordering — no leaderboards, no contribution-percentage scoreboards, no "top contributors" panel. This applies to tooltips and inspection panels as well as to the tree itself.
- **Pseudonymous by default (N9).** The user running treepo may identify themselves; every other contributor is shown as a stable pseudonym plus a consistently seeded color, derived from a hash of the author identity (or platform avatar colors where available). Real names, emails, and handles of others are off by default.
- **Live view and exports share one setting.** Whatever identity level is active applies identically to what is on screen and to any exported artifact. The two must not be independently configurable, or an export will eventually leak what the live view concealed.
- **Revealing identities is an explicit, unprominent opt-in**, scoped per repository, behind a confirmation that states the consequence plainly. Working draft of that confirmation, for PRD refinement:

  > Real identities of other contributors will appear in the tree and in any exports or shared artifacts you create. Only enable this if you have the right to share those identities (or have obtained permission). treepo ships with everyone except you anonymized for a reason. Enabling this is your responsibility.

**Why this is cheap.** The visual language already relies on stable seeded per-author colors and material families (§8.4), never on names. Pseudonymization therefore removes nothing expressive — the mosaic, the accents, and the emotional resonance of "a single pixel of a contributor's color" all survive intact. Names were never doing the visual work.

Open items for the PRD: pseudonym generation (stable, pronounceable, collision-resistant), how self-identification is established from local git configuration, and behavior when the user is not a contributor to the repository at all — the common case when visualizing a repository they merely cloned, in which every contributor is pseudonymous.

### 3.5 Derived / Quality Signals

Kept intentionally lightweight in early versions.

Reliably implementable now (mostly derived from composition + temporal):
- Comment density / ratio
- Documentation freshness (last commit age of docs-like paths relative to code)
- Test-to-source ratio (heuristic from naming and folder signals)
- TODO / FIXME density (simple text search)
- Large-file or generated-file “debt” indicators

More expensive or judgment-heavy signals (naming regularity, deeper architectural consistency, true cyclomatic-style complexity beyond structural profiles) are marked optional and may be supplied by an agent skill that writes results into the manifest.

Derived signals remain first-class members of the Feature System; they simply carry higher extraction cost or an external dependency.

---

## 4. Context, Lenses, and Interpretation

### 4.1 Supported Lenses (initial set)

| Lens              | What it represents                          | Typical character                  |
|-------------------|---------------------------------------------|------------------------------------|
| `remote`          | Last known published / remote state         | Cleaner, more stable               |
| `local_committed` | Current local HEAD                          | Intermediate                       |
| `working_tree`    | Working directory + untracked + modified    | More vital, messier, in-progress   |
| `index` (optional)| Staging area                                | Transitional                       |

A path may have a complete or partial primitive vector under each lens. Absence of a path under a lens is itself information.

### 4.2 How Context Propagates

1. **Data layer (primitives)**  
   Lens selection determines which files exist and what their measured values are.

2. **Interpretation layer (rules / interactions)**  
   The same primitive values can be fed into different mapping tables or receive different modifiers depending on the active lens and/or the high-level character of the tree or limb.  
   Example: high churn under `working_tree` may increase secondary branching and particle emission (vitality). The same numeric value under `remote` may increase visible stress marks (caution).

This two-stage application of context is deliberate and keeps the system extensible.

---

## 5. Classification

Classification is not a single step; it can (and should) operate at multiple levels:

1. **Primitive-shape classification**  
   Direct inspection of the multi-dimensional primitive vectors and their distributions.  
   Examples:  
   - Deep hierarchy + balanced branching histogram + strong conventional folder signals → classic world-tree character  
   - High size variance + many untracked binaries + high recent churn in working-tree lens → more chaotic / scrap-adjacent growth  

2. **Post-interpretation / emergent classification**  
   After mapping rules have been applied, the resulting visual and structural properties can themselves be classified or used as further signals (e.g., “this limb ended up heavily mottled and restless”).

Both forms of classification can be context-sensitive. A repository may classify differently under the remote lens than under the working-tree lens; that difference is useful information and can drive explicit transformations during Grow.

The design leaves both routes open so that later interaction rules and visual heuristics can contribute to classification without forcing every decision into the primitive layer.

---

## 6. Implementation Notes (Extraction)

- Core structural, size, and basic temporal primitives: pure filesystem walk + `git log` / summarized `git blame`. Cache aggressively in the manifest.
- Language and LOC: `cloc` or equivalent, run only during Grow.
- Blame: line-range aggregation (tentative). Full per-line blame is avoided for performance.
- Working-tree detection: standard git status + untracked walk.
- All primitives are stored per-path (with upward aggregation) and keyed by lens.
- Enormous monorepos are **not** aggressively pruned or normalized into small visual packages. A massive repository is expected to produce a massive, potentially tangled tree. Users of such repositories should understand the visual and performance consequences. Natural aggregation and level-of-detail behavior should emerge from the multi-scale design rather than from forced early reduction of the data.

---

## 7. Tentative / Open Decisions (Primitives Layer)

- Blame summarization strategy: line-range aggregation (accepted as working direction).
- Monorepo scale handling: prefer fidelity over aggressive normalization.
- Exact schema (TypeScript / JSON shapes) for the multi-dimensional primitives and signal records — still to be locked.
- Whether the working-tree view is best implemented as a full parallel skeleton or as a differential layer on top of the remote skeleton — both remain viable; differential is likely cheaper.

---

## 8. Interaction Physics & Rules Layer (New in v0.5)

This section defines how the primitives *interact* with one another and with the two engine phases to produce visual structure, material behavior, animation, and liveliness. It is deliberately inspired by the local-rule material systems of Powder Toy, Noita’s Falling Everything engine, Terraria block physics, Minecraft growth and gravity, and related cellular / particle simulations, while remaining constrained to what is performant and meaningful for a desktop repository visualizer.

The Interaction Physics layer is the bridge between raw measurements and the living World Tree. It is phase-aware: Grow and Thrive share many of the same underlying rules but apply them at very different temporal scales and visual intensities.

### 8.1 Core Philosophy of Interactions

- **Pixels represent data.** Empty decorative filler is minimized. Wherever possible, every visible cluster of pixels should be accountable to one or more primitives (size, age, ownership, type, etc.).
- **Organic flow over discrete jumps.** When structure or material changes (especially during Grow), prefer continuous-looking migration, swelling, or redistribution rather than instantaneous teleportation of mass.
- **Local rules, global emergence.** Most visual behavior should arise from simple, local decisions (a material cell prefers distal positions when its recency is high; a high-churn region emits more particles) rather than global optimization passes.
- **Valence is contextual.** Age, churn, size disparity, and ownership concentration can read positively or negatively depending on lens, surrounding primitives, and high-level tree character. The rules must support both readings.
- **Normalization is mandatory but not purely logarithmic.** Extreme size or contribution disparities must be handled so that small but important elements remain legible and large elements do not completely dominate. Logarithmic scaling is the starting point; additional soft clamps, minimum representation floors, and disparity-aware layout bias are required.
- **Phase separation is strict for performance.** Grow may run multi-pass cellular or flow simulations. Thrive must remain O(visible elements) per frame with pre-baked weights.

### 8.2 Phase-Specific Interaction Contracts

#### Grow Phase Interactions (Long Tick / Structural & Narrative)

Grow is the “rewarding, detailed, story-telling” phase. It runs on significant repository change (commit, large edit, manual refresh, or configured milestone) and defaults to reflecting the current HEAD / configured lens (usually `local_committed` or `working_tree`).

Responsibilities of Grow interactions:
- Recompute spatial layout of material according to age/recency gradients (older material migrates or remains basal/inward; recent material becomes more distal/tip-ward).
- Animate material “swimming” or flowing along a limb when an existing path’s recency or size changes significantly. This is a multi-frame, cellular-style redistribution that makes the change feel organic and observable.
- Adjust limb thickness, secondary branching density, and platform placement from updated size and structural primitives.
- Apply ownership mosaic updates (contributor color/material families redistribute).
- Trigger visible transformation sequences when classification thresholds are crossed (a limb may visibly thicken, scar, sprout new secondary growth, or change material family).
- Optionally spawn short-lived contributor sprites or “work pulses” that travel with the flowing material.
- Rebuild or heavily update enrichment structures (bookshelves for docs, resource stockpiles for assets, etc.).

Grow animations are allowed to be relatively expensive and multi-second. They are the visual payoff for the user having made (or observed) a real change to the repository.

#### Thrive Phase Interactions (Short Tick / Continuous Liveliness)

Thrive is the always-on, cheap, contemplative layer. It never re-analyzes the repository; it only animates what Grow has already established, modulated by the current primitive values (which are treated as static until the next Grow).

Responsibilities of Thrive interactions:
- Continuous low-amplitude sway, breathing, and micro-movement of limbs and foliage, weighted by local churn and recency heat.
- Glow / saturation / brightness modulation driven by recency heat and churn (high-heat regions appear more alive).
- Particle emission rates and types (pollen, sparks, dust, small insects) scaled by activity primitives.
- Simple autonomous worker / creature behaviors that path along existing structure according to ownership or activity heat.
- Hover and focus feedback that temporarily heightens the local interaction rules (brighter glow, more particles, subtle outline).
- Very lightweight secondary cellular detail (if any) running only on dirty or high-heat regions.

Thrive must stay smoothly interactive even on large trees. All expensive decisions are pre-computed or reduced to simple weighted lookups during Grow. Frame budgets are authoritative in [`../PRD.md`](../PRD.md) §7 — a 30 fps floor with 60 fps explicitly not required, so headroom is spent on visual quality rather than on hitting a number.

### 8.3 Spatial & Positional Interaction Rules (Age / Recency Gradient)

**Primary rule (Age → Position)**  
Within any limb or structural mass, material corresponding to older paths prefers basal / inward positions (closer to the trunk or the origin of the limb). Material corresponding to more recent paths prefers distal / tip-ward positions.

This creates a natural “growth rings + tip vitality” reading without requiring explicit ring geometry.

**Grow-phase manifestation**  
When a previously old path receives new commits or significant size change, its associated material is allowed to *migrate* outward along the limb over a short animated sequence. The migration can be implemented as a constrained cellular or particle flow:
- Material cells have a preferred direction bias proportional to the change in their recency score.
- Movement is blocked or slowed by denser / more stable neighboring material.
- Volume is approximately conserved (or gently expanded if size also increased).
- Optional small contributor sprite or light pulse travels with the moving mass.

The result is that the user can watch an old, inward section of a branch brighten and “swim” toward the tip when that code is revived.

**Thrive-phase manifestation**  
No migration. Instead, distal high-recency regions simply receive higher animation amplitude, brighter saturation, and elevated particle rates. The positional layout remains fixed until the next Grow.

### 8.4 Volume, Mass & Pixel Representation Rules (Size)

**Core principle**  
At macro and meso scales, size (bytes / LOC) primarily controls visual volume and pixel count of the corresponding structural element (limb thickness, foliage mass, platform size).  

At micro / detail scales (especially semantic enrichments such as a docs bookshelf), size more often modulates *quality*, *fanciness*, *shelf placement*, or *material richness* rather than pure scale. A larger documentation file becomes a better-bound book or sits on a more prominent shelf; it does not necessarily become a book that is 10× taller.

**Normalization**  
- Primary scaling: logarithmic (or log + soft power) to compress extreme disparities.
- Minimum representation floor: every path that survives filtering receives at least a small but visible pixel budget so that important small files are not erased.
- Maximum soft clamp: prevents a single enormous file or directory from consuming the entire visual budget of its parent.
- Disparity-aware bias: when size variance within a parent is extremely high, the layout may intentionally become slightly unbalanced or “weighted” (a heavy limb droops, a massive side-branch pulls the silhouette) rather than forcing artificial equality. This is an aesthetic choice that communicates the real data.

**Multi-contributor fairness**  
When `author_distribution` shows multiple significant contributors, the visual mass is partitioned (mosaic, striped, or clustered) according to contribution share, with a minimum visible quota for each author above a threshold. Pure majority-rule erasure of minority contributors is avoided. Seeded color / material families per author remain stable across Grow cycles.

### 8.5 Material & Type Interaction Rules

**Primary material** is driven by data type / language / binary vs text / asset class.  
These determine the base color family, texture, and physical “feel” of the pixels (wood-like, crystalline, metallic, leafy, dusty, etc.).

**Accent / dressing / mosaic** is driven primarily by ownership (blame segments, dominant author, author distribution) and secondarily by activity or quality signals.  
A limb whose primary material is “TypeScript wood” can still carry author-colored veins, scars, or surface markings.

**Special material treatments**
- Binary / asset-heavy regions may be rendered as denser, more “resource-like” material (bullion, ore, stacked crates, raw stockpiles) rather than living wood.
- Generated or vendor code may receive a slightly different, more uniform or “machined” material treatment.
- High TODO / debt signals can introduce subtle stress materials (cracks, sparse density, restless micro-particles) that coexist with the primary material.

**Grow vs Thrive**  
Material family itself is largely set during Grow. Thrive modulates only secondary properties (glow intensity, particle type emitted by that material, micro-animation of surface detail).

### 8.6 Flow, Migration & Cellular Secondary Rules

Inspired by Powder Toy / Noita material simulation, but heavily constrained:

- During Grow, a limited cellular or particle system can be used for:
  - Age/recency-driven material migration along limbs.
  - Gentle redistribution when size changes.
  - Secondary organic texturing (moss, scarring, growth rings, small debris) that respects the primary L-system / herringbone structure.
- Density and “preferred direction” rules are simple and local.
- Full falling-sand gravity or free destruction is **not** required for the core experience; the tree remains topologically coherent.
- Connectivity cleanup after any flow pass ensures the result still reads as continuous structure.

Thrive may run an extremely sparse, dirty-rectangle version of secondary detail only on high-heat regions, or may omit cellular simulation entirely and rely on pre-baked animation weights + particles.

### 8.7 Semantic Enrichment Interactions (Special Structures)

Certain folder signals trigger specialized, higher-detail representations at closer zoom levels. These are still driven by the same primitives:

- **Docs / library folders** → small pixel bookshelves or archive platforms built into or hanging from the limb.  
  Individual files become books (or scrolls). Size modulates binding quality, shelf position, or number of visible pages rather than pure height. Click target maps back to the real file.
- **Assets / binary / media** → stockpiles, crates, or resource nodes near the base of the relevant limb or on platforms. Simple back-and-forth or idle animations possible in Thrive.
- **Tests** → distinct secondary growth, hanging markers, or small “proving ground” platforms.
- **High-churn or recently active clusters** → elevated particle systems, more restless workers, or temporary “work sites.”

These enrichments are placed and parameterized during Grow; Thrive only animates them.

### 8.8 Liveliness, Glow & Continuous Animation Rules (Primarily Thrive)

- Recency heat and churn rate drive:
  - Base glow / saturation intensity
  - Sway / breathing amplitude
  - Particle emission rate and variety
- Ownership can tint the glow or particle color.
- Quality / debt signals can introduce slight visual “unease” (asymmetric sway, occasional dark particles).
- All of these are continuous, low-cost, and never require re-scanning the repository.

### 8.9 Transformation & Threshold Interactions (Grow)

When aggregate primitives cross classification thresholds (e.g., a limb that was stable and low-churn becomes high-churn and large), Grow may play an explicit transformation sequence:
- Visible thickening or thinning
- Material family shift
- New secondary branching
- Scarring or healing
- Change in enrichment density

These sequences are celebrated as features of the living system rather than hidden.

### 8.10 Inspiration Mapping (Simulation Games → treepo)

| Familiar Mechanic (Powder Toy / Noita / Terraria / Minecraft) | treepo Analogue |
|---------------------------------------------------------------|-----------------|
| Material density & preferred direction (powder falls, liquid flows) | Age/recency gradient + constrained flow along limbs during Grow |
| Local reaction / transformation rules | Material family changes, scarring, enrichment appearance on threshold cross |
| Growth stages / spreading | Secondary branching and foliage density driven by size + activity |
| Block / pixel identity carrying properties | Every visible cluster of pixels is accountable to primitives |
| Gravity / support | Soft “weight” bias for extreme size disparity (limbs can droop) |
| Biome / environmental influence | Lens + high-level tree character modulate valence and rule tables |
| Minimal empty space; world is made of interacting matter | Prefer data-driven pixels over pure decorative filler |
| Deterministic seeds for structures | Hierarchical path-hash seeds for all generative decisions |

We deliberately do **not** import full free-form falling-sand physics or destructive rigid-body simulation. The tree must remain a coherent, readable, navigable structure. Local cellular behavior is used only where it produces organic, observable change that still respects the higher-level L-system and herringbone topology.

### 8.11 Open Questions for the Interaction Layer

- Exact cellular update order and neighborhood for material migration (von Neumann vs Moore, multi-pass, etc.).
- How aggressively the minimum representation floor should protect tiny but high-churn or high-ownership files.
- Whether contributor sprites during Grow migration are always shown or gated by settings / zoom level.
- Degree of disparity-aware “imbalance” that feels expressive rather than broken.
- Precise mapping tables (still to be written as data) from normalized primitive vectors → concrete parameters (thickness multiplier, glow base, particle rate, migration speed, etc.).
- How much of the interaction rule set should be data-driven (JSON / YAML tables) versus code.

---

## 9. Relationship to Other Documents

- [**Core Visual Construction**](visual-construction.md) defines the L-system skeleton, herringbone surfaces, hybrid trunk, and layered generative architecture that these interaction rules operate upon; [**L-System Parameterization**](l-system-parameterization.md) defines the specific parameters this primitive vector must drive.
- [**Full Design Outline**](design-outline.md) remains the high-level living summary; this Feature System document is the detailed source of truth for primitives and the physics that turn them into appearance and behavior.
- [**Engine Architecture**](engine-architecture.md) defines the Grow / Thrive phases that Section 8 is written against.
- Future agent-monitoring and live event systems (PR trucks, issue posters, coding-agent reactions) will plug into the Thrive interaction surface defined here.

---

*End of draft v0.5. This document is revised in place. The Interaction Physics section (Section 8) is the primary addition in this version and is expected to evolve rapidly as concrete mapping tables and first implementation experiments begin.*
