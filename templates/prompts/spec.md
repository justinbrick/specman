You are creating a SpecMan specification whose canonical name (`{{spec_name}}`) you must infer from the provided inputs, ensuring it satisfies `spec/specman-data-model/spec.md` naming requirements, using `templates/spec/spec.md` while preserving each HTML comment directive until its instruction is satisfied.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the specification:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Scratch Pad Requirement (Revision Work):
- Run the revision scratch prompt at `templates/prompts/scratch-revision.md`, feeding it the same {{context}} and {{arguments}} used here so all change analysis is centralized.
- Provide `{{target_spec_path}}` as the scratch target, keep the derived `{target_name}/revision/{{scratch_name}}` (or supplied `{{branch_name}}`) branch checked out, and let `{{revised_headings}}` capture the sections you expect to update.
- Continue with the specification steps only after `.specman/scratchpad/{{scratch_name}}/scratch.md` reflects the revision scope and its branch is active locally.

Steps:
1. Copy the canonical template verbatim, then update the YAML front matter with the specification name, semantic version, and dependency list (retain `spec/specman-data-model/spec.md` unless a newer official dependency replaces it).
2. Keep the "Terminology & References" heading and RFC 2119 guidance intact, and flesh out Concepts, Key Entities, Constraints, and Additional Notes with declarative content aligned to the summary and dependencies.
3. Reference related specifications or entities inline, ensuring headings remain unique and dependencies stay non-conflicting while HTML comments are removed only after fulfillment.
4. Validate the result against `spec/specman-data-model/spec.md` (Specifications, Dependencies, Specification Metadata) before returning the Markdown ready for `spec/{{spec_name}}/spec.md`.
