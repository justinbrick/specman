---
target: {{target_path}}
branch: null
work_type:
  feat: {}
dependencies: []
---

<!-- AI TODO: Set `branch` to `{target_name}/feat/{scratch_pad_name}` when this workspace uses git branches. -->

# Scratch Pad — Feature — Replace With Feature Name

## Flow Overview

- [ ] Phase 0: Confirm scope + constraints
- [ ] Phase 1: Design approach + acceptance criteria
- [ ] Phase 2: Implement incrementally
- [ ] Phase 3: Tests (add missing tests first)
- [ ] Phase 4: Docs / comments / polish
- [ ] Phase 5: Verify end-to-end behavior

## Phase 0 — Scope & Constraints

### Action: Write problem statement + success criteria
<!-- AI INSTRUCTIONS:
- Summarize what is being added and why.
- Define success criteria as observable outcomes.
- Call out explicit non-goals.
-->

#### Example

**Problem**: The CLI currently cannot list known templates.

**Success criteria**:

- `specman templates list` prints available template scenarios.
- Command exits with code 0.

**Non-goals**:

- No changes to template resolution order.

### Action: Identify impacted artifacts and boundaries
<!-- AI INSTRUCTIONS:
- List the target implementation and any relevant specs.
- Record file paths and public APIs expected to change.
- Note any compatibility constraints.
-->

#### Example

- Target: `impl://specman-library`
- Related spec: `../../spec/specman-core/spec.md#concept-template-orchestration`
- Boundaries: do not change CLI flags; add a new subcommand only.

## Phase 1 — Design

### Action: Draft a minimal design
<!-- AI INSTRUCTIONS:
- Describe the approach at the level of modules/symbols.
- Include alternatives and why you chose this one.
- Define how errors are reported and surfaced.
-->

#### Example

- Add `TemplatesCommand::List` in `src/crates/specman-cli/src/commands/templates.rs`.
- Reuse existing `TemplateCatalog` query surface.
- Errors propagate as a single `SpecmanError` message.

### Action: Define acceptance tests
<!-- AI INSTRUCTIONS:
- Prefer a small unit/integration test that asserts behavior.
- If no tests exist nearby, document why and add a lightweight check.
-->

#### Example

- Add an integration test that runs the command against a temp workspace and asserts output contains `spec`, `impl`, `scratch-*`.

## Phase 2 — Implementation

### Action: Implement the smallest working slice
<!-- AI INSTRUCTIONS:
- Create the smallest end-to-end working path.
- Keep changes localized; avoid refactors unrelated to the feature.
- Keep intermediate states runnable.
-->

#### Example

- First commit: add command wiring + stub output.
- Second commit: connect to real catalog and print results.

### Action: Extend behavior carefully
<!-- AI INSTRUCTIONS:
- Add options/flags only when required by the spec.
- Maintain backward compatibility for existing commands.
-->

#### Example

- Add `--json` only if a consumer needs it; otherwise keep plain text.

## Phase 3 — Tests

### Action: Check for existing tests and add missing coverage
<!-- AI INSTRUCTIONS:
- Search for existing tests in the target crate.
- If missing, add a test that fails before your change and passes after.
-->

#### Example

- Existing: `src/crates/specman-cli/tests/` has integration tests for other commands.
- Add: `templates_list_smoke`.

## Phase 4 — Docs / Comments / Polish

### Action: Update docs and inline comments
<!-- AI INSTRUCTIONS:
- Update READMEs or docs only where the user-facing behavior changed.
- Ensure error messages are actionable.
-->

#### Example

- Update `src/crates/specman-cli/README.md` with the new subcommand.

## Phase 5 — Verification

### Action: Run minimal verification
<!-- AI INSTRUCTIONS:
- Run the narrowest test/build command that covers the change.
- Record what you ran and the result.
-->

#### Example

- Ran: `cargo test -p specman-cli`
- Result: pass

## Notes

<!-- Keep a running log of findings, decisions, and partial progress so an AI can resume later. -->

## Decisions

<!-- Record final decisions and rationale. -->

## Next Steps

<!-- List the next concrete actions and who owns them. -->
