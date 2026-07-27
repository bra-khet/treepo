# treepo — Product Requirements Document

**Version:** 1.2  
**Status:** Approved. Architecture produced — see `.planning/architecture-treepo.md`.  
**Last updated:** 2026-07-27 *(v1.2: Grow staging stack, user-controlled playback, refined first-run Watch/Skip — see `design/engine-architecture.md` v0.3; prior: E1 `AC-GROW-2`, E2 `F-MAN-2`/`F-MAN-6`, NFR-8 / D10)*

Companion to [`CONSTITUTION.md`](CONSTITUTION.md), which governs intent and holds the
non-negotiable constraints `N1`–`N9`, principles `P1`–`P10`, and ratified decisions `R1`–`R6`.
This document does not restate them; it cites them. Where a requirement below exists *because*
of a constraint, the constraint ID appears inline.

---

## 1. Scope & Reading Guide

This PRD covers everything from an empty project directory to a shippable v1. It defines
capabilities, user-facing behavior, acceptance criteria, sequencing, and the technical
constraints that bear on requirements.

It does **not** define architecture (module boundaries, ECS layout, crate structure) or
implementation. Those follow.

**Requirement IDs** are stable and citable: `F-<AREA>-<n>` for functional requirements,
`AC-<AREA>-<n>` for acceptance criteria. Architecture and task breakdowns should reference
these rather than restating them.

**Priorities:** `P0` = v1 cannot ship without it. `P1` = v1 is materially weaker without it.
`P2` = valuable, cut first under pressure. Anything post-v1 is listed in §8.

---

## 2. Users & Primary Jobs

One user persona in v1: **a developer looking at a repository on their own machine.** Under
`R1` they arrive as a consumer — they bought or installed a desktop product, not a dev tool.

| # | Job | Frequency | Served by |
|---|-----|-----------|-----------|
| J1 | "Show me what this codebase *looks* like." | First run, then occasionally | §5.1, §5.7 |
| J2 | "Let me wander around it and find things." | Every session | §5.9, §5.10 |
| J3 | "Show me what changed since I last looked." | After commits | §5.7 |
| J4 | "Let me keep it around while I work." | Ambient, continuous | §5.8, §5.13 |
| J5 | "Let me show someone else this thing." | Occasional, high value under `R1` | §5.11 |

J1 and J5 are the acquisition and retention drivers under consumer positioning. J2 and J4 are
what make it worth keeping installed.

**Not a persona in v1:** team leads, managers, or anyone evaluating *other people's* work.
This is a deliberate exclusion (`N4`, Constitution §5).

---

## 3. Reference Corpus & Scale Tiers

Every performance and quality target below is stated against these tiers. The corpus is a
build artifact: synthetic fixtures for T0–T1 generated deterministically in CI, plus real
public repositories pinned by commit SHA for T2–T4.

| Tier | Files | LOC | Commits | Authors | Shape |
|------|-------|-----|---------|---------|-------|
| **T0** | 0–20 | <1k | 0–20 | 1 | New or empty project |
| **T1** | ~1k | ~50k | ~2k | 1–10 | A library |
| **T2** | ~10k | ~500k | ~20k | 10–100 | An application |
| **T3** | ~80k | ~5M | ~200k | 100–2k | Kernel-scale monorepo |
| **T4** | 300k+ | 20M+ | 1M+ | 2k+ | Browser-scale; best-effort only |

**F-CORP-1** (P0) — T0–T3 are supported tiers with the budgets in §7. T4 must not crash,
must remain cancellable, and must warn the user before starting (`P5` permits an unwieldy
result; it does not permit a hang).

**F-CORP-2** (P0) — The corpus includes at least one repository per tier plus these shapes:
single-author, 1000+-author, no-history (§6), shallow clone (§6), deep nesting >15 levels,
one file >50 MB, and a repository with `.mailmap`.

**F-CORP-3** (P0) — Storage and identity fixtures, required by `R7`: a **read-only** repository
(restricted permissions or read-only mount), a repository with **no remote**, one with
**multiple remotes and no `origin`**, one with **no commits**, and **two clones of one remote at
different paths**. These exercise all three identity tiers (`F-MAN-3`) and `AC-MAN-2`,
`AC-MAN-4`, and `AC-MAN-5`.

---

## 4. Milestones & Sequencing

Development milestones. **Only M3 is shippable** — M0–M2 are internal gates.

### M0 — Skeleton Proof *(internal, no UI)*
Extraction → L-system → debug renderer drawing lines and thickness only. Per
`design/l-system-parameterization.md` §6, this exists to look at silhouettes across the
corpus and tune parameters before any visual investment.

**Exit:** T0–T3 produce distinguishable, plausible silhouettes. Determinism verified
(`AC-DET-1`, `AC-DET-2`). Parameter row `A3+B2/B3+C1+D1+E3+F2+G1` validated or revised with
evidence.

### M1 — Static Tree
Full extraction, app-data store with identity resolution, materials, ownership mosaic,
enrichment placement, LOD navigation, inspection. Rendered as a still, high-quality image you
can zoom and click. No animation, no Grow cinematics.

**Exit:** A T2 repository is legible at all zoom levels and a user can find a known directory
by eye in under 30 seconds. `AC-MAN-2` (zero writes to the working tree) holds and is enforced
in CI from this milestone onward.

### M2 — Living Tree *(feature-complete MVP)*
Grow phase with cinematic diff, **user-controlled staging** (stage on trigger, play/commit on
demand), first-run Grow with **Watch the birth / Skip to present**, Thrive liveliness,
working-tree dirtiness overlay, identity model, export.

**Exit:** The five jobs in §2 are all servable. This is the internal "is the idea good?" gate.

### M3 — Shippable Product
Multi-checkpoint staged history replay (`F-GROW-7`), workers, enrichment depth, settings
surface, widget mode, onboarding polish, storefront requirements, hardware-floor performance
work.

**Exit:** v1 release criteria in §7 met on minimum spec.

**Critical path:** extraction → skeleton → determinism harness → materials → Grow → Thrive.
The determinism harness must exist before materials (§10, RISK-2).

---

## 5. Capabilities

### 5.1 Repository Association & First Run

**F-ASSOC-1** (P0) — The user selects a repository via a native folder picker or drag-and-drop
onto the window. No path typing, no CLI argument required (`R1`).

**F-ASSOC-2** (P0) — On association, treepo validates the target and reports specifically what
it found: git repository, plain directory (no `.git`), shallow clone, bare repository
(unsupported), or inaccessible.

**F-ASSOC-3** (P0) — Before any long extraction, the user sees an estimate derived from a fast
file count, and for T3/T4 an explicit confirmation with the expected duration.

**F-ASSOC-4** (P0) — Extraction and first-run Grow computation / playback are cancellable at
any point, leaving no partial store (`F-MAN-7`) and nothing written to the working tree.

**F-ASSOC-7** (P0) — Association works on repositories the user cannot write to — read-only
mounts, restricted permissions, foreign clones. Under `F-MAN-1` this is an ordinary path, not
a degraded one.

**F-ASSOC-5** (P1) — Recently opened repositories are listed on launch and reopen from cached
manifest without re-extraction.

