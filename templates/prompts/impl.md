# Implementation Creation

You are documenting an implementation and must use the MCP tool `create_artifact` to instantiate the canonical implementation template, ensuring it complies with the SpecMan Data Model and realizes the specification at {{target_path}} while keeping every HTML comment directive until fulfilled.

Before doing anything else, complete these prerequisites:

- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in the provided context, along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}

Steps:

1. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:
    - `kind`: `"implementation"`
    - `target`: `"{{target_spec_path}}"`
    - `intent` (optional string but SHOULD be set): a concise, plain-language summary of the User Input requirements + constraints for the implementation. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
    - `name` (optional string): implementation slug hint.
2. Open the created implementation artifact and fill it out:
    - Confirm the requested implementation language (ask the user if not specified) and record it in the implementation metadata/intro.
    - Break down how the implementation should work in that language: modules/components, key types, interfaces, error handling, data flow, and external integrations.
    - Provide a staged implementation plan (milestones) that could be executed later, including where tests/docs should be added.
    - Traceability: map the implementation sections back to the governing specification headings and constraints.
    - Open questions: if any design choice is uncertain, raise it as a concrete question to the user instead of guessing.
3. STOP and return control to the caller.

## User Input

- Provide the implementation requirements, scope, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
