# Scratch Pad Prompt — Refactor

## Scope

Your task is to create the scratch pad artifact and then fill it out with a refactor decision plan.

- Do NOT start or perform the refactor.
- Do NOT edit code.
- Only edit the newly created scratch pad artifact.
- After the scratch pad is created and filled out, STOP and return control to the caller.

Target: {{target_path}}

Immediately query the user about the refactor they want to plan — its motivation, scope, and any constraints. Keep asking clarifying questions until you have enough to proceed.

Dependencies:

{{context}}

## Steps

1. Query the user about the refactor guidance and keep prompting until everything is clear.
2. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following (do not refactor yet):
    - Current-state inventory: key modules, data flows, and pain points motivating the refactor.
    - Refactor options: list at least 2 viable approaches; compare pros/cons, complexity, and migration risk.
    - Refactor plan: propose a staged approach (safe intermediate commits) with invariants to preserve.
    - Spec alignment: identify the governing specification constraints and confirm the refactor preserves externally observable behavior unless explicitly allowed.
    - Open questions: list any ambiguous design choices as questions to ask the user (do not guess).
3. STOP and return control to the caller.

## Tool Calls

Only once the user has answered every clarifying question and no unclear areas remain, call the MCP tool `create_refactor` to create a new refactor scratch pad artifact for the given target, following the tool-call schema exposed by the current environment. Then fill it out per the steps above.