**F-ASSOC-6** (P0) — **First-run agency (front door under `R1`).** On first association,
background computation of the first-run Grow sequence begins immediately, and an onboarding
surface appears that explains Grow vs Thrive, staging, dirtiness, and the world-tree metaphor,
with a thematically consistent progress indicator. Two options remain available at all times:
**Watch the birth** (begin or continue cinematic playback of staged material once enough is
ready — the recommended front door) and **Skip to present** (load the final committed world
state into Thrive without watching). Skip must remain affordable even on large histories
(final-state-only path). The first Grow is never an unavoidable long wait, and never a blank
window while work proceeds (`design/engine-architecture.md` §3.5).

- **AC-ASSOC-1** — Associating a T2 repository from a cold start reaches either first visible
  growth (if the user chooses Watch) or an interactive Thrive view of the final state (if they
  choose Skip) within 10 s of a usable path existing — progressive staging/compute is
  mandatory; a blank window is a defect. Skip-to-present must not wait on full cinematic frame
  pre-render of history.
- **AC-ASSOC-2** — Cancelling mid-extraction returns to the picker with no partial store in app
  data and nothing written to the repository at any point.
- **AC-ASSOC-3** — Pointing at a non-repository directory produces a tree from filesystem
  primitives alone, with a clear notice that age, churn, and ownership are unavailable.
- **AC-ASSOC-4** — On first association, both **Watch the birth** and **Skip to present** are
  reachable without completing the full cinematic sequence; Skip commits (or loads) the final
  world state and enters Thrive.

### 5.2 Primitive Extraction

Implements the catalog in `design/feature-system.md` §3.

**F-EXT-1** (P0) — Structural and size primitives from a single filesystem walk: `depth`,
`child_count`, `descendant_*`, `max_subtree_depth`, `branching_histogram`, `depth_profile`,
`balance_score`, `hierarchy_skew`, bytes, and file-size distribution.

**F-EXT-2** (P0) — Temporal and ownership primitives derived from **one pass over
`git log --numstat`** — not from `git blame`. A single traversal emitting commit hash, author
name, author email, timestamp, and per-file added/deleted line counts yields `first_commit_age`,
`last_commit_age`, `commit_count`, `churn_rate` across all windows, `recency_heat`,
`modification_burstiness`, `author_count`, `author_distribution`, `dominant_author`,
`bus_factor_proxy`, and `contribution_recency_per_author` in `O(history)` rather than
`O(files × history)`. See §10 RISK-1.

**F-EXT-3** (P1) — `blame_segments` (within-file spatial attribution, feeding the intra-limb
mosaic of `design/feature-system.md` §8.4) is the **only** primitive requiring `git blame`. It
is extracted as a separate, resumable, deferrable pass that runs after the first Grow has
already completed using `F-EXT-2` data. Line-range aggregated, and sampled above a
configurable file-count threshold.

**F-EXT-4** (P0) — Language and LOC breakdown (total/code/comment/blank) per path, and
code : asset : config : docs : generated : binary ratios. Extraction runs only during Grow.

**F-EXT-5** (P0) — Folder-signal records per `design/feature-system.md` §3.1 — structured, not
boolean, carrying `signal_name`, `default_semantic_weight`, `content_modulation`,
`effective_weight`, and `position_in_hierarchy`.

**F-EXT-6** (P1) — Lightweight derived signals: comment density, test-to-source ratio,
TODO/FIXME density, documentation freshness, generated/large-file debt indicators.

**F-EXT-7** (P0) — **Lens scope for v1: one lens.** Structure is extracted from
`local_committed` (HEAD). The working tree is *not* a second skeleton; it appears only as a
Thrive overlay (`F-THR-4`). This resolves the drafts' ambiguity between "working tree is the
primary target" and "Grow reflects HEAD" — HEAD is stable and fully attributed, dirtiness is
transient by nature and belongs to the cheap phase (`N6`). `remote` and `index` lenses are
post-v1 (§8).

**F-EXT-8** (P0) — Filtering rules, resolving open task #6 in `design/visual-construction.md`:
1. Always skip `.git/` and `.treepo/`.
2. Honor the repository's `.gitignore` (paths ignored by git are not structure).
3. Apply a built-in default exclusion set (`node_modules`, `target`, `dist`, `build`, `vendor`,
   `.venv`, `__pycache__`, `Pods`, and similar).
4. Detect generated and vendored content via `.gitattributes` `linguist-*` markers where present.
5. All of the above are overridable per repository and persisted in the manifest.

**F-EXT-9** (P0) — Author identity normalization honors the repository's `.mailmap` when
present, then falls back to case-normalized email as the identity key. One human with three
email addresses must be one contributor (this materially affects `author_distribution` and
every downstream color assignment).

- **AC-EXT-1** — Full extraction of a T2 repository completes within the §7 budget on
  reference hardware, using `git log --numstat` and *without* invoking `git blame`.
- **AC-EXT-2** — Re-extraction after a single new commit is incremental and touches only
  affected paths.
- **AC-EXT-3** — A repository with `.mailmap` collapses aliased identities correctly; the same
  repository without it yields a higher `author_count`.
- **AC-EXT-4** — Extraction never executes repository content (`N1`): no build scripts, no
  hooks, no language plugins that evaluate source.

### 5.3 Storage, Repository Identity & Persistence

Implements `R7`. Application data is the primary and default store; in-repo storage is an
explicit opt-in.

**F-MAN-1** (P0) — All extracted primitives, caches, committed world state, recorded Grow
sequences, per-repository settings, and user or agent annotations live in application data,
keyed by repository identity (`F-MAN-3`). Opening a repository — local folder or remote
clone — writes nothing into the working tree (`N1`).

**F-MAN-2** (P0) — Platform-conventional application data root:

| Platform | Location |
|----------|----------|
| Windows | `%LOCALAPPDATA%\treepo\` |
| macOS | `~/Library/Application Support/treepo/` |
| Linux | `$XDG_DATA_HOME/treepo/`, falling back to `~/.local/share/treepo/` |

```
<app-data>/treepo/
  settings.json                 # global settings
  repositories/
    <identity-hash>/
      identity.json             # resolved identity + how it was derived, for inspection
      manifest.bin              # primitives, classifications, annotations (canonical binary)
      manifest-meta.json        # schema_version, treepo_version, counts — human-readable
      config.json               # per-repository settings (identity level, filter overrides)
      world/                    # committed world state
      cache/                    # frame buffers, blame cache, derived render state
```

**F-MAN-3** (P0) — **Repository identity resolution**, in strict order, first match wins:

1. **Normalized primary remote URL.** Prefer `origin`; otherwise the alphabetically first
   remote, deterministically. Normalization collapses protocol and formatting variance —
   `git@github.com:foo/bar.git`, `https://github.com/foo/bar.git`, and
   `https://github.com/Foo/Bar/` all resolve to one identity (lowercase host and path, strip
   scheme, credentials, trailing slash, and `.git` suffix).
2. **Root commit SHA**, for a git repository with history but no remote. This survives folder
   moves and renames, which a path hash does not. Where multiple root commits exist (merged
   histories), take the earliest by commit date, tie-broken by SHA.
3. **Normalized absolute path hash**, for a non-git directory or a git repository with no
   commits.

`identity.json` records both the resolved key and which tier produced it, so a user can see
why two checkouts did or did not share a store.

**F-MAN-4** (P0) — Two clones of the same remote share one identity and therefore one store.
This is intended: the same repository is the same tree. Where the two are at different commits,
the store records the commit it was built from and the mismatch triggers a normal Grow — no
special handling.

