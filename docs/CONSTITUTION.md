# treepo — Project Constitution

**Version:** 1.3  
**Status:** Ratified 2026-07-27. No constitutional questions outstanding — see §10.  
**Last updated:** 2026-07-27 *(R7 supersedes R3; N1 restored, N2 extended)*

This document holds the enduring intent of the project: why it exists, what it must always
be, and what it must never become. It is deliberately abstract. Concrete capabilities,
acceptance criteria, and delivery sequencing belong to the companion PRD, which does not
yet exist.

Where any document disagrees with this one on *intent*, this document governs. Where they
disagree on *detail*, the relevant design document governs. See
[`README.md`](README.md) for the full documentation map.

> **On the name.** `treepo` is the project's name, locked 2026-07-27 (§10 R5). It is no
> longer provisional in any document, and it also fixes the application data directory and the
> opt-in in-repository directory as `treepo` / `.treepo/` (R7). The identity described in §2
> remains the thing that matters; the name is now simply settled.

---

## 1. Purpose & Vision

treepo turns a software repository into a single living world-tree, grown from the
repository's real structure, size, age, churn, ownership, and activity.

The tree is not a metaphor decorating a dataset. It is *grown from* the data. Every limb,
every material, every glowing tip traces back to something measurable about the code.

**The goal is cognitive and aesthetic, in that order of foundation and reverse order of
priority.** A developer should be able to look at their repository's tree and *recognize
it* — to point at a heavy, restless limb and know that is the audio encoding path, or at a
patinated inward mass and know that is the UI layer nobody has touched since spring. That
recognition should arrive through the eye, not through a legend.

We are building an instrument of intuition. Its output is not a report or a
recommendation. Its output is a *feeling for the shape of a codebase* — one that is
accurate because it is derived from real measurements, and memorable because it is
beautiful and strange.

The passage of time is the project's central theme. Age, churn, and activity are what make
a repository a living history rather than a directory listing, and treepo exists to make
that history visible.

---

## 2. Product Identity

**treepo is a toy in the serious sense: something you play with, which teaches you
something true.**

It is not a game — there is no objective, no progression, no score, nothing to win. It is
not an analytics product — it renders consequences but issues no verdicts. It sits closer
to an aquarium, an orrery, or a weather map than to a dashboard or a linter.

The toy framing is primary, and it is a distribution decision as much as a design one:
treepo is built and sold as a consumer desktop product, not as developer tooling that
happens to be pretty. Three consequences follow, and they are constitutional rather than
tactical:

- **The polish bar is a shipped product's, not a tool's.** Visual quality, first-run
  experience, and moment-to-moment feel are not finishing work applied at the end. They are
  the product.
- **Nothing essential may require a terminal.** Pointing treepo at a repository, watching it
  grow, and sharing the result must be achievable entirely within the application.
- **The first Grow is the front door.** Watching a repository's whole history play out is
  the strongest thing treepo does, and it is what a new user must encounter first.

None of this softens §4 or §5. A consumer surface does not license a game's objectives, an
analytics product's verdicts, or a single decorative pixel.

Three commitments define the identity and should survive any amount of feature churn:

**It is grown, never drawn.** The tree's form is the repository's form. No hand-authored
composition, no aesthetic overrides that contradict the data. When the result is ugly, the
repository is ugly, and that is worth seeing.

**It is specific, never generic.** The tree should look like *this* repository made flesh —
organic, sometimes alien, occasionally overgrown. A stock oak with tidy green leaves is a
failure condition regardless of how pretty it is.

**It is alive, never static.** Between structural changes the world continues to breathe,
drift, and move. A frozen frame is a broken product even when the frozen frame is correct.

The first era of the product goes deep on one scene — a large, lively, slightly otherworldly
world-tree menagerie — rather than broad across many metaphors. Depth in one metaphor is
the identity; breadth across many is a different product.

---

## 3. Core Principles

These are the durable rules of thought. They constrain design decisions without dictating
implementations.

