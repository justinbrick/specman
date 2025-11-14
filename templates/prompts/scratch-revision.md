You are applying user suggestions to revise the specification {{target_spec_path}} within the scratch pad "{{scratch_name}}".

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the revision:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (≤4 words) that reflects the revision scope, ensuring it meets SpecMan naming constraints, and set `{{scratch_name}}` to that value.
2. Copy `templates/scratch/scratch.md`, retain HTML comments until fulfilled, and update the front matter with `target: {{target_spec_path}}`, `branch: {{branch_name}}` (or the branch you will create in Step 5 using `{target_name}/revision/{{scratch_name}}`), and `work_type: { revision: { revised_headings: {{revised_headings}} } }`.
3. Use Context plus Scope & Goals to summarize {{change_summary}}, the impacted headings, and acceptance criteria, keeping the prose concise and decision-ready.
4. Track supporting analysis in Notes, record firm Decisions with rationale, and convert remaining work into Tasks with handoff-ready Next Steps, then ensure the result complies with `spec/specman-data-model/spec.md` requirements for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
5. After the scratch pad is finalized, create and check out the Git branch combining the target artifact identifier and scratch pad name (for example `git switch -c {target_name}/revision/{{scratch_name}}` or reuse `{{branch_name}}` if supplied) so further commits track the revision work there.
