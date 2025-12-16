# Implementation Creation

You are documenting an implementation and must use the MCP tool `create_artifact` to instantiate the canonical implementation template, ensuring it complies with the SpecMan Data Model and realizes the specification at {{target_path}} while keeping every HTML comment directive until fulfilled.

Before doing anything else, complete these prerequisites:

- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in the provided context, along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}

Steps:

1. Call the MCP tool `create_artifact` with `kind = implementation` and `target = {{target_spec_path}}`.

## User Input

- Provide the implementation requirements, scope, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
