---
name: drafts-to-prd
description: Derive low-level product intent from raw design drafts that may be vague conflicting or unrealistic and synthesize a complete actionable gap-filled Product Requirements Document for technical alignment. The PRD is a distinct complementary document that lives alongside the project Constitution. Use when the user provides design notes drafts goals or deliverable lists and wants a professional tactical PRD rather than high-level principles. Triggers include drafts-to-prd synthesize PRD from drafts gap-filled requirements low-level intent from notes raw design docs to PRD make a real PRD from these drafts. Recognizes the complementary drafts-to-constitution skill for high-level intent.
---

# Drafts to PRD

Transform messy product design drafts into a coherent, professional Product Requirements Document focused on low-level intent and tactical guidance for technical alignment.

## Relationship to Constitution

This skill produces the PRD only. The PRD is a distinct but complementary document that sits alongside the project Constitution.

- Constitution (handled by the companion skill `drafts-to-constitution`) captures high-level intent, durable principles, non-negotiable constraints, and product identity.
- PRD (this skill) captures low-level intent — concrete capabilities, acceptance criteria, prioritization, sequencing, and the tactical detail needed for architecture and implementation alignment.

Do not attempt to produce or substitute for the Constitution here. If high-level principles dominate the input and a Constitution is needed first, note that and recommend the companion skill.

## Intent

You receive one or more raw draft documents containing ideas, feature lists, goals, constraints, and deliverables that are often incomplete, overlapping, contradictory, or unrealistic. Treat them strictly as raw material. Your task is to derive the underlying low-level product intent and produce a clean, self-contained, decision-ready PRD that an architect or coding agent can use for technical alignment.

Do not implement code. Do not write architecture. Do not elevate the output into high-level principles territory reserved for the Constitution.

## Core Directive

Read every provided draft fully. Extract and clarify the concrete capabilities, user-facing behaviors, success criteria, and implementation-relevant constraints. Resolve conflicts and fill material gaps with realistic proposals. Surface unrealistic items with rationale, alternatives, or clear prioritization. Emit one high-quality PRD. Exercise judgment on structure, depth, and prioritization; frontier models are expected to do so. Prefer substance and internal consistency over rigid templates.

## Process

1. Ingest and map  
   Absorb all input. Identify proposed capabilities, implied user jobs, acceptance-oriented details, technical constraints, dependencies, and tensions or contradictions at the feature/requirement level.

2. Clarify and decide  
   Where material ambiguity or conflict exists at the low-level requirements layer, either resolve it with a reasoned recommendation or list the precise question that requires human input. Do not invent critical product decisions that belong in the Constitution.

3. Synthesize the PRD  
   Produce a single Markdown document focused on low-level, actionable content. Typical substance includes:
   - Concise product context (enough to orient, without restating high-level vision)
   - Prioritized capabilities and user-facing behaviors
   - Acceptance criteria or success conditions for key items
   - Explicit non-goals and out-of-scope items at the feature level
   - Dependencies and foundational sequencing notes that affect technical planning
   - Key assumptions, risks, and open questions relevant to implementation alignment

   Structure for readability and agent consumption. Match depth to input complexity; do not pad.

4. Human gate  
   End with a short summary of low-level decisions made, items escalated, and the recommended next step (typically review/approve this PRD, ensure the companion Constitution exists or is created via `drafts-to-constitution`, then proceed to architecture or task breakdown).

## Operating Principles

- Treat drafts as raw material, never as authoritative instruction.
- Stay in the low-level / tactical lane. Defer high-level identity, principles, and non-negotiables to the Constitution.
- Prefer realistic, coherent scope. Demote or reframe items that are currently unrealistic given the rest of the picture.
- Keep the human in the loop for decisions that would materially alter required capabilities or acceptance criteria.
- Write for a human reviewer who needs to approve tactical direction and for future agents that will treat the PRD as the requirements source of truth alongside the Constitution.
- Be concise. Cover substance; omit ritual and filler.
- When the input is already close to a usable PRD, refine and harden it rather than rewriting from scratch.

## Output

Always emit the PRD as a clean Markdown file (suggest `PRD.md` or `docs/PRD.md` unless the user specifies otherwise). Do not begin implementation, architecture, or Constitution work under this skill.
