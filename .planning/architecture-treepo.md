# Architecture: treepo
> PRD: `docs/PRD.md` v1.0 | Constitution: `docs/CONSTITUTION.md` v1.3 | Mode: greenfield | Date: 2026-07-27

Rust / Bevy desktop application. Two runtime phases — **Grow** (rare, expensive, cinematic)
and **Thrive** (continuous, cheap, interactive) — with the boundary between them enforced by
crate dependencies rather than by discipline.

**Governing idea:** the Constitution's hardest constraints (`N1`, `N3`, `N6`, `N9`) are all
"you must never do X" rules. Every one of them is made *structurally impossible* rather than
*prohibited by convention* — a crate that cannot see Bevy types cannot touch the live world, a
crate that cannot link `std::time` cannot read the clock, a process that never opens a repo for
write cannot mutate one. Where a constraint could not be structurally enforced, it has a CI
gate instead, and both are listed per phase.

---

## Constraint Enforcement Map

How each non-negotiable becomes a mechanism rather than an intention. This table is the
architecture's spine; every decision below serves it.

| Constraint | Mechanism | Verified by |
|---|---|---|
| `N1` repo read-only | `treepo-vcs` uses `gix` (pure Rust, no subprocess) and opens repositories read-only. No hook, filter, or fsmonitor execution path exists. | `xtask readonly-audit` traces all filesystem writes during a full session; `tests/readonly.rs` |
| `N2` data stays local | No HTTP client in the dependency graph at all. `cargo-deny` denies `reqwest`, `hyper`, `ureq`, `tokio-net`. | `deny.toml`; dependency audit in CI |
| `N3` determinism | `treepo-det` is the only source of randomness and trig. Generative crates forbid `std::time`, `rand::thread_rng`, and `HashMap` iteration by lint. | `xtask determinism` — triple-run + tri-platform hash compare |
| `N4` never rank people | No type in `treepo-model` exposes an ordered contributor collection. `AuthorShare` is unordered and carries no public accessor returning a percentage or rank. | `tests/privacy.rs` asserts no UI string formats a share as a figure |
| `N5` coherent structure | `treepo-grow` migration passes operate on a connectivity-checked graph; a cleanup pass runs after every flow pass. | Connectivity assertion in `treepo-grow`; `tests/degenerate.rs` |
| `N6` no structural work in the loop | `treepo-gen` and `treepo-grow` do not depend on `bevy`. They cannot be called from a Thrive system with a `World` in scope. | Compile-time (dependency graph); `xtask` asserts crate deps |
| `N7` appearance from primitives only | Every baked pixel carries an element ID in a parallel ID buffer. A pixel with color and no ID is an unaccountable pixel. | Automated ID-coverage scan (see D5) — this makes `P1` machine-checkable |
| `N8` desktop-native | Bevy desktop targets only; no wasm feature. | CI build matrix |
| `N9` pseudonymous default | `treepo-model::AuthorKey` carries no name. Real identity lives in `treepo-id` behind `IdentityPolicy`, which is the only way to resolve a display string. | `tests/privacy.rs`; type-level — the render layer never receives a real name |

---

## File Tree

Complete tree for v1. Every file listed.

