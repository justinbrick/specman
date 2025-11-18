
You are capturing active feature execution in a scratch pad whose name (`{{scratch_name}}`) you must infer from the provided context, ensuring it satisfies `spec/specman-data-model/spec.md` naming constraints while targeting implementation {{target_impl_path}}.

Before you begin, complete these reading prerequisites:
- Review `spec/specman-data-model/spec.md` and every dependency it lists (for example `docs/founding-spec.md`) so the scratch pad complies with the latest rules.
- Open {{target_impl_path}}, the specification it references, and every dependency/reference enumerated in that implementation so you understand upstream and downstream contracts.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the feature work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Create or refine a short, lowercase, hyphenated scratch pad name (no more than four words) that describes the feature effort, ensuring it meets the SpecMan naming constraints, and assign it to `{{scratch_name}}` for all remaining steps.
2. Copy `templates/scratch/scratch.md`, keep every HTML comment directive until satisfied, and update the front matter with `target: {{target_impl_path}}`, `branch: {{branch_name}}` (or define it now as `{target_name}/feat/{{scratch_name}}` by combining the target artifact identifier with the scratch pad slug), and `work_type: { feat: {} }`.
3. Immediately create (if needed) and check out the branch you just defined (for example `git switch -c {target_name}/feat/{{scratch_name}}` or `git switch {{branch_name}}`) so that all subsequent work happens on the correct branch from the start.
4. Summarize {{objectives}} inside Context plus Scope & Goals, and convert {{task_outline}} into a concise checklist that links to `tasks.md` when present.
5. Use Notes to log discoveries and blockers, capture decisions as they occur, and record immediate follow-ups in Next Steps, then confirm compliance with `spec/specman-data-model/spec.md` requirements for Scratch Pads, Work Type, Git Branches, and Scratch Pad Metadata before returning `.specman/scratchpad/{{scratch_name}}/scratch.md`.