**F-MAN-5** (P1) — When identity resolution misses but a root-commit match exists in the store
— a repository renamed or moved on its host — treepo offers to relink the existing store rather
than silently re-extracting.

**F-MAN-6** (P0) — `manifest-meta.json` carries `schema_version` and the `treepo_version` that
wrote it. A schema mismatch triggers regeneration rather than a best-effort parse.

*Amended 2026-07-27 (architecture E2).* The manifest is a canonical binary encoding
(`manifest.bin`), not JSON. At T3 it holds ~80k rich primitive records; JSON would run to
hundreds of megabytes and seconds of parse time against `NFR-4`'s 5 s cold-launch budget, and a
canonical binary encoding makes `AC-MAN-1`'s byte-identical requirement independent of key
ordering. The diff-friendliness that originally motivated JSON left this document when `R7`
moved the store out of the repository. `identity.json`, `config.json`, `settings.json`, and
`manifest-meta.json` remain JSON so that `N2`'s inspectability promise holds where a person
would actually look.

**F-MAN-7** (P0) — Writes are atomic: temporary file, then rename. Thrive never observes a
partially written manifest, and cancellation never leaves one.

**F-MAN-8** (P0) — Fully regenerable. Deleting the app-data store costs time, never data.

**F-MAN-9** (P0) — The store is enumerable and deletable from within the application: the user
can see every repository treepo holds data for, **with its size on disk**, and purge any of
them, or all of them, without leaving the app or hand-editing directories. Manual purge is the
v1 answer to a store that has grown too large (`F-MAN-13`).

**F-MAN-10** (P1) — **In-repo storage, opt-in only.** "Install local `.treepo/` for this repo"
mirrors the store into `.treepo/` inside the working tree. On creation, treepo writes
`.treepo/.gitignore` containing `*` — a **self-ignoring directory**. It must not add an entry
to the repository's root `.gitignore`, which is a tracked file and therefore off-limits
(`N1`). The directory is co-location convenience, not a sharing mechanism: it travels when the
folder is copied and survives an app-data wipe. A user who wants it committed can un-ignore it
themselves; that is their decision, not a default.

**F-MAN-11** (P1) — **Shareable package export** is the sharing path: a single self-contained
file bundling manifest and world state for another person or machine. Subject to the identity
setting (`F-ID-5`) exactly as visual exports are — a package must not carry identities the live
view conceals.

**F-MAN-12** (P1) — The manifest is a supported target for external agent annotation
(`design/design-outline.md` §9), in app data by default. Annotation keys are namespaced and
preserved across regeneration; treepo-owned keys are not.

**F-MAN-13** (post-v1) — **Automatic eviction above a configurable total store cap.** v1 ships
with no cap: the store grows without bound and `F-MAN-9` is the manual remedy. A later release
adds a user-configurable ceiling on total store size, above which eviction runs
least-recently-used. Eviction is confined to `cache/` — regenerable derived state, frame
buffers, blame cache. It must never touch `manifest.bin`, `manifest-meta.json`, `config.json`,
or `world/`, since
evicting those would discard work rather than reclaim scratch space, and would silently lose
per-repository settings and any agent annotations (`F-MAN-12`).

The v1 requirements this depends on are already in place: `F-MAN-2` separates `cache/` from
durable data, and `F-MAN-9` surfaces per-repository size. Adding the cap later requires no
migration.

- **AC-MAN-1** — Deleting the store and re-running Grow reproduces a byte-identical
  `manifest.bin` for unchanged repository state.
- **AC-MAN-2** — Opening any repository with default settings produces **zero writes** to the
  working tree. Verifiable by filesystem tracing, and by `git status` and file mtimes being
  unchanged across association, extraction, first Grow, and a full Thrive session.
- **AC-MAN-3** — Killing the process mid-write leaves the previous store intact and valid.
- **AC-MAN-4** — Cloning the same remote to two different local paths resolves to one identity;
  the second open reuses the existing store without re-extraction.
- **AC-MAN-5** — Moving or renaming a repository folder with no remote does not orphan its
  store (identity tier 2).
- **AC-MAN-6** — After opting into `.treepo/`, `git status` remains clean — the self-ignoring
  directory is invisible to git and the root `.gitignore` is untouched.

### 5.4 Identity & Privacy Model

Implements `N9` and `R6`. Operational policy is recorded in `design/feature-system.md` §3.4.

**F-ID-1** (P0) — Self-identification reads `user.email` from git config (repository, then
global). Identities matching after `.mailmap` normalization are "you."

**F-ID-2** (P0) — Every other contributor is rendered as a stable pseudonym plus a seeded
color. Real names, emails, and handles are absent from the UI and from exports by default.

**F-ID-3** (P0) — Pseudonyms are deterministic (`N3`) from the normalized identity key:
two-word, pronounceable, drawn from a themed wordlist, with deterministic collision resolution
within a repository. The same contributor yields the same pseudonym on every machine.

**F-ID-4** (P0) — Author colors are seeded from the identity key and drawn from a palette with
enforced minimum perceptual separation so that adjacent mosaic segments remain distinguishable.
Stable across Grow cycles (`design/feature-system.md` §8.4).

**F-ID-5** (P0) — One setting governs both live view and exports (`N9`). It is not
separable — an export cannot reveal what the live view conceals.

**F-ID-6** (P0) — The reveal opt-in is per repository, stored in the store's `config.json`
(`F-MAN-2`), placed under a privacy/sharing settings section rather than in any primary flow,
and gated behind an explicit confirmation carrying the disclaimer drafted in
`design/feature-system.md` §3.4.

**F-ID-7** (P0) — When the user is not a contributor to the repository — the common case for a
cloned public repository — every contributor including the viewer is pseudonymous. This is the
default state, not an error.

**F-ID-8** (P1) — Exported artifacts carry no repository path, repository name, or contributor
identity in file metadata unless identity reveal is enabled (`N2`).

- **AC-ID-1** — With default settings, no real name, email, or handle of any contributor other
  than the user appears anywhere in the UI or in any exported file, including its metadata.
- **AC-ID-2** — The same repository produces identical pseudonyms and author colors on Windows,
  macOS, and Linux.
- **AC-ID-3** — Enabling reveal requires an explicit confirmation and is reachable only from
  settings — never from the export dialog, where the pressure to enable it is highest.
- **AC-ID-4** — Toggling reveal changes live view and subsequent exports together.

### 5.5 Structural Skeleton Generation

Implements `design/l-system-parameterization.md`.

**F-SKEL-1** (P0) — The L-system is a pure function:
`(subtree primitives, path seed, parameter table) → oriented, thickened segments`. No global
state, no time input (`N3`).

**F-SKEL-2** (P0) — Hierarchical composition: each major limb runs its own parameterized
instance seeded by its path hash. Not one flat derivation.

**F-SKEL-3** (P0) — Hybrid trunk: minimal basal axiom sized from total root mass and primary
limb count, with visual trunk mass emerging from overlapping primary limbs
(`design/visual-construction.md`).

**F-SKEL-4** (P0) — Parameter row `A3+B2/B3+C1+D1+E3+F2+G1` is the v0.1 default: recursion
hard-capped at 4–5 with deeper content aggregated, medium-to-wide angles rising with skew,
length falling off faster than thickness, noise near-zero for clean repositories and rising
with churn and skew, size-driven droop, small top-level directories grouped into fewer
thicker limbs, and age/churn excluded from the skeleton.

