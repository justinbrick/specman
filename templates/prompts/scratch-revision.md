---
artifact_type: scratch-pad
work_type: revision
scenario: synthesis
inputs:
  - scratch_name
  - target_spec_path
  - branch_name
  - revised_headings
  - change_summary
success_criteria:
  - front_matter_complete
  - revision_scope_captured
  - html_comments_preserved
  - data_model_validated
---

# Prompt — Scratch Pad (Revision Synthesis)

Use this blueprint to synthesize outcomes and hand off work related to a specification revision.

```
You are summarizing the scratch pad "{{scratch_name}}" for a specification revision affecting {{target_spec_path}}.

Inputs you have:
- Branch name: {{branch_name}}
- Revised headings: {{revised_headings}}
- Change summary: {{change_summary}}

Instructions:
1. Load the template at `templates/scratch/scratch.md`, copying it verbatim while preserving all HTML comment guidance.
2. State explicitly that every HTML comment directive will remain in place until you fulfill it.
3. Update the YAML front matter:
   - `target`: set to {{target_spec_path}}.
   - `branch`: use {{branch_name}} or craft one following `{target_name}/revision/{{scratch_name}}`.
   - `work_type`: replace the placeholder with the `revision` object containing a `revised_headings` list (one Markdown heading fragment per entry) populated from {{revised_headings}}.
4. Use Context to recap the specification areas touched and any prerequisites for acceptance.
5. Capture scope, goals, and non-goals in Scope & Goals, emphasizing how the revision aligns with quality criteria.
6. In Notes, record investigation details and references that future reviewers need.
7. Document final decisions, including rationale, in the Decisions section.
8. Summarize outstanding work in Tasks and translate them into handoff-ready next steps in Next Steps.

Validation Checklist:
- Ensure the scratch pad satisfies the metadata and structural requirements detailed in `spec/specman-data-model/spec.md` (sections: Scratch Pads, Work Type, Scratch Pad Content, Git Branches).
- Confirm that only the `revision` object is present under `work_type` and that `revised_headings` references real headings.
- Verify all HTML comment directives are satisfied or intentionally left for follow-up.

Deliverable:
- Return the finalized Markdown scratch pad prepared for `.specman/scratchpad/{{scratch_name}}/scratch.md`.
```