### P1 — Every visible pixel is accountable to data
If something is on screen, a primitive explains why. Decorative filler with no referent is
the one thing the visual language does not permit. This is what separates treepo from a
generative art piece that happens to take a repository as a seed.

### P2 — Determinism is a feature, not an optimization
The same repository state always produces the same tree, on any machine, at any time.
Generation is seeded hierarchically from path hashes, never from wall-clock time or ambient
machine state. This matters far beyond caching: a tree you cannot return to and recognize
is a picture, not a mental model. Recognizability over time *is* the product.

### P3 — Measurement and meaning are separate layers
Primitives stay close to measurable fact. Interpretation — what a value *means*, whether it
reads as vitality or instability, how it becomes color and motion — lives in a distinct
contextual rules layer. Baking interpretation into the data layer makes the system brittle
and dishonest. Keeping them apart lets both the data and its meaning respond to context.

### P4 — The tree depicts; it does not judge
treepo renders consequences honestly: neglect looks neglected, churn looks restless,
abandonment accumulates patina. It does not score, rank, grade, or advise. The distinction
is absolute — depiction invites the user's own judgment, while scoring substitutes for it.
A user should be able to disagree with what they see and still trust that it is true.

### P5 — Mess is signal
Chaotic repositories produce strange, heavy, tangled trees. Clean repositories produce
clearer, more harmonious ones. The system never normalizes reality into prettiness. An
enormous monorepo is entitled to produce an enormous, unwieldy tree, and its owner is
entitled to see that.

### P6 — Legibility bounds detail; honesty bounds data
These two obligations meet at the level of *presentation*, not data. Detail is quantized
past a practical depth threshold — deep subtrees aggregate into proportional containers, and
at the limit a single object may represent an entire directory. But aggregation is a
rendering concern. The underlying data is never pruned, normalized, or simplified to make
the picture more comfortable.

### P7 — Nothing important is erased
Scale disparities are compressed, never flattened to zero. Every path that survives
filtering receives a visible pixel budget; every contributor above the significance
threshold keeps visible presence in the material. A single pixel of a person's color
carries real meaning. Majority-rule erasure of small files and minor contributors is
forbidden even when it would be visually convenient.

### P8 — Local rules, global emergence
Visual behavior arises from simple local decisions — a material cell prefers distal
positions as its recency rises, a high-churn region emits more particles — rather than from
global optimization passes. Emergence is the design method, not an accident of it.

### P9 — Change is an event, never a refresh
When structure changes, the user watches it happen. Mass migrates, limbs thicken, material
flows, old growth is reclaimed. Threshold crossings are celebrated as transformations, not
executed as silent swaps. The moment of change is the product's highest-value moment, and
the first time a repository's whole history plays out is its single best one.

### P10 — Liveliness is continuous and cheap; truth is occasional and expensive
The two rhythms are permanently separate. Establishing what is true about the repository is
rare, heavy, and may be slow. Expressing it is constant, light, and must never stutter. Any
proposal that moves expensive analysis into the continuous loop is rejected on principle,
not on benchmark.

---

## 4. Non-Negotiable Constraints

Bright lines. Violating one of these is not a trade-off to be weighed; it is a defect.

> N1, N2, N4, and N9 do not appear in the source drafts. They were inferred from product
> intent, asserted here because they are load-bearing, and ratified by the owner on
> 2026-07-27. Their derivation and scope are recorded in §10.

**N1 — The repository is read-only.**  
treepo writes nothing into a repository. Not source, not history, not staging, not branches,
and not its own data (§10 R7 — that lives in application data). The sole exception is a
directory the user has explicitly asked treepo to create there, and even then treepo never
modifies a file it did not create — the repository's own `.gitignore` included.

treepo also never executes code found in the repository — no build scripts, no git hooks, no
plugins, no project tooling. Analysis reads bytes and git metadata; it does not run the
project.

**N2 — Repository data stays on the user's machine.**  
Source content, paths, contributor identities, and derived primitives never leave the
machine except through an artifact the user explicitly chooses to export. No telemetry
containing repository data. The application is fully functional offline. Storefront and
platform integrations — achievements, cloud sync, crash reporting, analytics — are never a
channel for repository content, and the product must remain complete without them.