**F-SKEL-5** (P0) — Mapping tables are **data**, loaded from a file, not compiled constants.
Revising them must not require a rebuild (`design/l-system-parameterization.md` §6).

**F-SKEL-6** (P0) — Deterministic trigonometry. Turtle interpretation must not depend on
platform `libm`: use a fixed lookup table with deterministic interpolation, or a pinned
software implementation. See §10 RISK-2.

**F-SKEL-7** (P0) — Aggregation past the recursion cap produces a proportional container object
rather than truncating content (`P6`). The container records the full descendant set for
inspection (`F-INSP-3`).

- **AC-SKEL-1** — A clean, conventionally structured T1 repository produces an orderly,
  near-symmetric silhouette; a high-skew, mixed-language, unconventional repository of similar
  size produces a visibly wilder one, from the same parameter table.
- **AC-SKEL-2** — An empty repository (T0) produces a seed and root-boulder cluster, not a
  lonely trunk (`design/visual-construction.md`).
- **AC-SKEL-3** — Skeleton generation for T3 completes within the §7 Grow budget.
- **AC-SKEL-4** — Editing the parameter table file and reloading changes the silhouette with
  no recompilation.

### 5.6 Material, Ownership & Enrichment

Implements `design/feature-system.md` §8.4–§8.7.

**F-MAT-1** (P0) — Primary material family is driven by language, binary-vs-text, and asset
class. Binary and asset-heavy regions render as resource-like material rather than living wood.

**F-MAT-2** (P0) — Ownership drives accent, vein, and mosaic treatment over the primary
material — proportional partitioning only, never a figure or ranking (`N4`).

**F-MAT-3** (P0) — Size normalization is logarithmic with a soft clamp, a **minimum
representation floor** guaranteeing every surviving path a visible pixel budget, and a minimum
visible quota per significant contributor (`P7`).

The floor applies to paths surviving filtering **and aggregation**. An aggregated container
(`F-SKEL-7`) discharges the floor on behalf of everything it represents — its contents are
reachable through inspection (`F-INSP-3`), not through pixels of their own. This is the
confirmed reading (§11 Q1) and is what makes `P7` and T3 legibility compatible.

**F-MAT-4** (P0) — Age/recency gradient: older material sits basal/inward, recent material
distal/tip-ward (`design/feature-system.md` §8.3).

**F-MAT-5** (P1) — Semantic enrichment structures placed during Grow: docs → bookshelves or
archive platforms; assets/binaries → stockpiles and crates; tests → distinct secondary growth
or proving-ground platforms; high-churn clusters → work sites.

**F-MAT-6** (P2) — Quality/debt signals introduce subtle stress materials (cracks, sparse
density) coexisting with the primary material.

- **AC-MAT-1** — A 3-line file in a repository dominated by a 50k-line file remains visible and
  clickable at appropriate zoom, or is reachable by inspecting the container that aggregates it
  (`P7`, `F-MAT-3`).
- **AC-MAT-2** — A contributor responsible for 2% of a limb retains visible presence in its
  mosaic.
- **AC-MAT-3** — No UI surface anywhere displays a contribution percentage, count, rank, or
  ordering of people (`N4`).
- **AC-MAT-4** — Two adjacent authors in a mosaic are visually distinguishable at medium zoom
  on the minimum-spec display.

### 5.7 Grow Phase

Implements `design/engine-architecture.md` §3 (v0.3 — staging, playback surface, first-run).

**F-GROW-1** (P0) — Grow **computation** runs off the main thread and reports progress via
events. Thrive continues rendering the previous **committed** world throughout (`N6`). Staging
and playback never block the main thread.

**F-GROW-2** (P0) — Triggers, all user-tunable, conservative by default: new commits on HEAD,
merge/rebase moving HEAD, explicit "Stage Grow" / "Grow Now," and first association. Periodic
background checking defaults to off. **A met threshold stages work; it does not seize the
session with forced playback** (`design/engine-architecture.md` §3.2–§3.3).

**F-GROW-3** (P0) — Grow computes a diff between world states (committed baseline → target, or
stage-to-stage for multi-stage sequences) and produces a transition timeline — added mass grows
outward, removed mass retracts or is reclaimed, material migrates along the age/recency
gradient, ownership changes propagate as recoloring waves. It never swaps geometry silently
(`P9`).

**F-GROW-4** (P0) — **Stage playback controls.** While a stage plays cinematically, the user may
pause, scrub, and cancel. Across the stack, the user may step one stage, play a contiguous
segment continuously, jump to any stage, play all remaining, or collapse to the final staged
state without watching every intermediate frame (`design/engine-architecture.md` §3.4).

**F-GROW-5** (P0) — The new world state is committed **atomically on user promote** (Grow
commit). Thrive never observes a half-built tree. Cancelling playback or discarding stages
leaves the previous committed world intact.

**F-GROW-6** (P0) — **First-run Grow (v1 form):** a single empty→HEAD transition staged as the
first-run sequence, offered via `F-ASSOC-6` (Watch the birth / Skip to present). This is
explicitly *not* multi-checkpoint historical replay — see `F-GROW-7`. User-facing copy should
not overclaim a single diff as "the entire history played through every era."

**F-GROW-7** (P1, M3) — **Multi-checkpoint staged history replay.** First-run (and optionally
later "replay history") populates the stage stack with K checkpoint stages rather than one
transition. Path existence and approximate size at each checkpoint are reconstructed from the
`F-EXT-2` log stream — **no checkouts required**, which is what makes this affordable.
Materials interpolate toward their final values. Stack navigation (`F-GROW-4`, `F-GROW-11`)
applies unchanged. This is the highest-value post-MVP item under `R1`.

**Checkpoint sampling: tags where enough exist, falling back to time.** Tags tell the project's
story and time tells its rhythm; a tagged project usually has the better narrative. The
checkpoint count and the "enough tags" threshold are set at M3 against real footage rather than
guessed now — this is a decision that wants to be watched, not reasoned about.

**F-GROW-8** (P1) — Classification threshold crossings play explicit transformation sequences —
thickening, material family shift, new secondary branching, scarring, healing
(`design/feature-system.md` §8.9).

**F-GROW-9** (P2) — Reverse playback (time-lapse rewind), visually distinct from forward
trimming (`design/engine-architecture.md` §3.6). Open whether reverse reuses the same frames or
a distinct vocabulary.

**F-GROW-10** (P0) — **Render rate and playback rate are decoupled.** Grow frames are produced
into the same offscreen buffer the export path already uses (`F-EXP-4`) and played back at a
smooth fixed rate. Where a frame costs more than its playback interval to render, Grow buffers
ahead rather than dropping frames or stuttering. Playback may begin before rendering completes
once sufficient lead exists. Prefer eager stage recipes so user-initiated play is instant when
frames are already prepared.

This is what lets Grow spend real time on visual quality — multi-pass cellular work, dense
particle churn during a transformation — without that cost reaching the viewer as jitter. It
also means the exported artifact and the watched sequence are the same frames, so what a user
shares is what they saw.

**F-GROW-11** (P0) — **Ordered stage stack.** Each threshold-meeting structural update produces
a discrete, replayable **staged Grow change** (target or diff, pre-computed transition
recipe/timeline, metadata). Stages push onto an ordered stack of arbitrary length — the single
source of pending structural history. Stages are independently addressable and deterministic
via path-hash seeds (`N3`). State Sync (`F-THR-6`) never creates stack entries.

