# Engine Architecture: Grow vs Thrive
**Design Document — Supplemental Living Section**  
**Version:** 0.3  
**Status:** Active draft  
**Last updated:** 2026-07-27  
**Project name:** treepo *(locked 2026-07-27 — [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5)*  

This document expands and supersedes the high-level description in [`design-outline.md`](design-outline.md) §4. It is the authoritative reference for the dual-phase engine while we implement in Bevy (Rust ECS).

*v0.3 (2026-07-27):* ratifies user-controlled **Grow staging**, the ordered **stage stack**, the dedicated **playback / navigation surface**, and a refined **first-time association** path (Watch the birth / Skip to present). Dual-phase ownership is unchanged; staging defers the world-state commit rather than moving topology work into Thrive. Source draft: workspace `engine-architecture-grow-staging-supplement.md` (promoted).

---

## 1. Purpose & Philosophy

The Grow / Thrive split is both a **performance architecture** and a **value-adding system**.

- **Performance**: Expensive repository analysis, L-system regeneration, and large-scale cellular material updates are rare and off the main thread.
- **Value**: Grow becomes a cinematic, emotionally resonant event. The user does not merely “refresh a diagram”; they watch the living history of their codebase play out as a procedural pixel simulation. Thrive keeps the world feeling continuously alive between those events.
- **Agency**: The value of Grow is highest when the user *chooses* to witness it. Structural updates that meet thresholds are **staged**, not forced into interruptive playback. Staging → commit maps to developer muscle memory while preserving the cinematic nature of Grow.

The design deliberately draws from the interaction and simulation languages of Powder Toy, Noita, Terraria, and Minecraft so that visual and semantic feedback feels immediately familiar to players while remaining tightly coupled to real repository state.

---

## 2. High-Level Contracts

| Concern                        | Grow Phase                                      | Thrive Phase                                      |
|--------------------------------|-------------------------------------------------|---------------------------------------------------|
| Structural topology            | Owns complete rebuild / diff-driven transition  | Structure is frozen                                |
| Material / cellular simulation | Full constrained CA pass on changed regions     | Lightweight local CA / particle only on dirty rects |
| Player interaction             | None during cinematic playback                  | All interaction (including stage panel, commit)   |
| Stage stack (pending structure)| Produces staged units; commits on user promote  | Displays stack UI; never mutates topology          |
| Export / recording             | Primary owner of cinematic export               | Can record short loops or stills                  |
| Frequency                      | Event-driven computation; user-timed playback   | Continuous (smooth interactive rate — see PRD §7) |
| Threading                      | Background / async                              | Main render + simulation thread                   |

**Invariant (unchanged):** Grow owns topology and permanent material change. Thrive never rebuilds the skeleton or runs a full material CA. Staging only **defers** the atomic world-state commit; it does not reassign ownership.

---

## 3. Grow Phase — Structural Update & Cinematic Diff

### 3.1 Core Responsibility

Grow is the phase that answers:  
> “What should the world *become*?”

It calculates the difference between a previous committed world state and a new target state, materializes that work as one or more **staged Grow changes**, and — when the user chooses — plays a high-value, feature-rich, passive cinematic animation that transforms one into the other using procedural cellular automata and L-system growth rules. Only a user-initiated **Grow commit** (or equivalent) promotes staged work into the live world state Thrive reads.

### 3.2 Triggers → Stage, Do Not Auto-Play

Triggers remain user-configurable and conservative by default. When a threshold is met they **enqueue computation and staging**, they do **not** seize the session with forced playback:

- New meaningful commit(s) on the watched branch / HEAD
- Merge or rebase that moves HEAD significantly
- Explicit user “Stage Grow” / “Grow Now” / “Replay History”
- First-time association of the application with a repository
- Configurable periodic background check (default off or very infrequent)
- Large working-tree changes that the user chooses to stage into the tree (future)

Defaults stay conservative so Grow remains special. The user decides when (and how) to apply and watch any staged sequence.

### 3.3 Grow Staging Model

When a trigger threshold is met:

1. The system performs the deterministic computation in the background (or on a background schedule).
2. The result is stored as a **staged Grow change** — a discrete, replayable unit containing:
   - Target structural state (or the diff from the previous committed / previous-stage state)
   - Pre-computed transition frames / animation recipe (eagerly generated so playback is instant when the user starts it)
   - Metadata (source commits / time range, size of change, classification crossings, etc.)
3. Staged changes are pushed onto an ordered **stack** (arbitrary length). The stack is the single source of pending structural history.

Because every stage is produced from hierarchical path-hash seeds, the same logical change always yields the same visual transition. Stages remain discrete and independently addressable.

Thrive continues to show working-tree dirtiness and lightweight pending previews. **State Sync never produces staged Grow entries.** Only a user-initiated Grow commit promotes one or more stages into the live world state.

### 3.4 Playback & Navigation Surface

The staged stack is exposed through a dedicated control surface:

- **Visual language:** a simple, VS Code–style source-tree abstraction (lines + dots / nodes) for the ordered stages.
- **Aesthetic:** carved wood / organic panel so the control feels native to the world-tree rather than a generic IDE widget.
- **Capabilities:**
  - Step-by-step playback of individual stages
  - Continuous playback of the entire stack (or any contiguous segment)
  - Forward and reverse direction
  - Direct jump to any stage (click a node)
  - Optional “play all remaining” or “collapse to final state”

During **cinematic playback of a stage**, interaction stays limited to pause, scrub, cancel, and stack navigation — the camera is not free-roaming the half-applied tree. Outside playback, the panel is ordinary Thrive UI over the last **committed** world.

This gives precise mental navigation over history the user is about to (or is currently) watching, without requiring them to understand L-system or CA mechanics.

### 3.5 First-Time Association & Large-Repo Experience

First association (empty seed → full history) is a special, high-value Grow sequence, and the product front door under Constitution R1 — but it is **never** an unavoidable long wait.

**Recommended flow:**

1. Background computation of the staged history begins immediately on association.
2. An onboarding modal appears with:
   - A short visual tutorial (Grow vs Thrive, staging, dirtiness, world-tree metaphor).
   - An attractive, cinematic progress indicator (procedurally themed — e.g. growing branch / material deposition) so wait time feels consistent with the product.
3. Two clear options remain available at all times:
   - **Watch the birth** — begin (or continue) cinematic playback of the staged sequence once enough material is ready. This is the recommended front door.
   - **Skip to present** (escape hatch) — load the final committed world state directly into Thrive. Computing the final state alone is comparatively cheap and must always be offered.

**v1 form of the first-run sequence** may be a single empty→HEAD stage; **staged history replay** (multiple checkpoints as stack entries) is the richer form (`F-GROW-7`). Either way the same agency model applies: stage in the background, user chooses Watch or Skip.

Chunked / progressive computation of history epochs (and starting early-frame playback while later epochs finish) is a welcome performance refinement. It is **not** required for the first implementation and must not be locked out by data-model decisions made now.

### 3.6 Diff-Driven Cinematic Animation

Grow does **not** simply replace the old geometry with the new. It generates a transition:

- Added mass / new limbs grow outward (L-system expansion + material deposition).
- Removed or reduced mass retracts, withers, or is reclaimed by surrounding material.
- High-churn regions show more energetic particle and cellular activity during the transition.
- Ownership or material-family changes can produce visible “re-coloring” or migration waves.
- Classification threshold crossings (e.g., a subtree becoming “core” or “abandoned”) are rendered as explicit, beautiful transformations rather than silent swaps.

The animation is passive during stage playback (pause, scrub, cancel, stack jump). It is designed to be watched.

**Time direction support (design goal)**  
The same diff engine should be able to run forward (growth) or reverse (time-lapse rewind). “Trimming” (removing history while moving forward) is a distinct visual vocabulary from pure reverse playback and should be distinguishable if we implement both. Whether reverse re-uses the same frame sequence or regenerates a distinct vocabulary is an open implementation note (§9).

### 3.7 Export System (MVP Plan)

Grow owns the primary export path because the cinematic sequence is the high-value artifact.

**MVP targets**
- Animated GIF (simple, universally shareable)
- Image sequence (PNG frames) for further processing
- Optional short video container (WebM or similar) if Bevy + ffmpeg integration is low-friction

**Simple architecture**
1. During Grow (stage render-ahead and/or playback), frames are captured at a controlled rate into an offscreen buffer or frame queue.
2. On completion (or user request) the queue is encoded.
3. User chooses format and optional length / quality presets.
4. Export can also be triggered on a previously recorded Grow session stored in the project’s local cache.

Later: higher-quality video, transparent background, selective region export, and “story mode” multi-Grow compilations.

### 3.8 Implementation Notes (Bevy)

- Grow **computation** runs as an async task or on a dedicated background schedule; it produces staged units, not an immediate live-world mutation.
- Progress is reported via events so the UI can show non-blocking progress, onboarding progress art, or a “cinema mode” overlay during playback.
- The final committed world state (skeleton + materials + enrichment) is written **atomically on Grow commit** so Thrive never sees a half-built tree.
- Hierarchical path-hash seeds guarantee that the same logical diff always produces the same visual transition.
- Open implementation details (threshold UI defaults, stack persistence across restarts, memory budget for pre-computed transition assets, reverse-playback vocabulary, panel ↔ camera integration) are deferred to Bevy experiments; product direction above is enough to keep work aligned.

---

## 4. Thrive Phase — Live World & Interaction

### 4.1 Core Responsibility

Thrive is the continuous real-time loop. It answers:  
> “How does the current world *feel* and how does the player interact with it?”

The overall procedural structure produced by the last Grow is treated as stable. Thrive never performs a full structural rebuild. Its job is to keep that structure feeling alive and to surface transient or pending state.

### 4.2 What Belongs in Thrive

- All player input and camera control
- Idle and ambient animation (wind, breathing, light pulsing, subtle bobbing)
- Small critters / workers in stable equilibria (pathing, idle behaviors, reactions to local state)
- Dirtiness visualization:
  - Untracked
  - Modified
  - Staged
  - Pending delete
  - Conflicted
- Pending-change previews that will become structural only on the next Grow
- Lightweight local cellular / particle effects on dirty rectangles only
- Real-time reactions to external signals that do not require topology change (see §5)
- Hover, selection, and inspection feedback

**Design rule**: If the user can click it, drag it, or cause an immediate visual reaction, it belongs to Thrive.

### 4.3 Stable Equilibria for Creatures

Creatures and small agents must remain in constrained, looping, or goal-directed behaviors that do not require global pathfinding every frame. Prefer:

- Local steering + simple state machines
- Attraction / repulsion to nearby material or activity heat
- Periodic “home” or rest behaviors so populations stay balanced

This keeps Thrive cheap and prevents the world from feeling chaotic or performance-hungry.

### 4.4 Ambient & State Animation Vocabulary

Examples of Thrive-only expression (inspired by familiar simulation games):

- Wind / leaf sway modulated by local activity heat
- Soft breathing or pulsing of high-ownership or recently-touched limbs
- Material “settling” or micro-flow on high-churn surfaces (very limited CA)
- Firefly / spore particles near documentation or test-rich areas
- Small workers that walk established routes or react to dirtiness
- Notice boards or hanging tags that appear for open issues (visual only)
- Subtle glow or desaturation for commits-behind-remote state

These are continuous or looping. They communicate meaning without changing topology.

---

## 5. Handling External & Semi-Structural State

Some signals sit between pure structure and pure transient state:

| Signal                        | Recommended Home          | Notes |
|-------------------------------|---------------------------|-------|
| Working-tree dirtiness        | Thrive                    | Visible immediately; becomes structural on Grow |
| Open issues / PRs             | Thrive (overlay)          | Visual indicators, workers, notice boards. Only become structural if merged/committed |
| Commits behind / ahead remote | Thrive (lightweight)      | Can be refreshed by a small “State Sync” callable |
| Fetch / pull progress         | Thrive                    | Progress feedback only |
| Agent tool-call reactions     | Thrive (future)           | Speech bubbles, themed reactions |
| Actual new commit / merge     | Triggers full Grow        | Topology may change |

### 5.1 Decision: Mini-Grow / State Sync Inside Thrive

**Recommendation**: Do **not** create a full mini-Grow inside Thrive.

Instead provide a narrowly scoped **State Sync** routine that Thrive may call:

- Can refresh remote-tracking counts, open issue/PR metadata, and other non-topological signals.
- May update enrichment overlays and creature goals.
- Must never rebuild L-system skeleton or run a full material CA pass.
- Runs infrequently (on focus, on explicit user refresh, or on a long timer) and is cancellable.

This keeps the mental model clean:  
**Grow = topology & permanent material change.**  
**Thrive = everything else that makes the world feel current and alive.**

---

## 6. Interaction Language & Simulation Inspiration

We deliberately borrow visual and semantic conventions from games players already understand:

**Powder Toy / Noita-style**
- Materials have density, temperature, reaction affinities.
- Local cellular rules produce secondary detail (moss, scars, settling dust) without global simulation.
- Destruction or growth is visible as material movement rather than instantaneous replacement.

**Terraria / Minecraft-style**
- Clear “this block / this limb belongs to this system” readability.
- Ambient creatures that react to the player and to world state without complex AI.
- Lighting and particle cues that telegraph activity or danger (here: churn, dirtiness, ownership).

**General principles applied to treepo**
- Every visual change should be readable as either “the structure just changed” (Grow) or “the current state is expressing itself” (Thrive).
- Pending changes are shown as temporary material or markers that the next Grow will resolve.
- Transformations (classification shifts) are celebrated with extra particle and animation budget during Grow.

---

## 7. Data Flow Summary

```
Repository (filesystem + git)
        │
        ▼
   [Grow Trigger]  ──►  background compute (does not seize UI)
        │
        ▼
┌───────────────────────┐
│  Grow (compute)       │
│  • Scan / Diff        │
│  • Primitive extract  │
│  • L-system + tiles   │
│  • Constrained CA     │
│  • Timeline / frames  │
└──────────┬────────────┘
           │ push staged unit(s)
           ▼
┌───────────────────────┐
│  Stage Stack          │  ← ordered pending structural history
│  (replayable units)   │
└──────────┬────────────┘
           │ user: play / step / jump / commit
           ▼
┌───────────────────────┐
│  Grow (playback)      │
│  • Cinema playback    │
│  • Atomic commit      │  only on user promote
│  • Optional export    │
└──────────┬────────────┘
           │ new World State
           ▼
┌───────────────────────┐
│  Thrive Phase         │
│  • Render + animate   │
│  • Player interaction │
│  • Stage panel UI     │
│  • Dirtiness overlays │
│  • Creatures / idle   │
│  • State Sync (light) │  never stages Grow entries
│  • Event reactions    │
└───────────────────────┘
```

---

## 8. Bevy / Rust Implementation Sketch

- **Committed world state** lives in Bevy resources / components mutated only on Grow commit (or by carefully controlled State Sync overlays that never touch topology).
- Grow **computation** is an async task producing staged units + timelines; it does not depend on Bevy types in generative crates.
- Grow **playback** and the stage panel live in `treepo-app`; Thrive systems run every frame against the last committed snapshot.
- Events (`GrowComputeStarted`, `GrowStageReady`, `GrowPlayback*`, `GrowCommitted`, `StateSyncRequested`, etc.) keep UI, staging, and recording decoupled.
- Hierarchical seeds and the Feature System configuration are pure data that both phases read.

### 8.1 Agent live control — Bevy Remote Protocol (BRP)

Coding agents inspect and drive a **running** Bevy shell via BRP. This is **developer/agent
tooling**, not a product surface. Full decision: architecture **D10**
([`.planning/architecture-treepo.md`](../../.planning/architecture-treepo.md)).

| Piece | Role |
|-------|------|
| Cargo feature `brp` on `treepo-app` | Opt-in only; never default; never release |
| `bevy` feature `bevy_remote` | Core BRP methods (entities, components, resources, query) |
| `bevy_brp_extras::BrpExtrasPlugin` | Adds `RemotePlugin` + HTTP if missing; screenshots, input, shutdown, diagnostics |
| Default port | **15702** (`BRP_EXTRAS_PORT` to override) |
| Host MCP | Globally installed `bevy_brp_mcp`, registered as MCP server `bevy_brp` |

**Registration (Phase 5 scaffolding):** under `#[cfg(feature = "brp")]` only, call
`app.add_plugins(BrpExtrasPlugin::default())` from `treepo-app` (see
`crates/treepo-app/src/debug/brp.rs` in the architecture file tree). Do not add BRP plugins
unconditionally.

**`N2` note:** BRP uses HTTP on **localhost** only when the `brp` feature is enabled. Product
paths remain fully offline (`NFR-8`). CI `cargo deny check` and storefront builds use default
features so network clients never enter the shipped dependency graph.

**Agent run recipe:** `cargo run -p treepo-app --features brp`, then use `bevy_brp` MCP tools
against port 15702.

---

## 9. Open Questions & Next Decisions

### Resolved (product direction, 2026-07-27)

- **Triggers stage rather than auto-play.** User promotes stages via Grow commit; dual-phase ownership unchanged.
- **First-run agency.** Background compute + onboarding modal; always offer **Watch the birth** and **Skip to present**.
- **Stage stack + navigation panel** are first-class surface area (carved-wood tree of stages).

### Still open (implementation / tuning)

1. Exact threshold configuration UI and defaults for what counts as a stage-worthy change.
2. Persistence of the staged stack across application restarts (and eviction if oversized).
3. Memory / disk budget for pre-computed transition assets on very large histories.
4. Whether reverse playback re-uses the same frame sequence or regenerates a distinct visual vocabulary.
5. How the carved-wood navigation panel integrates with the main camera / Thrive interaction model.
6. Frame-capture and encoding pipeline details for GIF / image-sequence export.
7. How aggressively pending dirtiness should influence local CA during Thrive.
8. Creature population limits and equilibrium rules for the first Thrive inhabitants.
9. Precise event contract between a future agent layer and Thrive reactions.
10. Checkpoint count / “enough tags” threshold for multi-stage history replay (`F-GROW-7`) — deferred to M3 footage review per PRD §11 Q4.

---

## 10. Relationship to Other Documents

- Builds directly on [`design-outline.md`](design-outline.md) §4.
- Consumes the primitive vectors and Interaction Physics defined in [`feature-system.md`](feature-system.md).
- Generates the structural skeleton and enrichment described in [`visual-construction.md`](visual-construction.md), parameterized per [`l-system-parameterization.md`](l-system-parameterization.md).
- Operates within the constraints set by [`../CONSTITUTION.md`](../CONSTITUTION.md) — notably strict phase separation, determinism, continuous liveliness, and R1’s first-Grow-as-front-door (realized as **Watch the birth**, not forced autoplay).
- Requirements: [`../PRD.md`](../PRD.md) §5.1 (association / first run), §5.7 (Grow, including staging and `F-GROW-7`).
- Build plan: [`.planning/architecture-treepo.md`](../../.planning/architecture-treepo.md) (stage stack data model and phases).

This document will be updated as implementation decisions solidify. All new durable decisions about phase ownership should be recorded here first.

---

*End of document — Engine Architecture: Grow vs Thrive v0.3*
