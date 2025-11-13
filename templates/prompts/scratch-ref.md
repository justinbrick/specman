---
artifact_type: scratch-pad
work_type: ref
scenario: discovery
inputs:
  - scratch_name
  - target_impl_path
  - branch_name
  - refactor_focus
  - investigation_notes
success_criteria:
  - front_matter_complete
  - discovery_insights_logged
  - html_comments_preserved
  - data_model_validated
---

# Prompt — Scratch Pad (Refactor Discovery)

Use this blueprint to guide discovery work for refactoring an implementation.

```
You are capturing exploratory notes in the scratch pad "{{scratch_name}}" for a refactor of {{target_impl_path}}.

Inputs you have:
- Branch name: {{branch_name}}
- Refactor focus: {{refactor_focus}}
- Investigation notes: {{investigation_notes}}

Instructions:
1. Copy `templates/scratch/scratch.md` as the baseline document, ensuring all HTML comment directives remain intact initially.
2. Affirm that you will preserve every HTML comment directive, removing them only after the instructions are satisfied.
3. Update the YAML front matter:
   - `target`: set to {{target_impl_path}}.
   - `branch`: use {{branch_name}} or suggest one following `{target_name}/ref/{{scratch_name}}`.
   - `work_type`: replace the placeholder with the `ref` object that includes a `refactored_headings` list summarizing the areas under review.
4. Fill the Context section with the current architecture description, known issues, and baseline performance metrics if available.
5. In Scope & Goals, outline the exploratory questions and constraints guiding the refactor.
6. Use the Notes section to log investigation results, links to code locations, and experiments—focus on gathering data for decision-making.
7. Record preliminary Decisions as they emerge, even if they require validation.
8. Capture potential task candidates in Tasks and propose follow-up experiments or redesign steps in Next Steps.

Validation Checklist:
- Confirm compliance with `spec/specman-data-model/spec.md` (sections: Scratch Pads, Work Type, Scratch Pad Content, Git Branches).
- Ensure the `ref` work type appears alone under `work_type` and its `refactored_headings` list references real headings.
- Verify that HTML comment directives are fulfilled or documented for later completion.

Deliverable:
- Return the Markdown scratch pad suitable for `.specman/scratchpad/{{scratch_name}}/scratch.md`.
```
