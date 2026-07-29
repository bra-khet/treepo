# treepo — progress

> Read `.planning/campaign-treepo.md` for the phase list and `.planning/architecture-treepo.md`
> for the file tree and decisions. This file records only where the build actually is.

**Last updated:** 2026-07-29 · **Phases 0–3 closed (M0 EXIT); Phase 4 open — `treepo-id` landed**

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
| `tests/degenerate.rs` | **done** — 19 rows covered |
| `crates/treepo-vcs/lang.rs` (`F-EXT-4`) | **done** — plus `F-EXT-8` rule 4 and `F-EXT-6` |
| `assets/languages/languages.ron` | **done** |
| `crates/treepo-vcs/signals.rs` + `assets/params/folder-signals.ron` (`F-EXT-5`) | **done** |
| `xtask readonly-audit` (`AC-MAN-2`, `AC-EXT-4`) | **done** — 16 fixtures, 0 writes, wired into CI on all three platforms |
| `crates/treepo-vcs/status.rs` (`F-THR-4`) | **done** — all five states, overlay-only by construction |
| `tools/corpus/pins.ron` + `xtask budget` (`AC-EXT-1`) | **done** — T1–T3 pinned and measured, no §7 ceiling exceeded |

**Phase 1 is complete.** Every deliverable and every end condition is met and verified.

The three deliberately-`None` fields are now filled: `BalanceScore::kind`,
`TemporalPrimitives::stability`, and `DerivedSignals`. The `Option` stays in every case —
`None` still means "not measured", and the pass leaves it there wherever there is no honest
denominator (a directory of assets has no line count to divide churn by, and a repository
with no `Code` files has nothing for its docs to be stale against).

### `lang.rs` — the three decisions

1. **Comment counting is a state machine, never a parser.** `AC-EXT-4` forbids evaluating
   repository content, so a language plugin that loads project config is the exact code path
   `N1` closes. Two rules keep the approximation from failing badly rather than slightly:
   only a line whose *first* non-blank content is a comment marker counts as a comment (so a
   URL in a string is code), and a block comment is only tracked across lines when it opens
   at the start of one. Tracking mid-line `/*` would let a single `"/*"` in a string literal
   make the rest of a file read as one enormous comment. The chosen failure undercounts a
   rare construct; the rejected one is invisible.
2. **Extension matching folds ASCII case; `filter.rs` deliberately does not.** Not an
   inconsistency. Honouring `core.ignorecase` in the *filter* would make the same repository
   produce a different tree depending on the platform it was cloned on — the exact
   `AC-DET-2` failure. Folding case *here* applies to path bytes treepo already holds, so
   `.PNG` and `.png` classify identically everywhere. One introduces platform variance, the
   other removes it.
3. **`.gitattributes` is read from the tree, not from `gix`'s attribute stack.** The stack
   needs the index and can reach into the working directory, and `walk` advertises that the
   HEAD-tree path touches neither. Two attributes do not justify giving that up. The cost is
   macros, `info/attributes`, and the user's global attributes file — none of which anyone
   uses for `linguist-*`. What it buys is that extraction of a committed tree depends on
   nothing outside that tree, which is also what makes it reproducible elsewhere.

`linguist-generated` and `linguist-vendored` both map to `ContentCategory::Generated`. There
is no `Vendored` variant because `design/feature-system.md` §8.5 gives both the same
"machined" material — a variant no renderer would read differently is a variant that only
creates the chance of reading it wrong. This also stays in `lang.rs` rather than `filter.rs`:
a marker changes what a path *is*, not whether it is structure. Filtered-out vendored code
would be missing from the tree; classified as generated it is present and rendered as what
the repository said it was.

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

### `signals.rs` — the two decisions, and the bug the dictionary had

**Evidence rules, not a tuned formula.** Each dictionary entry declares conditions on the
content ratios and a signed per-mille adjustment; the sum moves the conventional weight.
Independent rules were chosen over one formula because each is a sentence a reader can check
against the folder in front of them — "a `docs` with almost no documents in it loses 0.4" is
auditable in a way that a weighting function's coefficients are not. Weights are per-mille
integers, not floats, for `N3`: `950` also reads better than `950000` in a file that exists
to be hand-tuned.

**Nesting is carried, not applied.** `HierarchyPosition::ancestor_signals` records that a
`docs` sits inside a `vendor`, and that does *not* reach `effective_weight`. Damping a nested
signal by its ancestor's weight is tempting and no design document asks for it: `F-MAT-5` is
where nesting is meant to be interpreted, and a compounding rule invented here would bake one
guess into the manifest where a later phase could make a better one from the same data.

**The first draft of the dictionary had a self-confirming rule.** The `tests` entry was
modulated by `TestLike` share — but test-likeness is decided partly by *directory name*, so
everything under a folder named `tests` is test-like *because* it is under a folder named
`tests`. The rule meant to catch "a `tests` folder with no tests in it" could never fire. It
now uses code share, which is independent evidence, and `no_signal_tests_a_ratio_its_own_name_determines`
refuses any future `TestLike` rule on an entry whose names overlap the catalogue's test
directories. Verified the way the `compile_fail` gates were: the bad rule was reinstated, the
test failed with the right message, and it was reverted.

### `readonly-audit` — the observer problem, and what it found

`cargo xtask readonly-audit` censuses every corpus fixture, runs every Phase 1 pass over it,
censuses it again, and compares. **15 fixtures, 14 extracted, 0 writes.** It is a step in the
existing three-platform test job rather than a job of its own — what it audits is filesystem
behaviour, so Windows is the interesting runner, and the symlink and non-UTF-8 shapes only
exist on unix and Linux.

**The observer must not share code with the observed.** The census uses `std::fs` and
`treepo_det::Sha256` and nothing else — no `gix`, no reuse of `treepo-vcs`'s walk. An auditor
built from the thing it audits cannot see a defect the two have in common, which is the same
argument `tools/corpus` already makes for building fixtures with `git` and reading them with
`gix`. For the same reason the audit calls each extraction pass by name rather than through a
pipeline helper: a helper is somewhere a pass could quietly stop being called while the audit
stayed green.

**`git status` is kept as a second oracle**, because `AC-MAN-2` names it and because it can
see an index whose stat cache no longer matches the tree — dirty without any byte changing.
Two hazards, both real: it must run with `GIT_OPTIONAL_LOCKS=0` or the oracle writes the index
refresh it was brought in to detect, and it must be skipped where the fixture has no `.git`,
because git searches *upward* and the corpus lives under `target/` inside treepo's own working
tree. Un-skipped, `no-git` and `bare` would have been answered about treepo. An oracle pointed
at the wrong repository agrees with itself perfectly and means nothing. The count it covered
(13 of 15) is printed, so an oracle that quietly stopped being asked is visible.

**`N3` bans `std::time::SystemTime` and this is the one place in the workspace outside what
the ban protects.** The exception is a single type alias with the reasoning attached, not a
per-use allow. What stays banned is the part that matters: `SystemTime::now` is a disallowed
*method*, and the audit never reaches it — so it still cannot depend on when it runs.

**The detector is tested on every run.** After a clean report the command mutates a throwaway
directory four ways — added, removed, content changed *at the same length*, and modification
time moved with the bytes untouched — and confirms all four are caught with the right cause
named. The last two are the pair that matters: a write that restores the length and content
still moves the mtime, and a write that restores the mtime still changes the content, so
between them no shape of write is invisible. It is also a `cargo test` case, so breaking the
detector fails the suite rather than waiting for someone to run the audit.

The whole audit was then verified the way the `compile_fail` gates and the signals dictionary
were: a write was injected into the extraction path, and the run failed naming four separate
findings — a created file and a moved directory mtime under `[repository]`, a resized file
under `[working tree]`, and the `git status` disagreement — before being reverted.

**One real bug found, in `corpus::ensure`.** Its cache-hit path returned every shape while
`build_all` skips shapes the platform cannot build, so a second call handed back `symlinks` on
Windows and `non-utf8` anywhere but Linux, pointing at directories that had never been created.
`degenerate.rs` never saw it because those tests are platform-gated and ask for fixtures by
name. The audit walks the list, so it hit it immediately.

### `status.rs` — how `F-EXT-7` is enforced, and what the audit caught

`F-EXT-7` says the working tree is not a second skeleton. That is a compile-time fact rather
than a rule someone follows:

- **`Dirtiness` is defined in `treepo-vcs`, and `treepo-model` does not depend on it.** No
  manifest field can hold a dirtiness value because `treepo-model` cannot name the type. A
  paired `compile_fail` doctest asserts `PathRecord` has no such field, verified by pointing
  it at a field that *does* exist and watching it fail with "compiled successfully, but it's
  marked `compile_fail`".
- **No signature in the module takes a `&mut PathRecord`.** `signals::apply` and
  `log_pass::apply` do, because they are extraction. `status()` takes no structure at all and
  `overlay()` borrows it immutably.

**The overlay attaches, it does not add.** Most dirty paths are absent from the skeleton — an
untracked file is by definition not in HEAD. A path with no record of its own attaches to the
nearest present ancestor as `beneath` rather than `here`, so a folder can say "something new
is under me" without the file growing a limb. That is "the next Grow resolves them" expressed
in data.

**Five flags, not one enum.** Git's states genuinely overlap; staging a change and then
editing the file again is both. `dominant()` collapses them for a renderer that wants one
marker, and is documented as a default that Phase 8 may override — choosing a marker is a
rendering decision, and the flags are the measurement.

**`gix`'s status offers to write the index and is never allowed to.** It computes a stat-cache
refresh as a side effect, exposed as `Outcome::write_changes`. Not calling it is one absent
line of code — exactly the kind of thing someone "fixes" after reading gix's own note that
writing it back makes subsequent reads faster. `readonly-audit` now runs `status` over every
fixture; calling `write_changes` once, deliberately, made it fail with `.git/index` 615 → 647
bytes. The restraint is load-bearing and now has a witness.

**`NeedsUpdate` is not a modification.** gix reports it for unchanged files whose cached stat
is stale, which is most of a repository right after a checkout. Reading it as a change would
light up the whole tree on first run — the single most plausible way to make this feature look
broken while every unit test still passed. `an_untouched_file_is_not_dirty` reads the fixture
twice and requires the two to agree, because the first read is the one with the cold cache.

**Attachment climbs the path rather than scanning back.** The obvious implementation takes the
insertion point for a dirty path and scans backwards for a prefix. It is correct and
quadratic: ancestors sort before all of their descendants, so an untracked file under a
directory with fifty thousand entries scans past every one. Binary-searching each parent in
turn is `O(depth · log n)`, and depth is what PRD §6 treats as bounded.

**One measurement worth keeping.** A status read costs ~15 ms; eleven concurrent ones cost
twenty seconds, because gix spawns worker threads and polls a channel and a test harness
oversubscribes the machine. Not a product property — `F-THR-6` reads one repository
infrequently — but it was being paid on three platforms per push, so the tests that can share
a read do. `AC-THR-2`'s real budget belongs with the T2 repositories, not a synthetic fixture.

**New fixture: `dirty-worktree`**, in all five states at once including a live merge conflict.
Born dirty rather than dirtied by a test — a test that wrote to a fixture in order to read it
would be indistinguishable from the defect `readonly-audit` exists to catch. `Builder::try_git`
is new, and exists only so the merge that is *supposed* to fail can.

**Dependency change:** gix gains the `status` feature, which adds `gix-dir` and `gix-status`
(129 → 131 packages). `cargo deny` and `dep-guard` both still clean; no network crate.

### `AC-EXT-1` — measured, 2026-07-27

`cargo xtask budget`, Windows, 16 logical cores, `log_pass` held to 4 threads (§7 minimum
spec). Release build. Pins are in `tools/corpus/src/pins.ron`; `--fetch` is the only thing that
touches the network and nothing calls it by default.

| Pin | Tier | Files | Commits | Total | vs §7 |
|---|---|---|---|---|---|
| `ripgrep` 14.1.1 | T1 | 212 | 2,045 | **0.69 s** | target 10 s ✓ |
| `bevy` v0.17.1 | T2 | 2,714 | 9,858 | **6.56 s** | target 60 s ✓ |
| `godot` 4.3-stable | T2 | 11,030 | 66,099 | **132.61 s** | over 60 s target, inside 180 s ceiling |
| `rust` 1.83.0 | T3 | 49,029 | 268,290 | **252.25 s** | target 600 s ✓ |

