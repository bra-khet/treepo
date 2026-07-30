# treepo

treepo grows a single living world-tree from a repository's real structure, size, age,
churn, ownership, and activity. The goal is perceptual: to let a developer *recognize* their
codebase by looking at it.

A consumer desktop application (Rust / Bevy). Not a dashboard with tree decorations — the
tree is grown from real repository measurements.

## Status

Design and planning are complete. **No implementation exists yet** (no Cargo workspace, no
crates). This directory is ready for campaign **Phase 0** (workspace &
determinism foundation).

| Layer | State |
|-------|--------|
| Constitution | Ratified — [`docs/CONSTITUTION.md`](docs/CONSTITUTION.md) |
| PRD | Approved v1.1 — [`docs/PRD.md`](docs/PRD.md) |
| Design | Living set — [`docs/design/`](docs/design/) |
| Architecture | [`.planning/architecture-treepo.md`](.planning/architecture-treepo.md) |
| Campaign | [`.planning/campaign-treepo.md`](.planning/campaign-treepo.md) — 13 phases, Phase 0 next |

## Agent live control (Bevy BRP)

When the Bevy app exists (**Phase 5+**), agents control a running instance over the Bevy Remote
Protocol. Planned wiring (architecture **D10**): optional Cargo feature `brp` →
`bevy_brp_extras::BrpExtrasPlugin` (includes `RemotePlugin`) on port **15702**. Never enabled in
default or release builds (`N2` / `NFR-8`).

Host side (already available on this machine for Grok): globally installed `bevy_brp_mcp`,
registered as MCP server **`bevy_brp`** in `~/.grok/config.toml`.

```text
# Agent: launch via shell with the feature (do not use bevy_brp MCP brp_launch —
# it rebuilds without --features brp and can silently drop the listener).
cargo run -p treepo-app --features brp -- <path-to-repository>
# then use the other bevy_brp MCP tools against localhost:15702
```

## Documentation

- [`docs/README.md`](docs/README.md) — documentation map, reading order, and agent BRP summary
- [`docs/CONSTITUTION.md`](docs/CONSTITUTION.md) — enduring vision, principles, and
  non-negotiable constraints
- [`docs/PRD.md`](docs/PRD.md) — capabilities, acceptance criteria, and sequencing
- [`docs/design/`](docs/design/) — living design documents (start with
  [`design-outline.md`](docs/design/design-outline.md))
- [`.planning/`](.planning/) — architecture and phased build campaign
- [`LICENSE-THIRD-PARTY.md`](LICENSE-THIRD-PARTY.md) — third-party notices (MPL-2.0 `uluru`
  and companions to `deny.toml`)
