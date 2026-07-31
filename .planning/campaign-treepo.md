# Campaign: treepo
> Architecture: `.planning/architecture-treepo.md` | PRD: `docs/PRD.md` v1.2 | Constitution: `docs/CONSTITUTION.md` v1.3
> Created: 2026-07-27 | Phases: 13 | Complexity: high | Mode: greenfield
> Updated: 2026-07-27 — D11 Grow stage stack, user-controlled playback, first-run Watch/Skip

Rust / Bevy desktop application. Grow (rare, expensive, cinematic) and Thrive (continuous,
cheap, interactive) as first-class phases separated by crate boundaries. Grow **stages** on
trigger and **commits** on user promote (architecture D11); dual-phase contracts unchanged.

## Standing Rules

These apply to **every** phase. A phase is not complete if any of them regress.

- `cargo build --workspace` succeeds on Windows, macOS, and Linux.
- `cargo clippy --workspace -- -D warnings` is clean.
- `cargo xtask dep-guard` passes — no crate in the generative set (`treepo-det`,
  `treepo-model`, `treepo-vcs`, `treepo-id`, `treepo-store`, `treepo-gen`, `treepo-grow`,
  `treepo-export`) depends on `bevy`. This is how `N6` is enforced.
- `cargo deny check` passes — no network-capable dependency enters the **default** feature
  graph (`N2`). The optional `brp` feature on `treepo-app` is **never** enabled for deny checks,
  CI release builds, or storefront packages (architecture D10).
- From Phase 1 onward: `cargo xtask readonly-audit` reports zero writes to any fixture working
  tree (`N1`, `AC-MAN-2`).
- From Phase 3 onward: `cargo xtask determinism` is green across three runs on three platforms
  (`N3`, `AC-DET-1/2/3`).
- From Phase 5 onward: `cargo xtask id-coverage` reports zero colored pixels without an element
  ID (`P1`, `N7`).
- From Phase 5 onward: agents may run `cargo run -p treepo-app --features brp -- <path>` for
  live Bevy Remote Protocol control via host MCP server `bevy_brp` (`bevy_brp_mcp`, port 15702).
  Launch that binary via shell with the feature flag — not `brp_launch` (rebuilds without `brp`).
  Product and default builds must not register BRP.

## Execution Order

```
0 → 1 → ┬→ 2 ──────────────┐
        └→ 3 (M0) → 4 → ┬→ 5 (M1) ─┬→ 7 → ┬→ 8 → ┬→ 11 → 12 (M3)
                        └→ 6 ──────┘      └→ 9 (M2) → 10 ─┘
```

Parallel-safe: **2 ∥ 3**, **5 ∥ 6**, **10 ∥ 11**. Flag these for Fleet.

---

## Phase 0 — Workspace & determinism foundation
- **Goal**: Establish the workspace, determinism primitives, and the CI gates every later phase
  is measured against.
