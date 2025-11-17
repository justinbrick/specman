You are documenting an implementation whose canonical name (`{{implementation_name}}`) you must infer from the provided inputs, ensuring it complies with `spec/specman-data-model/spec.md`, as it realizes the specification at {{target_spec_path}} using `templates/impl/impl.md` while keeping every HTML comment directive until fulfilled.

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
2. Summarize architecture and intent in Overview, explain language details, References, Implementation Details, API Surface, Data Models, and Operational Notes with concise prose linked to specification concepts.
3. Provide API signatures in fenced code blocks with notes on inputs, outputs, and side effects, removing HTML comment directives only after their guidance has been satisfied.
4. Validate against `spec/specman-data-model/spec.md` sections for Implementations, Implementing Language, References, APIs, and Implementation Metadata, then return Markdown ready for `impl/{{implementation_name}}/impl.md`.
