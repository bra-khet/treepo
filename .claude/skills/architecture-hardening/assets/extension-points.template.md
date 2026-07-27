<!--
  TEMPLATE — copy to docs/architecture/extension-points.md on first run.
  Thereafter update in place and bump the version.
-->

# Extension Points Registry

**Version:** v1.0 · **Updated:** `<YYYY-MM-DD>`  
**Status:** Canonical list of intentional seams where new capabilities should plug in.  
**Wins for:** “Where does a new X belong?”

### Changelog
- `v1.0` (`<YYYY-MM-DD>`) — initial registry.

---

## How to use this registry

When planning a new feature or capability:
1. Find the matching extension point (or decide a new one is required).
2. Check the associated first-class concerns and invariants in the architecture map.
3. Prefer extending an existing seam over creating a parallel path.

---

## Registry

| Extension Point | Purpose | How to extend | Related concerns / invariants | Notes |
|-----------------|---------|---------------|-------------------------------|-------|
|                 |         |               |                               |       |

---

## Guidance

- Prefer intentional, documented seams over ad-hoc coupling.
- When a new class of extension appears repeatedly, promote it to a first-class extension point and update this registry.
- Link back to the architecture map and any relevant ADRs.
