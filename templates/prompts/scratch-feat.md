---
artifact_type: scratch-pad
work_type: feat
scenario: execution
inputs:
  - scratch_name
  - target_impl_path
  - branch_name
  - objectives
  - task_outline
success_criteria:
  - front_matter_complete
  - execution_sections_populated
  - html_comments_preserved
  - data_model_validated
---

# Prompt — Scratch Pad (Feature Execution)

Use this blueprint to drive an execution-oriented scratch pad that tracks active feature development.

```
You are preparing the scratch pad "{{scratch_name}}" for feature work tied to the implementation at {{target_impl_path}}.

Inputs you have:
- Branch name: {{branch_name}}
- Objectives: {{objectives}}
- Task outline: {{task_outline}}

Instructions:
1. Copy the canonical scratch pad template from `templates/scratch/scratch.md` and use it without removing any HTML comment directives upfront.
2. Explicitly confirm that you will honor every HTML comment instruction; remove a directive only after fulfilling it.
3. Update the YAML front matter:
   - `target`: set to {{target_impl_path}}.
   - `branch`: use {{branch_name}} or propose one that follows `{target_name}/feat/{{scratch_name}}`.
   - `work_type`: set to `feat: {}` (add key-value pairs if additional execution metadata is needed).
4. Populate the Context and Scope & Goals sections with the objectives and current state of the feature work.
5. Use the Notes section to capture discoveries, blockers, and references encountered while executing the feature.
6. Turn the Tasks section into an actionable checklist that aligns with the supplied outline, linking to `tasks.md` if it exists.
7. Fill the Decisions and Next Steps sections with concise status updates suitable for daily execution tracking.

Validation Checklist:
- Confirm the scratch pad structure and metadata comply with `spec/specman-data-model/spec.md` (sections: Scratch Pads, Work Type, Git Branches, Scratch Pad Metadata).
- Ensure the `feat` work type is the only object present under `work_type`.
- Verify that all HTML comment directives are satisfied or intentionally retained for future action.

Deliverable:
- Return the completed Markdown scratch pad ready to reside at `.specman/scratchpad/{{scratch_name}}/scratch.md`.
```
