---
name: docs-archiving
description: After major milestones or on demand, archives historical documentation and progress tracking for sustainable long-term development (inspired by Karpathy LLM Wiki patterns). Creates a clean top-level archive/ structure, produces slimmed living documents with conditional archive pointers, and maintains a lightweight living history reference (docs/HISTORY.md). Supports Initialize and Refresh modes. Ideal for projects like locutorium where docs/ and root progress files (claude-progress.md) accumulate over versions. Load this skill when files grow large, after releases, or when you want to reset context debt while preserving every decision.
---
 
# docs-archiving
 
## Why This Skill Exists
 
Long-running projects like the locutorium Chrome MV3 extension naturally accumulate detailed history: version-specific release notes, handoff and checkpoint documents, evolving roadmaps, and rich progress tracking in `claude-progress.md` (and branch-phase files). Without deliberate maintenance, these files grow large, making it harder for both humans and agents to focus on current work.
 
This skill solves that by following a simple, sustainable pattern inspired by Andrej Karpathy's 2026 LLM Wiki approach:
- **Raw/verbose history** lives in an immutable archive layer (never lost).
- The **agent maintains a living synthesis layer** (slimmed active documents + a compact history reference) that compounds value over time.
- **Progressive/conditional disclosure**: Living files contain short, standard notices that point to archives only when deeper history is actually needed.
The result: focused living documents that stay small and actionable, while the full history remains perfectly accessible. The skill works in two modes (Initialize for first-time setup, Refresh/Maintain for ongoing use after releases or big features) and is generalized so the same logic can be reused on other projects with only minor path adjustments.
 
This directly supports clean, high-velocity development of locutorium as it evolves beyond v5.3.3 (Character Voice Studio, demo site, etc.) into future milestones.
 
## Instructions
 
### Step 1: Determine Mode and Assess Current State
 
- Detect the requested mode from user input:
  - **Initialize**: First-time setup (no `archive/` exists, or user explicitly says "initialize").
  - **Refresh/Maintain**: Ongoing use after a milestone (user mentions a release, feature completion, or the progress/docs files have grown noticeably).
- Scan the project:
  - List files in `docs/` (especially anything with version numbers, "handoff", "checkpoint", "roadmap", "release-notes", "implementation-notes").
  - Check size and recent activity of root progress files: `claude-progress.md`, `*-branch.md` (eloquent-branch.md, dulcet-branch.md, pretty-branch.md, etc.).
  - Note current project version/milestone (from package.json, latest release notes, or user statement — currently v5.3.3 on main for this repo).
  - Check whether `archive/` already exists and what it contains.
- Identify the "current living baseline": What represents active work right now (current roadmap, active implementation notes, open tasks in progress file, latest features like the Voice Studio).
### Step 2: Identify Historical vs. Living Content
 
- Mark content as **historical** if it meets any of these:
  - Refers to completed milestones, previous major versions (e.g. v3.x, v4.x, pre-v5.3 studio work), resolved handoffs/checkpoints, or superseded roadmaps.
  - Sections of `claude-progress.md` or branch files that describe finished phases, old decisions that are no longer active, or verbose logs of past iterations.
  - Old release-notes files or version-specific design docs that are no longer the primary reference.
- Mark content as **living / active** if it describes:
  - Current open tasks, active feature work, the latest roadmap/implementation notes, or decisions that still guide ongoing development.
  - The most recent progress entries that have not yet been superseded.
- When in doubt, keep it in the living layer for the first pass and let the user refine — safety over aggressive pruning.
### Step 3: Create/Refresh Archive Structure and Snapshot History
 
- Ensure the following directory structure exists at repo root:
  ```
  archive/
  ├── docs/          # Formal project documentation history
  └── progress/      # Root-level progress and phase tracking history
  ```
- For every identified historical item:
  - Create a dated snapshot inside the appropriate subfolder.
    - Progress files → `archive/progress/claude-progress-pre-[milestone-or-date].md`
    - Docs files → `archive/docs/[original-name]-pre-[milestone-or-date].md` (or organize by version/topic if many files).
  - Copy the full original content into the archive (never move/delete the source until the slimmed living version is confirmed good).
  - Add a small header inside the archived file noting the date archived and the milestone it was captured after.
- Do **not** delete or overwrite anything in the original locations yet — work on copies/snapshots first.
### Step 4: Produce Slimmed Living Versions with Conditional Disclosure
 
- For each living document that was bloated (especially `claude-progress.md` and any active docs/ files):
  - Create an updated version that keeps only:
    - Current active work and open tasks.
    - The most recent relevant decisions that still matter.
    - A short, standard **Archive Notice** section near the top (or in a consistent location) using this pattern:
      ```
      ## Archive Notice (Living Document)
      This is the *living* version focused on current work.
      For complete history before [specific milestone, e.g. v5.3.3 Character Voice Studio + demo site], see:
      - `archive/progress/claude-progress-pre-2026-06-28.md`
      - Or consult the living history reference at `docs/HISTORY.md`
      The full prior content has been preserved in the archive so this file stays focused and easy to maintain.
      ```
 
- The notice should be concise, use backticks for paths, and make it obvious to any agent (or human) reading the file that deeper history is available on demand without cluttering the active context.
- Write the slimmed content back to the original file paths (or produce a clear diff the user can apply).
### Step 5: Maintain the Living History Reference
 
