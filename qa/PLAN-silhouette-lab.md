# Plan: Silhouette lab + forced structural tuning

**Status:** open  
**Opened:** 2026-07-28  
**Close when:** structural fixes S1–S3 done, HTML lab MVP (S4) usable, and the first full tuning campaign has promoted evidence into `assets/params/lsystem.ron` (or recorded a deliberate decision not to).  
**Related:** `design/l-system-parameterization.md` §6 · PRD M0 / AC-SKEL-* · `claude-progress.md` “What the pictures say”

When this work is finished, mark **Status: closed**, add a one-line close note below, and leave the file in place as a historical contract (or move to `docs/workspace/` if archiving).

---

## Context

`tools/m0-silhouette` already draws deterministic line-and-thickness PNGs from any corpus fixture, pin, or path (`--table` is AC-SKEL-4). Four eye findings are recorded in `claude-progress.md`; none are fixed by “slide numbers on the current model.”

We want a **cheap HTML harness** (sliders, click-to-render, non-overwriting gallery, export findings) on a **gitignored session workspace**, one parameter family at a time, multi-subject views, handoff artifacts for the agent.

Those two tracks must be ordered carefully: **structural fixes first (or interleaved so each fix is visible in the lab), then number tuning in the lab.** Sliding `width_ratio` cannot fix “every limb restarts at full `base_width`”; sliding `basal_length` alone fights a `stem_width` rule that is inconsistent by construction.

---

## Goals

1. Make silhouettes *structurally* tree-like enough that eye judgment is about parameters, not bugs.
2. Give a tight local loop: experiment table → one knob → render multi-subject strip → keep history → export findings.
3. Keep product/determinism boundaries clean: lab artifacts untracked; promoted `lsystem.ron` + short decision notes tracked.
4. Unlock AC-SKEL-1 judgment with a clean-vs-messy comparable pair.

## Non-goals

- Full Bevy / real-time continuous slider (no “live GPU tree”).
- Separate parallel repo for the lab.
- Materials, Thrive, enrichment forms (Phase 4+).
- Committing PNG galleries to git.

---

## Architecture decision: HTML lab = thin UI over in-process render

| Piece | Role |
|-------|------|
| `tools/m0-silhouette` CLI | Unchanged default: batch corpus → `target/m0-silhouette/` |
| `m0-silhouette lab` (new subcommand) | Local HTTP server + static HTML/JS; **render in-process** via existing `draw`/`png` + `treepo_gen::grow` |
| `qa/` (mostly gitignored) | Sessions, experiment tables, render history, exports |
| `assets/params/lsystem.ron` | Only updated when a finding is **promoted** |

Stack preference: keep deps minimal (stdlib HTTP or one small crate). Static assets under `tools/m0-silhouette/lab/` (tracked). Session data under `qa/sessions/` (ignored).

---

## Session / export schema

See [`README.md`](README.md) for the authoritative folder layout and finding JSON shape.

---

## Structural fixes (forced order) — S1–S3b

1. **Width falloff across the composition boundary** — child limb start width must inherit taper from parent geometry, not only table `base_width`.
2. **Upward tropism as data beside droop** — table row + apply so multi-generation headings stay upright.
3. **Reconcile `basal_length` with `stem_width`** — rule so multi-primary roots are short stems, not pancakes; AC-SKEL-2 still holds.
3b. **Trunk column** — S3 made the base *consistent* and it still read as an oversized seed, because the co-origin construction gives a primary nowhere to leave *from*. Replaced by a pipe-model support column grown as primary internodes; `docs/workspace/trunk-pipe-rework.md`, promoted into `design/visual-construction.md` v2.1.

That S3 → S3b sequence is the forced order working as intended, and worth recording: the number was inconsistent *and* the model was wrong, and only fixing the number is what showed which. The plan's own warning — that sliding one row cannot fix a construction — applied one level deeper than it was written for.

## Lab + campaign — S4–S7

4. HTML lab MVP (family lock, one-field sliders, non-overwrite renders, export).
5. AC-SKEL-1 clean/messy comparable subjects.
6. Tuning campaign (user drives; agent promotes).
7. Skeleton digest in determinism harness / budget row as needed.

---

## Sprint checklist

| Sprint | Scope | Status |
|--------|--------|--------|
| **S0** | `qa/` contract: README, subjects, gitignore, this plan | **done** 2026-07-28 |
| **S1** | Compose-boundary width inheritance | **done** 2026-07-28 (`4d01807`) |
| **S2** | Tropism row + apply | **done** 2026-07-28 (`4d01807`) |
| **S3** | Basal vs stem rule + validate | **done** 2026-07-28 (`4d01807`) |
| **S3b** | Trunk column: pipe support + primary internodes | **done** 2026-07-28 |
| **S4** | HTML lab MVP | **done** 2026-07-28 (`m0-silhouette lab`) |
| **S5** | AC-SKEL-1 subject pair | pending |
| **S6** | Tuning campaign via lab | **partial** 2026-07-29 — first high-confidence promote into `lsystem.ron` from session `20260728_234906_lab` (see progress). Joint re-judge still open. |
| **S7** | Determinism / budget gate hardening | pending |

---

## Success criteria

- [ ] Structural: taper across limbs; upright bias; non-pancake multi-primary stem; AC-SKEL-2 still holds by eye.
- [ ] Lab: one command, multi-subject strip, non-destructive render history, export package for agent.
- [ ] Tuning: each family has ≥1 exported finding; promoted table changes are evidence-backed in progress.
- [ ] AC-SKEL-1: clean vs messy comparable pair judged in lab.
- [ ] Gates remain green; determinism digests updated only when geometry contract intentionally changes.

---

## Close note

_(fill when closed)_