Under R7 that data accumulates locally, in one place, across every repository the user has
ever opened. Such a store must be enumerable and deletable from within the application: the
user can see what treepo holds and remove any of it, or all of it, without hand-editing
directories. Data kept on someone's behalf that they cannot find is not local-first in any
sense that matters.

**N3 — Generation is deterministic.**  
No wall-clock input, no unseeded randomness, no machine-specific state anywhere in the
generative pipeline. All variation derives from hierarchical path-hash seeds.

**N4 — Contributor data is depicted, never ranked.**  
Ownership and contribution patterns appear as material families, color mosaics, accents, and
inhabitant associations. treepo never produces ranked lists, leaderboards,
contribution-percentage scoreboards, or any ordering of people by volume or importance. This
holds even where the extracted primitives would trivially support it, and it is what
preserves the collaborative, non-judgmental character of the living tree.

The line runs between *proportion* and *score*. Using contribution share to size a mosaic,
allocate material, or seed an accent is depiction, and is the intended use. Surfacing that
same share as a figure, a rank, or an ordering is a scoreboard — including in tooltips,
inspection panels, and exported artifacts.

**N5 — The world remains coherent and navigable.**  
The tree is a connected, readable structure at all times. Local cellular and particle
behavior is permitted where it produces organic, observable change; free-form falling-sand
physics and destructive simulation are not, because they would destroy the structure the
user is trying to read.

**N6 — Structural work never enters the continuous loop.**  
Full topology rebuilds, repository scans, and heavy material simulation belong exclusively
to the rare, expensive phase. The continuous phase animates what already exists and stays
interactive on the reference desktop target.

**N7 — Appearance derives only from primitives.**  
No visual property is set by hand-authored exception, hard-coded special case, or aesthetic
override that contradicts the extracted data. The path from measurement to appearance may be
long and rule-laden, but it is never bypassed.

**N8 — Desktop-native.**  
Windows, macOS, and Linux desktops are the target. The product is a local application, not a
hosted service, browser tool, or multi-user system.

**N9 — Contributors other than the user are pseudonymous by default.**  
The person running treepo may identify themselves. Every other contributor is represented by
a stable pseudonym and a consistently seeded color, with real names, emails, and handles
disabled by default — in the live view and in every exported artifact alike, on the same
setting. Real identities may be revealed only through an explicit, deliberately unprominent
opt-in that states the consequence plainly and places responsibility with the user.

Pseudonymity is the shipped default, not a privacy mode the user must find. A user who never
opens settings must never accidentally publish a colleague's name, and the product must be
fully expressive without ever resolving a real identity — which it is, because the visual
language depends on stable per-author *colors and materials*, not on names.

---

## 5. Enduring Boundaries

What treepo is permanently not. These are not "later" items; they are outside the product.

- **Not a static analyzer, linter, or refactoring advisor.** No prescriptions, no
  remediation suggestions, no quality gates. Precision sufficient for static analysis is
  explicitly not a goal, and pursuing it would compromise P4.
- **Not a dependency or import-graph visualizer.** Structure comes from the filesystem and
  git hierarchy. A tree whose topology is derived from module imports is a different product
  with a different mental model.
- **Not a productivity or contributor-performance instrument.** See N4. This boundary exists
  because the ownership primitives make the violation easy and the violation would be
  corrosive.
- **Not a game.** No objectives, progression, unlocks, economy, or win state. Playfulness is
  central; game design is not. Shipping through a games storefront does not change this — the
  storefront is a distribution channel, and the toys, instruments, and ambient software
  already sold there are the relevant precedent.
- **Not a general visualization framework.** treepo renders one metaphor deeply. It is not a
  pluggable engine for arbitrary data-to-scene mappings, and additional scene types — if they
  ever arrive — are a later era, not a hidden requirement of the first.
- **Not an editor.** Clicking an element identifies it and maps it back to real code. treepo
  does not become a place where code is written.
