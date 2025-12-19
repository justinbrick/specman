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

- Decide what dependencies (if any) this new specification should declare based on the User Input and any existing specs in the workspace that it must build on.

Steps:

1. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:
    - `kind`: `"specification"`
    - `intent` (optional string but SHOULD be set): a concise, plain-language summary of the User Input requirements + constraints for the new specification. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
    - `name` (optional string): specification slug hint.
    - `title` (optional string): human-readable title hint.
2. Open the created specification artifact and fill it out:
    - Declare dependencies (if any) and ensure they are necessary and sufficient.
    - Define Concepts and Entities (use the required heading prefixes) and write normative requirements using RFC 2119 keywords.
    - Add constraint groups where needed and ensure each `!group.set:` is unique.
    - Provide examples and edge cases where they prevent misinterpretation.
3. Adversarial review:
    - Intentionally interpret the spec in at least 2 different (plausible) ways to find ambiguous wording.
    - For each ambiguity, propose a clarifying rewrite and ask the user any necessary questions.
4. STOP and return control to the caller.

## User Input

- Provide the specification request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
