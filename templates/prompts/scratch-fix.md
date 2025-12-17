# Scratch Pad Prompt — Fix

## Scope

Your task is to create the scratch pad artifact and then fill it out with a fix decision plan.

- Do NOT implement the fix.
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
    - `scratchKind`: `"fix"`
    - `intent` (required string): a concise, plain-language summary of the User Input defect/fix requirements + constraints. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
2. After `create_artifact` returns, infer `scratch_pad_name` from the returned handle (it will look like `scratch://{scratch_pad_name}`), then create and check out a branch:
    - Branch naming: `<target_name>/fix/<scratch_pad_name>` (example: `specman-mcp-rust/fix/handle-errors`).
    - If the branch does not exist yet: `git checkout -b <target_name>/fix/<scratch_pad_name>`.
    - If it already exists: `git checkout <target_name>/fix/<scratch_pad_name>`.
3. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following (do not implement yet):
    - Observed behavior vs expected behavior; reproduction notes; scope of impact.
    - Candidate fixes: list at least 2 plausible approaches; for each, note risks, blast radius, and required changes.
    - Decision process: pick a preferred fix approach and justify it (or explicitly say what info is missing to decide).
    - Spec compliance check: identify the governing specification statements that apply; confirm the fix does not violate them, or call out where the spec needs revision.
    - Impact review: note any API changes, behavior changes, migrations, tests, docs, and backward-compat concerns.
    - Open questions: list any ambiguous areas or missing details as questions to ask the user (do not guess).
4. STOP and return control to the caller.

## User Input

- Provide the fix details, reproduction clues, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
