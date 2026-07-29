# qa — silhouette iteration workspace

Local workspace for §6-style silhouette judgment: experiment tables, render history, and
export packages for the agent. **Nothing under `sessions/` is authoritative product state.**
The product parameter table remains `assets/params/lsystem.ron` and is only updated when a
finding is deliberately promoted.

The living plan for this work is [`PLAN-silhouette-lab.md`](PLAN-silhouette-lab.md). Close
that file when the lab + structural tuning campaign is finished.

---

## Tracked vs ignored

| Path | Git | Purpose |
|------|-----|---------|
| `README.md` | tracked | This ritual |
| `PLAN-silhouette-lab.md` | tracked | Closeable plan / checklist |
| `subjects.ron` | tracked | Default multi-subject gallery |
| `findings.schema.json` | tracked | Shape of an exported finding |
| `sessions/**` | **ignored** | Working tables, PNGs, per-session findings |
| `current` | **ignored** | Optional pointer at the active session |

CLI batch output defaults to `target/m0-silhouette/` (also ignored via `/target/`).

---

## Session layout

Created by you or by a future `m0-silhouette lab` command — never required to commit:

```text
qa/sessions/<stamp>_<label>/
  meta.json                 # started_at, subjects, table_source, notes
  experiment.ron            # working copy of the parameter table
  baseline.ron              # table at session start (diff target)
  renders/
    0001/
      meta.json             # family, focused param, values, digests
      experiment.ron        # full table used for this render (immutable)
      <subject>.png         # one PNG per subject in the strip
    0002/
      ...
  findings/
    <family>_<param>.json   # exported judgment (see findings.schema.json)
```

**Renders never overwrite.** Each click or CLI batch that belongs to a session gets a new
monotonic `NNNN` directory.

### Minimal `meta.json` (session)

```json
{
  "started_at": "2026-07-28T12:00:00Z",
  "label": "baseline",
  "table_source": "assets/params/lsystem.ron",
  "subjects": ["empty", "single-author", "deep-nesting"],
  "notes": ""
}
```

### Minimal `renders/NNNN/meta.json`

```json
{
  "index": 1,
  "family": "C",
  "parameter": "width_ratio.base",
  "notes": "",
  "subjects": {
    "empty": { "png": "empty.png", "skeleton_digest": "…" },
    "single-author": { "png": "single-author.png", "skeleton_digest": "…" }
  }
}
```

---

## Export package (agent handoff)

When a single parameter looks best across the subject strip, write a finding under
`findings/` (or copy one out of the session to share). Required shape is documented in
[`findings.schema.json`](findings.schema.json).

What to give the agent:

1. The finding JSON.
2. The chosen render directory (or the PNGs it names).
3. Optionally a diff of `experiment.ron` vs `baseline.ron`.

Do **not** hand over entire `sessions/` trees unless asked — one finding + chosen PNGs is enough.

---

## HTML lab (S4)

```text
cargo run -p m0-silhouette -- lab
# open http://127.0.0.1:7420/
```

What it does:

1. Creates `qa/sessions/<stamp>_<label>/` with `experiment.ron` + `baseline.ron` (copy of the
   product table, or `--table`).
2. Locks one §5 family, focuses one field, slider / exact value.
3. **Render strip** → next monotonic `renders/NNNN/` (never overwrites), multi-subject PNGs.
4. **Export finding** → `findings/<family>_<param>.json` matching [`findings.schema.json`](findings.schema.json).

The lab never writes `assets/params/lsystem.ron`. Promote by hand from an exported finding.

Options: `--port`, `--host`, `--label`, `--table`, `--size` (see `lab --help`).

### CLI fallback (still valid)

```text
cargo run -p m0-silhouette -- --table qa/sessions/.../experiment.ron --out qa/sessions/.../renders/0002
```

One parameter family at a time (`design/l-system-parameterization.md` §6).

### Default subject strip

Listed in [`subjects.ron`](subjects.ron). Includes the AC-SKEL-1 pair `skel1-clean` /
`skel1-messy` (synthetic, comparable size; see `tools/corpus` shapes).

---

## Promote

Lab / sessions never auto-write the product table.

1. Confirm the finding across the subject strip.
2. Copy the agreed numbers into `assets/params/lsystem.ron` (or apply the agent’s patch).
3. Re-run `m0-silhouette` once against the product table.
4. Commit table + a short progress note of *why*. Leave PNGs untracked.
