# Implementation Creation

You are documenting an implementation and must use the MCP tool `create_implementation` to instantiate the canonical implementation template, ensuring it complies with the SpecMan Data Model and realizes the specification at {{target_path}} while keeping every HTML comment directive until fulfilled.

## Request

The user wants the implementation to cover the following requirements, scope, and constraints:

{{request}}

If the request above is missing any of the information you need (scope, target modules/components, interfaces, error handling, or external integrations), you MUST ask the user clarifying questions before proceeding. Continue prompting until you have enough to proceed.

## Prerequisites

- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in the provided context, along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}

## Steps

1. If any prerequisite information is unclear, prompt the user for clarification and wait for their answer.
2. Open the created implementation artifact and fill it out:
    - Break down how the implementation should work: modules/components, key types, interfaces, error handling, data flow, and external integrations.
    - Provide a staged implementation plan (milestones) that could be executed later, including where tests/docs should be added.
    - Traceability: map the implementation sections back to the governing specification headings and constraints.
    - Open questions: if any design choice is uncertain, raise it as a concrete question to the user instead of guessing.
3. STOP and return control to the caller.

## Tool Calls

Only once the user has answered every clarifying question and no unclear areas remain, call the MCP tool `create_implementation` to create a new implementation artifact for the governing specification, following the tool-call schema exposed by the current environment (avoid hard-coding any specific field names). Then fill it out per the steps above.