**F-GROW-12** (P0) — **Stage navigation surface.** A dedicated, attractive panel presents the
stack as a simple ordered tree (lines + nodes), with carved-wood / organic treatment native to
the world-tree aesthetic. It drives `F-GROW-4` and remains usable outside full cinema chrome.

**F-GROW-13** (P0) — **User-initiated Grow commit.** Promoting one or more stages applies them
to the live world via the atomic commit in `F-GROW-5`. Until commit, Thrive continues on the
previous committed snapshot (plus dirtiness / lightweight pending previews). Discarding the
stack (or cancelling mid-playback without commit) leaves committed state unchanged.

- **AC-GROW-1** — During Grow computation and playback, the main thread never blocks; the
  window remains responsive and the previous committed world keeps animating.
- **AC-GROW-5** — Grow playback is free of visible stutter at its stated rate on minimum spec,
  including through the most expensive transformation sequences (`F-GROW-8`), by buffering
  ahead where necessary (`F-GROW-10`).
- **AC-GROW-2** — The same diff produces a **timeline-identical** transition on every run and
  every platform: every frame's element positions, material states, and particle seeds are
  identical, verified by hashing the serialized `GrowTimeline` (`N3`, `P2`).

  *Amended 2026-07-27 (architecture E1).* Originally worded "frame-identical." Rasterized
  output is not bit-identical across GPU vendors, drivers, or driver versions, so the literal
  reading was achievable only with a software rasterizer — forfeiting the GPU and the `R1`
  quality bar. Determinism is therefore required of the timeline (the data), not of the pixels
  rendered from it. This preserves exactly what `P2` protects: the same change always produces
  the same performance.
- **AC-GROW-3** — Cancelling mid-playback or discarding stages without commit leaves the
  previous committed world intact; no half-applied topology is visible in Thrive.
- **AC-GROW-4** — Adding one file to a T2 repository produces a staged Grow whose visible
  change (when played) is localized to the affected limb, not a whole-tree reflow.
- **AC-GROW-6** — Meeting a Grow trigger while the user is in Thrive stages work without
  interrupting ambient Thrive; the stack gains an entry and the stage panel reflects it.
- **AC-GROW-7** — From a stack with multiple stages, the user can jump to an intermediate stage
  node and play from there, or collapse to the final staged state, without being forced through
  every prior stage in order.

### 5.8 Thrive Phase

Implements `design/engine-architecture.md` §4.

**F-THR-1** (P0) — Continuous ambient animation — sway, breathing, drift — weighted by local
churn and recency heat. Never re-analyzes the repository (`N6`).

**F-THR-2** (P0) — Glow, saturation, and particle emission modulated by recency heat, from
weights pre-baked during Grow. Per-frame cost is a weighted lookup, not a computation.

**F-THR-3** (P0) — All user interaction — camera, zoom, hover, selection, inspection — belongs
to Thrive.

**F-THR-4** (P0) — Working-tree dirtiness overlay: untracked, modified, staged, pending-delete,
conflicted. Rendered as transient markers and material over the frozen HEAD structure, visibly
provisional — the next Grow resolves them (`F-EXT-7`). **On by default at low intensity** — it
is the most useful live signal and the most visually noisy, and the default resolves that
tension toward calm.

**F-THR-5** (P1) — Workers and creatures under stable equilibria: local steering, simple state
machines, attraction to activity heat, periodic rest behavior. No global pathfinding
(`design/engine-architecture.md` §4.3).

**F-THR-6** (P1) — State Sync: a narrowly scoped, cancellable, infrequent refresh of
non-topological state (ahead/behind counts, dirtiness). Never rebuilds the skeleton, never runs
a material pass (`design/engine-architecture.md` §5.1).

**F-THR-7** (P2) — Secondary cellular detail on dirty rectangles and high-heat regions only, or
omitted entirely in favor of pre-baked weights.

**F-THR-8** (P1) — **Dirtiness intensity override, development/QA only.** A debug toggle forces
a high-emphasis rendering of the `F-THR-4` overlay so that dirtiness states can be inspected
and tuned without squinting at the shipped default. It lives on a debug surface, not in normal
settings (`F-SET-*`), and does not alter the production default. Both intensities are to be
QA'd when the overlay is implemented; changing the default requires that evidence.

- **AC-THR-1** — Thrive holds the §7 frame budget on a T2 repository at minimum spec with no
  repository access whatsoever during steady state (verifiable by filesystem tracing).
- **AC-THR-2** — Editing a file in the working tree updates its dirtiness overlay within 2 s
  without triggering a Grow.
- **AC-THR-3** — Creature population remains bounded and stable over a 30-minute idle session;
  no unbounded growth, no clustering collapse.

### 5.9 Navigation & Level of Detail

**F-NAV-1** (P0) — Continuous or stepped zoom across three bands: far (silhouette, limb masses,
global heat, roots), medium (major branches, platforms, ownership coloring), near (fine
structure, detail density, workers, abstracted file/directory objects).

**F-NAV-2** (P0) — Pan, zoom, and recenter via mouse and keyboard. A "frame everything" control
always returns to a sensible view.

**F-NAV-3** (P0) — LOD transitions do not pop distractingly; detail fades or resolves.

**F-NAV-4** (P0) — Beyond the aggregation threshold, deep subtrees render as proportional
containers — bookshelf, spiral shelf, or a single object representing a directory and all its
contents (`P6`).

**F-NAV-5** (P1) — Structured containers open a modal or inventory-style inspection rather than
subdividing geometrically further (`design/design-outline.md` §6).

**F-NAV-6** (P1) — Search by path or filename, moving the camera to the match and highlighting
it. This is the practical escape hatch when the visual metaphor alone does not locate a target.

- **AC-NAV-1** — A user familiar with a T2 repository can locate a known top-level directory by
  eye within 30 s.
- **AC-NAV-2** — Zooming from far to near on T3 holds the §7 frame budget throughout.
- **AC-NAV-3** — No zoom level produces an unreadable tangle at T3; aggregation engages before
  legibility fails.

### 5.10 Inspection & Identification

**F-INSP-1** (P0) — Clicking any element surfaces its identity: full path, kind, size, age,
last activity, dominant material reason, and contributor presence as pseudonyms and colors
(`N4` — presence, never proportion as a figure).

**F-INSP-2** (P0) — Hover produces lightweight feedback — outline, heightened glow, path label
— without a click.

**F-INSP-3** (P0) — Aggregated containers report what they represent and allow drilling into
their contents (`F-SKEL-7`).

**F-INSP-4** (P1) — "Reveal in file manager" and "copy path" for any element. treepo does not
open an editor (Constitution §5).

**F-INSP-5** (P2) — A why-panel explaining which primitives produced the element's appearance.
This is the strongest available answer to "is this real?" and directly serves `P1`.

- **AC-INSP-1** — Every visible element resolves to a real path or an explicit aggregate
  (`P1`).
- **AC-INSP-2** — No inspection surface displays a contribution percentage or ranking (`N4`).

### 5.11 Export

**F-EXP-1** (P0) — Export a completed Grow sequence as animated GIF and as a PNG frame
sequence.

**F-EXP-2** (P1) — WebM or MP4 export where encoder integration is low-friction.

**F-EXP-3** (P0) — Export a still image of the current view at selectable resolution.