```
treepo/
├── Cargo.toml                              # workspace manifest
├── Cargo.lock
├── rust-toolchain.toml                     # pinned toolchain — determinism input
├── deny.toml                               # cargo-deny: forbidden dependency classes (N2)
├── clippy.toml
├── .cargo/
│   └── config.toml
├── .github/
│   └── workflows/
│       ├── ci.yml                          # build + test, 3 platforms
│       ├── determinism.yml                 # AC-DET-1/2/3
│       └── budgets.yml                     # §7 performance budgets, nightly
├── .gitignore                              # exists
├── README.md                               # exists
├── docs/                                   # exists — Constitution, PRD, design set
├── .planning/
│   └── architecture-treepo.md              # this document
│
├── assets/
│   ├── params/
│   │   ├── lsystem.ron                     # F-SKEL-5 — parameter table, hot-reloadable
│   │   ├── materials.ron                   # F-MAT-1 material families
│   │   ├── enrichment.ron                  # F-MAT-5 placement rules
│   │   ├── classify.ron                    # classification thresholds
│   │   └── folder-signals.ron              # F-EXT-5 conventional folder dictionary
│   ├── palettes/
│   │   ├── author-palette.ron              # F-ID-4 — perceptually separated
│   │   └── material-families.ron
│   ├── wordlists/
│   │   └── pseudonyms.ron                  # F-ID-3 — themed two-word source
│   ├── filters/
│   │   └── default-exclusions.ron          # F-EXT-8 built-in exclusion set
│   ├── fonts/
│   │   └── ui.ttf
│   ├── shaders/
│   │   ├── tree_static.wgsl
│   │   ├── heat_overlay.wgsl
│   │   ├── dirtiness_overlay.wgsl
│   │   └── id_pick.wgsl                    # N7/P1 element-ID buffer
│   └── textures/
│       └── tiles/
│           ├── herringbone_atlas.png       # constraint-tile vocabulary
│           └── herringbone_atlas.ron       # adjacency constraints
│
├── crates/
│   ├── treepo-det/                         # determinism primitives — zero deps beyond core
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fixed.rs                    # fixed-point scalar + angle types
│   │       ├── trig.rs                     # F-SKEL-6 — table trig, no libm
│   │       ├── rng.rs                      # ChaCha8, explicit seed only
│   │       ├── hash.rs                     # stable path + identity hashing
│   │       └── ordered.rs                  # BTree-backed map/set wrappers
│   │
│   ├── treepo-model/                       # core types — no I/O, no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── path.rs                     # RepoPath, non-UTF8 handling
│   │       ├── primitives/
│   │       │   ├── mod.rs
│   │       │   ├── structural.rs           # F-EXT-1
│   │       │   ├── size.rs
│   │       │   ├── temporal.rs             # F-EXT-2
│   │       │   ├── ownership.rs            # N4 — unordered by construction
│   │       │   ├── derived.rs              # F-EXT-6
│   │       │   └── folder_signal.rs        # F-EXT-5
│   │       ├── manifest.rs                 # schema_version, per-path records
│   │       ├── identity.rs                 # RepoIdentity, AuthorKey (no name field)
│   │       ├── snapshot.rs                 # WorldSnapshot — the phase handoff type
│   │       ├── segment.rs                  # skeleton segments
│   │       ├── material.rs                 # families, mosaics
│   │       ├── enrichment.rs
│   │       └── aggregate.rs                # F-SKEL-7 container nodes
│   │
│   ├── treepo-vcs/                         # gix extraction — no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── discover.rs                 # F-ASSOC-2 — validate, detect shallow, reject bare
│   │       ├── walk.rs                     # F-EXT-1 filesystem walk, sorted
│   │       ├── filter.rs                   # F-EXT-8 filtering rules
│   │       ├── log_pass.rs                 # F-EXT-2 single history traversal
│   │       ├── blame.rs                    # F-EXT-3 deferred, resumable, sampled
│   │       ├── mailmap.rs                  # F-EXT-9
│   │       ├── status.rs                   # F-THR-4 dirtiness read
│   │       └── lang.rs                     # F-EXT-4 language/LOC classification
│   │
│   ├── treepo-id/                          # N9 identity policy — no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pseudonym.rs                # F-ID-3
│   │       ├── palette.rs                  # F-ID-4
│   │       ├── self_ident.rs               # F-ID-1
│   │       └── policy.rs                   # F-ID-5 — single gate for live view + export
│   │
│   ├── treepo-store/                       # app-data persistence — no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── paths.rs                    # F-MAN-2 platform roots
│   │       ├── resolve.rs                  # F-MAN-3 three-tier identity
│   │       ├── manifest_io.rs              # F-MAN-6/7 versioned, atomic
│   │       ├── world_io.rs
│   │       ├── cache.rs
│   │       ├── browse.rs                   # F-MAN-9 enumerate, size, purge
│   │       ├── in_repo.rs                  # F-MAN-10 self-ignoring opt-in dir
│   │       └── package.rs                  # F-MAN-11 shareable package
│   │
│   ├── treepo-gen/                         # pure generation — no bevy, no I/O
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── params.rs                   # F-SKEL-5 table loading
│   │       ├── lsystem/
│   │       │   ├── mod.rs
│   │       │   ├── grammar.rs              # parametric + stochastic productions
│   │       │   ├── turtle.rs               # uses treepo-det::trig
│   │       │   └── compose.rs              # F-SKEL-2 hierarchical composition
│   │       ├── trunk.rs                    # F-SKEL-3 hybrid basal axiom
│   │       ├── aggregate.rs                # F-SKEL-7 container synthesis
│   │       ├── material.rs                 # F-MAT-1/2
│   │       ├── normalize.rs                # F-MAT-3 log + clamp + floor
│   │       ├── gradient.rs                 # F-MAT-4 age/recency positioning
│   │       ├── enrichment.rs               # F-MAT-5
│   │       └── classify.rs                 # threshold classification
│   │
│   ├── treepo-grow/                        # diff + timeline simulation — no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── diff.rs                     # F-GROW-3 snapshot diff
│   │       ├── timeline.rs                 # deterministic keyframe timeline
│   │       ├── migration.rs                # constrained CA material flow
│   │       ├── connectivity.rs             # N5 cleanup pass
│   │       ├── transform.rs                # F-GROW-8 threshold sequences
│   │       ├── checkpoints.rs              # F-GROW-7 staged replay
│   │       └── budget.rs                   # cancellation + progress reporting
│   │
│   ├── treepo-render/                      # bevy render layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── bake.rs                     # static layer baking, chunked
│   │       ├── chunk.rs                    # chunk streaming + residency
│   │       ├── id_buffer.rs                # N7/P1 element-ID raster + picking
│   │       ├── lod.rs                      # F-NAV-1/3/4
│   │       ├── material_bind.rs
│   │       ├── particles.rs
│   │       ├── overlay_dirty.rs            # F-THR-4
│   │       └── camera.rs                   # F-NAV-2
│   │
│   ├── treepo-export/                      # encoders — no bevy
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ring.rs                     # F-GROW-10 frame ring buffer
│   │       ├── gif.rs                      # F-EXP-1
│   │       ├── png_seq.rs                  # F-EXP-1
│   │       ├── video.rs                    # F-EXP-2
│   │       └── scrub.rs                    # F-EXP-6 / F-ID-8 metadata stripping
│   │
│   └── treepo-app/                         # the Bevy application
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── phase.rs                    # ★ phase boundary — states, events, transitions
│           ├── grow_task.rs                # off-thread Grow driver
│           ├── snapshot_sync.rs            # snapshot → ECS reconciliation
│           ├── triggers.rs                 # F-GROW-2 trigger watcher
│           ├── thrive/
│           │   ├── mod.rs
│           │   ├── ambient.rs              # F-THR-1
│           │   ├── heat.rs                 # F-THR-2
│           │   ├── dirtiness.rs            # F-THR-4
│           │   ├── workers.rs              # F-THR-5
│           │   └── state_sync.rs           # F-THR-6
│           ├── playback.rs                 # F-GROW-4/10 cinema playback
│           ├── interact/
│           │   ├── mod.rs
│           │   ├── pick.rs                 # F-INSP-1/2 via ID buffer
│           │   ├── search.rs               # F-NAV-6
│           │   └── inspect.rs              # F-INSP-3/4/5
│           ├── ui/
│           │   ├── mod.rs
│           │   ├── theme.rs
│           │   ├── onboarding.rs           # F-ASSOC-1/2/3/6
│           │   ├── progress.rs
│           │   ├── inspector.rs            # F-INSP-1
│           │   ├── export_dialog.rs        # F-EXP-*
│           │   └── settings/
│           │       ├── mod.rs
│           │       ├── triggers.rs         # F-SET-1
│           │       ├── privacy.rs          # F-SET-2, F-ID-6, F-MAN-9/10
│           │       ├── filters.rs          # F-SET-3
│           │       ├── performance.rs      # F-SET-4
│           │       ├── display.rs          # F-SET-5
│           │       └── advanced.rs         # F-SET-6
│           ├── window.rs                   # F-WIN-1/2/3
│           └── debug/
│               ├── mod.rs                  # egui debug surface, dev builds only
│               └── intensity.rs            # F-THR-8
│
├── tools/
│   ├── m0-silhouette/                      # M0 headless debug renderer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   └── corpus/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                     # F-CORP-* fixture builder
│           ├── synth.rs                    # deterministic synthetic repos
│           └── pins.ron                    # real repos pinned by SHA
│
├── xtask/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── determinism.rs                  # AC-DET-1/2/3 harness
│       ├── readonly_audit.rs               # AC-MAN-2 zero-write verification
│       ├── id_coverage.rs                  # N7/P1 unaccountable-pixel scan
│       ├── dep_guard.rs                    # N6 crate-dependency assertions
│       └── budget.rs                       # §7 budget benchmarks
│
└── tests/
    ├── determinism.rs                      # AC-DET-1/2/3
    ├── readonly.rs                         # AC-MAN-2, AC-EXT-4
    ├── identity.rs                         # AC-MAN-4/5, AC-ID-2
    ├── privacy.rs                          # AC-ID-1/3/4, AC-MAT-3, AC-INSP-2
    ├── degenerate.rs                       # PRD §6 edge-case table
    └── budgets.rs                          # §7 Grow and frame budgets
```

