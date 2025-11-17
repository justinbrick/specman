You are gathering refactor discovery insights in a scratch pad whose name (`{{scratch_name}}`) you must infer from the supplied inputs, keeping it compliant with `spec/specman-data-model/spec.md` naming rules while focusing on implementation {{target_impl_path}}.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the refactor work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (≤4 words) that captures the refactor focus, ensuring it satisfies SpecMan naming constraints, and apply it to `{{scratch_name}}` moving forward.
2. Copy `templates/scratch/scratch.md`, retain HTML comments until satisfied, and set the front matter with `target: {{target_impl_path}}`, `branch: {{branch_name}}` (or define it now as `{target_name}/ref/{{scratch_name}}` by pairing the target artifact identifier with the scratch pad slug), and `work_type: { ref: { refactored_headings: {{refactor_focus}} } }`.
3. Immediately create (if needed) and check out the branch you just defined (for example `git switch -c {target_name}/ref/{{scratch_name}}` or `git switch {{branch_name}}`) so the refactor work proceeds on the correct branch from the outset.
4. Capture the current architecture, issues, and constraints in Context plus Scope & Goals, then seed Notes with {{investigation_notes}} including links to code and experiments.
5. Record emerging decisions, candidate tasks, and follow-up experiments in their sections so the refactor plan stays actionable, then verify the scratch pad follows `spec/specman-data-model/spec.md` guidance for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
