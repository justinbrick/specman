# Scratch Pad Prompt — Feature

## Scope

Your task is to create the scratch pad artifact and then fill it out with a concrete plan.

- Do NOT implement the feature.
- Do NOT edit code.
- Only edit the newly created scratch pad artifact.
- After the scratch pad is created and filled out, STOP and return control to the caller.

Target: {{target_path}}

## Request

The user wants to plan the following feature:

{{request}}

If the request above is missing any of the information you need (goals, non-goals, acceptance criteria, or constraints), you MUST ask the user clarifying questions before proceeding. Continue prompting until you have enough to proceed.

Dependencies:

{{context}}

## Steps

1. If any requirement is unclear, prompt the user for clarification and wait for their answer.
2. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following:
    - Feature requirements breakdown: goals, non-goals, acceptance criteria, edge cases, and constraints.
    - Implementation breakdown: outline the major components/modules, data structures, APIs, and execution flow; include a staged implementation plan (milestones) that could be executed later.
    - Documentation plan: identify which implementation document(s) must be updated, what new sections/headings should be added, and what content must be recorded there to document the feature.
    - Spec alignment: map each requirement/decision back to the governing specification headings; call out any mismatch or missing spec coverage.
    - Open questions: list any ambiguous design choices or missing information as questions to ask the user (do not guess).
3. STOP and return control to the caller.

## Tool Calls

Only once the user has answered every clarifying question and no unclear areas remain, call the MCP tool `create_feature` to create a new feature scratch pad artifact for the given target, following the tool-call schema exposed by the current environment. Then fill it out per the steps above.
