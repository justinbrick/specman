---
target: {{target_path}}
branch: null
work_type:
  revision:
    revised_headings: []
dependencies: []
---

<!-- AI TODO: Set `branch` to `{target_name}/revision/{scratch_pad_name}` when this workspace uses git branches. -->
<!-- AI TODO: Populate `revised_headings` with spec heading fragments changed by this revision. -->

# Scratch Pad — Revision — Replace With Revision Focus

## Flow Overview

- [ ] Phase 0: Document conflicts / motivation
- [ ] Phase 1: Adversarial review of proposed change
- [ ] Phase 2: User questions + decision points (HARD STOP)

## Phase 0 — Conflicts & Motivation

### Action: Document the conflict with current spec
<!-- AI INSTRUCTIONS:
- State what is wrong/insufficient/contradictory in the current spec.
- Quote or link to the specific headings in question.
- Clarify what property must be preserved.
-->

#### Example

- Conflict: `spec/specman-core/spec.md#concept-template-orchestration` implies templates must not contain front matter,
  but scratch templates currently ship with front matter.
- Must preserve: backwards compatibility for existing workspaces.

### Action: Define desired outcome and compatibility constraints
<!-- AI INSTRUCTIONS:
- Define what the revised spec should say.
- Define compatibility/migration constraints.
-->

#### Example

- Desired: explicitly carve out scratch templates as allowed to include front matter (or move front matter generation out of templates).
- Constraint: do not break existing scratch pad creation.

## Phase 1 — Adversarial Review

### Action: Argue against your own proposal
<!-- AI INSTRUCTIONS:
- List the strongest objections, edge cases, and unintended consequences.
- Identify which stakeholders would disagree and why.
-->

#### Example

- Objection: allowing front matter in templates weakens the “templates are body-only” invariant.
- Edge case: workspace overrides might now accidentally include invalid YAML.

### Action: Evaluate alternatives
<!-- AI INSTRUCTIONS:
- List at least two alternatives.
- Compare complexity, migration risk, and spec clarity.
-->

#### Example

- Alternative A: permit front matter only for scratch templates.
- Alternative B: remove front matter from templates entirely; inject via lifecycle.

## Phase 2 — Questions for User (HARD STOP)

### Action: Present concrete questions that block progress
<!-- AI INSTRUCTIONS:
- Ask only questions that materially affect the revision.
- Provide the decision options and the tradeoff in one sentence each.
- Do not edit code or specs until answers are received.
-->

#### Example

1. Should scratch templates be exempt from the “no front matter in templates” rule?
   - Option A: Yes (lowest migration risk).
   - Option B: No; move all front matter generation into lifecycle (cleaner invariant, larger change).

<!-- AI STOP: Do not modify any specification or code until the user answers the Phase 2 questions above. -->

## Notes

<!-- Keep a running log of findings, decisions, and partial progress so an AI can resume later. -->
