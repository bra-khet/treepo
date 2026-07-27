<!--
  TEMPLATE — copy to docs/architecture/architecture-map.md on first Phase 1 run.
  Then fill placeholders, embed current diagrams, and DELETE this comment.
  Thereafter, UPDATE this file in place and bump the version — never create
  architecture-map-v2.md.
-->

# Architecture Map

**Version:** v1.0 · **Reflects branch/tag:** `<branch-or-tag>` · **Updated:** `<YYYY-MM-DD>`  
**Status:** Canonical cross-cutting architecture index. Wins for *how subsystems fit together*; subsystem internals are owned by the canonical docs linked below.  
**Re-run:** `architecture-hardening` (full) or a named phase.

### Changelog
- `v1.0` (`<YYYY-MM-DD>`) — initial map.

> Bump MINOR for additive refreshes; MAJOR when a fundamental boundary, component class, or first-class concern is added or removed. Keep newest entry on top.

---

## 1. Major components / contexts / layers

| Component / Layer | Responsibility | Key entry points |
|-------------------|----------------|------------------|
|                   |                |                  |

**Boundary notes:** (record any important constraints on how fixes or changes transfer across boundaries)

---

## 2. Diagrams

### 2.1 Component / context map

```mermaid
flowchart LR
  %% fill from diagram-cookbook
```

### 2.2 Data flow

```mermaid
flowchart TD
  %% fill from diagram-cookbook
```

### 2.3 Key state machine

```mermaid
stateDiagram-v2
  %% fill from diagram-cookbook
```

### 2.4 Representative sequence

```mermaid
sequenceDiagram
  %% fill from diagram-cookbook
```

---

## 3. First-class concerns

These are the load-bearing architectural concerns for this project. Each should have a short description and, after Phase 2, a one-line invariant.

### 3.1 <Concern name>

<!-- description -->

### 3.2 <Concern name>

<!-- description -->

---

## 4. Extension points

See `docs/architecture/extension-points.md` (versioned registry).

---

## 5. Invariants & Confidence ledger

*(Filled in Phase 2)*

| Area | Invariant (one sentence) | Evidence | Confidence |
|------|--------------------------|----------|------------|
|      |                          |          | High/Med/Low |

**Low-confidence items / open questions:**

-

---

## 6. Resume in a new chat

<!-- ≤20 lines. Paste-ready seed for a cold session. Update every major refresh. -->

- Current architecture focus:
- Highest-priority open questions:
- Key docs to read first:
- Recent hardening or ADRs to be aware of:
