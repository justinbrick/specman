# Scratch Pad Prompt — Feature

## Scope

Your task is to create the scratch pad artifact and then fill it out with a concrete plan.

- Do NOT implement the feature.
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
    - `scratchKind`: `"feat"`
    - `intent` (required string): a concise, plain-language summary of the User Input requirements + constraints for this scratch pad. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
2. After `create_artifact` returns, infer `scratch_pad_name` from the returned handle (it will look like `scratch://{scratch_pad_name}`), then create and check out a branch:
    - Branch naming: `<target_name>/feat/<scratch_pad_name>` (example: `specman-mcp-rust/feat/action-being-done`).
    - If the branch does not exist yet: `git checkout -b <target_name>/feat/<scratch_pad_name>`.
    - If it already exists: `git checkout <target_name>/feat/<scratch_pad_name>`.
3. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following:
    - Feature requirements breakdown: goals, non-goals, acceptance criteria, edge cases, and constraints.
    - Implementation breakdown (requested language): outline the major components/modules, data structures, APIs, and execution flow; include a staged implementation plan (milestones) that could be executed later.
    - Documentation plan: identify which implementation document(s) must be updated, what new sections/headings should be added, and what content must be recorded there to document the feature.
    - Spec alignment: map each requirement/decision back to the governing specification headings; call out any mismatch or missing spec coverage.
    - Open questions: list any ambiguous design choices or missing information as questions to ask the user (do not guess).
4. STOP and return control to the caller.

## User Input

- Provide the feature requirements and any constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
