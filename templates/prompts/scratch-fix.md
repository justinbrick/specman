# Scratch Pad Prompt — Fix

Target: {{target_path}}

Dependencies:

{{context}}

Steps:

1. {{branch_name}}
2. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:

- `kind`: `"scratch_pad"`
- `target`: `"{{target_path}}"`
- `scratchKind`: `"fix"`
- `intent` (optional string but SHOULD be set): a concise, plain-language summary of the User Input defect/fix requirements + constraints. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
- `name` (optional string): scratch pad slug hint.
- `branch` (optional string): explicit branch name to record in scratch front matter.

 Use `scratchKind` (camelCase) as written above.

## User Input

- Provide the fix details, reproduction clues, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