**F-EXP-4** (P0) — Frames are captured during Grow at a controlled rate into an offscreen
buffer and encoded on completion or on request (`design/engine-architecture.md` §3.5).

**F-EXP-5** (P1) — Re-export from the most recent recorded Grow without re-running it.

**F-EXP-6** (P0) — All exports obey the identity setting (`F-ID-5`) and strip identifying
metadata (`F-ID-8`). This covers shareable packages (`F-MAN-11`) as well as images and
animations.

**F-EXP-7** (P1) — Length and quality presets, since a full T3 Grow at native resolution is not
a shareable GIF.

- **AC-EXP-1** — A T1 first-run Grow exports to a GIF under 10 MB at a shareable resolution
  without manual tuning.
- **AC-EXP-2** — An exported file inspected with a metadata tool reveals no repository path,
  repository name, or contributor identity under default settings.
- **AC-EXP-3** — Export never blocks Thrive.

### 5.12 Settings

**F-SET-1** (P0) — Grow triggers, their frequency, and (where exposed) what counts as a
stage-worthy change. Defaults remain conservative so Grow stays special.
**F-SET-2** (P0) — Privacy and sharing: the `F-ID-6` reveal opt-in, the stored-data browser and
purge controls (`F-MAN-9`), and the `F-MAN-10` in-repo opt-in.
**F-SET-3** (P0) — Per-repository filtering overrides (`F-EXT-8`).
**F-SET-4** (P1) — Performance: particle density, CA intensity, LOD aggressiveness, frame cap
(`design/design-outline.md` §8 anticipates user-exposed stacking).
**F-SET-5** (P1) — Display: window mode, theme, zoom sensitivity.
**F-SET-6** (P2) — Advanced: parameter table path override for users who want to tune
silhouettes (`F-SKEL-5` makes this nearly free).

- **AC-SET-1** — Global settings persist across sessions; per-repository settings live in the
  store keyed by repository identity and survive the repository folder being moved.

### 5.13 Window & Presentation Modes

**F-WIN-1** (P0) — Standard resizable desktop window.
**F-WIN-2** (P1) — Cinema mode for Grow playback: chrome hidden, overlay controls only.
**F-WIN-3** (P2, M3) — Always-on-top widget mode — small, low-opacity, minimal-chrome ambient
companion serving J4 (`design/design-outline.md` §10).

- **AC-WIN-1** — Widget mode holds a reduced frame budget at materially lower CPU/GPU cost than
  the full window.

---

## 6. Degenerate & Edge Cases

Each is a supported path with defined behavior, not an error state.

| Case | Required behavior |
|------|-------------------|
| Empty repository | Seed and root-boulder cluster. Never a lonely trunk. (`AC-SKEL-2`) |
| No `.git` | Filesystem primitives only. Tree generates. Explicit notice that age, churn, and ownership are unavailable. |
| **Shallow clone** | Detect `--depth` truncation, warn explicitly that the tree will read as ageless, and offer to proceed or unshallow. Silently producing a history-less tree is a defect — this is common and would otherwise look like a bug. |
| Single file | Minimal but valid structure. |
| Single author | Mosaic degenerates to one material family. No empty ownership UI. |
| 1000+ authors | Palette assignment stays distinguishable; minimum quota (`F-MAT-3`) does not fragment limbs into noise. |
| One enormous file | Soft clamp prevents it consuming the parent's entire budget (`P7`). |
| Deep nesting >15 | Aggregation engages; no stack overflow, no microscopic geometry. |
| Submodules | Rendered as sealed objects. Not recursed into in v1. |
| Symlinks | Not followed. Cycles impossible by construction. |
| Non-UTF8 paths | Rendered with lossy display names; the raw path is preserved for `F-INSP-4`. |
| Case-colliding paths | Handled deterministically on case-insensitive filesystems; no duplicate or vanished nodes. |
| Detached HEAD | Supported; treated as the current commit. |
| Bare repository | Rejected at association with a clear message (`F-ASSOC-2`). |
| Repository modified mid-Grow | Grow completes against its snapshot; the change is picked up by the next Grow. |
| Store present but corrupt | Regenerate rather than fail (`F-MAN-6`). |
| **Read-only repository** | Fully supported — read-only mount, restricted permissions, or a foreign clone. Nothing is written to the working tree under default settings, so this is an ordinary path (`F-ASSOC-7`, `F-MAN-1`). |
| No remote configured | Identity falls back to root commit SHA (`F-MAN-3` tier 2). |
| Multiple remotes, no `origin` | Alphabetically first remote, deterministically (`F-MAN-3` tier 1). |
| Remote URL changed upstream | Identity misses; treepo offers to relink an existing store matched by root commit (`F-MAN-5`). |
| Git repository with no commits | No root commit available; identity falls back to path hash (`F-MAN-3` tier 3). |
| Repository folder moved or renamed | Store is retained — identity is remote URL or root commit, not path (`AC-MAN-5`). |
| Two clones of one remote | Share a single identity and store by design (`F-MAN-4`, `AC-MAN-4`). |
| Repository on a slow network share | App-data store is local, so cache reads stay fast; only extraction pays the network cost. |
| Fork of an upstream repository | Distinct remote URL yields a distinct identity, despite the shared root commit (tier 1 precedes tier 2). |
| App-data store deleted externally | Full regeneration on next open; no data loss (`F-MAN-8`). |

---

## 7. Non-Functional Requirements

**Reference hardware.** Minimum: integrated graphics (Intel Iris Xe, Apple M1, or Ryzen 5000
iGPU), 4-core CPU, 8 GB RAM, 1080p. Recommended: discrete GPU 2020 or later, 16 GB RAM.

### Frame budget

**30 fps is the floor. 60 fps is not a hard requirement anywhere** and is not a release gate
(§11 Q5). Thrive is designed to sit comfortably above the floor rather than to hit a fixed
target; where headroom exists it is spent on visual quality, not on chasing a number.

| Phase / tier | Floor | Design intent |
|--------------|-------|---------------|
| Thrive, T0–T2 | 30 fps @ 1080p, minimum spec | Comfortably above the floor; smoothness is what matters, not the figure |
| Thrive, T3 | 30 fps, LOD reduces detail to hold it | Aggressive enough LOD that the floor is rarely approached |
| Thrive, T4 | Best effort; must remain interactive | — |
| Grow playback | 24 fps | Smooth motion takes precedence over frame rate (`F-GROW-10`) |

**NFR-1** (P0) — Steady-state Thrive performs zero repository I/O (`AC-THR-1`).
**NFR-2** (P0) — Thrive frame time is independent of repository size once LOD has culled;
budget scales with visible elements, not total paths (`P10`).
**NFR-10** (P0) — Grow may spend more wall-clock time per frame than real-time playback would
allow. It is a cinematic, not a simulation the user is steering, so render rate and playback
rate are decoupled (`F-GROW-10`). A visibly stuttering Grow is a defect; a Grow that took
longer to render than to watch is not.

### Grow budget — first run, full extraction, reference hardware

| Tier | Target | Hard ceiling |
|------|--------|--------------|
| T1 | 10 s | 30 s |
| T2 | 60 s | 3 min |
| T3 | 10 min | 30 min |
| T4 | Unbounded; cancellable, progress-reporting, warned in advance |  |

### Grow budget — incremental, after one commit

| Tier | Target |
|------|--------|
| T1–T2 | 5 s |
| T3 | 30 s |

