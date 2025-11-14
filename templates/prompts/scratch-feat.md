
You are capturing active feature execution in the scratch pad "{{scratch_name}}" for implementation {{target_impl_path}}.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the feature work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (no more than four words) that describes the feature effort, ensuring it meets the SpecMan naming constraints, and assign it to `{{scratch_name}}` for all remaining steps.
2. Copy `templates/scratch/scratch.md`, keep every HTML comment directive until satisfied, and update the front matter with `target: {{target_impl_path}}`, `branch: {{branch_name}}` (or the branch you will create in Step 5 using `{target_name}/feat/{{scratch_name}}`), and `work_type: { feat: {} }`.
3. Summarize {{objectives}} inside Context plus Scope & Goals, and convert {{task_outline}} into a concise checklist that links to `tasks.md` when present.
4. Use Notes to log discoveries and blockers, capture decisions as they occur, and record immediate follow-ups in Next Steps, then confirm compliance with `spec/specman-data-model/spec.md` requirements for Scratch Pads, Work Type, Git Branches, and Scratch Pad Metadata before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
5. After the scratch pad is ready, create and check out the Git branch that combines the target artifact and scratch pad name (for example `git switch -c {target_name}/feat/{{scratch_name}}` or reuse `{{branch_name}}` if supplied) so all further commits land on that branch.
