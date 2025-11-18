You are applying user suggestions to revise the specification {{target_spec_path}} using a scratch pad whose name (`{{scratch_name}}`) you must infer from the provided inputs while keeping it compliant with `spec/specman-data-model/spec.md` naming guidance.

Before proceeding, satisfy these reading prerequisites:
- Review `spec/specman-data-model/spec.md` and every dependency it lists (notably `docs/founding-spec.md`) so revision notes stay aligned with the canonical rules.
- Open {{target_spec_path}} and read each dependency from its front matter to understand all upstream specifications driving the revision.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the revision:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (≤4 words) that reflects the revision scope, ensuring it meets SpecMan naming constraints, and set `{{scratch_name}}` to that value.
2. Copy `templates/scratch/scratch.md`, retain HTML comments until fulfilled, and update the front matter with `target: {{target_spec_path}}`, `branch: {{branch_name}}` (or define it now as `{target_name}/revision/{{scratch_name}}` by combining the target artifact identifier with the scratch pad slug), and `work_type: { revision: { revised_headings: {{revised_headings}} } }`.
3. Immediately create (if needed) and check out the branch you just defined (for example `git switch -c {target_name}/revision/{{scratch_name}}` or `git switch {{branch_name}}`) so the revision work proceeds on that branch before editing the scratch pad further.
4. Use Context plus Scope & Goals to summarize {{change_summary}}, the impacted headings, and acceptance criteria, keeping the prose concise and decision-ready.
5. Track supporting analysis in Notes, record firm Decisions with rationale, and convert remaining work into Tasks with handoff-ready Next Steps, then ensure the result complies with `spec/specman-data-model/spec.md` requirements for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
