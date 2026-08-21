Guide the user through their first complete Speckit workflow cycle. This is a teaching experience—you'll do real work in their codebase while explaining each step.

**Store selection:** If the user names a store (a store is a standalone Speckit repo registered on this machine) or the work lives in one, run `speckit store list --json` to discover registered store ids, then pass `--store <id>` on the commands that read or write specs and changes (`new change`, `status`, `instructions`, `list`, `show`, `validate`, `archive`, `doctor`, `context`, `schemas`, `view`). Once selected, treat `--store <id>` as sticky for the rest of the workflow. Every unscoped example of those commands below is shorthand: before running it, append the flag. For example, run `speckit status --change "<name>" --json --store "<id>"`, not the unscoped form shown below. Other commands do not take the flag. Hints printed by commands already carry the flag; keep it on follow-ups. Without a store, commands act on the nearest local `speckit/` root.

---

## Phase 1: Welcome

Walk the user through the canonical Speckit cycle: explore → new change → proposal → specs → design → tasks → apply → archive.

Briefly explain each phase in plain language:
- **explore** — think before you build
- **new change** — scaffold a planning folder
- **proposal** — capture *what* and *why*
- **specs** — define *what the system must do*
- **design** — capture *how*
- **tasks** — break implementation into steps
- **apply** — implement the tasks
- **archive** — finalize and merge

## Phase 2: Task Selection

Scan the codebase for small improvement opportunities. Look for:
- `TODO` / `FIXME` / `XXX` comments
- Missing error handling in obvious places
- Missing tests for non-trivial code
- Small refactors that improve clarity

Present 3-4 specific suggestions with scope estimates (small / medium / large). Let the user pick what feels right, or propose something else.

## Phase 3: Explore Demo

Briefly demonstrate explore mode by investigating the area relevant to the chosen task. Show how it surfaces context, options, and tradeoffs without committing to anything yet.

## Phase 4: Create the Change

```bash
speckit new change "<derived-name>"
```

Use the name derived in Phase 2. Confirm the change directory was scaffolded (status should report it).

## Phase 5: Proposal

Draft `proposal.md`:
- **Why** — the motivation
- **What Changes** — a concise list
- **Capabilities** — which spec capabilities this touches
- **Impact** — what breaks / who is affected

Show the draft, get approval, then save.

## Phase 6: Specs

For each affected capability (or a new one), draft a delta spec under `specs/<capability-path>/spec.md` using the standard `## ADDED Requirements` format.

## Phase 7: Design

Draft `design.md` capturing key decisions, tradeoffs, and the chosen approach. Keep it focused.

## Phase 8: Tasks

Draft `tasks.md` as an ordered checklist:
- One task per checkbox
- Scoped to fit in a single sitting
- Implementable in the chosen order

## Phase 9: Apply (Implementation)

Implement each task in order, checking them off as you go. Run `speckit status --change "<name>"` periodically. Use `speckit instructions apply --change "<name>" --json` if you need the apply context.

If a task surfaces a design issue, pause and offer to revise the artifacts before continuing.

## Phase 10: Archive

```bash
speckit archive "<name>" --yes
```

If delta specs exist, offer to run `speckit-sync-specs` first (or do it inline during archive).

## Phase 11: Recap & Next Steps

Summarize:
- What was built
- Where the spec lives now (`specs/<capability-path>/spec.md`)
- How to find archived changes (`speckit list`)

Provide the command reference:

| Workflow       | Slash command             |
|----------------|---------------------------|
| Explore        | `/speckit:explore`        |
| New change     | `/speckit:new`            |
| Continue       | `/speckit:continue`       |
| Apply          | `/speckit:apply`          |
| Update         | `/speckit:update`         |
| Sync specs     | `/speckit:sync`           |
| Archive        | `/speckit:archive`        |
| Bulk archive   | `/speckit:bulk-archive`   |
| Verify         | `/speckit:verify`         |
| Propose        | `/speckit:propose`        |

---

## Guardrails

- Follow EXPLAIN → DO → SHOW → PAUSE pattern at key transitions
- Keep narration light during implementation
- Don't skip phases even if the change is small
- Handle exits gracefully - never pressure the user
- Use real codebase tasks - don't simulate