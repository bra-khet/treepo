# Engine Architecture: Grow vs Thrive
**Design Document — Supplemental Living Section**  
**Version:** 0.1  
**Status:** Active draft  
**Last updated:** 2026-07-26  
**Project name:** treepo *(locked 2026-07-27 — [`../CONSTITUTION.md`](../CONSTITUTION.md) §10 R5)*  

This document expands and supersedes the high-level description in [`design-outline.md`](design-outline.md) §4. It is the authoritative reference for the dual-phase engine while we implement in Bevy (Rust ECS).

---

## 1. Purpose & Philosophy

The Grow / Thrive split is both a **performance architecture** and a **value-adding system**.

- **Performance**: Expensive repository analysis, L-system regeneration, and large-scale cellular material updates are rare and off the main thread.
- **Value**: Grow becomes a cinematic, emotionally resonant event. The user does not merely “refresh a diagram”; they watch the living history of their codebase play out as a procedural pixel simulation. Thrive keeps the world feeling continuously alive between those events.

The design deliberately draws from the interaction and simulation languages of Powder Toy, Noita, Terraria, and Minecraft so that visual and semantic feedback feels immediately familiar to players while remaining tightly coupled to real repository state.

---

## 2. High-Level Contracts

| Concern                        | Grow Phase                                      | Thrive Phase                                      |
|--------------------------------|-------------------------------------------------|---------------------------------------------------|
| Structural topology            | Owns complete rebuild / diff-driven transition  | Structure is frozen                                |
| Material / cellular simulation | Full constrained CA pass on changed regions     | Lightweight local CA / particle only on dirty rects |
| Player interaction             | None (or only “direct the next Grow”)           | All interaction                                   |
| Export / recording             | Primary owner of cinematic export               | Can record short loops or stills                  |
| Frequency                      | Event-driven + configurable                     | Continuous (smooth interactive rate — see PRD §7) |
| Threading                      | Background / async                              | Main render + simulation thread                   |

---

## 3. Grow Phase — Structural Update & Cinematic Diff

### 3.1 Core Responsibility

Grow is the phase that answers:  
> “What should the world *become*?”

It calculates the difference between a previous committed world state and a new target state, then plays a high-value, feature-rich, passive cinematic animation that transforms one into the other using procedural cellular automata and L-system growth rules.

### 3.2 Triggers (Initial Set — User Configurable)

- New commit(s) detected on the watched branch / HEAD
- Merge or rebase that moves HEAD significantly
- Explicit user “Grow Now” / “Replay History”
- First-time association of the application with a repository
- Configurable periodic background check (default off or very infrequent)
- Large working-tree changes that the user chooses to “commit into the tree” (future)

All triggers are intended to be user-tunable. The default set is intentionally conservative so that Grow remains special.

### 3.3 First-Time Initialization (Special Grow)

When a repository is associated for the first time:

1. The world begins at a pure seed (root boulder cluster + minimal basal axiom).
2. A single (or multi-stage) Grow animation plays that represents the *entire history* up to the current HEAD as one continuous cinematic sequence.
3. For MVP simplicity this is calculated as one large diff from empty → current.  
   Future refinement: tag-to-tag or selected commit-range stages so the user can watch the project’s eras unfold.

This first Grow is the strongest onboarding and storytelling moment in the product.

### 3.4 Diff-Driven Cinematic Animation

Grow does **not** simply replace the old geometry with the new. It generates a transition:

- Added mass / new limbs grow outward (L-system expansion + material deposition).
- Removed or reduced mass retracts, withers, or is reclaimed by surrounding material.
- High-churn regions show more energetic particle and cellular activity during the transition.
- Ownership or material-family changes can produce visible “re-coloring” or migration waves.
- Classification threshold crossings (e.g., a subtree becoming “core” or “abandoned”) are rendered as explicit, beautiful transformations rather than silent swaps.

The animation is passive and non-interactive during playback (the user can pause, scrub, or cancel). It is designed to be watched.

**Time direction support (design goal)**  
The same diff engine should be able to run forward (growth) or reverse (time-lapse rewind). “Trimming” (removing history while moving forward) is a distinct visual vocabulary from pure reverse playback and should be distinguishable if we implement both.

### 3.5 Export System (MVP Plan)

Grow owns the primary export path because the cinematic sequence is the high-value artifact.

**MVP targets**
- Animated GIF (simple, universally shareable)
- Image sequence (PNG frames) for further processing
- Optional short video container (WebM or similar) if Bevy + ffmpeg integration is low-friction

**Simple architecture**
1. During Grow, frames are captured at a controlled rate into an offscreen buffer or frame queue.
2. On completion (or user request) the queue is encoded.
3. User chooses format and optional length / quality presets.
4. Export can also be triggered on a previously recorded Grow session stored in the project’s local cache.

Later: higher-quality video, transparent background, selective region export, and “story mode” multi-Grow compilations.

### 3.6 Implementation Notes (Bevy)

- Grow runs as an async task or on a dedicated background schedule.
- Progress is reported via events so the UI can show a non-blocking progress indicator or a “cinema mode” overlay.
- The final committed world state (skeleton + materials + enrichment) is written atomically so Thrive never sees a half-built tree.
- Hierarchical path-hash seeds guarantee that the same logical diff always produces the same visual transition.

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
   [Grow Trigger]
        │
        ▼
┌───────────────────────┐
│  Grow Phase           │
│  • Scan / Diff        │
│  • Primitive extract  │
│  • L-system + tiles   │
│  • Constrained CA     │
│  • Cinematic play     │
│  • Atomic commit      │
│  • Optional export    │
└──────────┬────────────┘
           │ new World State
           ▼
┌───────────────────────┐
│  Thrive Phase         │
│  • Render + animate   │
│  • Player interaction │
│  • Dirtiness overlays │
│  • Creatures / idle   │
│  • State Sync (light) │
│  • Event reactions    │
└───────────────────────┘
```

---

## 8. Bevy / Rust Implementation Sketch

- **World state** lives in a set of Bevy resources / components that are only mutated by Grow (or by a carefully controlled State Sync).
- Grow is scheduled as an async task or on `Update` with a heavy `run_if` condition.
- Thrive systems run every frame and query the frozen structural components plus dynamic animation / particle / creature components.
- Events (`GrowStarted`, `GrowProgress`, `GrowFinished`, `StateSyncRequested`, etc.) keep the UI and recording systems decoupled.
- Hierarchical seeds and the Feature System configuration are pure data that both phases read.

---

## 9. Open Questions & Next Decisions

1. Exact user-facing controls for Grow triggers and first-time history depth.
2. Frame-capture and encoding pipeline details for GIF / image-sequence export.
3. How aggressively pending dirtiness should influence local CA during Thrive.
4. Creature population limits and equilibrium rules for the first Thrive inhabitants.
5. Whether reverse-time Grow playback is required for MVP or can be deferred.
6. Precise event contract between a future agent layer and Thrive reactions.

---

## 10. Relationship to Other Documents

- Builds directly on [`design-outline.md`](design-outline.md) §4.
- Consumes the primitive vectors and Interaction Physics defined in [`feature-system.md`](feature-system.md).
- Generates the structural skeleton and enrichment described in [`visual-construction.md`](visual-construction.md), parameterized per [`l-system-parameterization.md`](l-system-parameterization.md).
- Operates within the constraints set by [`../CONSTITUTION.md`](../CONSTITUTION.md) — notably strict phase separation, determinism, and continuous liveliness.

This document will be updated as implementation decisions solidify. All new durable decisions about phase ownership should be recorded here first.

---

*End of document — Engine Architecture: Grow vs Thrive v0.1*