---

## Component Breakdown

### Feature: Deterministic foundation (`N3`, `F-SKEL-6`)
- **Files**: `crates/treepo-det/**`
- **Dependencies**: none — this is the root of the graph
- **Complexity**: medium — table trig and fixed-point need care, but the surface is small

### Feature: Repository extraction (`F-EXT-1`…`F-EXT-9`)
- **Files**: `crates/treepo-vcs/**`, `crates/treepo-model/src/primitives/**`
- **Dependencies**: `treepo-det`, `treepo-model`, `gix`
- **Complexity**: high — `F-EXT-2`'s single-pass history traversal is the performance
  linchpin (RISK-1), and `gix` diff-with-line-counts is the least trodden path in the stack

### Feature: Storage & identity (`F-MAN-1`…`F-MAN-13`)
- **Files**: `crates/treepo-store/**`, `crates/treepo-model/src/identity.rs`
- **Dependencies**: `treepo-det`, `treepo-model`, `gix` (remote/root-commit reads)
- **Complexity**: medium

### Feature: Contributor identity policy (`F-ID-1`…`F-ID-8`, `N9`, `N4`)
- **Files**: `crates/treepo-id/**`
- **Dependencies**: `treepo-det`, `treepo-model`
- **Complexity**: low-medium — the difficulty is type discipline, not algorithms

### Feature: Skeleton generation (`F-SKEL-1`…`F-SKEL-7`)
- **Files**: `crates/treepo-gen/src/{params,lsystem/**,trunk,aggregate}.rs`
- **Dependencies**: `treepo-det`, `treepo-model`
- **Complexity**: high — the visual identity lives or dies here

### Feature: Material, ownership & enrichment (`F-MAT-1`…`F-MAT-6`)
- **Files**: `crates/treepo-gen/src/{material,normalize,gradient,enrichment,classify}.rs`
- **Dependencies**: `treepo-gen` skeleton, `treepo-id`
- **Complexity**: high

### Feature: Grow simulation (`F-GROW-1`…`F-GROW-10`)
- **Files**: `crates/treepo-grow/**`, `crates/treepo-app/src/{grow_task,playback,triggers}.rs`
- **Dependencies**: `treepo-gen`, `treepo-model`
- **Complexity**: high — diff-driven CA migration is the most intricate subsystem

### Feature: Thrive world (`F-THR-1`…`F-THR-8`)
- **Files**: `crates/treepo-app/src/thrive/**`, `crates/treepo-render/**`
- **Dependencies**: `treepo-render`, snapshot handoff
- **Complexity**: medium

### Feature: Navigation, LOD & inspection (`F-NAV-*`, `F-INSP-*`)
- **Files**: `crates/treepo-render/src/{lod,chunk,id_buffer,camera}.rs`,
  `crates/treepo-app/src/interact/**`
- **Dependencies**: baked static layers
- **Complexity**: medium-high

### Feature: Export (`F-EXP-1`…`F-EXP-7`)
- **Files**: `crates/treepo-export/**`, `crates/treepo-app/src/ui/export_dialog.rs`
- **Dependencies**: Grow frame ring
- **Complexity**: medium

### Feature: Application shell, settings & windows (`F-SET-*`, `F-WIN-*`, `F-ASSOC-*`)
- **Files**: `crates/treepo-app/src/{main,window,ui/**}.rs`
- **Dependencies**: everything
- **Complexity**: medium — raised by `R1`'s polish bar, not by logic

---

## Data Model

No database. Two persisted artifacts per repository, plus global settings. Layout is
`F-MAN-2`; formats are D9 below.

### RepoIdentity
- **Fields**: `key: [u8;32]` (hash), `tier: Tier1Remote | Tier2RootCommit | Tier3PathHash`,
  `source_value: String` (the normalized remote URL, root SHA, or path), `resolved_at: u64`
- **Relationships**: one per store directory; names the directory (`F-MAN-3`)
- **Persisted**: `identity.json` — human-readable by design (`N2` inspectability)

### Manifest
- **Fields**: `schema_version: u32`, `treepo_version: String`, `built_from_commit: Option<Oid>`,
  `paths: Vec<PathRecord>`, `authors: AuthorTable`, `filters: FilterOverrides`
- **Relationships**: one per identity; regenerable (`F-MAN-8`)
- **Persisted**: `manifest.bin` + `manifest-meta.json` (D9)

### PathRecord
- **Fields**: `path: RepoPath`, `structural: StructuralPrimitives`, `size: SizePrimitives`,
  `temporal: TemporalPrimitives`, `ownership: OwnershipPrimitives`, `derived: DerivedSignals`,
  `folder_signal: Option<FolderSignal>`, `seed: u64`
- **Relationships**: parent/child by path; aggregated into `AggregateNode` past the LOD cap

### AuthorKey / AuthorTable
- **Fields**: `key: [u8;16]` (hash of mailmap-normalized email), `share: AuthorShare`,
  `recency: u64`, `is_self: bool`