**No §7 ceiling is exceeded, and `AC-EXT-1` is met.** godot is the only over-target reading and
it is over-target because it carries 3.3× T2's commit count; at T2's nominal 20,000 commits its
own rate gives 40.1 s, inside the target. Both T2 pins normalize to inside 60 s.

Three findings worth keeping:

**`log_pass` is 95–99% of every measurement.** On the T3 pin: walk 0.57 s, scan 1.03 s, signals
0.12 s, `log_pass` 272 s. All the content work `F-EXT-4` and `F-EXT-5` added this phase costs
about a second on a fifty-thousand-file tree. `AC-EXT-1` is a question about the history pass
and nothing else, which is what the RISK-A spike said and is now measured rather than assumed.

**Cost is not proportional to commit count, and the spike's extrapolation axis was wrong.**
Seconds per thousand commits: ripgrep 0.34, bevy 0.67, godot 2.01, **rust 0.94** — rust has 4.4×
godot's files and *half* its per-commit cost. Tree size is not the driver either; gix skips
identical subtrees, so the real cost is the number of *changed paths per commit* summed over
history. godot's history is fewer, larger commits. This is why extrapolating the spike's bevy
figure on commit count alone was always going to be a guess, and it cuts both ways: a
repository of many small commits is cheaper than the spike predicted, one with sweeping commits
dearer.

**T3 came in better than the extrapolation.** The spike projected ~6.1 min at four threads;
measured is 4.2 min on a real 268k-commit tree.

*Re-measured 2026-07-29 alongside `AC-SKEL-3`: 256.79 s against 252.25 s, 1.8% apart. The
table above stands.*

Two honest caveats. This is a fast desktop with `log_pass` pinned to minimum-spec *parallelism*
— single-thread performance is better than a minimum-spec machine's, so the figures are a
floor, not a certification. And `budget` is not wired into CI: it needs ~4 GB of clones and
several minutes. The architecture puts `.github/workflows/budgets.yml` in Phase 12 and that is
where it belongs; what does run in CI is the cheap half — the pins parse, are full 40-character
lower-case SHAs, are unique, and every tier with a §7 row has one.

### Pinning — the two decisions

**The SHA is the pin; the tag is for humans.** `pins.ron` carries both. A tag can be moved and a
branch certainly can, so the fetch resolves the commit directly and `verify` re-reads HEAD
afterwards; a repository whose history was rewritten fails loudly rather than being measured as
though it were the pinned one. The tag is there so a reader can tell that `4649aa97` means
ripgrep 14.1.1, and so the SHA can be re-derived if this file is ever doubted.

**Tier bands split at the geometric mean, not at a multiple.** The first attempt used a fixed
factor of three either side of each PRD §3 nominal figure, and the "bands must tile" test caught
a hole: T0 reached 60 files, T1 began at 333, and a 200-file repository belonged to no tier. The
tiers are a decade apart, so no fixed multiple can tile them. Splitting at `sqrt(a·b)` tiles
exactly and has no arbitrary constant. The bands now classify every pin correctly and each one
reports what it does *not* cover: bevy is light on files for T2, godot heavy on commits. A pin
that covers half a tier should say so on every run rather than let its number be read as more
general than it is.

The clone is deliberately complete — no `--depth`, no `--filter=blob:none`. A shallow clone
would measure a truncated history and a blobless one would fetch objects *during* the timed
pass, putting network latency inside a figure about local I/O.

---

## Phase 2 — store & repository identity

| Deliverable | Status |
|---|---|
| `crates/treepo-store/{paths,resolve}.rs` (`F-MAN-2`, `F-MAN-3`) | **done** — three tiers, 29 tests |
| `tests/identity.rs` (`F-MAN-3`, `AC-MAN-4`/`5` identity half) | **done** — 10 tests |
| `crates/treepo-store/manifest_io.rs` (`F-MAN-6`/`7`, `AC-MAN-1`/`3`) | **done** — 13 tests |
| `crates/treepo-store/identity_io.rs` (`F-MAN-2`, `F-MAN-3`, `N2`) | **done** — 9 tests |
| `tests/persistence.rs` (`AC-MAN-1`, `AC-MAN-4`, `F-MAN-8`) | **done** — 7 tests |
| `F-CORP-3`: `read-only` fixture | **missing — see below** |

**Phase 2 is complete.** All six end conditions are met and verified:

| End condition | Where |
|---|---|
| Three identity tiers against the `F-CORP-3` fixtures (`F-MAN-3`) | `tests/identity.rs` |
| Two clones → one store, second open skips extraction (`AC-MAN-4`) | `identity.rs` + `persistence.rs` |
| Moving a no-remote fixture does not orphan its store (`AC-MAN-5`) | `identity.rs` + `persistence.rs` |
| Killed mid-write leaves the previous manifest valid (`AC-MAN-3`) | `manifest_io` |
| Delete-then-regenerate is byte-identical (`AC-MAN-1`) | `persistence.rs`, 5 fixtures |
| `manifest-meta.json` readable; schema mismatch regenerates (`F-MAN-6`) | `manifest_io` |

The remaining `treepo-store` files in the architecture's tree — `world_io.rs`, `cache.rs`,
`browse.rs`, `in_repo.rs`, `package.rs` — belong to Phase 5 and Phase 10 and are absent rather
than stubbed.

`treepo-store` takes a much smaller `gix` than `treepo-vcs` does: 114 packages against 131, no
`blob-diff`, no `status`, no `attributes`, no `mailmap`. It reads two things from a repository —
remotes and the root commit — and naming that minimum is what keeps a later "while we're here"
read from appearing in the crate that decides where a user's data lives.

### Identity resolution — the decisions

**Ordering is the whole design, and it is tested by inversion.** A fork shares its upstream's
root commit and must not share its store, which is only true because tier 1 beats tier 2. The
test suite was verified the way the `compile_fail` gates and the signals dictionary were:
tier 2 was moved above tier 1 and four tests failed with legible messages before it was
reverted.

**Raw remote URLs do not leave `resolve.rs`.** `https://x-access-token:ghp_…@github.com/o/r` is
what a CI checkout leaves in `.git/config`, and users paste tokens by hand too. `F-MAN-3`
already asks for credentials to be stripped; the reason it matters is not the one the PRD
gives. `Resolution` carries only normalized URLs, so no code path runs from `.git/config` to
`identity.json`, a store directory name, or a shared package (`F-MAN-11`).

**The port is dropped, and that follows from dropping the scheme.** `F-MAN-3` names scheme,
credentials, trailing slash and `.git`, and says nothing about ports. Keeping one would undo
the rest — `ssh://git@host:22/foo` and `https://host/foo` are one repository reached two ways,
and the point of stripping the scheme is that the route is not the identity.

**Lowercasing the path has a stated cost.** Two repositories on a case-sensitive host differing
only in case share a store. That is the PRD's rule and the right trade — `.../Foo/Bar/` and
`.../foo/bar` are the same repository far more often than they are two, and GitHub, GitLab and
Bitbucket all agree — but the failure, if it ever happens, will look inexplicable, so it is
written down.

**A shallow clone is refused tier 2 rather than approximated.** Its oldest commit is a boundary,
not a root; keying on it would give a repository one identity today and another the moment
somebody unshallows it, silently orphaning the store. It falls to tier 3 with the reason
recorded. In practice a shallow clone has an `origin` and never gets that far.

**Tier 2 walks every reference, not HEAD.** Identity is a property of the repository, not of
what is checked out — walking from HEAD would give one identity on `main` and another on an
orphan branch, and switching branches would orphan the store.

**Tier 1 does not compute a root commit, deliberately.** It costs a full graph walk, and paying
it on every open of every repository with a remote would put a T3 repository's *identity
resolution alone* into seconds against `NFR-4`'s 5 s cold launch. `F-MAN-5`'s relink index
wants root commits for every repository; the place to collect them is `log_pass`, which walks
the graph once anyway. Recorded here because the tempting fix is the expensive one.

**Tier 3 canonicalizes rather than lowercases.** `canonicalize` absolutizes, resolves symlinks
so a directory reached two ways is one identity, and on case-insensitive filesystems returns
the on-disk casing — case folding applied by the filesystem that knows whether it applies.
Doing it by hand would fold case on Linux too, where `~/src/Repo` and `~/src/repo` are two
directories. Invalid UTF-8 in a path is percent-escaped rather than converted lossily: every
invalid sequence maps to the same replacement character, so a lossy conversion would give two
sibling directories one store.

**Two bugs the tests found, neither by reading.** `file:///srv/git/app` lost its leading slash
and collided with a relative remote spelled `srv/git/app`; and `file:///C:/src/app` did not
match `C:\src\app`, which is the same directory and the exact form `tools/corpus` clones
through on Windows.

### `readonly-audit` now covers resolution

Resolution is the first thing an open does, and `AC-MAN-2` is about opening a repository — so
it runs inside the census, in the product's own order (association, identity, extraction). It
also reads a repository in two ways nothing else does, config and a full graph walk, and a pass
that reads in a new way is the kind that acquires a write. **16 fixtures, 0 writes**, and the
report now names each fixture's tier, which doubles as a readable check that the resolver
behaves sensibly on every shape in the corpus.

### `manifest_io` — the stored schema is not the model

`treepo-model` does **not** derive serde. `manifest_io::stored` mirrors it as plain data —
integers, strings, vectors — and serde derives on that. Three reasons, all of which were the
reason rather than a rationalization afterwards:

- **`treepo-det` keeps its zero-dependency claim.** `Manifest` holds a `Seed`, an `OrderedMap`
  and an `OrderedSet`, all defined there. Deriving serde on `Manifest` means deriving it on
  those, and `dep-guard` asserts `treepo-det` has exactly **one** package in its graph. It
  still does; `treepo-store` went 114 → 119.
- **`schema_version` now describes a type that exists to be the schema.** With serde on the
  model, adding a field silently changes the on-disk encoding and nothing prompts a version
  bump. Here, changing what is stored means editing a type whose only purpose is to be stored.
- **`N4` survives.** `AuthorShare` implements neither `Ord` nor `Serialize`; the mirror stores
  parts-per-million and the model stays exactly as strict as it was.

The cost is a conversion each way, and it is paid down by **destructuring**: every conversion
binds its source's fields by name, so a new field in `treepo-model` stops the file compiling
rather than quietly not being persisted. Four types cannot be destructured because their fields
are private — `OwnershipPrimitives`, `BranchingHistogram`, `DepthProfile`, and the two tables —
and each now has a round-trip test in `treepo-model` next to it. `OwnershipPrimitives::from_stored`
is the one place the "derived values cannot disagree with their inputs" discipline is suspended,
and it says so.

**`F-MAN-6`: the version is in the file header, not only in the sidecar.** "Regenerate rather
than best-effort parse" has to be decidable *before* the body reaches a decoder, so
`manifest.bin` starts with an 11-byte magic and a fixed-width `u32`. `manifest-meta.json`
carries the version too, because `N2` promises a user can see what treepo holds — but nothing
reads it. A file a person can edit must not be able to talk the loader into misparsing a
manifest.

**`F-MAN-7`: staged, then committed.** `stage` writes and `sync_all`s both files under
temporary names; `Staged::commit` renames them. Dropping a `Staged` deletes the temporaries,
which is "cancellation never leaves one" as a destructor rather than a discipline. The sidecar
is renamed **first** and the manifest **last**, so the manifest's rename is the single instant
the store changes. `sync_all` is load-bearing: without it the rename can be durable while the
contents are not, and a power cut leaves `manifest.bin` present and empty — the state `F-MAN-7`
exists to prevent, reached by a shorter route than a partial write.

**`AC-MAN-3` is tested with `mem::forget`.** A killed process runs no destructor, so forgetting
a `Staged` is an exact model of one: the temporary is left behind and the previous manifest is
still what `read` returns. Dropping normally is the cancellation case and is a separate test.

