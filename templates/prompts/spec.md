You are creating a SpecMan specification whose canonical name (`{{spec_name}}`) you must infer from the provided inputs, ensuring it satisfies `spec/specman-data-model/spec.md` naming requirements, using `templates/spec/spec.md` while preserving each HTML comment directive until its instruction is satisfied.

Before interpreting any inputs, complete these reading prerequisites:

- Open `spec/specman-data-model/spec.md` and every dependency it lists (including `docs/founding-spec.md`) so the governing data-model rules are fresh.
- Review each dependency referenced for the target specification (from the provided context or prior drafts), opening the local files/URLs so you understand the upstream requirements those documents introduce.

Read the following dependencies before continuing:
{{context}}

Scratch Pad Requirement (Revision Work):

- Run the revision scratch prompt at `templates/prompts/scratch-revision.md`, feeding it the provided context used here so all change analysis is centralized.
- Provide `{{target_spec_path}}` as the scratch target, keep the derived `{target_name}/revision/{{scratch_name}}` (or supplied `{{branch_name}}`) branch checked out, and let `{{revised_headings}}` capture the sections you expect to update.
- Continue with the specification steps only after `.specman/scratchpad/{{scratch_name}}/scratch.md` reflects the revision scope and its branch is active locally.

Steps:

1. {{artifact_name_or_request}} — confirm or request the compliant specification name before drafting.
2. Copy the canonical template verbatim, then update the YAML front matter with the specification name, semantic version, and dependency list (retain `spec/specman-data-model/spec.md` unless a newer official dependency replaces it).
3. Keep the "Terminology & References" heading and RFC 2119 guidance intact, and flesh out Concepts, Key Entities, Constraints, and Additional Notes with declarative content aligned to the dependencies and the User Input section.
4. Reference related specifications or entities inline, ensuring headings remain unique and dependencies stay non-conflicting while HTML comments are removed only after fulfillment.
5. Conduct an adversarial review by deliberately misinterpreting statements made and all context (including the User Input section): list potential misunderstandings, edge cases, or missing constraints, cite the impacted headings, and turn each finding into a clarifying question for the user before committing to final prose.
6. Summarize the discovered ambiguities/conflicts as issues presented back to the user (for example a table of "risk → why it matters → required clarification"), then integrate any resolved guidance into the draft while clearly flagging unresolved blockers.
7. Validate the result against `spec/specman-data-model/spec.md` (Specifications, Dependencies, Specification Metadata) before returning the Markdown ready for `spec/{{spec_name}}/spec.md`.

## User Input

- Provide the specification request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
