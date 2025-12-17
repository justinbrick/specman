# Scratch Pad Prompt — Revision

You are applying user suggestions to revise the specification referenced by {{target_path}} using a scratch pad.

## Standards Quick Reference (Standalone)

### Specification-structure reminders (apply to the revised spec)

- Headings that represent concepts MUST begin with the literal prefix `Concept:` followed by a space.
- Headings that represent key entities MUST begin with the literal prefix `Entity:` followed by a space.

Constraint groups (when adding/updating constraints):

- A constraint section starts with a standalone identifier line in the form `!group.set:`.
- The group set MUST contain at least two dot-delimited groups:
  - Group 1: the heading slug of the constrained concept/entity.
  - Group 2: a short category label (for example `formatting`, `ordering`, `referencing`).
- Each constraint identifier line MUST be the only content on its line.
- Each group set MUST be unique within the document.

Before proceeding, satisfy these reading prerequisites:

- Open {{target_path}} and read each dependency from its front matter to understand all upstream specifications driving the revision.

Read the following dependencies before continuing:

{{context}}

Steps:

1. {{branch_name}}
2. Call the MCP tool `create_artifact` with a JSON object that sets the exact schema fields:

- `kind`: `"scratch_pad"`
- `target`: `"{{target_path}}"`
- `scratchKind`: `"revision"`
- `intent` (optional string but SHOULD be set): a concise, plain-language summary of the User Input revision requests + constraints. This is used to drive sampling/elicitation—include the actual requirements, not placeholders.
- `name` (optional string): scratch pad slug hint.
- `branch` (optional string): explicit branch name to record in scratch front matter.

  Use `scratchKind` (camelCase) as written above.

## User Input

- Provide the revision request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