- **Not a physics sandbox.** See N5.
- **Not collaborative.** Single user, single machine, single repository at a time.

**Deferred, not excluded:** VCS branch topology, multi-repository views, remote/local twin
views, agent-activity reactions, and additional scene types are all compatible with this
Constitution. They are simply not part of the first era, and nothing in the near-term design
should be contorted to accommodate them.

---

## 6. Resolved Tensions

The source drafts contained genuine conflicts. These are the principle-level resolutions.

**Fidelity at scale vs. readable detail.**  
The drafts insist both that enormous repositories must not be normalized into tidy little
trees, and that branching depth must be capped and aggregated for readability. Resolution:
these operate on different layers. Data fidelity is absolute; presentation aggregates at the
perceptual limit. A monorepo yields a monstrous tree — rendered legibly. (P5, P6)

**Honest depiction of neglect vs. refusal to judge.**  
Age and churn produce readings that can feel negative — scarring, brittleness, decay. This
is not a contradiction with P4 as long as the system depicts a state without asserting a
verdict about it. Patina may read as wisdom or as abandonment depending on context, and
treepo deliberately supports both without choosing for the user. The line is crossed the
moment the product tells someone their code is bad.

**Collect everything vs. no decorative filler.**  
The feature system is deliberately over-complete; the visual language forbids unaccountable
pixels. Resolution: breadth of *collection* implies no obligation to *display*. Extract
liberally — unused primitives cost almost nothing and buy future expressiveness — but show
only what a primitive explains. (P1)

**Authored cinematic moments vs. determinism.**  
Grow transitions are dramatic and designed to be watched, which sounds authored. It is not:
the transition is a deterministic function of the diff between two deterministic states.
The same change always produces the same performance. Drama and reproducibility are
compatible, and any transition effect that cannot be made deterministic is out. (P2, P9)

**Toy vs. tool.**  
The drafts oscillate between "game toy" and "desktop visualization tool" — different
products with different audiences, polish bars, and distribution. **Resolved 2026-07-27 in
favor of the toy: treepo is a consumer desktop product.** Its utility remains cognitive and
its data remains honest — a toy's surface over a tool's substance — but where the two
framings pull in different directions, the consumer product wins. §2 states what that
obliges.

---

## 7. What This Constitution Does Not Govern

Recorded explicitly so that future readers do not mistake silence for prohibition.

- **Technology choices.** The Constitution binds *properties* — determinism, interactive
  liveliness, desktop-native, offline-capable — not the technologies that deliver them. The
  current implementation direction (a Rust ECS engine) is an active decision, not a
  constitutional one, and may change without amending this document.
- **Generative techniques.** L-systems, constraint tiles, and cellular passes are the
  present means to the constitutional ends of organic, deterministic, data-derived form.
  Better means may replace them.
- **Feature scope, prioritization, and sequencing.** All of it belongs to the PRD.
- **Numeric targets and acceptance criteria.** Frame rates, scan budgets, repository size
  ceilings, and parameter tables are PRD and design concerns. This document says the world
  must stay alive; the PRD says at what rate, on what hardware, at what scale.
- **Commercial model.** Pricing, launch strategy, and storefront mechanics are open. The
  *channel* is constitutional (§2); the business built on it is not.

---

## 8. Key Assumptions

Stated so they can be challenged rather than silently inherited.

- The user works on their own machine with local access to the repositories they visualize.
  They are technically literate enough to have a repository — but, given the consumer
  positioning in §2, must not be assumed to want a terminal, a config file, or a build step
  in order to use the product.
- Git is the version control system. A repository without usable git history degrades
  gracefully to filesystem-derived primitives rather than failing.
- Repositories vary by orders of magnitude — from nearly empty to large monorepos — and the
  product must behave sensibly across that entire range, including producing an honestly
  minimal form for a nearly empty repository.
- The user wants to *look* at their repository, not to be told about it. The value is
  perceptual, and the product's success is measured by recognition and returning, not by
  actions taken as a result of using it.