- **Depends on**: none
- **Parallel-safe with**: none
- **Files**: `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `clippy.toml`,
  `.cargo/config.toml`, `.github/workflows/{ci,determinism}.yml`, `crates/treepo-det/**`,
  `xtask/src/{main,determinism,dep_guard}.rs`
- **End Conditions**:
  - [ ] `cargo build --workspace` succeeds on all three platforms
  - [ ] `cargo test -p treepo-det` passes
  - [ ] `treepo-det::trig` is bit-identical across all three platforms for 10,000 sampled angles (`F-SKEL-6`)
  - [ ] `cargo xtask dep-guard` passes
  - [ ] `cargo deny check` passes with no network-capable crate in the graph (`N2`)

## Phase 1 — Model & repository extraction
- **Goal**: Turn a repository into a complete `Manifest` with one history traversal.
- **Depends on**: 0
- **Parallel-safe with**: none
- **Files**: `crates/treepo-model/**`, `crates/treepo-vcs/**`,
  `assets/filters/default-exclusions.ron`, `assets/params/folder-signals.ron`,
  `tools/corpus/**`, `tests/readonly.rs`, `tests/degenerate.rs`
- **⚠ Spike first**: RISK-A — validate `gix` blob-diff line counting against the T2 fixture
  before building on it. Keep a subprocess-`git` adapter behind the same trait as contingency.
  If the spike fails, **stop and escalate** — the `R1`/`N1` costs of the fallback need a
  decision, not a silent adoption.
- **End Conditions**:
  - [ ] Corpus fixtures for T0–T3 and all `F-CORP-2`/`F-CORP-3` shapes build reproducibly
  - [ ] T2 full extraction completes under 60 s on reference hardware (`AC-EXT-1`)
  - [ ] `git blame` is never invoked during extraction — asserted in test (`F-EXT-3`, RISK-1)
  - [ ] `.mailmap` fixture collapses aliases; same repo without it yields higher `author_count` (`AC-EXT-3`)
  - [ ] `cargo xtask readonly-audit` reports zero writes to any fixture working tree (`AC-MAN-2`, `AC-EXT-4`)
  - [ ] Every PRD §6 edge case has a passing test in `tests/degenerate.rs`

## Phase 2 — Store & repository identity
- **Goal**: Persist manifests in app data, keyed by an identity that survives folder moves.
- **Depends on**: 1
- **Parallel-safe with**: 3
- **Files**: `crates/treepo-store/**`, `tests/identity.rs`
- **Note**: implements E2 — `manifest.bin` + `manifest-meta.json`, not `manifest.json`.
- **End Conditions**:
  - [ ] All three identity tiers resolve correctly against the `F-CORP-3` fixtures (`F-MAN-3`)
  - [ ] Two clones of one remote resolve to one store; second open skips extraction (`AC-MAN-4`)
  - [ ] Moving a no-remote fixture does not orphan its store (`AC-MAN-5`)
  - [ ] Process killed mid-write leaves the previous manifest valid (`AC-MAN-3`)
  - [ ] Delete-then-regenerate produces a byte-identical `manifest.bin` (`AC-MAN-1`)
  - [ ] `manifest-meta.json` is human-readable; `schema_version` mismatch forces regeneration (`F-MAN-6`)

## Phase 3 — Skeleton generation → **M0 EXIT**
- **Goal**: Produce distinguishable, deterministic silhouettes from real repositories.
- **Depends on**: 1
- **Parallel-safe with**: 2
- **Files**: `crates/treepo-gen/src/{params,lsystem/**,trunk,aggregate}.rs`,
  `assets/params/lsystem.ron`, `tools/m0-silhouette/**`, `tests/determinism.rs`
- **End Conditions**:
  - [ ] `m0-silhouette` renders line-and-thickness PNGs for every corpus fixture
  - [ ] Triple-run on three platforms yields nine identical skeleton hashes (`AC-DET-1`, `AC-DET-2`)
  - [ ] Clean vs. high-skew T1 repos produce measurably different silhouettes from one parameter table (`AC-SKEL-1`)
  - [ ] T0 produces a seed and root cluster, not a lonely trunk (`AC-SKEL-2`)
  - [ ] T3 skeleton generation completes within the §7 Grow budget (`AC-SKEL-3`)
  - [ ] Editing `lsystem.ron` changes output with no recompile (`AC-SKEL-4`)
  - [ ] Parameter row `A3+B2/B3+C1+D1+E3+F2+G1` confirmed or revised **with recorded evidence**

## Phase 4 — Identity policy, materials & enrichment
- **Goal**: Give the skeleton material, ownership, and enrichment — pseudonymous from the first commit.
- **Depends on**: 3
- **Parallel-safe with**: none
- **Files**: `crates/treepo-id/**`,
  `crates/treepo-gen/src/{material,normalize,gradient,enrichment,classify}.rs`,
  `assets/palettes/**`, `assets/wordlists/pseudonyms.ron`,
  `assets/params/{materials,enrichment,classify}.ron`, `tests/privacy.rs`
- **End Conditions**:
  - [x] Pseudonyms and author colors identical across all three platforms (`AC-ID-2`)
  - [x] No real name, email, or handle in any generated output under default policy (`AC-ID-1`)
  - [x] No `treepo-model` type exposes an ordered contributor collection or a share as a figure (`AC-MAT-3`, `N4`)
  - [x] A 2%-share contributor retains visible mosaic presence on the T2 fixture (`AC-MAT-2`)
  - [x] Adjacent palette entries meet the minimum perceptual-separation threshold (`AC-MAT-4`)

## Phase 5 — Bevy shell, static baking & navigation → **M1 EXIT**
- **Goal**: A still, zoomable, clickable tree at consumer quality.
- **Depends on**: 2, 4
- **Parallel-safe with**: 6
- **Files**: `crates/treepo-render/**`,
  `crates/treepo-app/src/{main,phase,snapshot_sync,window}.rs`,
  `crates/treepo-app/src/ui/{mod,theme,onboarding,progress}.rs`,
  `crates/treepo-app/src/interact/**`,
  `crates/treepo-app/src/debug/{mod,brp}.rs` (D10 BRP, feature-gated),
  `crates/treepo-app/Cargo.toml` (optional feature `brp` → `bevy/bevy_remote` + `bevy_brp_extras`),
  `assets/shaders/**`, `assets/textures/tiles/**`,
  `assets/fonts/ui.ttf`, `xtask/src/id_coverage.rs`
- **⚠ Watch**: RISK-B — measure T3 baked-layer + ID-buffer memory against `NFR-3` (4 GB) early.
- **Agent tooling (D10)**: when scaffolding `treepo-app`, wire
  `bevy_brp_extras::BrpExtrasPlugin` under `#[cfg(feature = "brp")]` only. Host MCP server
  `bevy_brp` (`bevy_brp_mcp`) is already expected on the developer machine; default BRP port
  15702. Do **not** enable `brp` by default. Sketch in architecture D10.
- **End Conditions**:
  - [ ] T2 legible at far, medium, near zoom; known top-level directory findable by eye within 30 s — **recorded user test, ≥3 participants** (`AC-NAV-1`)
  - [!] Zoom far→near on T3 holds 30 fps at minimum spec (`AC-NAV-2`) — **regressed by the
        materials pass, on purpose and with the numbers recorded.** It was green on the dev
        machine before materials had an appearance: worst frame 14.5 ms over a full 12-band
        traversal, zero over 33.3 ms. Shading each texel rather than interpolating between two
        of them costs **13.7 → 71 ns per texel**, measured, and the same traversal now spends
        ~30 of roughly 2,500 frames over budget, worst 30–53 ms on frames that bake. The bake
        budget was re-cut against that measurement (`BAKE_TEXELS_PER_FRAME` replacing
        `BAKES_PER_FRAME`, `MAX_PIECE_SIDE` halved), which took it from 268 ms to 53 ms, and
        further tuning is the wrong lever: the bake is CPU rasterization **on the main thread**,
        and moving it to the async pool is what removes the trade. That is Phase 7's
        `grow_task`, where a producer already publishes while Thrive reads. Minimum spec remains
        unmeasured on top of this
  - [ ] `cargo xtask id-coverage` reports zero colored pixels without an element ID (`P1`, `N7`, `AC-INSP-1`)
  - [ ] Clicking any element resolves to a real path or an explicit aggregate (`AC-INSP-1`)
  - [x] T3 resident memory under 4 GB (`NFR-3`) — peak working set **676 MB** over a full
        far→near traversal, steady 380–507 MB. Caveat recorded rather than resolved: the
        *extraction* pass peaks at 3.1 GB working set / 4.05 GB private, which is the T3 memory
        risk that survives, and it is Phase 1 code rather than Phase 5's
  - [ ] `readonly-audit` green across association → extraction → session, wired into CI (`AC-MAN-2`)
  - [ ] Cold launch on a cached T2 repository under 5 s (`NFR-4`)
  - [ ] **D10**: `--features brp` run listens on localhost:15702; default build has no BRP;
        `cargo deny check` (default features) still green; release never ships with `brp`

## Phase 6 — Grow simulation & stage units
- **Goal**: Compute deterministic transitions as discrete staged units (not live-world mutations).
- **Depends on**: 4
- **Parallel-safe with**: 5
- **Files**: `crates/treepo-grow/src/{lib,diff,timeline,migration,connectivity,transform,stage,budget}.rs`
- **Note**: implements E1 — determinism is verified on the `GrowTimeline`, not on pixels.
  Stage unit type is D11 / `F-GROW-11`.
- **End Conditions**:
  - [ ] The same snapshot pair produces an identical `GrowTimeline` hash across three runs on three platforms (`AC-GROW-2`)
  - [ ] Connectivity assertion holds after every migration pass — no disconnected mass (`N5`)
  - [ ] Adding one file to the T2 fixture confines staged-unit changes to the affected limb (`AC-GROW-4`)
  - [ ] Cancellation mid-simulation publishes nothing to the committed world (`AC-GROW-3`)
  - [ ] Stage unit type is serializable and independently addressable (`F-GROW-11`)

## Phase 7 — Staging, playback, cinema & first-run agency
- **Goal**: Stack-based user control (stage on trigger, play on demand, commit on promote);
  first-run Watch the birth / Skip to present (D11).
- **Depends on**: 5, 6
- **Parallel-safe with**: none
- **Files**: `crates/treepo-app/src/{grow_task,playback,triggers,stage_stack}.rs`,
  `crates/treepo-app/src/ui/{stage_panel,onboarding,progress}.rs`,
  `crates/treepo-export/src/ring.rs`, `crates/treepo-app/src/window.rs` (cinema mode)
- **End Conditions**:
  - [ ] Grow playback holds 24 fps with no dropped frames through the most expensive transformation on minimum spec (`AC-GROW-5`)
  - [ ] Main thread never blocks during Grow compute/playback; previous committed world keeps animating — frame-time trace (`AC-GROW-1`)
  - [ ] Met trigger stages without interrupting Thrive (`AC-GROW-6`)
  - [ ] Stage panel: step, continuous play, jump, collapse-to-final (`F-GROW-4`, `F-GROW-12`, `AC-GROW-7`)
  - [ ] Grow commit atomic; discard/cancel leaves prior commit intact (`F-GROW-13`, `AC-GROW-3`)
  - [ ] First association offers Watch + Skip; T2 usable path within 10 s (`F-ASSOC-6`, `AC-ASSOC-1`, `AC-ASSOC-4`)
  - [ ] Pause, scrub, and cancel function during stage playback (`F-GROW-4`)

## Phase 8 — Thrive liveliness & dirtiness
- **Goal**: The world stays alive between Grows, and shows what is uncommitted.
- **Depends on**: 5, 7
- **Parallel-safe with**: 9
- **Files**: `crates/treepo-app/src/thrive/**`,
  `crates/treepo-render/src/{particles,overlay_dirty}.rs`, `crates/treepo-app/src/debug/**`
- **End Conditions**:
  - [ ] Steady-state Thrive performs zero repository I/O over 10 minutes — filesystem trace (`AC-THR-1`, `NFR-1`)
  - [ ] T2 holds 30 fps at minimum spec with ambient animation and particles active
  - [ ] Editing a working-tree file updates its overlay within 2 s without a Grow (`AC-THR-2`)
  - [ ] Creature population stays bounded over a 30-minute idle run (`AC-THR-3`)
  - [ ] `F-THR-8` debug toggle present in dev builds, absent from release builds — asserted by test

## Phase 9 — Export → **M2 EXIT**
- **Goal**: Get a shareable artifact out, carrying nothing it should not.
- **Depends on**: 7
- **Parallel-safe with**: 8
- **Files**: `crates/treepo-export/src/{lib,gif,png_seq,video,scrub}.rs`,
  `crates/treepo-app/src/ui/export_dialog.rs`
- **End Conditions**:
  - [ ] A T1 first-run Grow exports to a GIF under 10 MB with no manual tuning (`AC-EXP-1`)
  - [ ] Exported files carry no repository path, name, or contributor identity in metadata under default settings — verified with an external metadata tool (`AC-EXP-2`)
  - [ ] Export never blocks Thrive — frame-time trace shows no stall (`AC-EXP-3`)
  - [ ] **M2 gate**: all five PRD §2 jobs demonstrably servable end to end

## Phase 10 — Settings, store browser & privacy surface
- **Goal**: Give the user control over triggers, filters, and everything treepo has stored.
- **Depends on**: 9
- **Parallel-safe with**: 11
- **Files**: `crates/treepo-app/src/ui/settings/**`,
  `crates/treepo-store/src/{browse,in_repo,package}.rs`
- **End Conditions**:
  - [ ] Store browser lists every repository with size on disk and purges any or all (`F-MAN-9`, `N2`)
  - [ ] Identity reveal reachable only from settings, never from the export dialog, and requires explicit confirmation (`AC-ID-3`)
  - [ ] Toggling reveal changes live view and subsequent exports together (`AC-ID-4`)
  - [ ] Opting into `.treepo/` leaves `git status` clean and root `.gitignore` untouched (`AC-MAN-6`)
  - [ ] Per-repository settings survive a folder move (`AC-SET-1`)

## Phase 11 — Multi-checkpoint history, workers & enrichment depth
- **Goal**: Deepen the front door with multi-stage history on the same stack model (D11).
- **Depends on**: 8, 9
- **Parallel-safe with**: 10
- **Files**: `crates/treepo-grow/src/checkpoints.rs`,
  `crates/treepo-app/src/thrive/workers.rs`, `crates/treepo-gen/src/enrichment.rs`,
  `crates/treepo-app/src/interact/search.rs`
- **Cut line**: `F-GROW-7` is the designated descope if M3 overruns (RISK-6).
- **End Conditions**:
  - [ ] Multi-checkpoint history reconstructs from the log stream with **zero checkouts** and pushes stack stages — asserted by test (`F-GROW-7`)
  - [ ] Checkpoint sampling prefers tags, falls back to time; count and threshold recorded with the footage that set them (PRD §11 Q4)
  - [ ] Search locates a path and moves the camera to it (`F-NAV-6`)
  - [ ] Multi-stage first-run on T2 holds Phase 7 playback budget and uses the same panel as single-stage Grow

## Phase 12 — Widget mode, onboarding polish & packaging → **M3 EXIT**
- **Goal**: Ship it. Core Watch/Skip lands in Phase 7; this phase polishes and packs.
- **Depends on**: 10, 11
- **Parallel-safe with**: none
- **Files**: `crates/treepo-app/src/window.rs` (widget mode),
  `crates/treepo-app/src/ui/onboarding.rs`, `.github/workflows/budgets.yml`,
  `xtask/src/budget.rs`, `tests/budgets.rs`
- **Cut line**: `F-WIN-3` (widget mode) is the designated descope if M3 overruns (RISK-6).
- **End Conditions**:
  - [ ] Widget mode holds its reduced budget at materially lower CPU/GPU cost, measured (`AC-WIN-1`)
  - [ ] Idle widget-mode CPU under 5% of one core on recommended hardware (`NFR-6`)
  - [ ] Every §7 budget passes on minimum spec across T0–T3 (`F-CORP-1`)
  - [ ] Signed, installable artifacts build for Windows, macOS, Linux (`NFR-7`)
  - [ ] T4 repository warns before starting, remains cancellable, does not crash (`F-CORP-1`)
  - [ ] **Clean-machine walkthrough**: install → open repository → Watch or Skip → export, with no terminal and no `git` installed (`R1`, decision D3)

---

## Deploy (final step of Phase 12)

Not a gating phase — a failed deploy does not fail the campaign.

- **Platform**: consumer desktop, storefront-first (`R1`); Windows, macOS, Linux (`N8`)
- **Method**: per-platform `cargo` release builds in CI; Windows and macOS code signing plus
  notarization; storefront depot upload. Linux ships a self-contained archive and an optional
  Flatpak manifest.
- **Environment variables**: none at product runtime. `TREEPO_DATA_DIR` (test only),
  `TREEPO_DEBUG_UI=1` (dev builds only), `BRP_EXTRAS_PORT` (dev/agent only with `--features brp`,
  default 15702 — D10). No product key, endpoint, or credential exists — `N2` leaves nothing to
  configure for shipped builds.
- **Pre-deploy checks**: `determinism`, `readonly-audit`, `id-coverage`, `deny check` (**default
  features only**), and `budget` all green; clean-machine walkthrough passed; release artifact
  has BRP disabled.
