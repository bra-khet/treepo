# treepo — progress

> Read `.planning/campaign-treepo.md` for the phase list and `.planning/architecture-treepo.md`
> for the file tree and decisions. This file records only where the build actually is.

**Last updated:** 2026-07-27 · **Phase 0 complete and closed** · **Phase 1 in progress**

---

## Where things stand

Phase 0 — workspace and determinism foundation — is built and green.

| Gate | Command | Status |
|---|---|---|
| Build | `cargo build --workspace` | green on Linux, macOS, Windows |
| Tests | `cargo test -p treepo-det` | green — 38 unit tests + 1 doctest |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | green |
| Format | `cargo fmt --all -- --check` | green |
| `N6` | `cargo xtask dep-guard` | green |
| `N2` | `cargo deny check` | green — advisories, bans, licences, sources |
| `AC-DET-1` | `cargo xtask determinism` | green — 5 probes × 3 runs |
| `AC-DET-2` | determinism.yml compare job | **green — confirmed 2026-07-27** |

**Phase 0 is fully closed.** Every end condition in the campaign is met and verified.

### AC-DET-2, confirmed

Linux, macOS and Windows each ran the probes three times and produced **byte-identical
reports** — all five digests, on all three platforms, matching the values below. `RISK-2`
(cross-platform float determinism) is closed for the primitive layer; the table trig of
`F-SKEL-6` does what it was built to do.

This is the result Phase 3 depends on, and it is now evidence rather than expectation.

### Reference digests (Windows, release and debug alike)

```
trig       f175f604e7b1712fdac4e6fb3d113be101fb9dcca98587dc7ae34026c29faa79
fixed      ced4738c9f80e41393ca259714405b0834eddac9b7c215c3d8f75009397e902b
angle      ae2791792aeb6d1b437f33161b209c94a086b2b1922fe9c17022f8c304fd0cd7
rng        469790547d9a5d4d283b8a6cc6ebd4175fcc1231bfd4342417ea5c73bab89009
seed-tree  04c0a28a0928e93cc504a1e6a2afeddbe3f4ac874e508c90b6c789e887c6edd9
overall    39681da8a69904d23e4d7d5e38915fee55008deac7ecb259b9d56f145765fc92
```

`trig` is also pinned as a golden-digest unit test inside `treepo-det::trig`, so an
accidental edit to the sine table fails `cargo test`, not just CI.

**Any change to these numbers changes every tree treepo will ever generate.**

### Trig precision — locked 2026-07-27

The 1025-entry table with linear interpolation (absolute error under `3 × 10⁻⁷`) was reviewed
against the alternative of a wider table or a small-angle correction term, and **kept as
built**. The residual is below the L-system noise floor (`F-SKEL-4`) and far below the scale
at which `P6` aggregation moves a limb; the extra precision would buy nothing a viewer could
see, and Phase 3 will tune the parameter row against these exact values.

Frozen, along with the golden digests above. Reopening it is a recorded decision plus a
harness run on either side — not a code edit.

---

## What exists

```
Cargo.toml  rust-toolchain.toml  clippy.toml  deny.toml  .cargo/config.toml
.gitattributes                       # LF pinning — see note below
.github/workflows/{ci,determinism}.yml
crates/treepo-det/src/{lib,fixed,trig,rng,hash,ordered}.rs
xtask/src/{main,determinism,dep_guard}.rs
```

Two deviations from the architecture's file tree, both deliberate:

1. **`.gitattributes` is new.** From Phase 3 the pipeline reads `assets/params/*.ron`. A file
   checked out CRLF on Windows and LF elsewhere is a different file, so any hash of its bytes
   differs by platform — an `AC-DET-2` failure with no visible cause. Build and asset files
   are pinned to LF; `*.md` is excluded so the ratified docs do not renormalize.
2. **This file.** Session handoff, not a v1 deliverable.

## Decisions taken during the build

Recorded because they are not in the architecture and someone will otherwise re-open them.

- **`treepo-det` is `no_std`.** Not portability — it makes `std::time` and
  `std::collections::HashMap` *unreachable* from the crate every generated value flows
  through. The lint bans are the second line of defence, not the first.