- Contributor identity data (names, email-derived identity, blame attribution) is present in
  the repositories being visualized and is treated as personal data throughout.
- Users will point treepo at repositories they did not write and do not own — public,
  famous, or simply cloned — and will share the results. Under the consumer positioning in
  §2 this is expected behavior rather than misuse, and it is a primary reason N9 makes
  pseudonymity the default rather than an option.

---

## 9. Interpretation & Amendment

This document is stable by design. It is amended deliberately, not incidentally.

- A change to §§2–5 (identity, principles, constraints, boundaries) is an amendment and
  requires explicit owner decision, recorded with its rationale.
- Everything else — design documents, the PRD, architecture — is expected to change
  continuously and must be brought into line with this document, not the reverse.
- When a proposed feature appears to require violating a constraint in §4, the correct
  response is to reject the feature or amend the Constitution openly. Silent exceptions are
  how a product loses its identity.
- Resolved questions are retained rather than deleted, here and in the design set. The
  reasoning is more valuable than the tidiness.

---

## 10. Direction Decisions

Questions that affect product direction and could not be resolved from the drafts. Resolved
items are retained with their reasoning per §9.

### Resolved

**R1 — Positioning and distribution.** *Decided 2026-07-27: consumer toy, storefront-first.*  
treepo is built and distributed as a consumer desktop product rather than as developer
tooling. This raises the polish bar to that of a shipped product, makes the first-run
experience a primary design surface rather than a finishing task, and forbids requiring a
terminal for any essential interaction. It does **not** relax §5 — treepo remains not-a-game
and not-an-analytics-product. Consequences are written into §2; the boundary against game
design is restated in §5.

**R2 — Read-only and local-first.** *Decided 2026-07-27: N2 confirmed as written; N1
narrowed. **N1 subsequently restored by R7** — see below.*  
N2 (repository data never leaves the machine) is ratified in full and extended to cover
storefront and platform integrations, which is newly load-bearing under R1. N1 was narrowed at
this point from "the repository is read-only" to "the repository's source and history are
read-only," solely to permit an in-repository manifest directory under R3. When R7 reversed
R3, that narrowing lost its reason to exist and N1 was restored. The prohibition on executing
repository code was never affected and remains absolute.

**R3 — Manifest location.** *Decided 2026-07-27. **Superseded by R7 the same day.***  
Originally placed the manifest inside the repository at `.treepo/`, to keep it portable with
the repository and available to the agent-annotation path the drafts anticipate — accepting a
narrowed N1 as the cost. Retained here per §9 because the reasoning still explains why the
opt-in path in R7 exists at all.

**R7 — Storage model.** *Decided 2026-07-27: application data is primary; in-repository
storage is opt-in only. Supersedes R3 and restores N1.*

Extracted primitives, caches, committed world state, recorded Grow sequences, and user or
agent annotations live in application data, keyed by a stable repository identity — the
primary remote URL where one exists, falling back to a content-derived local identity.
Opening a repository writes nothing into the working tree.

