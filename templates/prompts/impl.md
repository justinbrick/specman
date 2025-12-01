You are documenting an implementation whose canonical name (`{{implementation_name}}`) you must infer from the provided inputs, ensuring it complies with `spec/specman-data-model/spec.md`, as it realizes the specification at {{target_spec_path}} using `templates/impl/impl.md` while keeping every HTML comment directive until fulfilled.

Before doing anything else, complete these prerequisites:
- Study `spec/specman-data-model/spec.md` plus its declared dependencies (for example `docs/founding-spec.md`) so you apply the latest implementation rules.
- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in {{context}} (including source paths under `impl/`), along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}


This user has said the following regarding the implementation work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Scratch Pad Requirement (Feature Work):
- Run the dedicated feature scratch prompt at `templates/prompts/scratch-feat.md`, reusing the same {{context}} plus {{arguments}} you have here so that all discovery happens in one place.
- Set `{{target_impl_path}}` in that prompt to `impl/{{implementation_name}}/impl.md`, ensuring the scratch pad tracks this implementation doc and that its branch follows `{target_name}/feat/{{scratch_name}}` (or an explicitly provided `{{branch_name}}`).
- Do not proceed until `.specman/scratchpad/{{scratch_name}}/scratch.md` exists and the feature branch from the scratch prompt is checked out locally.

Steps:
1. Copy the canonical template, set `spec` to {{target_spec_path}}, fill `location`, `library` (if applicable), and describe `primary_language` plus optional `secondary_languages`, replacing the references list with {{reference_items}}.
2. Summarize architecture and intent in Overview, explain language details, References, Implementation Details, and Operational Notes with concise prose that cites specification headings via inline links.
3. In `## Concept & Entity Breakdown`, enumerate every concept and entity from the governing specification that this implementation covers. Each heading MUST include an inline link to the originating spec fragment (for example `[Concept: Lifecycle](../spec/spec.md#concept-lifecycle)`) and contain:
	- A narrative describing how the concept/entity is realized, with additional inline links to related headings.
	- API signatures scoped to that concept/entity (fenced code blocks with notes on inputs, outputs, invariants, and dependencies).
	- For entities, an embedded Data Model snippet plus any constraints.
4. Remove HTML comment directives only after their guidance is satisfied and validate against `spec/specman-data-model/spec.md` sections for Implementations, Implementing Language, References, APIs, and Implementation Metadata before returning Markdown ready for `impl/{{implementation_name}}/impl.md`.