- **`Fx` arithmetic saturates.** Rust's `+` panics on overflow in debug and wraps in release.
  For a determinism type that is a build-profile-dependent result, which is the exact bug
  class the crate exists to eliminate. Division is the exception and panics on a zero divisor,
  since that is a caller bug rather than an extreme input; `checked_div` is there for callers
  who expect one.
- **`Angle` is a binary angle** — full turn = 2³². Rotation wraps exactly, never drifts, and
  needs no range reduction. Quadrant reflection then makes the sine symmetries
  (`sin(-a) == -sin(a)`, `cos(-a) == cos(a)`) exact rather than approximate.
- **The RNG has no `split()`, on purpose.** Child seeds come from `Seed::derive(label)`, so a
  path's seed is a function of its path and not of traversal order. With a stream-split, adding
  one file near the top of a repository would re-seed everything after it and reshape the whole
  tree — `AC-GROW-4` would be unachievable.
- **SHA-256 rather than a fast non-cryptographic hash.** Manifest keys and store directory
  names must mean the same thing for the life of the product; `DefaultHasher` explicitly does
  not. Cost is irrelevant on the rare side of `P10`.
- **Constants are generated, not transcribed.** The sine table came from exact
  arbitrary-precision evaluation; the SHA-256 tables from exact integer roots. Each is then
  checked against an independent published vector — RFC 8439 for the ChaCha core at 20 rounds,
  the standard `""`/`"abc"` digests for SHA-256, and the platform `libm` for the trig table
  within its stated interpolation bound. No constant in the crate rests on being typed
  correctly.

---

## RISK-A spike — **PASS, with caveats** (2026-07-27)

`tools/spike-numstat/` — temporary, delete once `treepo-vcs::log_pass` is written from it.

**Question.** Can `gix` assemble per-file added/deleted line counts over a real commit graph
fast enough for `F-EXT-2`? If not, `RISK-1`'s mitigation collapses and architecture D3 (`gix`
over subprocess `git`) has to be reopened.

**Corpus.** `bevyengine/bevy` — 11,870 commits, 2,956 files at HEAD, 1,646 authors. Roughly
60% of T2 on commits, ~30% on files, so the numbers below are extrapolated on commit count
rather than measured at T2. Windows, 16 cores; Windows is typically the slowest platform for
git object I/O, so this should be a pessimistic reading.

### Measured

| | bevy (11,870 commits) | extrapolated to T2 (~20k) | extrapolated to T3 (~200k) |
|---|---|---|---|
| gix, 1 thread | 38.65 s | ~65 s | ~10.9 min |
| gix, 4 threads *(min spec)* | 21.82 s | **~37 s** | **~6.1 min** |
| gix, 8 threads | 19.07 s | ~32 s | ~5.4 min |
| `git log --numstat` | 9.5–10.8 s | ~17 s | ~2.9 min |

Budgets: T2 full extraction target 60 s / ceiling 3 min; T3 target 10 min / ceiling 30 min.

**Verdict: proceed with `gix`.** On the 4-core minimum spec the log pass takes ~37 s of T2's
60 s budget, leaving ~23 s for the filesystem walk and language/LOC work — tight but viable,
and far inside the 3-minute ceiling. T3 lands at ~6 min against a 10-minute target. The
subprocess-`git` fallback is **not** needed, so `R1` and `N1` are not being traded away.

`AC-EXT-2` makes this a first-run cost only; incremental re-extraction after one commit
touches one commit.

### The three findings that matter

1. **Effectively all the cost is blob diffing.** `--no-counts` walks the whole graph in
   **0.20 s** of a 38.65 s run. gix's revision traversal is not the problem and never was;
   the cost is decompressing blobs and diffing them. Rename tracking was suspected and
   cleared — forcing `track_rewrites(None)` bought only 2.5%.
2. **Parallelism works but scales poorly.** 1.77× at 4 threads, 2.03× at 8, and it
   *regresses* at 16 — a shared bottleneck, most likely the object database. This is the
   main optimization lever if the budget tightens. Untested: `max-performance` (zlib-ng),
   which would likely help decompression but adds a C dependency against `NFR-7`.
   **Line counts were byte-identical at 1, 2, 4, 8 and 16 threads**, so parallelising this
   costs nothing under `N3` — summing line counts is associative.