- Create or update `docs/HISTORY.md` (or `docs/living-project-history.md` if you prefer that name) as the single lightweight index.
- Structure it simply:
  - Short intro: "This is the living history reference for locutorium. It indexes major milestones and points to both current living documents and their archived history."
  - A clear, scannable list or table of **Major Milestones** (date, version/tag, one-sentence outcome or focus, links to relevant living docs + archive pointers).
  - Example entry style:
    - **v5.3.3 (2026-06-28)** — Character Voice Studio + public demo site launched. Icon refresh. See living: `claude-progress.md` (current), `docs/v5-development-roadmap.md`. Archived history: `archive/progress/claude-progress-pre-2026-06-28.md`, `archive/docs/`.
  - A "How archives work" note explaining the conditional disclosure pattern.
- Keep this file deliberately small and high-signal — it is the go-to document when an agent needs orientation on "what happened before the current feature branch?"
### Step 6: Output Summary, Commit Suggestions, and Self-Verify
 
- Produce a clear, actionable summary:
  - What mode was used.
  - Which files were archived (with paths and dates).
  - Which living files were slimmed and what the Archive Notice points to.
  - Status of `docs/HISTORY.md`.
  - Approximate size reduction (e.g. "claude-progress.md reduced from ~80 KB focus to active sections only").
- Suggest ready-to-use Git commit message(s), for example:
  ```
  docs: archive pre-v5.3.3 history into archive/ structure; slim living progress + add conditional notices; update HISTORY.md
 
  - Created archive/docs/ and archive/progress/
  - Archived historical release notes, handoffs, and early progress
  - Slimmed claude-progress.md with Archive Notice
  - Added/updated docs/HISTORY.md as living milestone index
  ```
- Perform self-verification (see checklist below). If anything looks incomplete, note it for the user.
## Examples
 
### Example 1: Initialize archiving on locutorium (current v5.3.3 state)
 
User runs the skill in Initialize mode on the repo at commit acc75178effccc6834b57892eb5a8142ed4dcfff (post v5.3.3 icon update).
 
**What the skill does:**
- Creates `archive/docs/` and `archive/progress/`.
- Identifies as historical: all v3.x / v4.x release-notes, older eloquent-profile-handoff.md and checkpoint files, pre-studio roadmap sections, early parts of claude-progress.md describing phases before the Design Studio / Voice Studio work.
- Snapshots them with names like `archive/progress/claude-progress-pre-v5.3-studio.md` and `archive/docs/release-notes-v4.0.0.md`, `archive/docs/eloquent-profile-handoff.md`, etc.
- Slims `claude-progress.md` to focus on post-v5.3 active items + adds the standard Archive Notice pointing to the new archive files and `docs/HISTORY.md`.
- Creates `docs/HISTORY.md` with entries for v5.0.0 through v5.3.3, linking to living roadmaps/implementation notes and the newly created archives.
- Output includes a suggested commit and confirmation that a fresh agent could now continue v5.4+ work using only the slimmed living files + HISTORY.md.
### Example 2: Refresh after a future v5.4 milestone (hypothetical)
 
User says: "Run docs-archiving refresh after completing the new subtitle rendering feature for v5.4."
 
**What the skill does:**
- Scans and sees new activity in `claude-progress.md` and perhaps a new `docs/v5.4-subtitle-qol.md` or similar.
- Identifies the just-completed subtitle work as the new milestone boundary.
- Archives the previous "current" sections of progress and any now-superseded design docs into dated files under `archive/progress/` and `archive/docs/`.
- Updates the slimmed `claude-progress.md` (and any affected living docs) with a fresh Archive Notice that now points past v5.3.3 *and* the new v5.4 work.
- Appends a new entry to `docs/HISTORY.md` for v5.4 with links.
- Produces a clean summary and commit message focused on the incremental maintenance.
## Self-Verify / Quality Checklist
 
Before finishing, confirm:
- [ ] Full historical content exists in `archive/` with no data loss.
- [ ] Every slimmed living file contains a clear, consistent Archive Notice with accurate paths.
- [ ] `docs/HISTORY.md` (or equivalent) exists and provides a complete, scannable index of major milestones with working links to both living and archived artifacts.
- [ ] The living documents + HISTORY.md together allow a fresh agent (or human) to understand the current state and continue development without needing to read the full archives unless they explicitly follow a pointer.
- [ ] Archive structure follows the documented convention (`archive/docs/` + `archive/progress/`) and will be easy for future runs of this skill to extend.
- [ ] Suggested commit messages are clear and ready to use.
If any item fails, note it explicitly and offer a fix.
 
## Troubleshooting
 
**Archive already contains a file with a similar name**  
Append a more specific timestamp or version suffix (e.g. `-2026-07-02-final`). Never overwrite.
 
**Living file still feels too long after slimming**  
Re-run Step 2 more aggressively on sections that are clearly superseded. You can always keep a bit more in the living layer on the first pass — the archive has the full detail.
 
**User wants to exclude certain files from archiving**  
Honor explicit instructions (e.g. "never archive future-ideas.md"). Add a short note in the output explaining what was left living by request.
 
**Skill is being used on a different project**  
The core logic is generalized. Override the default paths (docs root, progress filenames, current version source) in your invocation if the layout differs. The archive/ + living-notice + HISTORY.md pattern remains the same.
 
**First run on a very messy repo**  
Start with Initialize mode. It is safe and creates a clean baseline. Subsequent Refresh runs become lightweight incremental updates.
 
This skill keeps the knowledge base of locutorium (and similar projects) healthy indefinitely while staying true to the spirit of focused, compounding documentation that Karpathy and modern agent workflows favor.