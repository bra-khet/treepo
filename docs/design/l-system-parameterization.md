# L-System Parameterization Design
**Design Document — Supplemental Living Section**  
**Version:** 0.1  
**Status:** Active draft  
**Last updated:** 2026-07-26  
**Project name:** treepo *(locked 2026-07-27 — [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5)*  

This document is the authoritative reference for how L-systems are parameterized and driven by the Feature System in treepo. It expands the high-level statements in [`design-outline.md`](design-outline.md) and [`visual-construction.md`](visual-construction.md), and is intended to be usable both by humans and by agents when deriving concrete mapping tables.

It draws on the classic literature (primarily Prusinkiewicz & Lindenmayer, *The Algorithmic Beauty of Plants*, and subsequent parametric / stochastic extensions) while remaining tightly scoped to the needs of a repository-driven world-tree.

---

## 1. Purpose & Scope

The L-system is responsible **only for the structural skeleton** of the world-tree:

- Topology (which limbs exist and how they connect)
- Geometry of those limbs (length, thickness, angles, recursion)
- The hybrid trunk (minimal basal axiom + overlapping primary limbs)

Everything else — materials, age/churn expression, enrichment (treehouses, platforms, scars), workers, particles, glow — lives in later layers (Semantic Annotation → Enrichment → Thrive). Age and churn therefore influence the *skeleton* only weakly or not at all; they dominate the material and animation layers. This separation is deliberate and already recorded in the Feature System and Outline.

The parameterization must support:

- Deterministic regeneration from hierarchical path-hash seeds.
- Clear visual response to structural and size primitives.
- Readable mental models (the user can still “read” major directories).
- Organic / sometimes alien character when the underlying repository data is chaotic.
- Practical depth limits so the tree never becomes an infinite fractal.

---

## 2. Core L-System Mechanics (Evergreen Reference)

### 2.1 Turtle Interpretation (Standard Symbols)

The string produced by the L-system is interpreted by a turtle that carries position, orientation, and current thickness:

| Symbol | Meaning |
|--------|---------|
| `F` or `F(s)` | Move forward distance *s* (or default step) and draw a segment |
| `f` | Move forward without drawing |
| `+` / `-` | Yaw left / right by current angle δ (or parameterized angle) |
| `&` / `^` | Pitch down / up (3-D) |
| `\` / `/` | Roll left / right (3-D) |
| `\|` | Turn 180° |
| `[` / `]` | Push / pop turtle state (the fundamental branching mechanism) |
| `!(w)` | Set current line width / thickness to *w* (or decrement) |
| `'` or similar | Optional color / material index change (used lightly or deferred to later layers) |

In treepo we primarily need the 2-D subset plus thickness (`!`) for the initial skeleton. 3-D extensions remain available for later canopy or camera work.

### 2.2 Parametric L-Systems

Symbols carry numeric parameters. A typical production for a tapering, branching stem looks like:

```
A(s, w) : s >= min  →  !(w) F(s) [+(θ1) A(s·r1, w·q^e)] [-(θ2) A(s·r2, w·(1-q)^e)]
```

Where:
- `s` = current internode length
- `w` = current width
- `r1`, `r2` = length reduction ratios for the two child branches
- `θ1`, `θ2` = branching angles
- `q`, `e` = width distribution parameters
- `min` = termination threshold

This is the classic form used throughout *The Algorithmic Beauty of Plants* and later work. It gives direct, continuous control over taper and branching proportions.

### 2.3 Stochastic L-Systems

Productions may be chosen probabilistically. This is the cleanest way to introduce controlled variation without breaking determinism when the random choices are seeded by the path-hash. Stochasticity is the primary lever for “mess” and alien character.

### 2.4 Hierarchical / Modular Use

Because every repository path already has a deterministic seed (path-hash), each major limb can run its own L-system instance with its own parameter vector derived from that subtree’s Feature System primitives. The global tree is therefore a hierarchical composition of smaller L-systems rather than one gigantic flat derivation. This matches both the directory structure and the layered generative architecture already decided for treepo.

---

## 3. Key Parameters and Their Visual Effects

These are the knobs that matter for treepo. Ranges are practical starting points drawn from botanical L-system literature and visual experimentation; they are not sacred.

| Parameter | Typical Range | Visual Effect | Primary Drivers in treepo |
|-----------|---------------|---------------|---------------------------|
| **Recursion / iteration depth** | 3–6 (hard cap recommended) | Number of successive branch generations. Higher = denser, finer canopy. | Directory depth, branching factor, total file count (softened) |
| **Branching angle (δ or θ)** | 15°–60° (or ± pairs) | Narrow = upright/columnar; wide = spreading or chaotic. | Hierarchy balance, ownership concentration, conventional structure signals |
| **Length reduction ratio (r)** | 0.5–0.9 | How quickly child segments shorten. Lower = faster fall-off, more compact. | Relative size (LOC/bytes) of child vs parent |
| **Thickness / width reduction** | Controlled by `!` and width parameters (q, e) | Taper rate. Slower taper = heavier limbs near base (critical for overlapping trunk mass). | Size primitives + mild age bonus for old mass |
| **Stochasticity / noise** | 0.0–0.4 (angle & length jitter) | Orderly vs wild. High values + high churn/skew produce alien/overgrown silhouettes. | Churn, hierarchy skew, language diversity, lack of conventional folders |
| **Tropism / bias** | None → mild upward → size-driven droop | Preferred growth direction. Extreme size disparity can produce visible “weight”. | Size disparity, optional activity heat |
| **Axiom / basal segment** | Short, data-driven length & radius | The minimal starter segment that primary limbs grow from. | Global root mass / number of primary limbs |
| **Termination threshold** | Small length or depth limit | Prevents infinite or microscopic branching. | Practical readability + performance |

**Important separation**  
Age and churn do **not** primarily rewrite these structural parameters. They act on material families, glow, particle rates, scarring, and Thrive animation (see Feature System Interaction Physics). Only mild, optional influence (e.g., high recent churn slightly raising local stochasticity) is considered at the skeleton level.

---

## 4. Mapping from Feature System Primitives (Guidelines)

### Structural → Topology & Angle
- High branching factor or deep hierarchy → higher recursion (within the hard cap).
- Balanced hierarchy + conventional folders → moderate, relatively symmetric angles.
- High skew or unconventional structure → wider angles + higher stochasticity.

### Size → Length & Thickness
- Larger subtrees produce longer and/or thicker primary limbs.
- Length reduction ratio can be modulated by the size ratio of child to parent.
- Thickness falls off more slowly than length when we want a substantial overlapping trunk (recommended default).

### Temporal (Age / Churn) → Mostly Deferred
- Default: skeleton ignores pure age/churn.
- Optional mild effects only: very old low-churn core limbs receive a small thickness bonus; high recent churn can raise local noise.

### Ownership & Quality Signals
- High ownership concentration → more coherent, less noisy branching on that limb.
- Fragmented ownership or high TODO density → slight increase in stochasticity or secondary branching (enrichment will do the heavier lifting).

### Mess / Chaos Signals
- Combination of high churn + hierarchy skew + mixed languages + missing conventional folders → elevated stochasticity and wider angle ranges. This is the primary route to the “alien / overgrown” look that the design explicitly wants for messy repositories.

---

## 5. Decision Menu — Actionable Choice Sets

These sets are designed so an agent (or a human) can select a coherent row that satisfies the product goals (organic, readable mental model, strong age/churn contrast in later layers, alien when data is chaotic, practical depth limits).

**A — Recursion depth policy**
- A1: Strictly proportional to directory depth
- A2: Softened by total file count (deep sparse folders stay simpler)
- A3: Hard-capped at 4–5 levels; deeper content is aggregated (recommended for readability & performance)

**B — Branching angle character**
- B1: Narrow (15–35°) — orderly, classical
- B2: Medium (30–55°) — balanced default
- B3: Wide + high stochasticity when skew/churn high — produces alien/overgrown silhouettes

**C — Length & thickness scaling**
- C1: Length falls off faster than thickness (chunky near trunk — good for hybrid overlapping trunk)
- C2: Similar fall-off rates (classic fractal look)
- C3: Thickness stays high longer on high-age limbs

**D — Stochasticity / mess response**
- D1: Near-deterministic for clean repos; noise rises sharply with churn + skew
- D2: Always a little noise so even clean trees feel alive
- D3: Noise also modulated by language diversity and missing conventional folders

**E — Tropism / weight**
- E1: None (pure L-system)
- E2: Mild upward bias
- E3: Size-driven droop on extremely heavy limbs

**F — Primary limb assignment**
- F1: One primary limb per top-level directory
- F2: Small top-level directories grouped into fewer, thicker limbs (recommended)
- F3: Logical groupings (tests, docs, etc.) may become limbs even if not top-level

**G — Age/churn influence on skeleton**
- G1: None (they live entirely in materials & Thrive) — recommended default
- G2: High recent churn slightly increases local stochasticity or secondary branching
- G3: Very old low-churn core limbs get a small thickness bonus

**Recommended first coherent combination (v0.1 mapping)**  
`A3 + B2/B3 hybrid + C1 + D1 + E3 + F2 + G1`

This combination already satisfies every major design decision recorded so far: practical depth limits, solid overlapping trunk mass, chaos only when the data is chaotic, and clean separation of age/churn into the material/Thrive layers.

---

## 6. Iteration & Visualization Strategy

L-system parameters are highly iterative. The correct workflow is:

1. Lock a coherent row from the decision menu above as “v0.1”.
2. Implement the absolute minimum skeleton drawer (even a Python turtle or a Bevy debug pass that draws only lines + thickness).
3. Feed it 3–4 real or synthetic repositories (clean library, medium application, messy legacy project, nearly empty repo).
4. Observe silhouettes. Adjust one parameter family at a time.
5. Because every path is seeded by its hash, every change is reproducible.

No final tables are required before the first skeleton exists. The Feature System already defines the input vector; this document defines the output parameters the L-system must consume. Mapping tables are data, not hard-coded magic, and can be revised later without breaking determinism.

---

## 7. Relationship to Other Documents & Engine

- [**Feature System**](feature-system.md) supplies the primitive vector and the Interaction Physics that interpret age/churn, ownership, etc.
- [**Core Visual Construction**](visual-construction.md) defines the hybrid trunk (basal axiom + overlapping primary limbs) and the four-layer generative stack.
- [**Engine Architecture (Grow vs Thrive)**](engine-architecture.md) owns when the L-system is re-run (only inside Grow) and guarantees the resulting skeleton is treated as stable by Thrive.
- **Hierarchical path-hash seeds** guarantee that the same logical subtree always produces the same geometry.

The L-system itself should be implemented as a pure function:  
`(subtree primitives + path seed + parameter table) → list of oriented, thickened segments`  
so that Grow can call it repeatedly and deterministically during cinematic transitions.

---

## 8. Open Implementation Notes (Not Design Blockers)

- Exact numeric ranges inside each choice will be tuned by looking at real silhouettes.
- Whether to expose a subset of these parameters to advanced users later is a product decision, not a skeleton decision.
- 3-D turtle extensions and tropism forces can be added without changing the core parameterization contract.
- Stochastic choices must be drawn from a seeded RNG derived from the path-hash so that “random” remains deterministic across runs.

---

*End of draft v0.1. This document is revised in place. The decision menu in Section 5 is the primary interface for both human and agent refinement of the first concrete mapping tables.*
