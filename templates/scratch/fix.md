---
target: ../relative/path/to/implementation.md
branch: target-name/fix/scratch-name
work_type:
  fix:
    fixed_headings:
      - ../path/to/spec.md#Heading Fragment
---

<!-- AI TODO: Update `target`, `branch`, and every `fixed_headings` entry so they reflect the real implementation, git branch, and impacted specification headings before editing the body. -->

# Scratch Pad — <!-- Replace With Defect Or Fix Identifier -->

<!-- Summarize the defect being remediated, why it matters, and what success looks like for this fix. -->

## Context

<!-- Outline the current architecture, dependencies, and prior decisions that frame this remediation. Reference code, specs, or earlier scratch pads as needed. -->

## Defect Summary

<!-- Capture a concise description of the defect, observable symptoms, and how it violates the specification or expected behavior. -->

## Reproduction & Evidence

<!-- Document reproduction steps, logs, failing tests, or other artifacts that prove the defect exists. -->

## Impact Assessment

<!-- Describe who or what is affected, severity, and any downstream consequences if the fix slips. -->

## Fix Scope & Goals

<!-- Enumerate the concrete remediation goals, explicitly tying each to a `fixed_headings` entry so reviewers understand the specification coverage. -->

## Notes

<!-- Track investigations, hypotheses, or discoveries. Link directly to code or experiments for easy follow-up. -->

## Code Comment Updates

<!-- List every structure, function, or file touched by the fix and confirm their comments explain what changed and why. Experiments and throwaway branches MUST keep these notes current, even if merged later. -->

## Decisions

<!-- Record finalized choices with enough rationale for reviewers to understand the tradeoffs. -->

## Tasks

<!-- Reference `.specman/scratchpad/<scratch-name>/tasks.md` if present, or maintain a lightweight checklist here. -->

## Next Steps

<!-- Outline immediate follow-up actions, owners, and checkpoints required to land the fix. -->
