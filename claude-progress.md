# treepo — progress

> Read `.planning/campaign-treepo.md` for the phase list and `.planning/architecture-treepo.md`
> for the file tree and decisions. This file records only where the build actually is.

**Last updated:** 2026-07-27 · **Phase 0 complete and closed** · **Phase 1 complete and closed**
· **Phase 2 in progress**

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
| `tests/persistence.rs` (`AC-MAN-1`, `AC-MAN-4`, `F-MAN-8`) | **done** — 5 tests |
| `F-CORP-3`: `read-only` fixture | **missing — see below** |

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

### Two gaps found, both recorded rather than quietly patched

**Nothing composes a `Manifest`.** Phase 1 produced `Structure` and `History` and stopped;
`tests/persistence.rs` assembles them in about fifteen lines of field copies. That is enough
for `AC-MAN-1`, whose question is about the *primitives* and those come from the real passes.
It is not enough for the product, and the composition belongs in `treepo-vcs`, whose own module
documentation already claims to be "turning a repository into a `Manifest`".

**`F-CORP-3`'s read-only fixture still does not exist.** Restricted permissions or a read-only
mount. It is about `AC-MAN-2` and `F-ASSOC-7` rather than identity, and it carries real platform
weight: `chmod` on unix, ACLs on Windows, and a builder that can clean up after itself. The
`two-clones` case is now covered — `tests/persistence.rs` builds it — so this is the last one.

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

## Next

Every Phase 2 end condition is met. What remains before the phase can be called closed is
`identity.json` — `F-MAN-2` lists it in the store layout and `F-MAN-3` says it records "both
the resolved key and which tier produced it, so a user can see why two checkouts did or did not
share a store". `Resolution` already carries exactly that; nothing writes it yet. It is a short
sprint on top of the atomic writer that now exists, and it closes `N2`'s inspectability promise
for the one file a person would actually open.

After that, Phase 3 — skeleton generation, and **M0 exit**.

### `LICENSE-THIRD-PARTY.md` — closed 2026-07-27

Promoted from `docs/workspace/LICENSE-THIRD-PARTY.md` to the repo root after verifying the
resolved graph: the only MPL-2.0 crate is `uluru` 3.1.0 (via `gix` → `gix-pack` →
`pack-cache-lru-static`). The notice pins the crate version and `Cargo.lock` checksum,
records the feature path, and embeds the MPL-2.0 text as shipped with that crate.
`deny.toml` points at the file. Full permissive-dep inventory remains Phase 12 packaging.

Carried forward, neither blocking:

- **`AC-THR-2`'s two seconds for the dirtiness overlay is still unmeasured.** A synthetic
  eight-file fixture says nothing about it, and `StatusOptions::max_paths` is a defensible guess
  rather than a measured number. The pinned repositories now make it measurable — a `status`
  row in `cargo xtask budget` would close it, and it costs one function.
- **`.github/workflows/budgets.yml`** — architecture puts it in Phase 12, and that is still the
  right place. Worth revisiting only if extraction cost starts moving between sprints.
