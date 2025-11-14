You are gathering refactor discovery insights in the scratch pad "{{scratch_name}}" for implementation {{target_impl_path}}.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the refactor work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (≤4 words) that captures the refactor focus, ensuring it satisfies SpecMan naming constraints, and apply it to `{{scratch_name}}` moving forward.
2. Copy `templates/scratch/scratch.md`, retain HTML comments until satisfied, and set the front matter with `target: {{target_impl_path}}`, `branch: {{branch_name}}` (or the branch you will create in Step 5 using `{target_name}/ref/{{scratch_name}}`), and `work_type: { ref: { refactored_headings: {{refactor_focus}} } }`.
3. Capture the current architecture, issues, and constraints in Context plus Scope & Goals, then seed Notes with {{investigation_notes}} including links to code and experiments.
4. Record emerging decisions, candidate tasks, and follow-up experiments in their sections so the refactor plan stays actionable, then verify the scratch pad follows `spec/specman-data-model/spec.md` guidance for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
5. Once the document is ready, create and check out the Git branch that combines the target artifact identifier and scratch pad name (for example `git switch -c {target_name}/ref/{{scratch_name}}` or reuse `{{branch_name}}` if supplied) so the refactor commits continue on that branch.