3. **gix disagrees with `git` on ~7% of commits, by ±1–3 lines.** Totals differ by 0.033%
   (2,191,935 vs 2,191,210 insertions). Every difference is *symmetric* — insertions and
   deletions shift together — which is the signature of hunk-boundary placement, not
   miscounting; `git` applies `--indent-heuristic` by default. Immaterial for churn
   primitives, which want magnitude, and irrelevant to `N3`, which requires that *we* are
   reproducible, not that we match another tool.

### Two things worth carrying forward

- **`N1`:** gix's blob-diff path hard-disables external diff commands
  (`unreachable!("we disabled that")`), so a repository cannot get code executed through a
  configured external differ. That is one of the concrete hazards D3 cited against
  subprocess `git`, and it is closed by construction.
- **`N2`:** gix adds 175 packages and `cargo deny check` reports **bans ok** — no
  network-capable crate. The one thing it did catch was a licence: `uluru` (MPL-2.0), an LRU
  cache inside `gix-pack`. MPL-2.0 was deliberately left off the allow-list so it would
  require a decision; it is now allowed, with the reasoning recorded in `deny.toml`. **Worth
  a lawyer's glance before the storefront release** — that note is an engineering reading,
  not legal advice.

### Phase 1 should re-measure

The extrapolation is honest but it is still an extrapolation. Once `tools/corpus/` builds a
real T2 fixture (~20k commits, ~10k files), re-run this before relying on the 37 s figure —
bevy is light on file count, which is exactly the axis the tree diff scales on.

---

## Phase 1 — model & repository extraction

Spike gate cleared; extraction is written on `gix` directly. `tools/spike-numstat` has been
deleted — its stated condition was "delete once `treepo-vcs::log_pass` is written from it",
and it now is.

| Deliverable | Status |
|---|---|
| `crates/treepo-model/**` | **done** |
| `crates/treepo-vcs/{discover,filter,walk}.rs` | **done** — `F-ASSOC-2`, `F-EXT-8`, `F-EXT-1` |
| `crates/treepo-vcs/{log_pass,mailmap}.rs` | **done** — `F-EXT-2`, `F-EXT-9` |
| `assets/filters/default-exclusions.ron` | **done** |
| `tools/corpus/**` | **done** — 16 shapes, T0/T1 and the §6 rows |
| `tests/degenerate.rs` | **done** — 14 rows covered |
| `crates/treepo-vcs/lang.rs` (`F-EXT-4`) | not started |
| `assets/params/folder-signals.ron` (`F-EXT-5`) | not started |
| `crates/treepo-vcs/status.rs` (`F-THR-4`) | not started |
| `xtask readonly-audit` (`AC-MAN-2`) | not started |
| T2/T3 pinned repositories, `AC-EXT-1` budget | not started |

Three fields are deliberately `Option` and still `None`, all waiting on `F-EXT-4`'s content
classification: `BalanceScore::kind`, `TemporalPrimitives::stability`, and every
`DerivedSignals` field. They are unmeasured rather than defaulted so nothing downstream
mistakes "not looked at" for "zero".

### What the fixtures found

Three real defects, none of which any amount of local testing would have surfaced:

1. **Shallow clones killed extraction.** A shallow boundary commit records a parent whose
   object was never fetched; `diff_chunk` died reading it, where PRD §6 requires a tree plus
   a warning. Git grafts those commits to parentless; `log_pass` now resolves the boundary
   set once and matches. Fixed at the source rather than by catching the error in the
   worker, so a missing object anywhere else stays loud — that would be real corruption.
2. **CI checked out shallow.** `actions/checkout` defaults to `fetch-depth: 1`, which broke
   a shallowness assertion on Windows and would have left every history test asserting
   almost nothing while passing. CI now sets `fetch-depth: 0`, and the history tests refuse
   to run against a shallow checkout rather than passing vacuously.
3. **"Not Windows" and "arbitrary filename bytes" are different questions.** macOS rejects
   non-UTF-8 filenames at the syscall with `EILSEQ` — APFS and HFS+ require valid UTF-8
   where Linux permits any byte but `/` and NUL. Only a three-platform matrix separates the
   two. The `case-collision` shape hit the mirror image: `git add --all` stages the
   *removal* of an injected index entry on a case-sensitive filesystem, and Windows folds
   the names so it never showed there.

