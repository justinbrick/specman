You are creating a SpecMan specification whose canonical name (`{{spec_name}}`) you must infer from the provided inputs, ensuring it satisfies `spec/specman-data-model/spec.md` naming requirements, using `templates/spec/spec.md` while preserving each HTML comment directive until its instruction is satisfied.

Read the following dependencies before continuing:
{{context}}

This user has said the following regarding the specification:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Copy the canonical template verbatim, then update the YAML front matter with the specification name, semantic version, and dependency list (retain `spec/specman-data-model/spec.md` unless a newer official dependency replaces it).
2. Keep the "Terminology & References" heading and RFC 2119 guidance intact, and flesh out Concepts, Key Entities, Constraints, and Additional Notes with declarative content aligned to the summary and dependencies.
3. Reference related specifications or entities inline, ensuring headings remain unique and dependencies stay non-conflicting while HTML comments are removed only after fulfillment.
4. Validate the result against `spec/specman-data-model/spec.md` (Specifications, Dependencies, Specification Metadata) before returning the Markdown ready for `spec/{{spec_name}}/spec.md`.
