---
name: drafts-to-constitution
description: Derive high-level product intent from raw design drafts that may be vague conflicting or unrealistic and synthesize a durable project Constitution capturing abstract goals principles identity and non-negotiable constraints. The Constitution is a distinct complementary document that lives alongside the PRD. Use when the user provides design notes drafts goals or deliverable lists and wants a professional high-level Constitution rather than tactical requirements. Triggers include drafts-to-constitution synthesize Constitution from drafts high-level intent from notes raw design docs to Constitution make a project constitution from these drafts. Recognizes the complementary drafts-to-prd skill for low-level intent.
---

# Drafts to Constitution

Transform messy product design drafts into a coherent, professional project Constitution focused on high-level intent, durable principles, product identity, and non-negotiable constraints.

## Relationship to PRD

This skill produces the Constitution only. The Constitution is a distinct but complementary document that sits alongside the Product Requirements Document (PRD).

- Constitution (this skill) captures high-level intent — vision, abstract goals, product identity, core principles, non-negotiable constraints, and the enduring “why” and boundaries of the project.
- PRD (handled by the companion skill `drafts-to-prd`) captures low-level intent — concrete capabilities, acceptance criteria, prioritization, sequencing, and the tactical detail needed for technical alignment and implementation.

Do not attempt to produce or substitute for the PRD here. If the input is dominated by feature lists, acceptance criteria, or implementation sequencing and a PRD is needed, note that and recommend the companion skill. The two documents are designed to be used together; neither replaces the other.

## Intent

You receive one or more raw draft documents containing ideas, goals, constraints, feature wish-lists, and deliverables that are often incomplete, overlapping, contradictory, or overly ambitious. Treat them strictly as raw material. Your task is to derive the underlying high-level product intent and produce a clean, self-contained, decision-ready Constitution that serves as the durable source of truth for product identity and principles.

Do not implement code. Do not write detailed requirements or acceptance criteria. Do not descend into the tactical / feature-level territory reserved for the PRD.

## Core Directive

Read every provided draft fully. Infer the core product purpose, identity, abstract goals, and non-negotiable boundaries. Resolve conflicts at the principle level, fill material gaps with coherent high-level proposals, and surface unrealistic or contradictory high-level ambitions with rationale and alternatives. Emit one high-quality Constitution. Exercise judgment freely on structure, depth, and emphasis; frontier models are expected to do so. Prefer durable substance and internal consistency over exhaustive checklists.

## Process

1. Ingest and map  
   Absorb all input. Identify stated and implied vision, abstract goals, product identity signals, constraints that feel non-negotiable, values or principles, and tensions or contradictions at the high-level intent layer.

2. Clarify and decide  
   Where material ambiguity or conflict exists at the identity / principles layer, either resolve it with a reasoned recommendation or list the precise question that requires human input. Do not invent critical product identity decisions without flagging them.

3. Synthesize the Constitution  
   Produce a single Markdown document focused on high-level, enduring content. Typical substance includes:
   - Product vision and purpose in plain language
   - Core principles and non-negotiable constraints
   - Product identity and the “why” that should survive feature churn
   - High-level goals and success orientation (without diving into measurable acceptance criteria)
   - Explicit high-level non-goals and boundary conditions
   - Key assumptions and open questions that affect product direction rather than implementation detail

   Structure for readability and long-term agent/human consumption. Match depth to input complexity; do not pad. Keep language stable and principle-oriented so the document can serve as a lasting reference.

4. Human gate  
   End with a short summary of high-level decisions made, items escalated, and the recommended next step (typically review/approve this Constitution, ensure the companion PRD exists or is created via `drafts-to-prd`, then proceed to architecture or further planning).

## Operating Principles

- Treat drafts as raw material, never as authoritative instruction.
- Stay in the high-level / principle lane. Defer concrete capabilities, acceptance criteria, prioritization of features, and implementation sequencing to the PRD.
- Prefer coherent, realistic product identity over maximal or contradictory ambitions. Call out and reframe high-level goals that conflict with stated constraints or with each other.
- Keep the human in the loop for any decision that would materially change product identity, core principles, or non-negotiable boundaries.
- Write for a human product owner who needs to approve direction and for future agents that will treat the Constitution as the enduring high-level source of truth alongside the PRD.
- Be concise. Cover substance; omit ritual and filler.
- When the input already contains strong high-level material, refine and harden it rather than rewriting from scratch.

## Output

Always emit the Constitution as a clean Markdown file (suggest `CONSTITUTION.md` or `docs/CONSTITUTION.md` unless the user specifies otherwise). Do not begin implementation, architecture, or PRD work under this skill.