In-repository storage survives as a deliberate opt-in ("install a local `.treepo/` for this
repository," or export a shareable package), and creating that directory always accompanies it
with an ignore entry scoped to the directory itself.

The reversal is worth its cost for four reasons. It is non-intrusive by default, which matters
under R1 because consumer users will point treepo at repositories they neither own nor
maintain. It gives a cleaner privacy story, since nothing about a repository is deposited into
that repository. It handles repositories the user cannot write to — read-only mounts,
restricted permissions, foreign clones — as an ordinary path rather than a degraded one. And
it provides a natural home for future interactive and user state that never had a good answer
under the in-repo model. The convenience of a co-located manifest is recovered through the
opt-in for the minority of cases that want it.

**Consequences.** N1 returns to its absolute form: treepo writes nothing to the repository at
all unless the user explicitly asks it to. N2 gains a clause on the store this creates — a
local accumulation of data about every repository the user has opened, which they must be able
to inspect and delete. Directory naming remains fixed by R5.

**R4 — Contributor data is depicted, never ranked.** *Confirmed 2026-07-27. N4 ratified.*  
Ownership and contribution patterns are expressed as material families, color mosaics,
accents, and inhabitant associations — never as ranked lists, leaderboards,
contribution-percentage scoreboards, or any ordering of people by volume or importance.

This was flagged rather than assumed because the primitives already specified —
`author_distribution`, `bus_factor_proxy`, `contribution_recency_per_author` — would support
a contributor leaderboard with almost no additional work, and shipping one would contradict
§2 and P4. The proportion/score distinction added to N4 marks where the constraint will
actually be tested: the feature system already partitions visual mass by contribution share,
which is correct, while displaying that share as a number is not.

**R5 — Name.** *Locked 2026-07-27: `treepo`.*  
No longer provisional in any document. The earlier candidates (Worldtree, Repo Arbor, The
Living Manifest, Grove of the Code, srcerer) are historical reference only. The lock also
fixes the storage directory names under R7 and settles storefront identity under R1.

**R6 — Contributor identity in exported artifacts.** *Decided 2026-07-27: pseudonymous by
default, opt-in to reveal. N9 ratified.*

The shipped default policy:

- The person running treepo may see and identify themselves.
- Every other contributor appears as a stable pseudonym plus a consistently seeded color,
  derived from a hash of the author identity (or from platform avatar colors where
  available).
- Real names, emails, and handles of others are **off by default in the live view and in
  every export**, governed by the same setting so the two can never diverge.

The opt-in path is deliberately unprominent — a privacy/sharing setting, scoped per
repository, gated behind an explicit confirmation that states plainly that real identities
will appear in the tree and in anything exported or shared from it, that the user should
enable it only where they have the right to share those identities, and that the consequence
is theirs. Exact wording, placement, and scope belong to the PRD; the default and the
requirement of informed opt-in are constitutional.

Two things make this cheaper than it appears. The visual language already depends on stable
seeded per-author colors rather than on names (see `design/feature-system.md` §8.4), so
pseudonymity costs nothing expressively. And under R1 users will routinely visualize and
share repositories they do not own, which makes a name-revealing default a liability rather
than a convenience.

### Open

None outstanding at the constitutional level.

Remaining questions are tactical and belong to the PRD: repository identity resolution and
store layout under R7, retention and eviction for the local store, the wording and placement
of the N9 opt-in, pseudonym generation and avatar-color sourcing, and how self-identification
is established from local git configuration.

---

## Summary of Decisions Made in Drafting

Synthesized from five design drafts spanning the full outline, the feature system, the
engine architecture, the visual construction thread, and L-system parameterization.

**Resolved at the principle level:** the fidelity-vs-legibility conflict (layer separation),
the depiction-vs-judgment tension (§6), the over-collection-vs-no-filler tension (§6), and
the cinematic-vs-deterministic concern (§6).

**Filled as gaps, all subsequently ratified:** a read-only repository (N1), local-first data
handling (N2), the prohibition on contributor ranking (N4), and pseudonymous-by-default
contributor identity (N9). None appear in the drafts; all four are load-bearing for the
product's identity and far cheaper to adopt now than to retrofit.

**Escalated and decided by the owner on 2026-07-27:** consumer/storefront positioning (R1),
the scope of read-only and local-first (R2), manifest location (R3, superseded), contributor
data depicted but never ranked (R4), the project name (R5), contributor identity in exports
(R6), and the storage model (R7).

**Outstanding:** nothing at the constitutional level. See §10 for the tactical questions
these decisions hand to the PRD.

**Recommended next step:** create the companion PRD via `drafts-to-prd`. The PRD inherits
every constraint in §4 as a given and is where MVP scope, capabilities, acceptance criteria,
and sequencing belong — none of which this document should acquire. Two decisions reshape
PRD priorities in particular: R1 moves onboarding, first-run experience, and export/sharing
up relative to the analytical depth the drafts emphasize, and R6 makes the identity model a
first-class part of the export path rather than a late privacy pass.
