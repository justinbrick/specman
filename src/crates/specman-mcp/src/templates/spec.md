# Specification Creation

You are creating a SpecMan specification named {{name}} and must use the MCP tool `create_specification` to instantiate the canonical specification template, preserving each HTML comment directive until its instruction is satisfied.

Immediately query the user about what they want this specification to define — its goals, scope boundaries, key concepts and entities, and any constraints. Keep asking clarifying questions until you have enough information to author an unambiguous specification.

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

- Decide what dependencies (if any) this new specification should declare based on what the user describes and any existing specs in the workspace that it must build on.

Steps:

1. Query the user about what the specification must define, and keep prompting until every requirement is clear.
2. Open the created specification artifact and fill it out:
    - Declare dependencies (if any) and ensure they are necessary and sufficient.
    - Define Concepts and Entities (use the required heading prefixes) and write normative requirements using RFC 2119 keywords.
    - Add constraint groups where needed and ensure each `!group.set:` is unique.
    - Provide examples and edge cases where they prevent misinterpretation.
3. Adversarial review:
    - Intentionally interpret the spec in at least 2 different (plausible) ways to find ambiguous wording.
    - For each ambiguity, propose a clarifying rewrite and ask the user any necessary questions.
4. STOP and return control to the caller.

## Tool Calls

Only once the user has answered every clarifying question and no unclear areas remain, call the MCP tool `create_specification` to create the new specification artifact (named {{name}}), following the tool-call schema exposed by the current environment (do not rely on older examples that enumerate specific fields). Then fill it out per the steps above.
