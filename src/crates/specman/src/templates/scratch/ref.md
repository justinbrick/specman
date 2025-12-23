---
target: {{target_path}}
branch: null
work_type:
  ref:
    refactored_headings: []
dependencies: []
---

<!-- AI TODO: Set `branch` to `{target_name}/ref/{scratch_pad_name}` when this workspace uses git branches. -->
<!-- AI TODO: Populate `refactored_headings` with spec heading fragments impacted by this refactor. -->

# Scratch Pad — Refactor — Replace With Refactor Focus

## Flow Overview

- [ ] Phase 0: Clarify motivation + boundaries
- [ ] Phase 1: Inventory + risks
- [ ] Phase 2: Proposed refactor plan (user gate)
- [ ] Phase 3: Mechanical refactor (small steps)
- [ ] Phase 4: Validate + clean up

## Phase 0 — Motivation & Boundaries

### Action: State the refactor motivation and constraints
<!-- AI INSTRUCTIONS:
- Explain what is being improved (maintainability, performance, safety).
- Identify constraints (no behavior change, keep public API stable).
- Call out explicit non-goals.
-->

#### Example

**Motivation**: Template resolution code is hard to reason about due to duplicated slug handling.

**Constraints**:

- No change in external behavior.
- Keep error messages stable.

**Non-goals**:

- Do not change template search order.

## Phase 1 — Inventory & Risk

### Action: Inventory impacted code paths and callers
<!-- AI INSTRUCTIONS:
- List key functions/types you will touch.
- Identify the callers and how you will validate correctness.
-->

#### Example

- Touch: `TemplateCatalog::override_candidates`, `sanitize_key`.
- Callers: `Specman::create` and `LifecycleController` creation plans.
- Validation: unit tests for slug normalization + an integration smoke test.

### Action: Identify risky behavior changes
<!-- AI INSTRUCTIONS:
- List failure modes if you refactor incorrectly.
- Add guardrails (tests, small diffs, feature flags if needed).
-->

#### Example

- Risk: changing path precedence breaks workspace overrides.
- Guardrail: assert resolution uses `.specman/templates/scratch/ref.md` first.

## Phase 2 — Plan (User Gate)

### Action: Draft a step-by-step refactor plan
<!-- AI INSTRUCTIONS:
- Break into small, reviewable commits.
- Define invariants per step.
- Stop and ask the user to confirm before starting Phase 3.
-->

#### Example

1. Add tests for current behavior.
2. Extract a helper for slug computation.
3. Replace duplicated logic and keep outputs identical.

### Action: Ask for user confirmation
<!-- AI INSTRUCTIONS:
- Present the plan + risks.
- Ask explicit yes/no to proceed.
-->

#### Example

**Question**: OK to proceed with the 3-step refactor plan above?

## Phase 3 — Refactor Execution

### Action: Apply mechanical changes in small steps
<!-- AI INSTRUCTIONS:
- Keep each commit minimal and reversible.
- Prefer pure refactors with tests proving behavior unchanged.
- Avoid “cleanup” unrelated to the plan.
-->

#### Example

- Commit A: rename local variables for clarity (no logic change).
- Commit B: replace duplicated slug logic with `sanitize_key` call.

## Phase 4 — Validate & Clean Up

### Action: Re-run tests and verify invariants
<!-- AI INSTRUCTIONS:
- Run the narrowest tests first, then broader ones.
- Explicitly verify no user-facing behavior changed.
-->

#### Example

- Ran: `cargo test -p specman`
- Verified: override order unchanged; output identical for fixture workspaces.

## Notes

<!-- Keep a running log of findings, decisions, and partial progress so an AI can resume later. -->

## Decisions

<!-- Record final decisions and rationale. -->

## Next Steps

<!-- List the next concrete actions and who owns them. -->
