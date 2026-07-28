# Trunk structure rework — pipe column + primary internodes

**Status:** **implemented and promoted, 2026-07-28** — superseded by
[`../design/visual-construction.md`](../design/visual-construction.md) v2.1, which is
authoritative. Kept only as the record of the reasoning; do not implement from it.  
**Opened:** 2026-07-28  
**Role:** Prompt supplemental for the follow-up that addresses silhouette finding 4 / squat base (and the deeper trunk construction problem behind it)

## What shipped, and where it differs from this draft

Reading B, as specified: a collar plus one internode per primary, width from remaining packed
support, a base flare drawn as a real `Segment` taper, `trunk.fan` demoted to lateral character.
`crates/treepo-gen/src/trunk.rs` and the `trunk.*` rows of `assets/params/lsystem.ron`
(`TABLE_VERSION` 5). The deferred decisions of §12 were taken at their defaults except where
noted:

| §12 | Chosen |
|-----|--------|
| Internode length | An aspect of the **support that departs**, floored — so the internodes sum to an aspect of the column's own width and a column keeps its proportions whether it carries three primaries or thirty. Preferred over "uniform floor + mild scale" because a uniform floor makes a thirty-primary column thirty blocks tall. |
| Flare | Linear, on the collar segment alone: `foot = flare × pipe`, tapering to the pipe at the collar's top. |
| Knot | A constant, `KNOT_HOLD = 7/10` of the fan offset, in `trunk.rs`. No table row. |
| Soft cap | Included, and needed. Piecewise linear above `support_knee`; `support_beyond` is strictly positive and `validate` insists on it, so the ordering stays honest. |
| Root cluster | Attaches to the **flared foot**, at three quarters of its width. The old rule (a fraction of the collar's length) drew the whole cluster *inside* the buttress once it flared. |

Two things this draft did not anticipate:

* **Departure order matters as much as departure height.** Fan position is path order; departure
  height is outermost-first, working inward. The sides then alternate, so the column does not
  lean, and the innermost primary leaves last and nearly vertical — the trunk hands off to a
  leader instead of stopping in mid-air.
* **`basal_aspect` is contested by two pictures.** It is a ratio, so an empty repository and a
  monorepo get the same collar *shape*. High enough for a populated base to stop being an egg
  and `empty.png` becomes a pill — `AC-SKEL-2`'s lonely trunk in miniature. 900 is where both
  read; the value is commented in the table with that argument.

**Related:**

| Artifact | Relevance |
|----------|-----------|
| `docs/design/visual-construction.md` | Current hybrid trunk decision (minimal basal + co-origin overlap) |
| `docs/design/l-system-parameterization.md` | §6 tuning loop; trunk rows; fan as family |
| `crates/treepo-gen/src/trunk.rs` | Implementation of current hybrid |
| `assets/params/lsystem.ron` → `trunk.*` | `basal_aspect`, `basal_min`, `packing`, `fan`, F2 threshold |
| `claude-progress.md` | Findings 1–3 closed; finding 4 open; fan bounds aspect |
| `qa/PLAN-silhouette-lab.md` | Structural vs tuning order (may need update after this rework) |

---

## 1. Why this draft exists

Silhouette review after Phase 3 showed a base that still reads wrong:

1. **Finding 3 (closed as an aspect rule):** absolute `basal_length` fought `stem_width` and produced a disc. `basal_aspect` fixed the *inconsistency*, not the *feeling*.
2. **Finding 4 (still open under the old model):** `trunk.fan` at high bushiness (~150°) leaves only ~half a stem-width of overlap, so the axiom cannot grow tall without outrunning the “trunk.” The base stays squat.
3. **Human diagnosis (this rework’s real driver):** the base looks like an **oversized seed** — the same glyph as empty-repo AC-SKEL-2, scaled up. A point fan does not give the base **volume**. Stacking a longer block under that point does not fix the metaphor.

The original hybrid (minimal basal segment + all primaries leaving one tip so thickness *overlaps into* a trunk) was the least solid of the early construction choices. It was wanted and designed deliberately, but **first pictures show it is not delivering** a planted, columnar base. A rework is justified. The enduring vision is not “preserve co-origin overlap at all costs”; it is:

> When you look at the tree, it feels right, and the silhouette remains **visually identifiable with the repository characteristics that drive it**.

This document freezes the *intent* of the rework for an implementer. It is not a ratified design document until promoted.

---

## 2. Problem statement (current construction)

### What the code does today

```
ORIGIN ──[one basal segment, width ≈ packing × Σ primary base widths]── tip
                                                                        │
                                         all primaries fan from this one tip
```

- Visual “trunk” is mostly **emergent overlap** of co-origin primary bases (`F-SKEL-3` hybrid).
- Axiom length is an **aspect of that width**, capped by how far the fan keeps limbs fused.
- Fan is therefore both **lateral character** and **vertical trunk budget**.

### Why it fails the eye

| Failure | Cause |
|---------|--------|
| Fat seed / oval base | Width is a full pipe sum applied as one short cylinder; height is fan-starved |
| No sense of volume | A **point** is not a **support column**; limbs have no vertical room to enter the trunk |
| Role collision | Multi-primary base shares the empty-repo **seed** glyph |
| Finding 3 “done” but still squat | Aspect rule is correct under co-origin math; the math is the wrong model |

### What is *not* the primary fix

- Only lowering `trunk.fan.max` (helps squat under the old model; does not introduce volume).
- Only raising `basal_aspect` (fights overlap height; can violate “axiom outlasts limbs”).
- Treating this as Phase 4 enrichment or Thrive polish.

This is a **skeleton / trunk placement** rework in Phase 3 territory.

---

## 3. Target construction (Reading B — preferred)

### 3.1 Name

**Hybrid trunk v2 — pipe column via primary internodes**  
(also: “knot-and-pipe trunk”)

### 3.2 Reading B (canonical for this rework)

**Internode-per-primary (or per attachment event):** the supporting axis **grows with** the primary structure rather than existing as one pre-sized basal stick that everything leaves from.

- Each primary that must leave the trunk **claims vertical room** on the axis.
- That room is not “a block stacked under the branch” in a Lego sense; it is **the space the branch needs to exist as volume** — an insertion zone that becomes trunk mass once envelopes and (later) organic fill run.
- The axiom is no longer a single minimal segment whose only job is to be short. It is the **chain of support internodes** that the primaries induce, plus an optional base collar at the roots.

Reading A (fixed-height axis, limbs peel at computed heights) is a possible *implementation approximation* of B, but **B is the intent**: growth of the support structure is **driven by the primaries that need to leave it**, not by an independent decorative height.

### 3.3 Pipe model (silhouette form)

At a height \(y\) on the trunk axis:

\[
W_{\mathrm{pipe}}(y) \approx \mathrm{packing} \times \sum_{\substack{\text{primaries still} \\ \text{supported at } y}} w_i
\]

- **Near the roots / below the first departure:** width reflects **full support** (all primaries still carried). Wide base is allowed and desirable.
- **As each primary leaves:** remaining support **drops by that primary’s contribution** (or a non-linear function of it — see §6).
- **Above the last primary:** width may fall to a small apex / handoff, or to whatever the table’s floor requires for continuity.

This is a **2D silhouette pipe**: sum of stroke widths (or a soft-capped \(f(\sum w_i)\)), not a claim of hydraulic area conservation (\(\sum r^2\)). Academic pipe models may use area or Murray exponents later; **legibility first**.

### 3.4 Base flare / envelope

Pure stepwise pipe can look mechanical. Apply a **smoothing envelope** that biases:

- **Bottom wider** (buttress / root collar feel),
- **Top of the trunk region narrower** (handoff into canopy),

e.g. \(W(y) = W_{\mathrm{pipe}}(y) \cdot F(y)\) with \(F\) elevated near \(y = 0\) and approaching ~1 higher up. Implement as real `Segment` tapers (`base_width` / `tip_width` already exist), not as a render-only trick — the skeleton is the contract for Grow, pick, and digests.

### 3.5 Knot metaphor (primary entry)

Each primary should feel like a **knot / insertion** into the trunk, not a ray from a vanishing point:

1. The limb **meets the trunk axis** over a vertical span (the internode / insertion zone), not only at a single mathematical tip.
2. Near the join, the limb path may **curve or bias downward/inward** so part of its mass **reads as contributing to the trunk column** before the free branch heading takes over.
3. Once the **pipe width profile + flare envelope** are applied to the axis (and later organic phases mold overlapping strokes), the trunk **emerges** from support + insertions rather than from co-origin overlap alone.

**Skeleton honesty:** M0 silhouette lines will still look “representative.” That is acceptable. Pixel-level / cellular / herringbone / Thrive molding that fuses strokes into bark is **later** (Grow enrichment and render). Do **not** block the structural rework on final organic aesthetics. Structure first; if the structure gives volume and progressive support, later phases can make it feel alive.

### 3.6 What this is *not*

| Rejected reading | Why |
|------------------|-----|
| Constant decorative trunk every repo shares | Original SWOT reject; still reject — widths and height must track data |
| Pure trunkless co-origin fan (current) | Insufficient volume; seed-glyph failure |
| Only fan tuning under current model | Incomplete fix for the human diagnosis |
| Full recursive pipe at every canopy fork in v1 | Scope: **primary-level (and F2 group stems)** first |

---

## 4. Enduring vision vs changed mechanism

| Keep (vision) | Change (mechanism) |
|---------------|--------------------|
| Data-driven base: characteristics remain readable in the silhouette | How mass becomes a column (pipe + internodes, not co-origin overlap alone) |
| L-system foundation for limb interiors | Trunk placement / primary attachment model |
| AC-SKEL-2: empty = seed in roots, not lonely pole | How empty falls out under pipe (no primaries ⇒ no pipe mass) |
| F2: fewer thicker limbs from small top-levels | Group stems may reuse the same pipe helper one level down |
| Determinism, pure gen, table-driven knobs | New/reinterpreted `trunk.*` rows; tests that encoded co-origin claims |
| “Feels like a tree of *this* repository” | Visual language of the base |

**Deviation note for promotion:** `visual-construction.md` currently says nothing draws a trunk and mass is overlap of primaries from one tip. After implementation, replace with: *nothing draws an arbitrary trunk; the trunk is a support column induced by primary internodes, with width from remaining pipe support plus base flare; primaries insert as knots along that column.*

---

## 5. Algorithm sketch (for implementers)

Order is intentional; do not reverse “structure before polish.”

### 5.1 Primaries (unchanged intent)

1. Resolve top-level entries; apply **F2** grouping (`group_below`) so tiny dirs share stems.
2. Each `Primary` has a base width \(w_i\) from the same mass/table path as today (limb `base_width` / packing inputs).

### 5.2 Attachment order (must be deterministic)

Choose a pure function of the manifest + seeds. Preferred first cut:

- **Stable primary identity** (path / group anchor) for order, so `AC-GROW-4` does not reshuffle the whole trunk when one folder gains bytes.
- **Mass** may drive **thickness** and optionally **internode length**, but should not casually reorder attachments every Grow if that makes the trunk “boil.”

Document the chosen rule in code and tests. If mass-weighted spacing is used, keep order path-stable.

### 5.3 Internodes (Reading B)

For each primary in order:

1. Allocate an **internode** (one or more `Segment`s) on the trunk axis whose length is enough for that primary’s **insertion volume** (table-driven: floor + function of \(w_i\) / bushiness).
2. At the insertion, spawn the primary limb from a **site** on that internode (not from a single global tip).
3. Optionally shape the primary’s first segment(s) with a **join bias** (heading initially closer to the trunk / slight downward curve) before free fan heading — the “knot.”
4. After the primary has left, **remaining pipe support** no longer includes that \(w_i\).

The chain of internodes *is* the grown axiom. A short **collar** below the first internode may exist for root-cluster attachment and base flare.

### 5.4 Width profile

- Collar / base: \(W = F(0) \cdot f_{\mathrm{pack}}(\sum_i w_i)\).
- After primary \(k\) leaves: drop \(w_k\) from the sum (or apply soft-cap \(f\)).
- Express with consecutive segments: `base_width` / `tip_width` interpolating pipe × flare at joints.

### 5.5 Fan’s new job

`trunk.fan` becomes primarily **lateral character** (how wide the crown spreads), **not** the sole vertical budget of the trunk. Tests that claim “fan alone controls how far the trunk extends” must be rewritten. Fan still matters for silhouette and AC-SKEL-1 wildness; it should not re-create the squat-seed trap by being the only height source.

### 5.6 Empty and sparse cases

| Case | Expected silhouette |
|------|---------------------|
| Empty repo | No primaries → no pipe internodes → floor seed + root cluster (**AC-SKEL-2**) |
| One primary | Short support + one insertion; may have a thin column (acceptable; better than zero trunk) |
| Many tiny top-levels | F2 compresses to fewer primaries before pipe sum |
| One huge + many tiny | Dominant limb + group stems; pipe reflects actual primaries after F2 |

### 5.7 F2 group stems

Prefer **one mechanism, used twice**: a group stem is a short pipe of its members’ widths with the same internode/knot idea, not a second construction.

---

## 6. Projection / non-literal width (do not overlook)

Even with pipe + internodes, a monorepo with many equal primaries can still produce a **telephone-pole-thick** base if \(W \propto \sum w_i\) is fully linear.

**In scope for the rework (or an explicit follow-up in the same family):**

- Soft-cap or diminishing returns: \(f(\sum w_i)\) with log/sqrt/saturation so “more top-level” does not scale Euclidean-linear forever.
- F2 already reduces primary count; pipe should run **after** F2.

This is cartographic / perceptual projection, not non-Euclidean geometry. Name it in the table comments when implemented.

---

## 7. Layering: skeleton vs later organic fill

| Layer | Responsibility in this story |
|-------|------------------------------|
| **Skeleton (this rework)** | Axis, internode lengths, pipe widths, insertion sites, knot bias, fan for free heading after join |
| **M0 silhouette** | Line + thickness preview; representative, not final art |
| **Enrichment / tiles / CA / Grow molding** | Fuse overlapping structure into organic bark, smooth knots, surface character |
| **Thrive** | Motion, inhabitants — not trunk topology |

Implementers should **not** over-polish skeleton curves to “look finished” in PNG form. Success is: base has **volume**, width **reads as support**, primaries have **room to leave**, empty still a **seed**, characteristics still **drive** the shape.

---

## 8. Scope boundaries

### In scope

- `trunk.rs` placement rewrite (primary attachment + trunk segment chain).
- `TrunkParams` / `lsystem.ron` trunk rows (height/internode policy, flare, packing, fan reinterpretation, basal floor).
- Unit tests for pipe drop, empty seed, determinism of order, F2 reuse.
- Silhouette eye-check on treepo (self), empty, single-primary, multi-primary fixtures.
- Progress note + later promotion into design docs.

### Out of scope (unless a later sprint)

- Full Bevy materials / herringbone baking.
- Recursive pipe at every canopy branch generation.
- True 3D trunk meshes.
- HTML silhouette lab (nice, not required to land the structural change).
- Reopening findings 1–2 (taper across compose, tropism) unless a regression appears.

### Deliberately deferred decisions (pick during implementation if unspecified)

1. Exact internode length formula (floor + weight of \(w_i\)).
2. Exact flare function \(F\).
3. How strong the knot curvature is (table row vs constant).
4. Soft-cap of \(\sum w_i\) constants.
5. Whether root cluster attaches to collar width or a fixed visual band.

Record choices in the PR / progress when made.

---

## 9. Likely code / test touch list

| Area | Notes |
|------|--------|
| `crates/treepo-gen/src/trunk.rs` | Main rewrite: internodes, sites, pipe profile, knot bias |
| `crates/treepo-gen/src/params.rs` | Validate new rows; refuse nonsense (flare, packing ≤ 1000, etc.) |
| `assets/params/lsystem.ron` | `trunk.*` comments + values; version bump if schema changes |
| `the_fan_controls_how_far_the_trunk_extends` | Replace or retarget — fan is no longer the trunk-height law |
| AC-SKEL-2 empty tests | Must still pass |
| F2 grouping tests | Still pass; group stem may gain pipe behavior |
| `tools/m0-silhouette` | No structural change required if it only draws segments |
| Design docs | Promote after eye + gates, not before |

**Gates (expected after change):** `fmt`, `clippy -D warnings`, full workspace tests, `xtask determinism` (skeleton digests will **change** — update golden digests intentionally), `dep-guard`, `cargo deny`, `readonly-audit` 0 writes.

Note: this rework **intentionally changes geometry**; determinism digests for skeletons are not required to stay bit-identical to pre-rework values. Primitive digests (`treepo-det`) should remain untouched.

---

## 10. Acceptance criteria (for the fix sprint)

### Structural / by construction

1. Multi-primary repositories produce a **vertical support region** whose width **decreases** as primaries leave (pipe), not a single co-origin disc of full \(\sum w\).
2. Primaries attach **along** that region (internodes / insertion zones), not exclusively from one tip.
3. Base may be **wide near roots**; mid/upper trunk region is **narrower** than the collar when flare + pipe agree.
4. Empty repository remains **seed + roots**, no lonely monumental trunk (**AC-SKEL-2**).
5. F2 still prevents “one limb per tiny top-level.”
6. Characteristics still drive outcomes: more root breadth / bushiness still reads differently from a sparse clean root — without pure Euclidean blow-up if soft-cap is present.

### Eye (M0 silhouette)

7. Treepo / multi-primary subjects no longer read as **oversized seeds**.
8. Base feels **planted**; limbs feel like they have **room to leave** (knots / insertions), not a starburst from a point.
9. Fan still allows wilder repos to spread more than clean ones (AC-SKEL-1 direction), even if full AC-SKEL-1 corpus pair is still pending.

### Process

10. One parameter family at a time *after* structure lands; do not retune the entire table in the same pass as the rewrite.
11. Local gate green; intentional digest updates only where geometry contracts changed.
12. This workspace draft either promoted or marked superseded with a pointer.

---

## 11. Suggested implementation order (single fix campaign)

1. **Rewrite placement in `trunk.rs`** (internodes + pipe widths + sites); keep F2; minimal knot bias (constant ok).
2. **Replace obsolete trunk-height-by-fan tests**; add pipe-drop + empty-seed + multi-primary height tests.
3. **Table rows** for internode scale + flare; validate; bump version if needed.
4. **Silhouette** self / empty / single-author / deep-nesting (or corpus subjects).
5. **Soft-cap** if eye still shows telephone-pole bases.
6. **Fan retune** as *lateral* only, one family.
7. **Progress + promote** design paragraph when accepted.

Do not start with fan-only under the old model if this document is the sprint’s source of intent.

---

## 12. Open questions for the implementer (clarify only if blocked)

1. Mass-weighted internode length vs uniform — default: **uniform floor + mild \(w_i\) scale**, path-stable order.
2. Knot curvature strength — default: **small constant bias** toward trunk for first segment; table row only if eye needs it.
3. Soft-cap on \(\sum w_i\) — default: **include a simple saturation** if linear sum already looked like a giant seed under the old model; otherwise land pipe first and add in the same campaign if needed.
4. Group stems — default: **same helper** as root trunk.

If unblocked, prefer the defaults above over expanding scope.

---

## 13. Handoff summary (one paragraph)

Replace the co-origin hybrid trunk (single basal segment + all primaries fanning from one tip so overlap *is* the trunk) with **Reading B**: a **pipe-model support column grown as internodes for each primary**, each primary **inserting like a knot** with room to leave, width equal to **remaining packed support** times a **base-wider flare envelope**. Keep data-driven character, F2, AC-SKEL-2, and L-system limb interiors. Accept that M0 lines are representative; organic fusion is later. Fan becomes lateral character, not the trunk’s height budget. This is a justified deviation from the original hybrid because the original failed the eye; the vision is “feels right and still reads the repo,” not “preserve co-origin overlap.”

---

## 14. Session provenance

- Silhouette findings 1–3 closed (taper across compose, tropism/ground band, basal aspect); finding 4 open under old fan/overlap math (`claude-progress.md`).
- Consultation (2026-07-28): pipe model + vertical support; Reading B chosen; knot metaphor; skeleton vs later CA/Grow molding; rework justified over fan-only.
- This file: workspace draft for the implementer prompt supplemental.