- **`N4` note**: `AuthorTable` is a `BTreeMap` keyed by hash with **no ordering by share and no
  public accessor returning a rank or percentage**. Real names live only in `treepo-id` behind
  `IdentityPolicy` and never enter `treepo-model`.

### WorldSnapshot *(the phase handoff type)*
- **Fields**: `snapshot_id: u64`, `built_from: Oid`, `segments: Vec<Segment>`,
  `materials: MaterialMap`, `enrichments: Vec<Enrichment>`, `aggregates: Vec<AggregateNode>`,
  `heat_weights: Vec<f32>`, `id_map: IdMap`
- **Relationships**: produced by Grow, consumed by Thrive, swapped atomically as
  `Arc<WorldSnapshot>` (D4). Immutable once published.
- **Persisted**: `world/snapshot-<id>.bin`

### GrowTimeline
- **Fields**: `from: u64`, `to: u64`, `keyframes: Vec<Keyframe>`, `duration_frames: u32`,
  `checkpoints: Vec<Checkpoint>`
- **Relationships**: derived from a snapshot pair; deterministic (`AC-GROW-2`, per D6 scope)
- **Persisted**: `cache/timeline-<from>-<to>.bin` — evictable

### Settings
- **Global**: `settings.json` — display, performance, default triggers
- **Per-repository**: `config.json` — identity policy level (`F-ID-6`), filter overrides
  (`F-SET-3`), trigger overrides. Keyed by identity, so it survives folder moves (`AC-SET-1`).

---

## Key Decisions

### D1 — Phase boundary: Grow is a pipeline, not a Bevy state
- **Chosen**: Grow is a pure, off-thread pipeline in crates that **do not depend on `bevy`**
  (`treepo-gen`, `treepo-grow`). It consumes a `Manifest` and produces an
  `Arc<WorldSnapshot>` + `GrowTimeline`. Thrive is the only ECS-resident phase. `treepo-app`
  owns a small `PhaseState` (`Idle | Growing | Playing`) driving playback UI and system run
  conditions — but the *work* never enters the World.
  Reasoning: `N6` says structural work must never enter the continuous loop. If Grow is a
  Bevy state mutating the same `World`, nothing prevents a future contributor from adding a
  scan to a system that happens to run in that state — the constraint survives only as
  discipline. With Grow in a crate that has no Bevy types at all, the violation does not
  compile. This is the single most load-bearing decision in the document.
- **Rejected**: *Bevy `States` + system sets with run conditions* — idiomatic and simpler, but
  leaves `N6` as a code-review rule; the whole World is reachable from any Grow system.
- **Rejected**: *Separate sub-`App`/`World` with handoff* — achieves isolation but pays Bevy's
  full scheduling cost for work that is not a game loop, and still lets ECS types leak into
  generation.

### D2 — Determinism: isolate the sources, then verify by hashing
- **Chosen**: A single `treepo-det` crate owns *all* nondeterminism sources — seeded RNG
  (ChaCha8), fixed-point trig tables (`F-SKEL-6`), stable hashing, and ordered collection
  wrappers. Generative crates deny `std::time`, `rand::thread_rng`, `std::collections::HashMap`,
  and float transcendentals by lint. CI runs each corpus fixture three times per platform and
  compares serialized-output hashes across all nine runs.
  Reasoning: `N3` is absolute and `AC-DET-2` requires cross-platform equality. The three
  classic Rust determinism leaks are `HashMap` iteration order, unsorted directory reads, and
  platform `libm` trig — all three are closed here at the type and lint level rather than by
  testing for their symptoms.
- **Rejected**: *Discipline plus CI hash tests alone* — catches violations late, and the
  failure mode (a tree that changes shape on another machine) is miserable to bisect.
- **Rejected**: *Fixed-point arithmetic everywhere* — closes the problem completely but
  poisons every downstream calculation with conversion noise and makes the material and
  layout code far harder to write and read. Fixed-point is confined to angles and turtle
  state, where the trig problem actually lives.

### D3 — Git access: `gix` (gitoxide), not subprocess `git`
- **Chosen**: `gix`, pure Rust, repositories opened read-only.
  Reasoning: two constraints converge on this. **`N1`** — a subprocess `git` honors repository
  config that can execute programs (`core.fsmonitor`, `core.pager`, aliases, textconv filters);
  that is a live code-execution path out of a repository treepo does not trust, and closing it
  by flag-auditing every invocation is a standing liability. **`R1`** — a consumer product
  cannot assume a `git` binary exists on the machine; on Windows many buyers will not have one.
  `gix` removes both problems and the external dependency along with them.
- **Rejected**: *subprocess `git`* — simplest and most familiar, `--numstat` is exactly the
  shape `F-EXT-2` wants, but it requires git installed (fails `R1`) and opens the config-driven
  execution surface (risks `N1`).
- **Rejected**: *`git2`/libgit2* — mature and safe on hooks, but a C dependency complicating
  three-platform packaging, and its blame/diff performance is not better than `gix` for the
  single-pass traversal that matters here.
- **Risk**: `gix` is younger; the per-file line-count diff for `F-EXT-2` is more assembly than
  a `--numstat` parse. Tracked as RISK-A.

### D4 — Phase handoff: immutable snapshot, atomic swap, ECS reconciliation
- **Chosen**: Grow publishes `Arc<WorldSnapshot>` into an `ArcSwap` resource. Thrive reads the
  current snapshot each frame; on change, `snapshot_sync.rs` reconciles ECS entities to match.
  Reasoning: satisfies `F-GROW-5` (atomic commit) and `AC-GROW-3` (cancel restores previous)
  for free — cancellation simply never publishes. Thrive can never observe a half-built tree
  because a partially constructed snapshot is not reachable.
- **Rejected**: *Grow mutates ECS entities directly* — requires locking or staged command
  buffers, and makes "half-built tree" a state that exists and must be defended against.
- **Rejected**: *Double-buffered mutable worlds* — doubles peak memory, which `NFR-3` cannot
  afford at T3.

