# Specification Creation

You are creating a SpecMan specification and must use the MCP tool `create_artifact` to instantiate the canonical specification template, preserving each HTML comment directive until its instruction is satisfied.

## Standards Quick Reference (Standalone)

### Headings checklist (Concepts & Entities)

- Headings that represent concepts MUST begin with the literal prefix `Concept:` followed by a space.
- Headings that represent key entities MUST begin with the literal prefix `Entity:` followed by a space.

### Constraints checklist (constraint groups)

- Constraint sections MUST start with a standalone identifier line in the form `!group.set:`.
- A group set MUST contain **at least two** groups separated by `.`.
 	- Group 1 MUST be the heading slug of the constrained concept/entity heading.
 	- Group 2 MUST be a short category name (for example `formatting`, `ordering`, `referencing`).
- Each constraint identifier line MUST be the only content on its line.
- Within a single document, each group set MUST be unique.

Before interpreting any inputs, complete these reading prerequisites:

- Open the SpecMan Data Model specification and every dependency it lists (including the Founding Spec) so the governing data-model rules are fresh.
- Review each dependency referenced for the target specification (from the provided context or prior drafts), opening the local files/URLs so you understand the upstream requirements those documents introduce.

Read the following dependencies before continuing:

{{context}}

Steps:

1. {{artifact_name_or_request}}
2. Decide the specification version constraints from the User Input and context.
3. Call the MCP tool `create_artifact` to create the Specification, then update the YAML front matter with the specification name, semantic version, and dependency list.
4. Keep the "Terminology & References" heading and RFC 2119 guidance intact, and flesh out Concepts, Key Entities, Constraints, and Additional Notes with declarative content aligned to the dependencies and the User Input section.
5. Reference related specifications or entities inline, ensuring headings remain unique and dependencies stay non-conflicting while HTML comments are removed only after fulfillment.
6. Conduct an adversarial review by deliberately misinterpreting statements made and all context (including the User Input section): list potential misunderstandings, edge cases, or missing constraints, cite the impacted headings, and turn each finding into a clarifying question for the user before committing to final prose.
7. Summarize the discovered ambiguities/conflicts as issues presented back to the user (for example a table of "risk → why it matters → required clarification"), then integrate any resolved guidance into the draft while clearly flagging unresolved blockers.
8. Validate the result against the SpecMan Data Model requirements for Specifications, Dependencies, and Specification Metadata before returning the result.

## User Input

- Provide the specification request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
