# Scratch Pad Prompt — Refactor

## Scope

Your task is to create the scratch pad artifact and then fill it out with a refactor decision plan.

- Do NOT start or perform the refactor.
- Do NOT edit code.
- Only edit the newly created scratch pad artifact.
- After the scratch pad is created and filled out, STOP and return control to the caller.

Target: {{target_path}}

Dependencies:

{{context}}

Steps:

1. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:
    - `kind`: `"scratch_pad"`
    - `target`: `"{{target_path}}"`
    - `scratchKind`: `"ref"`
    - `intent` (required string): a concise, plain-language summary of the User Input requirements + constraints for this scratch pad. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
2. After `create_artifact` returns, infer `scratch_pad_name` from the returned handle (it will look like `scratch://{scratch_pad_name}`), then create and check out a branch:
    - Branch naming: `<target_name>/ref/<scratch_pad_name>` (example: `specman-mcp-rust/ref/simplify-tools`).
    - If the branch does not exist yet: `git checkout -b <target_name>/ref/<scratch_pad_name>`.
    - If it already exists: `git checkout <target_name>/ref/<scratch_pad_name>`.
3. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following (do not refactor yet):
    - Current-state inventory: key modules, data flows, and pain points motivating the refactor.
    - Refactor options: list at least 2 viable approaches; compare pros/cons, complexity, and migration risk.
    - Refactor plan: propose a staged approach (safe intermediate commits) with invariants to preserve.
    - Spec alignment: identify the governing specification constraints and confirm the refactor preserves externally observable behavior unless explicitly allowed.
    - Open questions: list any ambiguous design choices as questions to ask the user (do not guess).
4. STOP and return control to the caller.

## User Input

- Provide the refactor guidance here. Keep this section at the bottom so user input stays isolated from the prompt structure.
