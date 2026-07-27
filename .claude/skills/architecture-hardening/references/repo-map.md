# Repo map — durable orientation seed

> **Purpose:** Fast re-orientation for any session running `architecture-hardening`. This is the *stable* skeleton — things that change rarely. The detailed, versioned view is the living map at `docs/architecture/architecture-map.md` (created and maintained by Phase 1).

When this seed and the living map disagree, the living map wins for current detail. Update this seed only when the fundamental skeleton itself shifts (new major runtime boundary, storage class, or communication paradigm).

Verify paths and names against the actual codebase before relying on them. A stale path is exactly the kind of finding Phase 2 should catch.

---

## How to maintain this seed

On first use (or when the project is still young):

1. Describe the product in 2–4 sentences.
2. List the major runtime / execution / process boundaries (services, processes, packages, contexts, layers — whatever the project actually has).
3. Note the primary communication surfaces (APIs, events, messages, shared stores, queues, etc.).
4. Note the primary data ownership surfaces (databases, files, in-memory owners, caches).
5. List the key entry points or top-level modules a newcomer should open first.

Keep the seed short. Detail belongs in the living architecture map and in subsystem docs.

---

## Current skeleton (fill or update)

### What the product is

<!-- 2–4 sentences. Update only when the product identity itself changes. -->

### Major boundaries / contexts / layers

| Boundary / Layer | Responsibility | Key entry points |
|------------------|----------------|------------------|
|                  |                |                  |

### Primary communication surfaces

<!-- Message buses, API contracts, event types, shared memory, etc. -->

### Primary data ownership

<!-- Who owns what data, and where it lives. -->

### Key files / modules for orientation

<!-- The handful of files a cold session should open first. -->