**NFR-3** (P0) — Memory: world state plus manifest under 1 GB for T2, under 4 GB for T3.
**NFR-4** (P0) — Cold launch to interactive on a cached repository: under 5 s (T1–T2).
**NFR-5** (P0) — Determinism verified in CI across all three platforms (`AC-DET-1`–`3`).
**NFR-6** (P1) — Idle Thrive CPU low enough for J4 (ambient background use) — target under 5%
of one core on recommended hardware in widget mode.

### Determinism acceptance criteria

- **AC-DET-1** — Two Grow runs on identical repository state produce byte-identical serialized
  skeletons, materials, and enrichment placements.
- **AC-DET-2** — The same repository at the same commit produces identical output hashes on
  Windows, macOS, and Linux (`F-SKEL-6` is what makes this achievable).
- **AC-DET-3** — No wall-clock, locale, filesystem-ordering, or hardware-dependent value enters
  the generative pipeline. Directory iteration is explicitly sorted (`N3`).

### Platform

**NFR-7** (P0) — Windows, macOS, Linux (`N8`).
**NFR-8** (P0) — Fully functional offline; no network dependency in any **product** path (`N2`).
  Localhost-only developer tooling that is never enabled in release builds (Bevy Remote Protocol
  under Cargo feature `brp`; architecture D10) does not count as a product network path and must
  not appear in default-feature or storefront dependency graphs.
**NFR-9** (P1) — Storefront requirements — packaging, launch options, controller-optional input,
storefront asset dimensions — are M3 work, not release-week work (`R1`).

---

## 8. Feature-Level Non-Goals for v1

Deferred, with the Constitution's blessing (§5, "Deferred, not excluded"). Listed so no one
designs around them prematurely.

| Deferred | Notes |
|----------|-------|
| `remote` and `index` lenses; twin views | v1 is single-lens (`F-EXT-7`). |
| VCS branch topology | No side-shoots or parallel growth for feature branches. |
| Multi-repository views | One repository at a time (Constitution §5). |
| Live event reactions (PR trucks, issue posters) | Architecturally anticipated via `F-THR-6`; not built. |
| Agent-activity reactions and speech bubbles | Post-v1. |
| Additional scene types | Later era (Constitution §5). |
| 3-D turtle extensions | `design/l-system-parameterization.md` §8 keeps the door open. |
| Full falling-sand physics | Permanently excluded (`N5`). |
| Cloud sync, accounts, sharing service | Excluded by `N2`, `N8`. |
| Steam achievements / cloud saves | If ever added, never a channel for repository data (`N2`). |
| Automatic store eviction / size cap | `F-MAN-13`. v1 grows without bound; `F-MAN-9` (browse and purge, with sizes shown) is the manual remedy. |

---

## 9. Dependencies & Critical Path

**External:** git (invoked as a subprocess or via a library — never repository hooks, `N1`), a
LOC/language counter, and a GIF/video encoder for §5.11.

**Ordering constraints that matter:**

1. `F-EXT-2` (log-based extraction) gates everything temporal and ownership-related. Build it
   before any material work.
2. The determinism harness (`AC-DET-1`–`3`) must exist **before** materials and enrichment.
   Retrofitting determinism onto a rendering pipeline is substantially harder than building to
   it, and `R3` raises the stakes by making manifests shareable between machines.
3. `F-SKEL-6` (deterministic trig) gates `AC-DET-2`. Decide it at M0, not at M3.
4. `F-EXT-3` (blame) must not gate the first Grow. If it does, T3 first-run budgets are
   unreachable.
5. `F-ID-3`/`F-ID-4` (pseudonyms and colors) gate all ownership visuals — the mosaic cannot be
   built against real names and retrofitted later without violating `N9` in the interim.
6. `F-MAN-3` (identity resolution) gates the store layout and therefore every persistence path.
   It is small but foundational — settle it before anything writes to app data, because
   re-keying an existing store later means orphaning every user's cached work.

---

## 10. Risks

**RISK-1 — `git blame` is the long pole.** *High likelihood, high impact.* Full per-line blame
across a T3 repository can take hours. The drafts treat blame as a primary ownership source; it
is not viable as one. **Mitigation:** `F-EXT-2` derives all path-level ownership from a single
`git log --numstat` pass in `O(history)`; blame is demoted to `F-EXT-3`, needed only for
within-file spatial mosaic, and runs deferred, resumable, and sampled. This mitigation must
hold, or T2/T3 first-run budgets are unreachable.

**RISK-2 — Cross-platform floating-point determinism.** *Medium likelihood, medium impact.*
`sin`/`cos` are not guaranteed bit-identical across platform math libraries, and a turtle
interpreter is built on them. `AC-DET-2` would fail in ways that are difficult to diagnose
late. **Mitigation:** `F-SKEL-6`, decided at M0.

*Reduced by `R7`.* Under the previous in-repo model, manifests could be committed and opened on
a different OS, making any mismatch routinely user-visible as a tree that changed shape for no
reason. With app-data-primary storage, cross-machine transfer happens only through an explicit
shareable package (`F-MAN-11`), so the blast radius is much smaller. The requirement is
unchanged — `N3` is absolute and packages still cross machines — but this is no longer a defect
users would hit by accident.

**RISK-3 — The first Grow may not carry the weight `R1` places on it.** *Medium likelihood,
high impact.* A single empty→HEAD transition (`F-GROW-6`) is a growth animation, not a story.
Consumer positioning makes this the acquisition moment. **Mitigation (updated 2026-07-27):**
user-controlled staging + Watch/Skip (`F-ASSOC-6`, `F-GROW-11`–`13`) convert interruptive load
and forced cinema into agency; treat `F-GROW-7` (multi-checkpoint stack) as the highest-value
M3 item; validate single-diff Watch-the-birth against real viewers at M2 before relying on it
as the shipped front door.

**RISK-4 — Aggregation may erase the recognition the product exists for.** *Medium likelihood,
medium impact.* `P6` caps depth; if the cap is too aggressive at T2, the user sees pleasant
shapes that map to nothing (`AC-NAV-1` fails). **Mitigation:** tune against the corpus at M1;
`F-NAV-6` (search) is the escape hatch; `F-INSP-5` (why-panel) rebuilds trust when the visual
alone is ambiguous.

**RISK-5 — Minimum representation floor versus legibility at T3.** *Closed 2026-07-27.* `P7`
guarantees every path pixels, which at 80k files competed directly with readability. Resolved
by the confirmed reading in `F-MAT-3`: the floor applies to paths surviving filtering **and
aggregation**, and a container discharges it for its contents. Retained here for the record.

**RISK-6 — Scope at M3.** *High likelihood, medium impact.* Consumer polish, storefront
requirements, workers, enrichment depth, and staged replay all land in one milestone.
**Mitigation:** `F-GROW-7` and `F-WIN-3` are the designated cut lines.

---

## 11. Open Questions

Tactical only. No constitutional questions remain (Constitution §10). Question numbers are
stable; resolved items stay in place rather than being renumbered away.

### Resolved

**Q1 — Does an aggregated container satisfy `P7`'s representation floor on behalf of its
contents?** *Decided 2026-07-27: yes. Confirmed reading.* A container discharges the floor for
everything it aggregates; the contents remain reachable through inspection (`F-INSP-3`) rather
than through guaranteed pixels of their own. Without this reading, T3 legibility targets and
the floor are in direct conflict. Recorded in `F-MAT-3`; RISK-5 is closed.

**Q2 — Does the app-data store need a size cap or eviction policy?** *Decided 2026-07-27: no
cap in v1; configurable cap with eviction in a later release.* `R7` consolidates what was
previously distributed across repositories into one growing local store, and `cache/` holds
frame buffers, which are large. v1 accepts unbounded growth and relies on `F-MAN-9` — browse
per-repository sizes and purge manually. `F-MAN-13` specifies the eventual automatic policy:
user-configurable total ceiling, least-recently-used eviction, confined to `cache/` and never
touching durable data. No migration is required to add it later.

**Q3 — Should the working-tree dirtiness overlay be on or off by default?** *Decided
2026-07-27: on, at low intensity, with a debug override.* Production default is low intensity.
A development/QA toggle (`F-THR-8`) forces a high-emphasis rendering for inspection and tuning.
Both intensities are to be QA'd when the feature is built; the default does not change without
that evidence.

**Q4 — How many staged checkpoints for `F-GROW-7`, and sampled by what?** *Decided 2026-07-27:
tags where enough exist, falling back to time.* Tags tell the project's story, time tells its
rhythm, and a tagged project usually has the better narrative. Checkpoint count and the
"enough tags" threshold are deliberately left to M3, to be set against real footage rather than
guessed now. Recorded in `F-GROW-7`.

**Q5 — Is 30 fps an acceptable floor, or must 60 hold everywhere?** *Decided 2026-07-27: 30 is
the floor; 60 is not a hard requirement anywhere.* Thrive is designed to sit comfortably above
the floor rather than to hit a fixed 60. Grow is explicitly permitted to run at 24–30 fps, and
may spend more wall-clock time per frame than real-time playback allows, buffering ahead so
that motion stays smooth (`F-GROW-10`). §7 is restated accordingly. This removes 60 fps as a
release gate throughout.

**Q6 — Are Grow triggers interruptive auto-play, or user-controlled staging?** *Decided
2026-07-27: user-controlled staging.* Thresholds enqueue deterministic staged units onto an
ordered stack; the user plays, steps, jumps, or commits. First-run always offers Watch the
birth and Skip to present. Dual-phase contracts unchanged — staging defers the atomic world
commit. Recorded in `F-GROW-2`, `F-GROW-11`–`13`, `F-ASSOC-6`, and
`design/engine-architecture.md` §3.3–§3.5.

### Open

None outstanding. Every question raised in drafting has been resolved.

Items deliberately left to the milestone that needs them — checkpoint count and threshold for
`F-GROW-7`, dirtiness intensity values, the eventual store ceiling in `F-MAN-13`, stage-stack
persistence and transition-asset budgets — are tuning / implementation decisions, not open
requirements. They are recorded where they will be made.

---

## Human Gate

**Tactical decisions made in drafting.**

Extraction was restructured around a single `git log --numstat` pass rather than `git blame`
(`F-EXT-2`, RISK-1). This is the most consequential decision here: it changes ownership
extraction from `O(files × history)` to `O(history)` and is what makes the T2 and T3 budgets in
§7 reachable at all. Blame survives only for within-file spatial mosaic, deferred and sampled.

Lens scope was resolved to one (`F-EXT-7`): structure from HEAD, working tree as a Thrive
overlay. The drafts pointed in two directions — "the local working tree is the primary target"
versus "Grow reflects HEAD" — and this reading fits the phase split without a second skeleton.

The first Grow was reframed (`F-GROW-6`): a single empty→HEAD diff is a growth animation, not a
historical replay, and the drafts' language overstates it. Multi-checkpoint staged history
replay (`F-GROW-7`) is specified as the richer form and made affordable by reconstructing
checkpoints from the log stream rather than checking commits out.

*Amended 2026-07-27 (v1.2).* Grow is **user-controlled**: triggers **stage** rather than
auto-play; an ordered stage stack (`F-GROW-11`), navigation surface (`F-GROW-12`), and Grow
commit (`F-GROW-13`) sit on top of unchanged dual-phase ownership. First association always
offers **Watch the birth** and **Skip to present** (`F-ASSOC-6`, `AC-ASSOC-4`). Product
direction lives in `design/engine-architecture.md` v0.3.

Filtering rules (`F-EXT-8`), store layout (`F-MAN-2`), and identity normalization via `.mailmap`
(`F-EXT-9`) were specified from scratch — all three were open tasks in the drafts. Scale tiers,
performance budgets, and the reference corpus (§3, §7) were invented outright; the drafts
deliberately locked no numbers, and requirements without them are not testable.

Storage was reversed to app-data-primary per `R7`, superseding `R3`. §5.3 was rewritten around
three-tier repository identity resolution (`F-MAN-3`) — normalized remote URL, then root commit
SHA, then path hash — which is new machinery the drafts never needed. Root-commit identity is
what lets a store survive a folder being moved, and it is why `AC-MAN-5` is achievable at all.
In-repo `.treepo/` survives as an opt-in that writes a self-ignoring `.treepo/.gitignore` rather
than touching the repository's root `.gitignore`, which is tracked and therefore off-limits
under the restored `N1`.

**Escalated, and since resolved.** Tactical questions raised in drafting were decided by the
owner on 2026-07-27 (Q1–Q5) and Q6 the same day as a product-direction refinement; each lands
in a requirement rather than staying as prose:

| | Decision | Lands in |
|---|---|---|
| Q1 | Aggregated containers discharge `P7`'s representation floor for their contents | `F-MAT-3`, `AC-MAT-1`; closes RISK-5 |
| Q2 | No store cap in v1; configurable LRU eviction later, `cache/` only | `F-MAN-9`, `F-MAN-13`, §8 |
| Q3 | Dirtiness overlay on at low intensity, with a debug override | `F-THR-4`, `F-THR-8` |
| Q4 | Staged checkpoints sampled by tags, falling back to time | `F-GROW-7` |
| Q5 | 30 fps floor; 60 fps is not a hard requirement anywhere | §7, `NFR-10`, `F-GROW-10`, `AC-GROW-5` |
| Q6 | Grow stages on trigger; user plays/commits; first-run Watch/Skip | `F-GROW-2`, `F-GROW-11`–`13`, `F-ASSOC-6`, engine-architecture §3 |

Q5 had the widest reach: it rewrote the §7 frame budget, removed 60 fps as a release gate
throughout, and produced `F-GROW-10` — decoupling Grow's render rate from its playback rate, so
that permission to spend more time per frame does not arrive at the viewer as stutter. Q1
closed RISK-5 outright.

**Remaining tuning decisions** — checkpoint count and the "enough tags" threshold, dirtiness
intensity values, the eventual store ceiling — are deliberately deferred to the milestone that
can measure them. They are recorded where they will be made, not left open here.

**Flagged as risk rather than resolved.** RISK-2 (cross-platform trig determinism) still needs
a decision at M0, well before it would otherwise surface. `R7` materially reduced its impact —
cross-machine manifest transfer is now explicit and rare rather than a side effect of
committing — but `N3` is absolute and shareable packages still cross machines, so the
requirement stands unchanged.

**Recommended next step.** No open questions remain; the PRD is ready for architecture. M0 is
deliberately small and answers the two things that constrain everything downstream: whether the
parameter row produces distinguishable silhouettes across the corpus, and whether determinism
holds across platforms. Both are cheap now and expensive later, and both are better settled
before architecture hardens around an answer.
