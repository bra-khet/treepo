# Diagram cookbook — Mermaid patterns

Copy a recipe, then correct it against current code before committing. Always render-check — Mermaid syntax errors fail silently in plain Markdown and a broken diagram is worse than none.

Conventions:
- Use **stable identifiers** (module names, type names, message/event names, store names) as labels. They survive refactors; line numbers do not.
- One altitude per diagram. If it needs more than ~15 nodes, split it.
- Keep node text short; put detail in the surrounding prose.

---

## 1. Component / context map (who talks to whom)

```mermaid
flowchart LR
  subgraph LayerA["Layer or Context A"]
    A1[Component A1]
    A2[Component A2]
  end
  subgraph LayerB["Layer or Context B"]
    B1[Component B1]
  end
  A1 -->|event / API / message| B1
  A2 -->|shared store| B1
```

Use this for the high-level “who talks to whom” view. Label edges with the actual communication mechanism.

---

## 2. Data flow

```mermaid
flowchart TD
  Source[Data source] --> Process1[Processing step]
  Process1 --> Store[(Primary store)]
  Process1 --> Process2[Downstream step]
  Process2 --> Output[Consumer / Output]
```

Mark which artifacts land in which store or ownership boundary.

---

## 3. State machine (one important lifecycle)

```mermaid
stateDiagram-v2
  [*] --> Idle
  Idle --> Running: start
  Running --> Succeeded: complete
  Running --> Failed: error
  Succeeded --> [*]
  Failed --> Idle: retry / reset
```

Pick **one** load-bearing lifecycle. Do not try to draw every state in the system at once.

---

## 4. Sequence (one representative end-to-end path)

```mermaid
sequenceDiagram
  participant Client
  participant ServiceA
  participant ServiceB
  participant Store

  Client->>ServiceA: request
  ServiceA->>ServiceB: downstream call
  ServiceB->>Store: write
  Store-->>ServiceB: ack
  ServiceB-->>ServiceA: result
  ServiceA-->>Client: response
```

Use this to reveal hops, contracts, and failure points that the component map hides.

---

## General tips

- Prefer `flowchart` for structure and data movement; `sequenceDiagram` for interaction; `stateDiagram-v2` for lifecycles.
- If a diagram becomes dense, extract a second diagram at a different altitude rather than cramming more nodes.
- After editing, re-render mentally or with a Mermaid previewer to catch syntax errors before the doc is committed.