**The golden digest is the encoding gate.** postcard writes fields in declaration order with no
names, so reordering one is a format change that compiles and looks harmless.
`the_encoding_has_a_golden_digest` hashes a fully-populated manifest's body; any change to the
schema fails it, which forces the `SCHEMA_VERSION` bump `F-MAN-6` requires rather than leaving
it to whoever made the change to remember.

Verified by sabotage, as usual: `bus_factor` was made to serialize as zero. Three tests failed
— the golden digest, the populated round-trip, and the mid-write test — and
`encoding_is_a_function_of_the_manifest_alone` correctly **passed**, because a dropped field is
still deterministic. `AC-MAN-1` and round-trip fidelity are different properties and need
different tests; that is now demonstrated rather than assumed.

**JSON is written by hand.** The sidecar is nine scalars treepo never reads back, so a
serializer would be a dependency bought entirely for output twenty lines can produce. When a
later phase needs to *read* `config.json` or `settings.json` (`F-SET-*`), that is the moment to
add one.

### The root seed is keyed on the identity

`treepo-model` deliberately left this open — "what it is derived *from* is `treepo-store`'s
decision in Phase 2". It is `resolve::root_seed`, and it hangs the seed tree off the identity
key. Two clones of one remote therefore grow the *same tree*, which is `F-MAN-4`'s "the same
repository is the same tree" made visible rather than merely stored; a fork shares its
upstream's root commit but not its identity, so it looks different. The visible consequence:
adding an `origin` to a local-only repository moves it from tier 2 to tier 1, so its tree
changes shape. That is the same event that moves it to a new store directory, so the two agree
— but someone will one day do it and want to know why.

### `identity.json` — a file written to be read by a person

`F-MAN-3` says it "records both the resolved key and which tier produced it, so a user can see
why two checkouts did or did not share a store". That is a *user-facing* specification, so the
file is pretty-printed, the tier is a word (`remote-url`) rather than a number, the timestamp
is written twice — seconds and an ISO date — and it opens with a plain sentence:

> "Identified by its remote URL, so every clone of example.invalid/backup shares this store —
> including one at a different path, and one on a different branch."

**Nothing in it is trusted.** `N2` invites a user to look, and looking leads to editing. So
`read` recomputes the key from the tier and source value beside it and refuses a file whose
recorded key disagrees, and refuses one sitting in a directory it does not name — the shape a
hand-copied app-data folder takes. Trusting either would serve one repository's tree under
another repository's name, and refusing costs nothing: the identity is a function of the
repository and is regenerated by opening it.

**Tier and skip names are written out in both directions rather than derived.** A `Serialize`
on `IdentityTier` would put a Rust identifier in a user-facing file and make renaming a variant
a format change. Written out, renaming a variant is a refactor and renaming a stored value is a
decision.

**`identity_io::now()` is the one clock read in the product's own crates.** `N3` bans
`SystemTime::now` workspace-wide; this is the sanctioned exception, confined to one function
with one `expect`, because `F-MAN-9`'s browser needs a last-opened time and something has to
read a clock. It is not hashed into the identity key. `readonly-audit` and `budget` hold their
own narrow exceptions for the same reason and in the same shape.

**The date is arithmetic, not a dependency.** `civil_from_days`, integer-only, tested against
seven known instants including three leap days, the 32-bit rollover, and a second before
midnight. A date crate for one cosmetic field would be the larger cost.

**`serde_json` arrives here, and only here.** The previous sprint deferred it on the grounds
that a serializer for output nothing reads back buys nothing; `identity.json` is the first file
treepo must *read*, and hand-rolling a parser for a file users are invited to open and may
hand-edit is the worse of two choices. `manifest-meta.json` moved onto it at the same time —
two JSON writers in one crate would be worse than one.

### Gaps

**Manifest assembly — closed.** `treepo_vcs::extract` runs the existing passes (walk → scan →
folder signals → history → history applications) and returns a complete `Manifest`. Root seed
and product version stay caller-supplied so identity remains `treepo-store`'s concern and no
new I/O or dependency lands in `treepo-vcs`. `tests/persistence.rs` calls that public API
instead of composing field copies locally. Individual passes stay public for `xtask budget`
and `readonly-audit`, which still name each pass so a helper cannot silently drop one.

**`F-CORP-3`'s read-only fixture still does not exist.** Restricted permissions or a read-only
mount. It is about `AC-MAN-2` and `F-ASSOC-7` rather than identity, and it carries real platform
weight: `chmod` on unix, ACLs on Windows, and a builder that can clean up after itself. The
`two-clones` case is now covered — `tests/persistence.rs` builds it — so this is the last one.

## Phase 3 — skeleton generation (in progress)

