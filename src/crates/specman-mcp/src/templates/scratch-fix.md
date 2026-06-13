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

1. Call the MCP tool `create_fix` to create a new fix scratch pad artifact for the given target, following the tool-call schema exposed by the current environment.
2. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following (do not implement yet):
    - Observed behavior vs expected behavior; reproduction notes; scope of impact.
    - Candidate fixes: list at least 2 plausible approaches; for each, note risks, blast radius, and required changes.
    - Decision process: pick a preferred fix approach and justify it (or explicitly say what info is missing to decide).
    - Spec compliance check: identify the governing specification statements that apply; confirm the fix does not violate them, or call out where the spec needs revision.
    - Impact review: note any API changes, behavior changes, migrations, tests, docs, and backward-compat concerns.
    - Open questions: list any ambiguous areas or missing details as questions to ask the user (do not guess).
3. STOP and return control to the caller.

## User Input

- Provide the fix details, reproduction clues, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
