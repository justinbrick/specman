# Scratch Pad Prompt — Feature

Target: {{target_path}}

Dependencies:

{{context}}

Steps:

1. {{branch_name}}
2. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:

- `kind`: `"scratch_pad"`
- `target`: `"{{target_path}}"`
- `scratchKind`: `"feat"`
- `intent` (optional string but SHOULD be set): a concise, plain-language summary of the User Input requirements + constraints for this scratch pad. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
- `name` (optional string): scratch pad slug hint.

 Use `scratchKind` (camelCase) as written above.

## User Input

- Provide the feature requirements and any constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