### D5 — Rendering: Grow bakes static layers; Thrive composites and animates
- **Chosen**: Grow rasterizes the static tree into chunked layer textures per LOD band, plus a
  parallel **element-ID buffer** (u32 per pixel). Thrive renders visible chunks as a small
  number of quads and adds dynamic elements (particles, workers, dirtiness) as ECS entities.
  Picking samples the ID buffer.
  Reasoning: this is what makes `NFR-2` true rather than hoped-for — Thrive's frame cost scales
  with *visible chunks*, not with the 80k paths of a T3 repository. One entity per element dies
  somewhere around T2. It also maps exactly onto the static/dynamic separation the design set
  borrowed from Noita, and the ID buffer turns `P1`/`N7` into a machine-checkable property: a
  pixel with color and no ID is an unaccountable pixel, and `xtask id-coverage` scans for
  exactly that.
- **Rejected**: *One Bevy sprite entity per visual element* — natural in ECS, and fails at T2/T3
  on both frame time and memory.
- **Rejected**: *Fully GPU-driven instanced rendering* — highest ceiling, but a large custom
  render-pipeline investment before M1, and it does not solve picking any better than an ID
  buffer does.

### D6 — Grow determinism boundary: the timeline is deterministic, pixels are not
- **Chosen**: `treepo-grow` produces a `GrowTimeline` — every frame's element positions,
  material states, and particle seeds as data. *That* is what `AC-DET-1/2` and `AC-GROW-2` are
  verified against. Rasterization of the timeline is GPU work and is **not** required to be
  bit-identical across machines.
  Reasoning: `AC-GROW-2` originally said "frame-identical transition on every run and every
  platform." Taken as *pixel*-identical it is unachievable without a software rasterizer — GPU
  vendors, drivers, and rounding differ. Taken as *timeline*-identical it is both achievable
  and the thing the constraint is actually protecting: the same change always produces the same
  performance. **Adopted 2026-07-27; `AC-GROW-2` amended in the PRD to match** (E1).
- **Rejected**: *Software rasterization for bit-identical frames* — satisfies the strictest
  reading, and forfeits the GPU, `NFR-1`, and the entire visual quality bar under `R1`.
- **Rejected**: *Dropping frame-level determinism* — would let a transition vary run to run,
  breaking `P2` in the place it is most visible.

### D7 — Grow playback: render-ahead ring buffer shared with export
- **Chosen**: A rasterization worker renders timeline frames into a bounded ring buffer ahead
  of playback; the playback system consumes at a fixed rate (24 fps, `NFR-10`); export drains
  the same buffer. Playback starts once lead exceeds a threshold; the worker applies
  backpressure when the ring is full.
  Reasoning: this is `F-GROW-10` as specified, and it makes `F-EXP-4`'s "same frames" property
  structural — the watched sequence and the exported artifact are literally one buffer.
- **Rejected**: *Render at playback rate, drop frames when slow* — simplest, and produces
  exactly the stutter `AC-GROW-5` forbids.
- **Rejected**: *Fully pre-render, then play* — smoothest, but adds the entire render time to
  time-to-first-pixel, which `AC-ASSOC-1` (10 s to first visible growth) cannot absorb at T2.

### D8 — UI: Bevy UI for product surfaces, egui for the debug surface only
- **Chosen**: Shipped UI (onboarding, inspector, settings, export) in Bevy UI with a bespoke
  theme. The dev/QA surface (`F-THR-8`, parameter tuning, determinism inspection) in
  `bevy_egui`, compiled only in dev builds.
  Reasoning: `R1` sets a consumer polish bar; egui's default look reads unmistakably as a
  developer tool and would undercut it. egui is simultaneously ideal for the debug surface,
  where iteration speed matters and appearance does not.
- **Rejected**: *egui throughout* — much faster to build, wrong product surface under `R1`.
- **Rejected**: *A native shell (Tauri/wry) around the Bevy view* — best-looking chrome, but
  two runtimes, two input models, and a packaging story that fights `NFR-7`.

### D9 — Persistence format: binary manifest with a JSON metadata sidecar
- **Chosen**: `manifest.bin` (postcard, canonical field order) + `manifest-meta.json`
  (`schema_version`, `treepo_version`, path count, sizes). `identity.json`, `config.json`, and
  `settings.json` stay JSON.
  Reasoning: a T3 manifest is ~80k rich primitive records; JSON would be hundreds of MB and
  seconds to parse, against a 5 s cold-launch budget (`NFR-4`). A canonical binary encoding
  also makes `AC-MAN-1`'s byte-identical requirement straightforward rather than dependent on
  JSON key ordering. The small files stay JSON so that `N2`'s inspectability promise — a user
  can see what treepo holds — is honored where a human would actually look.
  **Adopted 2026-07-27; `F-MAN-2` and `F-MAN-6` amended in the PRD to match** (E2).
- **Rejected**: *JSON throughout* — matches the PRD text, fails `NFR-4` at T3.
- **Rejected**: *SQLite* — good queries and incremental writes, but adds a C dependency, and
  nothing in v1 queries the manifest relationally; it is loaded whole.

---

## Build Phases

### Phase 0: Workspace & determinism foundation
- **Goal**: Establish the workspace, the determinism primitives, and the CI gates that every
  later phase is measured against.