### `treepo-model` — decisions worth finding again

Three of these are deviations or sharpenings, not restatements of the architecture.

- **`N4` is enforced by the type system.** `AuthorShare` implements neither `Ord` nor
  `PartialOrd`, so `sort_by_key`, `max_by_key`, `BinaryHeap`, and `>` on a contribution share
  do not compile. Two `compile_fail` doctests fail CI if someone opens that door; both were
  verified by temporarily deriving `Ord` and watching them fail, so they are testing the
  constraint rather than a typo. No accessor returns a percentage, and `Debug` renders a
  bucket (`major`, `minor`) rather than a figure — a manifest dump is not a scoreboard
  either. The one place contributors are ordered by volume is inside
  `OwnershipPrimitives::from_line_counts`, which exists to answer "who is dominant" and "how
  many make 80%"; neither answer leaves as an ordering.
- **Ages are not stored — only absolute timestamps.** `design/feature-system.md` names
  `first_commit_age`, and an age is a duration from *now*. Storing one would make the
  manifest a function of the clock and lose `AC-MAN-1` (regenerate to identical bytes) and
  `AC-DET-1` (same repository, same tree, any time) together. `Manifest::reference_time` is
  the newest commit timestamp in the repository — a property *of the repository* — and ages
  derive against it. **The visible consequence:** a repository nobody has committed to does
  not age; its tree is identical to the one it produced a year ago. That is correct, and it
  will look like a bug to someone.
- **Seeds derive, they are not stored.** The architecture's `PathRecord` field list has
  `seed: u64`; `PathRecord::seed(&root)` derives it instead. A stored seed is a cached copy
  of a value computed from the path, and a cached copy can disagree with its input. Deriving
  also protects `AC-GROW-4`: because the seed is a function of the path alone, adding a file
  cannot reseed its siblings, which is what keeps a Grow confined to one limb.
- **`RepoPath` is bytes, `/`-separated, ordered byte-wise.** Each is a determinism property
  rather than a portability one — `std::path::Path` would give Windows different path bytes,
  different path hashes, and therefore a different tree. Non-UTF8 paths keep their bytes and
  gain a lossy display name (PRD §6, `F-INSP-4`). Case-colliding paths stay distinct but
  expose `case_fold_key` so the walk can detect the collision without either path vanishing.
- **Language names are interned, ids assigned on first sight.** Sorted-index ids would
  renumber every existing record when a new language appears, rewriting a whole manifest on
  the incremental re-extraction `AC-EXT-2` requires touch only affected paths. The cost is
  that ids inherit their determinism from the sorted walk (`AC-DET-3`) rather than owning it.
- **`no_std`, like `treepo-det`.** Beyond making `HashMap` and `Instant` unreachable, it
  means this crate *cannot* accept a `std::path::Path` — which pushes the platform-specific
  path mess into `treepo-vcs`, where the differences are already in view.

Skeleton and material types (`segment.rs`, `material.rs`, `snapshot.rs`, `enrichment.rs`,
`aggregate.rs` in the architecture's tree) are deliberately absent. They arrive with the
phases that produce them; defining them now would be guessing at Phase 3.

---

## Next

**`lang.rs` (`F-EXT-4`)** — language and LOC breakdown, and the content categories that close
the three `Option` fields above. It is the last `P0` extraction requirement without an
implementation.

Then the two Phase 1 end conditions still open:

- **`cargo xtask readonly-audit`** (`AC-MAN-2`, `AC-EXT-4`) — assert zero writes to any
  fixture working tree. The corpus gives it something to audit; the HEAD-tree walk should
  make it trivially green, which is the point of having built it that way.
- **T2/T3 pinned repositories and `AC-EXT-1`** — the spike's ~37 s T2 figure is still an
  extrapolation from bevy, which is light on file count, the axis the tree diff scales on.
  PRD §3 specifies real public repositories pinned by commit SHA rather than synthetic ones,
  so this needs a pinning-and-caching mechanism, not another generator.