| Deliverable | Status |
|---|---|
| `crates/treepo-gen/src/params.rs` + `assets/params/lsystem.ron` (`F-SKEL-5`) | **done** — 18 tests |
| `crates/treepo-model/src/{segment,aggregate}.rs` (skeleton handoff types) | **done** — 5 tests |
| `crates/treepo-gen/src/lsystem/grammar.rs` (`F-SKEL-1`) | **done** — 7 tests |
| `crates/treepo-gen/src/lsystem/turtle.rs` (`F-SKEL-6`) | **done** — 8 tests |
| `crates/treepo-gen/src/lsystem/compose.rs` (`F-SKEL-2`, `F-SKEL-7`, `A3`) | **done** — 12 tests |
| `crates/treepo-gen/src/trunk.rs` (`F-SKEL-3`, `F2`, `AC-SKEL-2`) | **done** — 13 tests |
| `crates/treepo-gen/src/aggregate.rs` (container *forms*) | Phase 4 — see below |
| `tools/m0-silhouette/**` (M0's debug renderer) | **done** — 17 tests |
| `tests/determinism.rs` | next |

`treepo-gen` is 14 packages against `treepo-vcs`'s 131 — `ron` and `serde` and nothing else.
It is `no_std` for the same reason `treepo-det` and `treepo-model` are: it makes the
filesystem, `std::time` and `HashMap` unreachable from the crate every generated coordinate
flows through, which is the architecture's "pure generation — no bevy, no I/O" expressed as
something the compiler checks. `dep-guard` already listed it and picked it up on sight.

### The v0.1 parameter row — `D1` revised, with evidence

The end condition asks for `A3+B2/B3+C1+D1+E3+F2+G1` **confirmed or revised with recorded
evidence**. Five of the seven are confirmed as written and implemented in
`assets/params/lsystem.ron`. `F2` is not a geometry decision and belongs to `trunk.rs`.

**`D1` is revised to `D1 + D3`, and `AC-SKEL-1` is the evidence.** `D1` is "noise rises
sharply with churn + skew", and `G1` — also in the row — removes churn from that pair. That
leaves hierarchy skew as the only route to the alien, overgrown silhouette the design wants.
`AC-SKEL-1` asks for a "high-skew, **mixed-language**, **unconventional**" repository to read
as visibly wilder than a clean one, and mixed-language and unconventional are precisely the
two signals `D3` adds and `D1` does not carry. **`D1` as written cannot pass the acceptance
criterion that tests it.** §4's "Mess / Chaos Signals" paragraph already names all four
signals together, so this corrects §5's shorthand rather than departing from the design.

`mixed_and_unconventional_reads_as_wilder_without_any_skew` is that argument as a test: two
directories that agree exactly on skew, differing only in language mix and folder naming,
must still produce different silhouettes. Under `D1` alone they would be bit-identical.

What survives of `D1` is the half `D3` does not state — near-zero for a clean repository,
rising sharply — and `a_clean_directory_is_near_deterministic` holds it to the bottom fifth
of the jitter range rather than merely below the top, which is what rules out `D2`.

### `G1` is enforced, not merely intended

Age and churn do not reach the skeleton, and there is a sharper reason than the row saying
so. Churn windows are measured against `Manifest::reference_time` — the newest commit in the
**repository** — so one commit anywhere moves every path's window at once. Had churn fed
branch angles or noise, committing to `src/` would have re-shaped `docs/`: every limb moving
on every commit, which is `AC-GROW-4` lost outright and a Grow made unreadable. Age and churn
belong to materials precisely because a material can change without the tree moving.

`SkeletonInputs::from_record` destructures `PathRecord` so `temporal` is *named and
discarded* in the reader's view rather than merely unread, and a new primitive category
cannot be added to the model without someone deciding here whether the skeleton may see it.
`no_temporal_primitive_reaches_the_skeleton` fills every temporal field with what a churning,
ancient, bursty path would carry and requires the parameters not to move by one bit. Verified
by sabotage, as usual: `recency_heat` was added to the ownership driver, the test failed
naming `G1`, and it was reverted.

### Three decisions worth finding again

**The data/code seam is between "what the numbers mean" and "how strongly they show".**
Primitives become nine normalized drivers in code, because "hierarchy skew is how chain-like
against fan-like a subtree is" does not become a different sentence because a silhouette
looked wrong. Drivers become parameters through a weighted sum in the file, because that is
exactly what §6 says gets tuned. A table mapping raw byte counts to angles would need its
reader to know what a typical byte count is; a table choosing between named presets would not
survive "adjust one parameter family at a time".

**Normalization is against absolute scales, never against the repository.** `Scales` holds
the depth that reads as maximally deep and the language count that reads as maximally mixed.
Had those been maxima over the manifest, adding one deeply-nested file would renormalize every
limb and reshape the whole tree — `AC-GROW-4` and `AC-DET-1` both lost, in a way that would
look like nondeterminism rather than like a decision. The cost is that the scales are a tuning
liability, which is why they are in the file where they can be seen.

**The table validates itself against the design document.** `F-SKEL-5` makes the table
editable, which makes it editable into nonsense. Every rule §3 and §5 state is now a load-time
error naming the decision it came from: `A3`'s cap cannot be raised, `B2/B3` stays inside
15–60°, jitter stays inside §3's 0.0–0.4, and `C1` requires `length_ratio.max <
width_ratio.min` — non-overlapping ranges, because "length falls off faster than thickness"
has to hold for *every* limb and not on average, and the overlapping trunk mass `F-SKEL-3`
depends on is width outlasting length near the origin. Quietly clamping instead would leave a
user tuning a parameter that had stopped responding, which is worse than a refused file.
`each_design_rule_refuses_the_edit_that_breaks_it` breaks all six in turn.

`deny_unknown_fields` on the weights is part of the same argument: without it a typo'd driver
name parses as a success and contributes nothing, so the user edits, reloads, sees no change,
and concludes `AC-SKEL-4` is broken.

**One test bug found by the test itself.** The misspelled-driver case patched the first
`skew_abs:` in the RON text — which was an example inside a comment, so it asserted against
an unmodified table and passed vacuously. It now asserts its target appears exactly once
before patching it. Same failure shape as `readonly-audit`'s "an oracle pointed at the wrong
repository agrees with itself perfectly".

### The composition rule — resolved, and the arity question with it

**Composition is phase-aware, and the resolution is the user's.** Significant children stay
first-class hierarchical L-system instances while they stay below a critical density; past it
the excess collapses into first-class *aggregate nodes*; inside any one instance the
productions stay classic low-arity parametric and stochastic. The phase decision lives at the
composition boundary and never inside the grammar.

**The arity question dissolves rather than being answered.** A node's children are a fact
about the repository; a node's branching is a fact about how a limb divides. `compose` decides
how many attachment sites a limb needs; `grammar` produces a limb that has them, by binary
division. **The derivation depth, not the arity, absorbs the child count** — `n` generations
yield `2^n` sites. Fifteen children on a fan of fifteen reads as an org chart; fifteen
distributed across a limb that forked four times reads as a branch.

That is also the mechanism behind the threshold. A limb's sites are bounded by `A3`'s
recursion cap, so a limb asked for more children than it can carry is *exactly* the
aggregation condition — one bound, expressed once.

**Two limits meet, and both are `A3`.** Per limb, `branch_capacity` (a new table row) says how
many children stay first-class, driven by bushiness, convention, mass and skew. Per hierarchy,
`Table::max_levels` says how many levels of nesting get limbs at all; past it a subtree becomes
one container. `validate` refuses a table whose capacity exceeds the sites the grammar can
offer — the table cannot promise what the productions cannot keep.

**The three residue decisions**, each with a worse plausible alternative:

1. *Which children stay first-class:* the largest, ties broken by path. `N4` untouched — this
   ranks paths, never people.
2. *How many containers:* proportional to the residue's **mass**, not its count. A residue that
   is 60% of the children and 5% of the bytes gets one container; a flat directory of a
   thousand equal files gets containers across nearly every site. The silhouette reports the
   shape of what it compressed.
3. *What goes in which container:* path-adjacent runs, cut by equal **count**. Path-adjacent so
   that opening a cluster shows a region rather than an assortment; by count rather than mass
   because a mass-balanced cut moves every boundary when one file changes size, where this
   moves at most one member per boundary when a file is added (`AC-GROW-4`).

A container's seed comes from its anchor and index, never its membership — so a file arriving
in a container does not reroll it. The cluster stays the cluster and gains a member, and
`adding_a_file_to_a_container_does_not_reroll_it` holds that at 120 files against 121.

### `P6` has two questions and I had conflated them

The survival test failed on first run: a file five levels beneath an aggregated directory was
"missing". It was not missing — `AggregateNode::members` holds the *roots* of what a container
stands for, deliberately, because storing the transitive closure would copy a large part of a
T3 manifest into a structure rebuilt on every Grow.

The failure was in the predicate, not the code, and fixing the assertion to match would have
buried the distinction. `Skeleton` now answers both questions separately: `accounted_roots`
lists what the skeleton *names*, and `represents(path)` answers whether a path is drawn or
compressed. That is `P6`'s own sentence — *legibility bounds detail; honesty bounds data* —
turned into two methods, and `represents` is what `F-MAT-3`'s floor and `F-INSP-3`'s
drill-down will both ask.

### Tropism is one expression, and it is the physics

`E3`'s droop is `heading += droop × sin(heading)`, applied after each segment. Correct in all
four quadrants with no branch: vertical limbs do not sag (`sin` is zero), a limb pointing right
rotates clockwise and one pointing left rotates anticlockwise — both downward — and the
magnitude peaks at the horizontal, where a real limb's bending moment does. It falls out of the
heading convention (zero is up, increasing clockwise), which is why that convention was chosen:
a limb's angle from vertical is read directly rather than derived.

### Deviations and decisions

- **Skeleton types went to `treepo-model`, not `treepo-gen`.** The architecture's file tree
  puts `segment.rs` and `aggregate.rs` there, and it is right: `treepo-gen` writes them,
  `treepo-grow` diffs them, `treepo-render` draws them. A type three crates exchange belongs
  where they all already depend.
- **`treepo-gen/src/aggregate.rs` is deliberately absent.** Container *synthesis* — rolling up
  mass and membership — is twenty lines and belongs beside the decision that creates it, in
  `compose.rs`. What that file is for is choosing a container's visual *form*, which is
  enrichment and lands with Phase 4.
- **`TABLE_VERSION` bumped to 2.** A version 1 table has no `branch_capacity` row, and a table
  missing the row that decides when a limb aggregates would compose a different tree while
  parsing perfectly.
- **Segments taper through their joints**, resolved during interpretation via the turtle's own
  stack rather than by matching coordinates afterwards. Coordinate matching is quadratic and,
  worse, cannot tell a genuine joint from two segments that happen to meet.

Verified by sabotage twice. Turning aggregation into truncation failed six tests, the first
naming the vanished path outright. Removing the hierarchy cap failed the `A3` test with "a limb
appeared at depth 11, past A3's cap of 5". Both reverted.

### The hybrid trunk — nothing draws a trunk

> **Superseded 2026-07-28 by "The trunk column" below.** The co-origin construction this
> section describes — every primary leaving one basal tip, the trunk being purely their
> overlap — was replaced after its first silhouettes. What survives unchanged: the trunk's
> width is not a parameter, the mass comes from what the limbs carry, and nothing draws an
> arbitrary trunk. Kept as written because the reasoning is why the replacement is shaped the
> way it is.

`treepo_gen::grow` is now the product's entry point: a `Manifest` in, a `Skeleton` out.

`design/visual-construction.md` settled the trunk against two alternatives. A **dedicated
trunk** makes the trunk a constant, so every repository shares a silhouette in the region a
viewer looks at first. A **pure trunkless stack** was rejected for L-system compatibility and
redraw stability — with no axiom there is nothing for the productions to start from. The
hybrid takes the axiom from one and the mass from the other: a minimal basal segment, primary
limbs fanned narrowly enough that their base widths overlap, and **the trunk is that overlap**.

**The trunk's width is not a parameter.** A stem is as wide as its limbs' combined base widths,
packed — so a repository that grows a heavy new top-level directory thickens at the base
because the limb is thick, not because a number was tuned to agree. `packing` is the only knob,
and the loader refuses a value above 1000: a stem wider than what it carries is a trunk with
limbs stuck on, which is the construction that was rejected.

**Stated because it will look like a defect one day:** a repository with one top-level
directory has almost no trunk. There is nothing to overlap. The trunk depicts breadth at the
root, and a repository without breadth there has none to depict.

`the_fan_controls_how_far_the_trunk_extends` measures the claim rather than asserting it —
two limbs `θ` apart separate at `d = (w₁+w₂)/(4·sin(θ/2))`, so the trunk's height is where the
first adjacent pair parts. A narrow fan must give a taller trunk than a wide one, and even the
widest must outlast the axiom. The first version of this test probed at a fixed distance and
**passed at every fan setting** — it was measuring a point too close to the tip to discriminate.
Replaced rather than kept.

### `F2` and `F-SKEL-3` are one mechanism used twice

A group is a *stem*, not a container: the same shape as the basal axiom, one level down, with
its members fanning from its tip exactly as the primaries fan from the basal tip. So there is
no second construction to keep consistent with the first.

**How many groups is not a parameter.** A run of small entries closes once it is no longer
small — by the same threshold that made its members candidates — or once it holds as much as a
limb's `branch_capacity` allows. Without that second stop, a repository whose small entries are
*all* negligible would gather every one onto a single stem, and "fewer, thicker limbs" would
arrive at one limb with a fan of forty: the diagram `F2` exists to prevent, moved down a level.

`NodeRole` gained `Group` and `RootMass`, and `Group` is deliberately **not** `Aggregate` —
both gather several paths under one node, but only the aggregate replaces them. Conflating them
would report drawn paths as compressed and make `F2` indistinguishable from `F-SKEL-7`.

### `AC-SKEL-2` falls out rather than being special-cased

An empty repository has no primary limbs, so no overlap, so no trunk. What it has is the
root-mass cluster every tree has and a basal segment the table holds short — `validate` refuses
a table whose basal segment could exceed the shortest limb it can produce. The result is a seed
sitting in its roots, and the test asserts both halves: the cluster exists, and the whole thing
is no taller than the axiom. A dedicated trunk could not have produced it.

Sabotage-verified: making `spread` ignore the fan left every limb collinear, and the trunk-height
test failed with both fans reading `Fx::MAX` — never separating, therefore never measuring.

## `tools/m0-silhouette` — the first look at a tree

```
cargo run -p m0-silhouette                     # every corpus fixture
cargo run -p m0-silhouette -- --path .         # any repository on disk
cargo run -p m0-silhouette -- --pin ripgrep    # a pinned tier repository (--fetch to clone)
cargo run -p m0-silhouette -- --table other.ron
```

Extraction → `grow` → an indexed PNG per repository, plus a `.txt` sidecar carrying the world
extent the fitted view throws away and the full skeleton digest. 15 of the 16 corpus fixtures
draw; `bare` refuses, in PRD §6's own words, which is the right answer rather than a gap.

**No dependencies, including no PNG encoder.** A PNG's IDAT is a zlib stream, zlib streams may
be built from *stored* blocks, and a stored block is a length, its complement, and the bytes.
That is ninety lines in `src/png.rs`, against pulling a decoder, a filter bank and a compressor
into `cargo deny`'s report so a debug tool can write files nobody keeps. The cost is stated and
bounded: a 1024×1024 frame lands near 1 MB instead of 40 kB, in `target/`.

**The rasterizer is integer-only although nothing forces it to be.** `N3` binds `crates/`, not
`tools/`. It is `i64`/`i128` anyway because that makes the *PNG bytes* comparable across
platforms, not just the numbers behind them — `AC-DET-2` becomes a check anyone can run without
a debugger: same file, same bytes, three machines.

### `--table` is `AC-SKEL-4`, and it cost one flag

Load a parameter table at run time, draw, compare. `F-SKEL-5` made the criterion nearly free,
exactly as the PRD predicted. The loader still validates, so a table edited into nonsense is
refused by name rather than drawn as a strange tree.

### An aggregate had no geometry, so it was invisible

`compose` pushes an `Aggregate` node and **no segments** — correctly: the container's visual
*form* is Phase 4 enrichment by explicit decision. But a container that draws nothing is, to the
eye, exactly the truncation `P6` forbids, in the one picture built to check it is not one. So
the renderer marks them, inventing nothing: a disc as wide as the branch that ends there, a
width the skeleton already stated.

Worth recording *how* this was missed. `each_role_reaches_the_canvas_in_its_own_ink` passed —
against a hand-built skeleton whose aggregate had a segment. The fixture was more generous than
the pipeline. `a_grown_container_is_marked_even_though_it_has_no_geometry` now grows one
instead, and asserts both halves: that aggregates still carry no geometry, and that they are
still marked. If Phase 4 gives containers real geometry, that test fails and says so.

### What the pictures said — four findings, none of them code

This is §6 step 4, and it is the whole reason the tool exists. `lsystem.ron` was coherent,
validated and defended by 58 tests, and the tests could only prove a table *self-consistent*.
**The first three were then fixed — see the tuning sprint below; the fourth is still open.**

1. **Nothing tapers across the composition boundary.** `C1`'s width falloff applies *inside* an
   L-system instance; a child limb then draws a fresh `base_width` from the table. So a limb
   four levels out is as thick as one hanging off the trunk, and `single-author` renders at one
   uniform weight throughout. Structural, not a number: the falloff has to cross the boundary.
2. **The basal axiom is a pancake wherever there are many primaries.** `stem_width` is the sum
   of the primaries' base widths, packed, while `validate` caps `basal_length` at the shortest
   limb the table can produce. treepo's own repository has eight top-level entries — a stem
   ~1150 wide and ~150 long, which draws as a disc. The two rows are internally consistent and
   inconsistent with each other.
3. **No upward tropism.** `branch_angle` at 36° over up to five generations, plus the
   composition fan, walks headings to horizontal and past it. §8 of the parameterization
   document already names tropism as an addition that does not break the contract.
4. **Consequently the tree does not stand up.** `treepo.png` radiates from a black disc;
   `single-author` grows diagonally out of frame. `deep-nesting` is the one that reads properly
   as a tree, and it reads properly because it is narrow — which is findings 2 and 3 restated.

`AC-SKEL-2` **passes by eye**: `empty.png` is a dark seed sitting in a splayed slate root
cluster, no trunk. `A3` and `F-SKEL-7` are visible in `deep-nesting.png` — a >15-level fixture
capped at five with two terracotta containers holding what is past the cap. **`AC-SKEL-1` cannot
be judged yet**, and not because the tool is missing anything: the corpus has no clean-versus-
messy pair at comparable size. It needs the T1 pin (`ripgrep`) or a fixture built for it.

## The tuning sprint — findings 1 to 3, closed

Table version **3 → 4**. Each change was made, then looked at, before the next one started —
§6's "one parameter family at a time", with `m0-silhouette` as the instrument.

### 1. The falloff crosses the composition boundary

`LimbParams::grafted_onto(carried)` — the narrower of two claims:

* **what it carries**: one more step of `width_ratio` past the branch it grafts onto, which is
  exactly what the next generation *inside* that branch would have got, so a joint between two
  limbs is indistinguishable from a joint within one;
* **what it is**: its own mass-driven `base_width`, so a small directory on a thick trunk is
  drawn small rather than inheriting the trunk.

Taking the minimum means a heavy subtree on a thin twig stays thin, and that is right — the
parent is thick *because* of what it carries, so a thin parent already claims there is little
below it. Under equal drivers the inherited term always wins and the chain narrows strictly.

`compose::Site` is the vehicle: position, heading, and *what the branch had tapered to*, which
travel together everywhere a tip hands a child to `place`. Threading a bare third parameter
through four call sites was the alternative.

**Sabotage-verified.** Dropping the `.min(...)`: `params` reports the chain flat at `0.199999`
for five levels, and `compose` names the pair — `docs` drawing at `0.225` off a parent at
`0.220`, a child wider than its parent. `treepo.png` went from one uniform weight to a visible
root-to-twig gradient in the same edit.

### 2. Tropism, and the ground band that `sin` cannot provide

`tropism` is a row beside `droop`, driven by the wildness signals `D1 + D3` already uses with
the sign that says a disciplined repository grows straighter: `convention` up, `bushiness` and
`skew_abs` down. Nothing temporal — `G1` holds. The turtle applies **the difference**, so it is
one subtraction rather than a second code path, and a heavy limb still sags — from a heading
that was being lifted, which is what a loaded branch on a living tree does.

**Why a second mechanism was needed.** A `sin`-scaled uplift is zero at straight down: the one
heading it most needs to correct is its own fixed point, and a limb that reaches vertical-down
stays there forever. `the_ground_band_recovers_a_limb_pointing_straight_down` asserts exactly
that about the `sin` term before asserting the band fixes it.

So the band is **flat**, and it has two thresholds. Past `engage` from vertical a fixed `lift`
rotates the heading back every segment; it keeps hold until the heading is inside `release`.
The gap is the hysteresis and it is the whole design — with one threshold a limb chatters along
the boundary, corrected and released and corrected, drawing a saw edge; with two it dips, is
lifted clear, and travels freely until it dips again. `validate` refuses `release >= engage`,
because that is the band with its hysteresis removed.

`grounded` lives in the turtle's `State`, so it is pushed and popped with everything else: a
fork's children both inherit whether their parent was being lifted.

**`AC-SKEL-2` is untouched, and the digest proves it.** `empty.png`'s skeleton hash was
`36e951ce5d91588d` before the change and after it. Roots and the basal segment are placed by
`trunk.rs` directly and never run through the turtle, so no tropism can reach them — the roots
stay below ground because they are not branches.

### 3. The basal axiom is an aspect rule, not a number

`trunk.basal_length: Row` became `basal_aspect` (per-mille of the stem's own width) plus
`basal_min` (a floor), read through `TrunkParams::basal_length(width)` so `grow` and
`place_group` cannot disagree about it.

The old row fought `packing` and lost. A stem is as wide as its limbs' base widths summed, so a
repository with eight top-level entries got one eight limb-widths across while an absolute cap
held its length under a single limb-length. Both numbers were reasonable; together they
described a disc. An aspect ratio cannot have that argument, and `F-SKEL-3` already says the
axiom's "length/radius [is] driven by total root mass / primary limb count" — the stem's width
is precisely where that mass has been added up.

**"Minimal" is now a shape rather than a size.** `validate` caps the aspect at 2000: never more
than twice as long as it is wide, at any scale. A repository with one top-level directory still
gets almost no trunk, because it has almost no width to be long in proportion to.

**The value came from a failing test, not from taste.** At an aspect of 1400 the existing
`the_fan_controls_how_far_the_trunk_extends` failed with the axiom at `1.109` against an
overlap of `0.325` — the axiom out-reaching the limbs it was supposed to start, which is
`F-SKEL-3` violated from the other side. That number is the bound; `380` sits inside it.

### Finding 4 is still open, and finding 3 ran into it

The fan is the reason. `trunk.fan` reaches `150000` — limbs at ±75° from vertical — and
treepo's own bushiness takes it near that ceiling, which is why `treepo.png` still spreads
sideways more than it rises.

It also bounds finding 3, and the geometry says so exactly. Two limbs `θ` apart separate at
`d = (w₁+w₂)/(4·sin(θ/2))`, and with `n` primaries over a fan `F` that works out to an overlap
of about `stem_width/(packing × F)` — **independent of `n`, and set entirely by the fan.** At
150° that is 0.53 stem-widths of overlap, so an axiom may be at most about half as long as it
is wide before it becomes the trunk. A stem *shaped* like a stem needs an aspect near 1.0,
which needs the fan under about 80°.

So the axiom is as long as the current fan permits, and that is why the base still reads
squat. Narrowing `trunk.fan` is one row and it is a separate family — the next tuning pass, not
this one.

## The trunk column — finding 4 was not the fan

The next pass did not narrow the fan. The arithmetic above is correct and its conclusion was
wrong, and the reason is worth keeping.

`1/(packing × F)` is independent of the limb count. That is the tell, and I read it as "the fan
is the lever" when what it actually says is **there is no lever**. Whatever the fan does, the
overlap a point fan leaves is a fixed multiple of the stem's own width — so the base can be
made a little taller relative to itself, and never given any *volume*. Narrowing the fan would
have bought a slightly less squat seed and cost `AC-SKEL-1` its sprawl.

The human read of the pictures got there first and by a different route: the base looked like
an **oversized seed** — the same glyph as `AC-SKEL-2`'s empty repository, scaled up. Not a
tuning complaint. A primary leaving a single tip has nowhere to leave *from*.

### The pipe column

`docs/workspace/trunk-pipe-rework.md` specified the replacement and it is now
`design/visual-construction.md` v2.1. A **collar** at the foot, flared into the roots; one
**internode** per primary — the vertical room that limb needs to exist as volume rather than as
a ray; and the **width at any height is the support still carried there**, so each departure
drops its own share and each internode tapers across the drop. Above the last departure nothing
is left to carry and the column ends, which is why an empty repository still has none.

Three things in the design that were not obvious before building it:

* **Internode length is an aspect of the support that departs**, not a length in world units.
  The internodes then sum to an aspect of the column's own width, so a column has the same
  proportions carrying three primaries or thirty and the count only decides how the height is
  divided. The draft's suggested "uniform floor plus a mild scale" makes a thirty-primary
  column thirty blocks tall.
* **Departure order is as load-bearing as departure height.** Fan position is path order, so a
  directory gaining bytes never slides sideways. Departure height is then outermost-first,
  working inward — the sides alternate so the column does not lean, and the innermost primary
  leaves last and nearly vertical, so the trunk hands off to a leader rather than stopping in
  mid-air.
* **The width is a projection** (`P6`). `F2` bounds how many primaries survive but not what
  they sum to, and a linear column made a monorepo a telephone pole. Support past
  `support_knee` counts at `support_beyond`; `validate` refuses zero there, because a hard
  ceiling makes every large repository draw the same base — the constant trunk, arriving by the
  back door.

### Two pictures the arithmetic did not predict

**The roots were drawn inside the trunk.** The old rule made a root a fraction of the collar's
*length*; once the foot flared, the whole cluster fitted inside it and `empty.png` was a black
bulb with a grey smudge in the middle. Roots are now three quarters of the flared foot's
*width*, which clears its edge by a quarter. Setting them to the full width first produced a
starfish, which is in the comment.

**`basal_aspect` is contested by two subjects at once, and cannot satisfy both by being
right.** It is a ratio, so `empty` and a monorepo get the same collar *shape*. At 1600 a
populated base is a proper cone and `empty.png` is a pill — `AC-SKEL-2`'s lonely trunk in
miniature. At 400 `empty` is a seed and a populated collar is four times wider than it is tall,
which the renderer's round caps turn into an egg. 900 is where both read, and the number is
commented in the table with that argument rather than left looking arbitrary.

### What the gate said

Six invariants, each verified by breaking the code and watching the named test fail: the soft
cap (`a_broader_repository_thickens_at_the_base_without_scaling_with_it`), the fan's
decoupling (`the_fan_spreads_the_crown_without_touching_the_trunk`), the pipe drop and the
column's continuity (`the_column_narrows_as_each_primary_leaves`), departures along the axis
(`primaries_leave_along_the_column_rather_than_from_one_point`, which the first attempt at a
sabotage did *not* catch — shifting every departure down one internode leaves them distinct),
the internodes carrying the height (`the_column_keeps_its_proportions_however_broad_the_repository`),
and the flare (`the_foot_is_the_widest_part_of_the_tree`).

`cargo xtask determinism` is unchanged at `39681da8…` — it hashes primitives and seeds, not
skeletons, which is exactly the gap the next sprint closes. The skeleton digests every fixture
reports through `m0-silhouette` all moved, and that is the intended geometry change.

## Agent hygiene

Run `cargo clippy --workspace --all-targets -- -D warnings` and the relevant tests **locally,
and read the output**, before every push. CI is the second filter, never the first. This is
written down because it was violated once: commit `4ef8286` went out unread and failed on a
`dead_code` warning that a local clippy run would have shown in two seconds. Three further CI
round-trips in the same sprint went to platform differences that were reasonable-about-able
before pushing. The full local gate is:

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask determinism && cargo xtask dep-guard && cargo deny check
cargo xtask readonly-audit
```

`cargo xtask budget` is deliberately *not* in that list — it needs several gigabytes of clones
and several minutes. Run it when extraction changes, not before every push.

## S5 — AC-SKEL-1 subject pair (2026-07-29)

Two synthetic corpus fixtures of comparable size, wired into the lab strip:

| Id | Paths | Role |
|----|------:|------|
| `skel1-clean` | 38 | Conventional `src`/`tests`/`docs`, single language (Rust), balanced modules |
| `skel1-messy` | 44 | Unconventional names, mixed languages, fat `dump/` + thin leaves, multi-author |

Same order of magnitude so AC-SKEL-1 judges **shape** (orderly vs wild), not scale. Listed in
`qa/subjects.ron` for `m0-silhouette lab`. Eye judgment of the criterion is still a lab pass;
this sprint only supplies the pair.

## Joint-promote micro-pass — session `20260729_042807_joint-promote` (2026-07-29)

Second promote after joint eye on the first table. Four lab prefers plus D1-safe jitter:

| Parameter | From → to | Evidence |
|-----------|-----------|----------|
| `trunk.fan.base` | 90000 → **80000** | renders/0002 — 90° too lateral |
| `tropism.base` | 11000 → **23000** | renders/0013 — match stronger droop |
| `ground.engage` | 96000 → **92000** | renders/0011 — catch sideways outliers |
| `ground.release` | 72000 → **42000** | renders/0008 — bounce-back on heavy limbs |
| `angle_jitter.per` wild weights | skew_abs 10k→**16k**, diversity 5k→**9k**, fragmentation 3k→**6k** | base held at 2000 (D1) |

Not promoted: `branch_capacity.base` → 3 (`needs_code` — helps `self`, truncates `single-author`).

Gate: `cargo test -p treepo-gen` 72 ok including `a_clean_directory_is_near_deterministic`.

## First high-confidence promote — lab session `20260728_234906_lab` (2026-07-29)

S4 lab campaign: 111 strips, 25 findings. This sprint promotes the **high-confidence**
deltas into `assets/params/lsystem.ron` as a joint table (not one family at a time on the
product file — isolation already happened in the lab). Evidence paths are
`qa/sessions/20260728_234906_lab/findings/*.json` and each finding's chosen `renders/NNNN/`.

| Parameter | From → to | Lab render | Held / dropped |
|-----------|-----------|------------|----------------|
| `branch_angle.base` | 36000 → **30000** | 0110 | — |
| `droop.per.mass` | 12000 → **30000** | 0097 | — |
| `ground.lift` | 15000 → **25000** | 0086 | — |
| `base_length.base` | 1000 → **2000** | 0073 | — |
| `base_width.base` | 200 → **250** | 0078 | — |
| `trunk.flare` | 1350 → **1250** | 0028 | — |
| `trunk.internode_aspect` | 2000 → **4000** | 0032 | — |
| `trunk.internode_min` | 140 → **200** | 0030 | — |
| `trunk.support_knee` | 1200 → **800** | 0041 | — |
| `trunk.support_beyond` | 300 → **250** | 0045 | — |
| `trunk.fan.base` | 62000 → **90000** | 0048 | — |
| `trunk.group_below` | 55 → **10** | 0059 | — |
| `angle_jitter.base` | preferred 20000 | 0099 | **held at 2000** — breaks D1 (`a_clean_directory_is_near_deterministic`) |
| `length_jitter.base`, scales, fan min/max | various | — | low confidence / not this pass |

Theme of the promote: **de-clutter monorepo bases** (internode height, F2 threshold, fan,
support projection) plus a more organic branch angle and heavier mass droop with matching
ground lift.

Gate after promote: `cargo test --workspace` green, clippy `-D warnings` on gen + silhouette,
determinism overall still `39681da8…` (primitives only). Skeleton digests moved as intended —
corpus strip under `target/m0-silhouette-promote/` (untracked).

## S7 — the skeleton in the gate, and M0 EXIT (2026-07-29)

The last two acceptance gaps, and they were the same gap seen from two sides: the pipeline
could produce a tree and nothing outside a debug tool ever looked at one.

### The digest was a tool feature pretending to be a gate

`m0-silhouette` printed a skeleton digest beside every picture, and `cargo xtask determinism`
hashed five `treepo-det` probes. So the harness proved that the *arithmetic* reproduces on
three platforms while the thing a user would actually see was checked by nobody. Architecture
D2 had said what to do since Phase 0 — "each corpus fixture three times per platform" — and the
Phase 0 module docs even carried the note that Phase 3 would add skeleton hashes.

The hash moved onto `Skeleton::digest` in `treepo-model`, where the three consumers can share
one definition, and grew two fields on the way:

* **Parentage.** The tool's version hashed geometry and roles. A node re-hung on a different
  parent, with nothing moved, hashed the same — and `AC-GROW-4` diffs the tree, `F-INSP-3`
  walks it. `re_hanging_a_limb_changes_the_digest` is the test.
* **Aggregate membership.** A container that absorbed a different set of paths stands for
  something else. Hashing only its anchor called `F-SKEL-7`'s residue interchangeable.
  `a_container_that_absorbed_something_else_changes_the_digest`.

Lengths precede every variable-length field and the counts precede everything, so two distinct
skeletons cannot run into each other in the encoding. Tag bumped to `treepo-skeleton-v2`; `v1`
was the tool-local hash, and a digest from an old build must not read as a disagreement about
geometry.

### The seed had to be fixed, and that is not a compromise

Extraction takes its root seed from repository identity, and `F-MAN-3`'s third tier is a hash
of the absolute path. Seeding the corpus stage that way would have made `AC-DET-2` fail on
every machine for a reason that is not a finding — the checkout directory is *supposed* to
differ. So the stage supplies its own constant.

One consequence shows in the report and is worth expecting rather than debugging:
`detached-head`, `shallow`, `no-remote`, and `multi-remote` hold the same files and differ only
in refs and remotes, so under a fixed seed they print the same digest. That is the right
answer. Those four exist for `F-MAN-3`, and identity resolution is where they get told apart.

Thread count is deliberately *not* pinned. `AC-DET-3` forbids hardware-dependent values in the
generative pipeline and the history pass is threaded; leaving it at the product's default is
what would let a thread-order dependency surface as a cross-platform difference rather than be
hidden by the harness.

### `bare` refusing is a result, not an exception

The bare-repository fixture cannot be extracted, so the report records `refused` — a constant,
so it still compares byte for byte. The list of names allowed to say that is one entry long and
checked: any *other* fixture becoming unextractable now fails the command, rather than emitting
a line that would still match on all three platforms. Verified by emptying the list and
watching `bare` fail the run.

### `AC-SKEL-3` — the criterion reads §7 from the other side

The §7 table is titled *Grow budget — first run, full extraction*, and a first-run Grow is
extraction **and** skeleton generation. So `AC-EXT-1` asks whether extraction fits and
`AC-SKEL-3` whether extraction plus the skeleton still does; `cargo xtask budget` prints both
verdicts, because a run that fits one and not the other wants a different fix.

Growth is measured rather than argued from the composition bounds. `A3` caps recursion and
`branch_capacity` caps sites per limb, so the skeleton cannot grow without limit whatever the
repository does — but composing it still reads every path, and bounded-by-construction is a
proof about the output, not a measurement of the work.

### `AC-SKEL-3` — measured at T3, 2026-07-29

`rust` 1.83.0 (pin `90b35a62`), the T3 row: **49,029 files / 52,527 paths, 268,290 commits,
6,840 authors.**

| | Seconds | Against §7 T3 |
|---|---:|---|
| Extraction | 256.79 | within target (600 s target, 1800 s ceiling) — `AC-EXT-1` |
| Skeleton | **1.466** | 0.24% of the target |
| Grow total | 258.26 | within target — **`AC-SKEL-3` closed** |

Extraction re-measures the 2026-07-27 figure at 256.79 s against 252.25 s — 1.8% apart, which
is a useful corroboration of that run rather than a new claim. `log_pass` is 254 s of the 257;
99% of extraction is still history, at 0.96 s per 1,000 commits, exactly as recorded then.

The skeleton figure is worth reading beside its shape: **521 nodes and 1,042 segments for
52,527 paths**, with **236 containers**. That is `A3`'s cap and `F-SKEL-7`'s aggregation doing
exactly what `P6` asks — a kernel-scale monorepo compresses to a tree a person can look at,
and the containers say how much was compressed rather than dropping it. Growth cost scales
with paths read (0.279 s per 10,000), not with the skeleton produced, which is why the number
is small and stays small.

Skeleton generation is, in short, not where the Grow budget goes and is not close to being.
Every second of a first-run Grow at T3 is the history pass, and `AC-SKEL-3` is met with three
orders of magnitude to spare.

### What the gate said

Three sabotages, each producing the named failure:

| Sabotage | Caught by |
|---|---|
| A run counter perturbing the basal node in `trunk::grow` | `empty does not grow the same skeleton twice` on the first fixture |
| `REFUSED` emptied | `bare` fails the run instead of reporting `refused` |
| One digest edited in a saved report | `--check` diffs it |

Local gate green: fmt, clippy `-D warnings`, 428 tests, dep-guard, `cargo deny`,
readonly-audit (18 fixtures, 17 extracted, 0 writes, detector 4/4). Determinism overall moved
from `39681da8…` to `a82991f0…` — the primitive probes are unchanged and the twenty-three new
`skeleton/*` lines are the difference. Cross-platform compare for that report closed the same
day (see M0 EXIT scoreboard / `AC-DET-2` confirmed).

`cargo fmt --all --check` was **already failing on `main`** before this sprint, on a string
literal in `tools/corpus/src/shapes.rs` from the S5 fixture pair (`340c7b6`). Fixed here. The
lint job would have been red on `main`; the Agent hygiene rule below exists for exactly this
and was not followed that time.

## M0 EXIT — the scoreboard

| End condition | Criterion | Status |
|---|---|---|
| PNGs for every corpus fixture | — | **done** — `m0-silhouette`, 17 fixtures + `--pin` + `--path` |
| Nine identical skeleton hashes | `AC-DET-1`, `AC-DET-2` | **done** — local triple-run + **CI compare green** (see below) |
| Clean vs. high-skew differ | `AC-SKEL-1` | **done** — `skel1-clean` / `skel1-messy`, judged by eye 2026-07-29 |
| T0 is a seed and root cluster | `AC-SKEL-2` | **done** — falls out of the column, no special case |
| T3 within the §7 Grow budget | `AC-SKEL-3` | **done** — measured on `rust` 1.83.0: 1.47 s skeleton, 258 s grow total against a 600 s target |
| Table edit, no recompile | `AC-SKEL-4` | **done** — `--table`, and the lab is built on it |
| Parameter row confirmed with evidence | — | **done** — `A3+B2/B3+C1+D1&D3+E3+F2+G1`, `D1` revised to `D1&D3`; findings under `qa/sessions/` |

**Phase 3 / M0 is fully closed.** Every end condition above is met and verified.

### `AC-DET-2` with skeletons — confirmed 2026-07-29

Same shape as Phase 0: one machine can only prove `AC-DET-1`; the three-platform compare is
CI. On commit `4c8de03` (*Sprint: M0 EXIT — the skeleton in the determinism gate, and
AC-SKEL-3 measured.*):

* **probe (linux / macos / windows)** — all success
* **compare across platforms (AC-DET-2)** — success (byte-identical reports)

Run: https://github.com/bra-khet/treepo/actions/runs/30426210832

Local corroboration (WSL Ubuntu, untracked report discarded after recording): overall
`a82991f0…`, primitives unchanged from Phase 0, twenty-three `skeleton/*` lines present,
`skeleton/bare` = `refused`. That matches the post-S7 harness contract.

The residual risk if this ever goes red is fixture identity, not trig: Phase 0 already
cleared table math. If every `skeleton/*` line moves and no probe does, suspect
`tools/corpus`, not the L-system — the workflow failure hint says so.

### Reference digests after S7 (all three platforms)

Primitives are still the Phase 0 goldens above. The report now includes corpus skeleton
lines; the pin that changes when geometry or the digest encoding changes is:

```
overall    a82991f06a2a7994b47cf703a9168a1d8262abc395030cc270c96146b21e1aae
```

(Phase 0's `overall` `39681da8…` was five probes only — expected to differ once skeletons
entered the harness. **Superseded again by Phase 4's identity probes — see below.**)

---

## Phase 4 — identity policy, materials & enrichment (in progress)

| Deliverable | Status |
|---|---|
| `crates/treepo-id/src/pseudonym.rs` + `assets/wordlists/pseudonyms.ron` (`F-ID-3`) | **done** — 11 tests |
| `crates/treepo-id/src/palette.rs` + `assets/palettes/author-palette.ron` (`F-ID-4`, `AC-MAT-4`) | **done** — 10 tests |
| `pseudonym` / `author-color` probes in `cargo xtask determinism` (`AC-ID-2`) | **done** — local `AC-DET-1`; CI compare pending |
| `crates/treepo-vcs/src/self_ident.rs` (`F-ID-1`) | **done** — moved out of `treepo-id`, see S9 |
| `crates/treepo-id/src/policy.rs` (`F-ID-5`, `F-ID-7`) | **done** — 8 tests |
| `crates/treepo-vcs/tests/privacy.rs` (`AC-ID-1`) | **done** — 7 tests |
| `crates/treepo-gen/src/{material,normalize,gradient,enrichment,classify}.rs` (`F-MAT-*`) | next |
| `F-ID-6` reveal opt-in in `config.json`; `AC-ID-3`/`AC-ID-4` | Phase 10 (settings), per the campaign |

`treepo-id` is 14 packages, the same `ron` + `serde` as `treepo-gen`, and `no_std` for the
same reason. `dep-guard` already listed it under Phase 4 and picked it up on sight.

### S8 — the pseudonymous surface (2026-07-29)

Two pure functions of an `AuthorKey`, and nothing else. **No real name exists anywhere in
the workspace yet** — not because a policy forbids one but because no type holds one, which
is the strongest form `AC-ID-1` can take and is worth having for as long as it lasts. When
`policy.rs` lands and becomes the single gate, `Wordlist::draw` and `Palette::color_of` drop
to `pub(crate)`; that is written in the crate docs so the demotion is not forgotten.

#### `AC-MAT-4` needed a colour space, and sRGB is not one

"Minimum perceptual separation" is only testable in a perceptually uniform space, and
sRGB → OKLab wants a 2.4 power and a cube root — two transcendental functions `treepo-det`
has no integer implementation of, in a crate where `N3` forbids a float reaching generated
output.

**The palette is authored in OKLCh instead**, which removes the problem rather than solving
it. An entry is a lightness, a chroma and a hue — how a separated palette gets designed
anyway — and the only conversion needed is `a = C·cos(h)`, `b = C·sin(h)`, which is the trig
table Phase 0 already proved bit-identical on three platforms. sRGB becomes the render
layer's job in Phase 5, in float, downstream of everything `AC-DET-2` covers. The cost is
stated in the file: **nothing checks the sRGB gamut**, so gamut mapping lands with the
conversion.

**"Adjacent" means every pair.** Entries are chosen by hashing a key, so any two can end up
side by side in a mosaic; reading `AC-MAT-4`'s "adjacent" as "adjacent in the list" would
test a property no rendered pixel depends on.

#### The jitter is what makes eighteen entries enough, and it is bounded on purpose

Eighteen colours against a repository with thousands of contributors is not a palette, it is
a bucket. So each entry is a **family**: the key picks the family, then a point within a
declared neighbourhood of it. What keeps `AC-MAT-4` true is that `validate` subtracts *both*
neighbourhood radii from every pairwise distance before comparing to the threshold — so
widening the jitter tightens the palette and, far enough, fails the file. Without that
arithmetic the jitter would eat the guarantee silently: the file would still parse and two
colours would occasionally be closer than the threshold that was supposed to be enforced.

The bound is the arc bound, `ΔC + (C + ΔC)·Δh`, deliberately loose — erring high costs a
slightly stricter palette; erring low costs the guarantee. Built-in palette: tightest pair
**110** against a requirement of **95**, and both numbers are asserted by the test that
reads them, so an edit that eats the headroom has to change the file's comment too.

#### `F-ID-3`: two functions, because two properties pull apart

A pseudonym that is a pure function of one key cannot know whether someone else drew the
same pair; a pseudonym unique within a repository is a function of the whole key set. Both
exist and are named so the difference is visible at the call site — `draw` and `assign`.

Assignment walks keys in **ascending key order** and the first claimant keeps the pair. Key
order is hash order, which matters twice: it is uncorrelated with contribution volume, so
`N4` is untouched, and it is a property of the keys alone, so the roster does not depend on
how a caller happened to iterate a manifest.

**The one way a pseudonym moves, stated because someone will hit it:** a *new* contributor
appears who both draws the same word pair and sorts earlier by key. At 128 × 128 = 16,384
pairs that is one chance in 16,384 per contributor added. The wordlist is sized for that
claim rather than for elegance, and a test asserts the floor so trimming it forces the claim
to be re-derived.

**Nothing can fail.** Past eight salted redraws a contributor keeps its own base pair and
takes the first free discriminator — `Ash Willow 2`. Ugly, and a far better outcome than an
error on a repository with more contributors than the wordlist has pairs (the kernel has
~25,000). At the sizes treepo meets it never appears.

#### The sabotage found a vacuous test, which is the point of doing it

`assignment_does_not_depend_on_the_order_the_keys_arrive_in` **passed** with the key-ordered
walk replaced by the caller's arrival order. 200 keys against 16,384 pairs collide never, so
there was nothing for the resolution order to decide and the test asserted a property it
could not observe. Same shape as `readonly-audit`'s "an oracle pointed at the wrong
repository agrees with itself perfectly", and as the misspelled-driver case that patched a
comment.

It now runs on a deliberately crowded four-by-four wordlist and **refuses to proceed unless
resolution actually fired**, so a fixture that stops forcing collisions fails loudly instead
of quietly testing nothing. Re-sabotaged afterwards: it fails naming the key and both names.
The duplicate-key case was strengthened at the same time — it asserted only the roster
*length*, which survives a caller passing a key twice even though every pseudonym moves.

The palette's equivalent needed its own fixture for the same reason: the built-in palette
clears its requirement by 15, so dropping the radii from `validate` leaves it passing.
`a_palette_legal_only_without_jitter_is_refused_with_it` is built to sit in the gap — 70
apart, above the threshold and below the threshold plus both radii — and is the only test
that fails when the arithmetic goes. Verified by removing it: that test and the greedy-jitter
case both failed; reverted.

#### One fixture bug worth remembering

`AuthorKey::from_email` ASCII-lowercases its input (`F-EXT-9`), so keys built from
`n.to_le_bytes()` fold `0x41` and `0x61` into one contributor: a "400 contributors" fixture
was quietly 348. Two tests failed with an off-by-13% that read like a collision bug in
`assign`. Test keys are spelled as addresses now, with the reason attached.

### `AC-ID-2` — in the gate, CI compare pending

Two probes, `pseudonym` and `author-color`, over a fixed set of **512** contributors — a set
size chosen so the built-in wordlist actually collides and the probe covers the salted-redraw
path rather than only the happy one.

The pseudonym probe hashes the whole roster rather than a sample of draws: a draw is a hash
and a modulo, and the *assignment* is where a platform difference would live if one existed.
The colour probe hashes the tightest pair's separation alongside every drawn colour and its
OKLab coordinates, because that separation is what `AC-MAT-4` is about, it comes through the
trig table, and a platform disagreeing about it would be disagreeing about whether the
palette is legal at all.

```
pseudonym     26c0fbba3ef9d39c407f504aadfa040475b5e428de0a2d3e790a61edc3eaa5c3
author-color  38e2205deebae3f5a1a0078ccb434513d9fa84bd0030824e2306eed7de89ba79
overall       7bd8896fe7a19c603de363dedfa4944d428786fbdb8bcfb43d7724a1a20e5a79
```

`overall` moved from `a82991f0…`; the five primitive probes and all twenty-three
`skeleton/*` lines are unchanged, and the two new probe lines are the whole difference.
**`AC-DET-1` is met locally (triple-run, green); `AC-ID-2` proper is the three-platform
compare and needs a CI run on this commit** — same shape as Phase 0 and S7, where one
machine could only ever prove the within-platform half.

Local gate green: fmt, clippy `-D warnings`, 449 tests, dep-guard (6 crates clean),
`cargo deny` (advisories/bans/licences/sources ok), readonly-audit (18 fixtures, 17
extracted, 0 writes, detector 4/4).

### S9 — the gate, and where `F-ID-1` lives (2026-07-29)

**The open decision was taken: `treepo-vcs` reads `user.email`, `treepo-id` stays pure, and
the architecture document was amended rather than the roles bent to fit it.**

`treepo-id` is `no_std` with no I/O, and that is not a portability preference — it is what
makes the crate *unable* to acquire a repository dependency or a filesystem read on some
later afternoon. Giving it a `std` feature to open one config file would have traded a
structural guarantee for a file placement. So the crate that already opens repositories and
already reads `.mailmap` does this one too, and hands over an `AuthorKey`.
`.planning/architecture-treepo.md` carries the amendment under the identity feature and in
the file tree; only the *config read* moved, and the identity policy is entirely in
`treepo-id::policy`.

#### `F-ID-1` already existed, and that is why it got written twice

Phase 1 resolved the viewer in a private `self_author_key` inside `log_pass.rs` — correct,
tested through `is_self`, and invisible. A named feature with no file of its own is a
feature the next person re-implements, and the next person did: a duplicate landed in
`extract.rs` before the sabotage pass found it. A sabotage that *passes* is the signal —
removing the new marking changed nothing because the old one was still running.

Consolidated rather than shipped twice: the logic is in `self_ident.rs`, `log_pass` calls
it, and `extract.rs` carries a comment saying where it happens. Two things improved on the
way. `user.name` is now read and passed to `.mailmap`, because a mapping may be keyed on the
full `(name, email)` pair and the old helper passed an empty name — a `Name <canonical>
<alias>` rule would never have matched the viewer. And `IdentityScope` reports which config
file won, so a user who cannot work out why treepo does not recognise them can be told where
to look (`N2`).

`AuthorTable::mark_self` was written for the duplicate and then removed — an uncalled public
method is API surface with no caller. What survived is its documentation, moved onto
`AuthorEntry::is_self` where anyone reading the field will see it.

#### `is_self` is the one manifest field that depends on who is looking

Worth recording because it has a consequence nobody would look for. Every other value in a
manifest is a property of the repository; this one comes from the local git config. It
reaches no generated value — `treepo-gen` never reads `AuthorTable`, verified by grep, so no
skeleton and no digest can move with it — but:

* changing `user.email` and re-extracting produces a different `manifest.bin`. Correct, and
  outside `AC-MAN-1`'s "unchanged repository state".
* **a manifest shared through `F-MAN-11` would carry a bit saying "the sender is one of
  these keys"**, which against a public repository's author list is enough to name them.
  `package.rs` must clear it. That is Phase 10's job and it is now written on the field.

#### The viewer's own name is not carried anywhere, and that is a deliberate tightening

`AC-ID-1` protects contributors "other than the user", so the PRD permits showing the
viewer's real name. `Identification::Yourself` carries none and renders as `You`, and
`self_ident` discards `user.name` after the mailmap lookup.

What that buys is real: a rendered tree — and therefore an export, and therefore a
screenshot someone posts — says "You" where the viewer appears, so publishing one does not
announce who made it. The viewer already knows their own name, so nothing is lost.

#### How `F-ID-5` is enforced rather than promised

> One setting governs both live view and exports. It is not separable.

Three properties, and only the third is discipline:

1. **`IdentityView::identify` takes no policy argument.** The policy is fixed at
   construction, so there is no call site at which an exporter could ask for a different
   answer than the renderer got. "Not separable" is the absence of a parameter.
2. **A pseudonymous view holds no real names at all.** `RealNames` is stored only by
   `IdentityView::revealed`; the default constructor leaves the field `None`, so there is
   nothing in the structure a name could come out of. `AC-ID-1` survives a bug in the
   display path rather than depending on there being no bug.
3. **Both consumers must be handed the same view.** Architectural, and it lands with Phase
   10's settings where the view is built once per repository from `config.json` (`F-ID-6`).

`RealNames`'s `Debug` prints `RealNames(4 withheld)`, for the reason `AuthorShare`'s prints
a bucket: a debug dump, a log line, or a panic message that happens to include a view must
not become the disclosure the crate exists to prevent.

**`treepo-model` carries no names, so `RealNames` cannot be read out of a manifest** — it
has to come from the repository. A stored or shared manifest therefore cannot be
de-anonymized by toggling a setting; reveal needs the same access that would let someone run
`git log` anyway.

`Identification` is an enum rather than a string so a consumer can act on the *kind* without
inspecting a policy — `F-ID-8` and `AC-EXP-2` are the motivating case, where an exporter
writing file metadata refuses anything that is not `Pseudonymous` with a `match` rather than
with a flag it has to remember to check.

#### `AC-ID-1`, end to end

The unit tests hold the gate; they prove it is shut, not that nothing routes around it.
`crates/treepo-vcs/tests/privacy.rs` extracts three real fixtures whose contributors' names
and addresses are known — sixty of them in `many-authors` — and asserts those strings appear
nowhere in the manifest's debug output or in any rendered identification. Two layers,
failing for different reasons: a name in the manifest means extraction smuggled one into a
string field; a name in the rendering means the gate was bypassed.

`revealing_is_what_makes_the_default_view_worth_asserting` is the guard against the whole
file being vacuous — if a revealed view produced pseudonyms too, every assertion about the
default view would prove nothing. Same lesson as S8's vacuous order test, applied before it
could bite.

The test lives in `treepo-vcs` with `treepo-id` as a **dev**-dependency, because the
dependency must not point the other way. `dep-guard` walks `--edges normal,build`, so `N6`
is untouched — and `treepo-vcs` still reports 131 packages, which is the proof.

**`F-ID-7` is tested on a real repository, not a mock.** Every commit in the `mailmap`
fixture is authored by someone other than the configured identity, which is the shape of
every repository a user merely clones: the viewer is configured, `self_author()` is `None`,
and every contributor including them is pseudonymous. Nothing errors, because nothing is
wrong.

#### What the gate said

Three sabotages, each producing the named failure:

| Sabotage | Caught by |
|---|---|
| `is_self` never set in `log_pass` | `a_contributing_viewer_is_marked_in_the_manifest` (`None` vs the key) and `no_rendered_identification_carries_a_contributor_identity` |
| `IdentityView::revealed` ignores its names | `revealing_is_what_makes_the_default_view_worth_asserting` |
| Extraction appends the viewer's address to `treepo_version` | `no_manifest_carries_a_contributor_identity`, naming the string |

A fourth was attempted and could not be written: there is no way to make a pseudonymous view
emit a real name, because it holds none. That the sabotage is hard to author is the property
`F-ID-5` asked for.

Local gate green: fmt, clippy `-D warnings`, 464 tests, dep-guard (6 crates clean),
`cargo deny`, readonly-audit (18 fixtures, 0 writes), determinism unchanged at `7bd8896f…` —
correct, since nothing in the generative path moved.

### S10 — materials, first slice: families and the normalization (2026-07-29)

`F-MAT-1` and `F-MAT-3`. `treepo-model::material` (the handoff types), `treepo-gen::material`
and `treepo-gen::normalize` (the generation), `assets/params/materials.ron` (the table), and
one new determinism primitive. `F-MAT-2`'s mosaic arrangement, `F-MAT-4`'s gradient and
`F-MAT-5`/`F-MAT-6` are later slices.

#### The prerequisite nobody had written down: there was no logarithm

`F-MAT-3` opens with "size normalization is **logarithmic**", and
`#![deny(clippy::float_arithmetic)]` puts `f64::log2` out of reach in every crate that would
call it. Phase 0 built `sqrt` and a trig table and stopped there. So `Fx::log2_u64` landed
first, beside `sqrt` and for the same reasons.

It takes a `u64` rather than an `Fx`, and that is the decision worth recording. Q32.32 tops
out near 2.1 × 10⁹ — a repository above two gigabytes would saturate on the way *in*, and
every such repository would then normalize to the same budget. The narrower signature is the
one with no failure mode.

The algorithm is `log2(v) = e + log2(m)`: `u64::ilog2` for the integer part, then the
mantissa squared thirty-two times, one fraction bit per squaring. Accuracy is **one ulp of
Q32.32**, measured against `f64::log2` over a sweep rather than asserted — the first draft of
that test asserted hand-computed constants, they were wrong by 28 ulps, and the
implementation was blamed for it before the arithmetic was checked. The test now measures a
bound instead of pinning five numbers, which is the property that actually matters.

The `fixed` probe is byte-identical at `ced4738c…` afterwards, which is the evidence that
adding a method perturbed nothing.

#### The family question was escalated, and the answer improved the design

**Decided: dominant category sets the family, the runner-up veins it — with a distinction
between *secondary* and *subordinate* that the codebase turned out to be already drawing.**

Three readings were on the table. A threshold ladder ("asset-heavy claims the limb at 35%")
is closest to `F-MAT-1`'s literal wording and produces a worse picture: a directory that is
55% source and 45% images would be drawn as though it held no source, when the honest answer
— that it is both — was available. Winner-takes-all is simplest and makes a 51/49 directory
read as pure, flipping family on one commit, which `F-GROW-8` would then play as a full
material-family transformation for a trivial change.

Blending won, and it disposes of the tie problem for free.
`SizePrimitives::dominant_language` returns `None` on an exact tie precisely because breaking
one "would flip on the next commit" — but a family cannot return `None`, since every limb
needs a material. Under blending the flip is invisible: at an exact tie the weight is 1.0, so
swapping which family is primary and which is the vein produces very nearly the same limb.
Declaration order breaks the tie and nothing rests on it.

**The secondary/subordinate distinction is the part that was not in any of the three
options.** A limb whose bytes are part code and part image *is* a mixture. An `F-SKEL-7`
container standing for a directory it does not draw *holds* materials without being made of
them — the variety inside it is inventory, not surface. Those are different pictures of
different facts.

The distinction did not need a new flag, because `NodeRole` has drawn exactly this line since
Phase 3: `Group` (several paths, each still drawn) against `Aggregate` (several paths, and
this node *is* their representation), with a table in `segment.rs` explaining that collapsing
them would make `F2`'s "fewer, thicker limbs" indistinguishable from "this directory and all
its contents". `Composition` is that same line at the material layer, and the role selects
which arm applies. A container of pure documentation is still holding rather than being.

One consequence falls out that is worth expecting: `Blended` keeps the largest two families
and `Subordinate` keeps the whole mix. Three interleaved materials on one limb read as mud;
an inventory that dropped its tail would be answering `F-INSP-3` with a summary of itself.
Nothing is destroyed either way — the full category breakdown stays in the `SizePrimitives`
it came from, which is what `F-INSP-5`'s why-panel reads.

#### `F-MAT-3` is one requirement and two mechanisms

`Normalize::budget` normalizes a *path*; `Normalize::allocate` normalizes *contributors*.
They are in one module because `F-MAT-3` states them in one sentence and both exist to stop a
large thing erasing a small one.

The budget is log → soft clamp → floor, in that order, and the order is load-bearing:
clamping before flooring means the clamp shapes the top of the range without ever being able
to push something below the floor. The clamp reuses the piecewise-linear shape
`TrunkParams::support` already applies to carried limb width — the same problem twice, and two
differently-shaped clamps would be two opinions about what "too big" means.

`validate` refuses a knee/beyond pairing that would let *any* `u64` byte count reach a full
budget. Without that check the `min(ONE)` in `budget` becomes a real ceiling and the largest
paths all draw identically, which is the same failure `support_beyond == 0` is refused for,
arriving by arithmetic instead of by configuration.

**`AuthorShare::allocate` finally has a caller**, and its documentation turned out to be a
specification: "rounds down, so `F-MAT-3`'s minimum quota is applied on top by the caller —
a 2% contributor keeping visible presence (`AC-MAT-2`) is a material-policy decision, not an
arithmetic one." `Normalize::allocate` is that policy.

Two tiers. At or above `significant_ppm` a contributor gets their proportional share **or**
the quota, whichever is larger; below it they get their share, which may round to zero. The
table is refused above 2%, because `AC-MAT-2` names two percent and a table is not permitted
to tune its way out of an acceptance criterion.

The total may exceed the cells offered, and **that is the answer rather than a failure**.
Capping the guarantee breaks `AC-MAT-2`; dropping contributors to fit requires choosing
*which*, which is the ordering of people `N4` forbids. So the mosaic subdivides further. It
cannot run away: at most `1_000_000 / significant_ppm` contributors can be significant — a
hundred at one percent — so the overshoot is bounded by that count times the quota,
independently of how many people touched the path. PRD §6's thousand-author repository is
held to that bound by test rather than by hope.

Cells left over are **not** redistributed. `F-MAT-2` makes ownership "accent, vein, and mosaic
treatment **over** the primary material", so an unclaimed cell already has something to be.
Handing the remainder to the largest holder would be both a ranking and a small lie about who
wrote what.

#### What the tests caught, and what caught the tests

Three normalize tests failed on first run, all fixture bugs rather than code bugs, and one
root cause: `AuthorKey::from_email` case-folds (`F-EXT-9` — one human with `Foo@` and `foo@`
is one contributor), so `index.to_le_bytes()` collides wherever a byte lands on `A`–`Z`
against `a`–`z`. Exactly 3 collisions in `0..100` and 104 in `0..1000`, which is why a
thousand-author fixture produced 896 contributors. Fixtures now build decimal emails; digits
do not case-fold. **A fixture that quietly produces fewer contributors than it asks for reads
as an allocator that dropped people**, which is a bad afternoon to have later.

The fourth failure was a vacuity guard doing its job. `holders_iterate_in_key_order_not_by_size`
asserts key order, which proves nothing if key order happens to equal size order — so it
computes the size order and asserts the two differ. On the original fixture (1/998/1 of a
thousand lines) only *one* contributor survived allocation, and a one-element list is trivially
both orders. Rewritten over eight contributors with eight distinct shares: two orderings of
three items coincide once in six, which is a flaky test; once in forty thousand is not.

#### `AC-DET-1` names materials, so the harness has a `material` probe

> Two Grow runs on identical repository state produce byte-identical serialized skeletons,
> **materials**, and enrichment placements.

Synthetic mixtures rather than the corpus, in the same spirit as the `pseudonym` and
`author-color` probes: what it covers is arithmetic, and sampled inputs exercise the extreme
magnitudes and the exact ties no real repository reliably contains. Budgets sweep every power
of two from one byte to sixteen exabytes; every ordered pair of categories is sampled at a tie
and either side of one; both node roles, because the role selects blended against subordinate
and a platform disagreeing about one would not necessarily disagree about the other.

`Fx::log2_u64` is why the probe earns its place — the newest primitive in the generative path
and the only one computing a transcendental without the trig table. If `RISK-2` had a second
home this would be it.

The corpus-wide material digest joins the `skeleton/*` lines when a walk over the skeleton
exists to produce one. That walk is the next slice.

```
material      fc22bc3bca161458671dea777a3e5e326be676d7bc05944df42ecb4e76f1b6ca
overall       2c83c2bdda407e540ef4b8b3acfced6147bd959a6b3372fdcb3a20bab800142a
```

`overall` moved from `7bd8896f…`; **all seven prior probes and all eighteen `skeleton/*`
lines are byte-identical**, and the one new probe line is the whole difference. As at S8 and
Phase 0, one machine proves only the within-platform half — `AC-DET-2` proper needs the CI
run on this commit.

Local gate green: fmt, clippy `-D warnings` (workspace, all targets), 492 tests, dep-guard
(6 crates clean, `treepo-gen` still 14 packages — no new dependencies), `cargo deny`
(advisories/bans/licences/sources ok), readonly-audit (18 fixtures, 17 extracted, 0 writes,
detector 4/4), determinism reproducible over 3 runs.

---

## Next

**Phase 4, continued — the material walk, then `F-MAT-2`.** The first slice (S10) built the
families and the arithmetic; what it has no caller for is a walk over the skeleton.

The next slice is **`materialize(manifest, skeleton, table) -> MaterialMap`** — one pass over
`Skeleton::nodes()`, each node's `role.anchor()` looked up in the manifest, `material_of`
called with the role. Three things fall out of it and nothing else can start until it exists:

1. **`AC-MAT-2` on the real T2 fixture**, which is a Phase 4 end condition and is currently
   held only by unit tests over synthetic shares. The 2% contributor has to keep presence on
   an actual repository.
2. **A corpus-wide `material/*` digest** beside the `skeleton/*` lines, which is the half of
   `AC-DET-1` the synthetic `material` probe does not reach.
3. **`F-MAT-2`'s mosaic** — arrangement rather than allocation. `Normalize::allocate` already
   answers *how many cells each contributor holds*; what is missing is which cell is where,
   and that wants a skeleton to lay cells out on.

Note the aggregate case has a wrinkle worth expecting: an `AggregateNode` carries its own
`bytes`, but its *category mix* has to be rolled up from the paths beneath its `members`,
which the manifest holds and the aggregate does not. `material_of` already takes `bytes`
separately for this reason; the rollup itself is the walk's job.

Also outstanding for Phase 4's end conditions: **`AC-MAT-3`'s "no `treepo-model` type exposes
an ordered contributor collection or a share as a figure"** — enforced at the type level
already (`AuthorShare` implements neither `Ord` nor `PartialOrd`, with `compile_fail`
doctests) and still needing a test that says so at the crate level rather than the type's.

**`blend_floor` is the one material number set by argument rather than by looking.** 80 per
mille is a reasoned guess at where a vein reads as deliberate; it cannot be judged until
materials have an appearance, and it is the first thing the silhouette lab should be pointed
at once they do.

Three things carried in from the M0 tuning campaign, all recorded rather than resolved:

1. **Family C — `length_ratio` / `width_ratio` are unjudged.** Deliberate. They govern taper
   character, and Phase 4 changes what a limb *looks* like; tuning them against a
   line-and-thickness placeholder would be tuning against the wrong picture.
2. **The crown is lopsided on wide fans** — one long bare arm with no weight above it. The
   trunk rework freed `trunk.fan` to be lateral character alone, and this is the first thing
   that character should be retuned against.
3. **`branch_capacity` base=3, monorepo vs small repository** (the `needs_code` finding).
   Real design debt. Not an M0 exit condition — `AC-SKEL-1` asks that clean and messy differ,
   not that every repository size reads equally well.

The lab and the `qa/` session schema stay; they are Phase 4's instrument for the same
one-family-at-a-time loop, against materials instead of geometry.

Carried forward, neither blocking:

- **`AC-THR-2`'s two seconds for the dirtiness overlay is still unmeasured** (since Phase 1).
  The pinned repositories make it measurable — a `status` row in `cargo xtask budget`, one
  function.
- **`F-CORP-3`'s read-only fixture.** See above.
- **`.github/workflows/budgets.yml`** — architecture puts it in Phase 12, still the right place.

### `LICENSE-THIRD-PARTY.md` — closed 2026-07-27

Promoted from `docs/workspace/LICENSE-THIRD-PARTY.md` to the repo root after verifying the
resolved graph: the only MPL-2.0 crate is `uluru` 3.1.0 (via `gix` → `gix-pack` →
`pack-cache-lru-static`). The notice pins the crate version and `Cargo.lock` checksum,
records the feature path, and embeds the MPL-2.0 text as shipped with that crate.
`deny.toml` points at the file. Full permissive-dep inventory remains Phase 12 packaging.

`StatusOptions::max_paths` is still a defensible guess rather than a measured number; the
`AC-THR-2` item under "Next" is where that gets closed.
