# Implementation Creation

You are documenting an implementation and must use the MCP tool `create_artifact` to instantiate the canonical implementation template, ensuring it complies with the SpecMan Data Model and realizes the specification at {{target_path}} while keeping every HTML comment directive until fulfilled.

Before doing anything else, complete these prerequisites:

- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in the provided context, along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}

Steps:

1. Call the MCP tool `create_artifact` to create a new implementation artifact for the governing specification, following the tool-call schema exposed by the current environment (avoid hard-coding any specific field names).
2. Open the created implementation artifact and fill it out:
    - Break down how the implementation should work: modules/components, key types, interfaces, error handling, data flow, and external integrations.
    - Provide a staged implementation plan (milestones) that could be executed later, including where tests/docs should be added.
    - Traceability: map the implementation sections back to the governing specification headings and constraints.
    - Open questions: if any design choice is uncertain, raise it as a concrete question to the user instead of guessing.
3. STOP and return control to the caller.

## User Input

- Provide the implementation requirements, scope, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
