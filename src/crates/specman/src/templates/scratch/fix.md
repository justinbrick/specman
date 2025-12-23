---
target: {{target_path}}
branch: null
work_type:
  fix:
    fixed_headings: []
dependencies: []
---

<!-- AI TODO: Set `branch` to `{target_name}/fix/{scratch_pad_name}` when this workspace uses git branches. -->
<!-- AI TODO: Populate `fixed_headings` with spec heading fragments impacted by this fix. -->

# Scratch Pad — Fix — Replace With Defect Identifier

## Flow Overview

- [ ] Phase 0: Establish evidence + reproduction
- [ ] Phase 1: Check for existing test coverage
- [ ] Phase 2: Add missing test (must fail)
- [ ] Phase 3: Implement fix
- [ ] Phase 4: Validate (tests + no regressions)

## Phase 0 — Diagnose

### Action: Write defect summary and expected behavior
<!-- AI INSTRUCTIONS:
- State the observed behavior and the expected behavior.
- Link to the relevant spec headings and/or code paths.
- Define “done” in one sentence.
-->

#### Example

- Observed: creating a `fix` scratch pad uses the generic scratch template.
- Expected: `fix` scratch pad uses a `fix`-specific template.
- Spec: `../../spec/specman-data-model/spec.md#work-type`
- Done when: the intended template is selected by default and validated by tests.

### Action: Capture reproduction steps and evidence
<!-- AI INSTRUCTIONS:
- Provide the exact commands, inputs, and environment.
- Record the actual output (errors, logs, failing tests).
- Identify the minimal reproducer.
-->

#### Example

- Command: `specman create scratch --name scratch-repro --target impl://specman-library --work-type fix`
- Evidence: persisted `scratch.md` contains the generic sections, not the fix flow.

## Phase 1 — Existing Coverage

### Action: Search for existing tests that cover the bug
<!-- AI INSTRUCTIONS:
- Search nearby tests first.
- If tests exist, extend one.
- If none exist, note that explicitly.
-->

#### Example

- Searched: `src/crates/specman/tests/` and `src/crates/specman/src/service.rs` tests.
- Found: scratchpad creation smoke tests; no assertion on default template selection.

## Phase 2 — Add Test (If Missing)

### Action: Add a failing test that demonstrates the defect
<!-- AI INSTRUCTIONS:
- Prefer a unit/integration test that fails before the fix and passes after.
- Keep it small and deterministic.
-->

#### Example

- Add test: `scratchpad_worktype_fix_uses_fix_template_by_default`.
- Assert: provenance locator is `embedded://scratch-fix` (once Phase 3 wiring exists).

## Phase 3 — Fix

### Action: Implement the smallest fix that makes the test pass
<!-- AI INSTRUCTIONS:
- Fix the root cause.
- Keep changes minimal and localized.
- Update related error handling only if required.
-->

#### Example

- Map `WorkType("fix")` to an embedded asset key that loads `templates/scratch/fix.md`.

## Phase 4 — Validate

### Action: Run tests and record results
<!-- AI INSTRUCTIONS:
- Run the narrowest test suite that exercises the change.
- Record what you ran and the result.
-->

#### Example

- Ran: `cargo test -p specman`
- Result: pass

## Notes

<!-- Keep a running log of findings, decisions, and partial progress so an AI can resume later. -->

## Decisions

<!-- Record final decisions and rationale. -->

## Next Steps

<!-- List the next concrete actions and who owns them. -->