- **Files**: `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `clippy.toml`,
  `.cargo/config.toml`, `.github/workflows/{ci,determinism}.yml`, `crates/treepo-det/**`,
  `xtask/src/{main,determinism,dep_guard}.rs`
- **Dependencies**: none
- **End Conditions**:
  - [ ] `cargo build --workspace` succeeds on Windows, macOS, Linux
  - [ ] `cargo test -p treepo-det` passes
  - [ ] `treepo-det::trig` produces bit-identical output on all three platforms for 10,000
        sampled angles (`F-SKEL-6`)
  - [ ] `cargo xtask dep-guard` passes: no crate in the generative set depends on `bevy`
  - [ ] `cargo deny check` passes with no network-capable crate in the graph (`N2`)

### Phase 1: Model & repository extraction
- **Goal**: Turn a repository into a complete `Manifest` with one history traversal.
- **Files**: `crates/treepo-model/**`, `crates/treepo-vcs/**`,
  `assets/filters/default-exclusions.ron`, `assets/params/folder-signals.ron`,
  `tools/corpus/**`, `tests/readonly.rs`
- **Dependencies**: Phase 0
- **End Conditions**:
  - [ ] Corpus fixtures for T0–T3 plus all `F-CORP-2`/`F-CORP-3` shapes build reproducibly
  - [ ] T2 full extraction completes under 60 s on reference hardware (`AC-EXT-1`)
  - [ ] `git blame` is never invoked during extraction (`F-EXT-3` deferred) — asserted in test
  - [ ] `.mailmap` fixture collapses aliases; the same repo without it yields a higher
        `author_count` (`AC-EXT-3`)
  - [ ] `cargo xtask readonly-audit` reports zero writes to any fixture working tree
        (`AC-MAN-2`, `AC-EXT-4`)
  - [ ] Every PRD §6 edge case has a passing test in `tests/degenerate.rs`

### Phase 2: Store & repository identity
- **Goal**: Persist manifests in app data, keyed by a stable identity that survives moves.
- **Files**: `crates/treepo-store/**`, `tests/identity.rs`
- **Dependencies**: Phase 1
- **End Conditions**:
  - [ ] All three identity tiers resolve correctly against the `F-CORP-3` fixtures
  - [ ] Two clones of one remote resolve to one store; second open skips extraction
        (`AC-MAN-4`)
  - [ ] Moving a no-remote fixture does not orphan its store (`AC-MAN-5`)
  - [ ] Process killed mid-write leaves the previous manifest valid (`AC-MAN-3`)
  - [ ] Delete-then-regenerate produces a byte-identical `manifest.bin` (`AC-MAN-1`, E2)
  - [ ] `manifest-meta.json` is human-readable and a `schema_version` mismatch forces
        regeneration rather than a partial parse (`F-MAN-6`, E2)

### Phase 3: Skeleton generation — **M0 exit**
- **Goal**: Produce distinguishable, deterministic silhouettes from real repositories.
- **Files**: `crates/treepo-gen/src/{params,lsystem/**,trunk,aggregate}.rs`,
  `assets/params/lsystem.ron`, `tools/m0-silhouette/**`, `tests/determinism.rs`
- **Dependencies**: Phase 1 (Phase 2 not required — M0 is headless and can hold manifests in
  memory)
- **End Conditions**:
  - [ ] `m0-silhouette` renders line-and-thickness PNGs for every corpus fixture
  - [ ] Triple-run on three platforms yields nine identical skeleton hashes (`AC-DET-1`,
        `AC-DET-2`)
  - [ ] A clean T1 repo and a high-skew T1 repo produce measurably different silhouettes from
        one parameter table (`AC-SKEL-1`) — measured by a recorded shape metric, reviewed by eye
  - [ ] T0 produces a seed and root cluster, not a lonely trunk (`AC-SKEL-2`)
  - [ ] T3 skeleton generation completes within the §7 Grow budget (`AC-SKEL-3`)
  - [ ] Editing `lsystem.ron` changes output with no recompile (`AC-SKEL-4`)
  - [ ] Parameter row `A3+B2/B3+C1+D1+E3+F2+G1` confirmed or revised **with recorded evidence**

### Phase 4: Identity policy, materials & enrichment
- **Goal**: Give the skeleton material, ownership and enrichment — pseudonymous from the first
  commit.
- **Files**: `crates/treepo-id/**`, `crates/treepo-gen/src/{material,normalize,gradient,
  enrichment,classify}.rs`, `assets/palettes/**`, `assets/wordlists/pseudonyms.ron`,
  `assets/params/{materials,enrichment,classify}.ron`, `tests/privacy.rs`
- **Dependencies**: Phase 3
- **End Conditions**:
  - [ ] Pseudonyms and author colors are identical across all three platforms (`AC-ID-2`)
  - [ ] No real name, email, or handle appears in any generated output under default policy
        (`AC-ID-1`)
  - [ ] No type in `treepo-model` exposes an ordered contributor collection or a share as a
        figure — asserted by test (`AC-MAT-3`, `N4`)
  - [ ] A 2%-share contributor retains visible mosaic presence on the T2 fixture (`AC-MAT-2`)
  - [ ] Adjacent palette entries meet the minimum perceptual-separation threshold (`AC-MAT-4`)

### Phase 5: Bevy shell, static baking & navigation — **M1 exit**
- **Goal**: A still, zoomable, clickable tree at consumer quality.
- **Files**: `crates/treepo-render/**`, `crates/treepo-app/src/{main,phase,snapshot_sync,
  window}.rs`, `crates/treepo-app/src/ui/{mod,theme,onboarding,progress}.rs`,
  `crates/treepo-app/src/interact/**`, `assets/shaders/**`, `assets/textures/tiles/**`,
  `assets/fonts/ui.ttf`, `xtask/src/id_coverage.rs`
- **Dependencies**: Phase 2, Phase 4
- **End Conditions**:
  - [ ] A T2 repository is legible at far, medium and near zoom; a known top-level directory is
        findable by eye within 30 s (`AC-NAV-1`) — recorded user test, ≥3 participants
  - [ ] Zoom far→near on T3 holds 30 fps at minimum spec (`AC-NAV-2`)
  - [ ] `cargo xtask id-coverage` reports zero colored pixels without an element ID
        (`P1`, `N7`, `AC-INSP-1`)
  - [ ] Clicking any element resolves to a real path or an explicit aggregate (`AC-INSP-1`)
  - [ ] `readonly-audit` passes across a full association → extraction → session run and is
        wired into CI from this phase onward (`AC-MAN-2`)
  - [ ] Cold launch on a cached T2 repository under 5 s (`NFR-4`)

### Phase 6: Grow simulation
- **Goal**: Compute the deterministic transition between two world states.
- **Files**: `crates/treepo-grow/src/{lib,diff,timeline,migration,connectivity,transform,
  budget}.rs`
- **Dependencies**: Phase 4
- **End Conditions**:
  - [ ] The same snapshot pair produces an identical `GrowTimeline` hash across three runs on
        three platforms (`AC-GROW-2`, per D6 scope)
  - [ ] Connectivity assertion holds after every migration pass — no disconnected mass (`N5`)
  - [ ] Adding one file to the T2 fixture produces a timeline whose changed elements are
        confined to the affected limb (`AC-GROW-4`)
  - [ ] Cancellation mid-simulation publishes nothing (`AC-GROW-3`)

### Phase 7: Grow playback, cinema & triggers
- **Goal**: Make the transition watchable, smooth, and correctly triggered.
- **Files**: `crates/treepo-app/src/{grow_task,playback,triggers}.rs`,
  `crates/treepo-export/src/ring.rs`, `crates/treepo-app/src/window.rs` (cinema mode)
- **Dependencies**: Phase 5, Phase 6
- **End Conditions**:
  - [ ] Grow playback holds 24 fps with no dropped frames through the most expensive
        transformation on minimum spec (`AC-GROW-5`)
  - [ ] The main thread never blocks during Grow; the previous world keeps animating
        (`AC-GROW-1`) — asserted by frame-time trace
  - [ ] First-run Grow on a T2 repository shows first visible growth within 10 s
        (`AC-ASSOC-1`)
  - [ ] Pause, scrub and cancel all function during playback (`F-GROW-4`)

### Phase 8: Thrive liveliness & dirtiness
- **Goal**: The world stays alive between Grows, and shows what is uncommitted.
- **Files**: `crates/treepo-app/src/thrive/**`, `crates/treepo-render/src/{particles,
  overlay_dirty}.rs`, `crates/treepo-app/src/debug/**`
- **Dependencies**: Phase 5, Phase 7
- **End Conditions**:
  - [ ] Steady-state Thrive performs zero repository I/O over a 10-minute session, verified by
        filesystem trace (`AC-THR-1`, `NFR-1`)
  - [ ] T2 holds 30 fps at minimum spec with ambient animation and particles active
  - [ ] Editing a working-tree file updates its overlay within 2 s without a Grow (`AC-THR-2`)
  - [ ] Creature population stays bounded over a 30-minute idle run (`AC-THR-3`)
  - [ ] The `F-THR-8` debug intensity toggle is present in dev builds and absent from release
        builds — asserted by test

### Phase 9: Export — **M2 exit**
- **Goal**: Get a shareable artifact out, carrying nothing it should not.
- **Files**: `crates/treepo-export/src/{lib,gif,png_seq,video,scrub}.rs`,
  `crates/treepo-app/src/ui/export_dialog.rs`
- **Dependencies**: Phase 7
- **End Conditions**:
  - [ ] A T1 first-run Grow exports to a GIF under 10 MB with no manual tuning (`AC-EXP-1`)
  - [ ] Exported files contain no repository path, name, or contributor identity in metadata
        under default settings, verified with an external metadata tool (`AC-EXP-2`)
  - [ ] Export never blocks Thrive — frame-time trace during export shows no stall
        (`AC-EXP-3`)
  - [ ] All five §2 jobs are demonstrably servable end to end (**M2 gate**)

### Phase 10: Settings, store browser & privacy surface
- **Goal**: Give the user control over triggers, filters, and everything treepo has stored.
- **Files**: `crates/treepo-app/src/ui/settings/**`, `crates/treepo-store/src/{browse,
  in_repo,package}.rs`
- **Dependencies**: Phase 9
- **End Conditions**:
  - [ ] The store browser lists every repository with size on disk and purges any or all
        (`F-MAN-9`, `N2`)
  - [ ] Identity reveal is reachable only from settings, never from the export dialog, and
        requires explicit confirmation (`AC-ID-3`)
  - [ ] Toggling reveal changes live view and subsequent exports together (`AC-ID-4`)
  - [ ] Opting into `.treepo/` leaves `git status` clean and the root `.gitignore` untouched
        (`AC-MAN-6`)
  - [ ] Per-repository settings survive a folder move (`AC-SET-1`)

### Phase 11: Staged replay, workers & enrichment depth
- **Goal**: Make the front door carry the weight `R1` places on it.
- **Files**: `crates/treepo-grow/src/checkpoints.rs`, `crates/treepo-app/src/thrive/workers.rs`,
  `crates/treepo-gen/src/enrichment.rs`, `crates/treepo-app/src/interact/search.rs`
- **Dependencies**: Phase 8, Phase 9
- **End Conditions**:
  - [ ] Staged replay reconstructs checkpoints from the log stream with **zero checkouts** —
        asserted by test (`F-GROW-7`)
  - [ ] Checkpoint sampling prefers tags and falls back to time; count and threshold recorded
        with the footage that set them (§11 Q4)
  - [ ] Search locates a path and moves the camera to it (`F-NAV-6`)
  - [ ] Staged replay on the T2 fixture holds the Phase 7 playback budget

### Phase 12: Widget mode, onboarding & packaging — **M3 exit**
- **Goal**: Ship it.
- **Files**: `crates/treepo-app/src/window.rs` (widget mode),
  `crates/treepo-app/src/ui/onboarding.rs`, `.github/workflows/budgets.yml`,
  `xtask/src/budget.rs`, `tests/budgets.rs`
- **Dependencies**: Phase 10, Phase 11
- **End Conditions**:
  - [ ] Widget mode holds its reduced budget at materially lower CPU/GPU cost, measured
        (`AC-WIN-1`)
  - [ ] Idle widget-mode CPU under 5% of one core on recommended hardware (`NFR-6`)
  - [ ] Every §7 budget passes on minimum spec across T0–T3 (`F-CORP-1`)
  - [ ] Signed, installable artifacts build for Windows, macOS and Linux (`NFR-7`)
  - [ ] A T4 repository warns before starting, remains cancellable, and does not crash
        (`F-CORP-1`)
  - [ ] No essential flow requires a terminal (`R1`) — verified by a clean-machine walkthrough

---

## Phase Dependency Graph

```
Phase 0 (foundation)
   └→ Phase 1 (model + extraction)
         ├→ Phase 2 (store + identity)
         └→ Phase 3 (skeleton) ────────────────── M0 exit
               └→ Phase 4 (identity, materials, enrichment)
                     ├→ Phase 6 (grow simulation)
                     └→ Phase 5 (bevy shell + baking + nav) ── M1 exit
                              [requires Phase 2 + Phase 4]

Phase 5 + Phase 6 → Phase 7 (playback, cinema, triggers)
Phase 5 + Phase 7 → Phase 8 (thrive liveliness)
Phase 7           → Phase 9 (export) ──────────── M2 exit
Phase 9           → Phase 10 (settings + store browser)
Phase 8 + Phase 9 → Phase 11 (staged replay, workers, enrichment)
Phase 10 + Phase 11 → Phase 12 (widget, onboarding, packaging) ── M3 exit
```

**Parallel-safe pairs** (no shared files, independent end conditions):
- Phase 2 ∥ Phase 3 — store work and skeleton work touch disjoint crates after Phase 1
- Phase 6 ∥ Phase 5 — Grow simulation is headless; the Bevy shell does not gate it
- Phase 10 ∥ Phase 11 — settings UI and replay/workers are disjoint

---

## Resolved Deviations

Two places where the architecture could not satisfy the PRD as literally written. Both were
escalated, decided on 2026-07-27, and **amended in the PRD** — the requirement text now matches
the architecture, so neither is an outstanding divergence.

### E1 — `AC-GROW-2` is timeline-identical, not pixel-identical *(resolved; PRD amended)*
`AC-GROW-2` originally required "a frame-identical transition on every run and every platform."
GPU rasterization of identical input is not bit-identical across vendors, drivers, or driver
versions; only a software rasterizer could satisfy the literal reading, at the cost of the GPU
and the entire `R1` quality bar. **Decision: the determinism boundary is the `GrowTimeline`** —
every frame's element positions, material states, and particle seeds — with rasterization
outside it. This preserves what `P2` protects (the same change always produces the same
performance) and is verified by hashing the serialized timeline. Implemented per D6; gated in
Phase 6.

### E2 — the manifest is binary with a JSON sidecar *(resolved; PRD amended)*
`F-MAN-2` originally named `manifest.json`. At T3 that file holds ~80k rich primitive records;
JSON puts it in the hundreds of MB and seconds to parse, against `NFR-4`'s 5 s cold launch.
**Decision: `manifest.bin`** (postcard, canonical field ordering) **plus `manifest-meta.json`**
carrying `schema_version`, `treepo_version`, and counts. `identity.json`, `config.json`, and
`settings.json` stay JSON so `N2`'s inspectability promise holds where a person would look.
Nothing in v1 diffs or hand-edits the manifest — that rationale left the PRD when `R7` moved
the store out of the repository. Implemented per D9; gated in Phase 2.

---

## Risk Register

1. **RISK-A — `gix` maturity for the `F-EXT-2` single-pass traversal.** *(new)* `gix` has no
   direct `--numstat` equivalent; per-file line counts require assembling blob diffs over the
   commit graph. This is the performance linchpin for RISK-1's mitigation. **Mitigation:**
   spike it in Phase 1 against the T2 fixture *before* building on it; keep a subprocess-`git`
   adapter behind the same trait as a contingency, accepting the `R1`/`N1` costs only if
   forced, and re-escalating if so.
2. **RISK-1 (PRD) — blame is the long pole.** **Mitigation:** architecturally enforced —
   `blame.rs` is a separate module invoked only after first Grow, and Phase 1's end conditions
   assert blame is never called during extraction.
3. **RISK-2 (PRD) — cross-platform float determinism.** **Mitigation:** D2 confines trig to
   `treepo-det` tables; Phase 0 gates on tri-platform bit-identical trig before any generative
   code exists.
4. **RISK-B — Static baking memory at T3.** *(new)* Baked layer textures plus ID buffers for an
   80k-path tree can exceed `NFR-3`'s 4 GB if held resident. **Mitigation:** chunk residency
   with LOD-appropriate resolution and streaming; Phase 5 end conditions measure T3 memory,
   and the chunk budget is tunable via `F-SET-4`.
5. **RISK-3 (PRD) — the first Grow may not carry `R1`'s weight.** **Mitigation:** Phase 7
   delivers the single-diff version early enough to be watched by real viewers at M2; Phase 11
   (staged replay) is scheduled before M3 rather than after, and is the designated cut line if
   M3 overruns.
6. **RISK-4 (PRD) — aggregation may erase recognition.** **Mitigation:** Phase 5's `AC-NAV-1`
   is a recorded user test with ≥3 participants, not a self-assessment; `F-NAV-6` search lands
   in Phase 11 as the escape hatch.
7. **RISK-C — Bevy version churn across a long build.** *(new)* Bevy's release cadence breaks
   APIs, and `treepo-app`/`treepo-render` are the only exposed crates. **Mitigation:** pin the
   Bevy minor version in the workspace; the phase-boundary design (D1) already confines Bevy to
   two of nine crates, so an upgrade is bounded work rather than a rewrite.
8. **RISK-6 (PRD) — M3 scope.** **Mitigation:** Phases 11 and 12 carry the designated cut lines
   (`F-GROW-7`, `F-WIN-3`); Phase 10 is the last phase whose omission would be user-visible as
   a missing capability rather than missing polish.

---

## Deployment Strategy

- **Platform**: Consumer desktop, storefront-first (`R1`). Windows, macOS, Linux (`N8`).
- **Method**: `cargo` release builds per platform in CI; code signing on Windows and macOS
  (notarization required); storefront depot upload as the release step. Linux ships as a
  self-contained archive plus an optional Flatpak manifest.
- **Environment variables**: none required at runtime. `TREEPO_DATA_DIR` overrides the app-data
  root for testing only; `TREEPO_DEBUG_UI=1` enables the egui debug surface in dev builds.
  No key, endpoint, or credential exists — `N2` leaves nothing to configure.
- **Pre-deploy checks**:
  - [ ] `cargo xtask determinism` green on all three platforms
  - [ ] `cargo xtask readonly-audit` green (`AC-MAN-2`)
  - [ ] `cargo xtask id-coverage` green (`P1`, `N7`)
  - [ ] `cargo deny check` green (`N2` — no network-capable dependency)
  - [ ] `cargo xtask budget` green on minimum spec across T0–T3
  - [ ] Clean-machine walkthrough: install → open repository → watch Grow → export, with no
        terminal and no git installed (`R1`, D3)

Deployment is Phase 12's final step. A failed deploy does not fail the campaign.
