# Scratch Pad Prompt — Revision

## Scope

Your task is to create the scratch pad artifact and then fill it out with a concrete revision plan.

- Do NOT modify the specification yet.
- Do NOT edit code.
- Only edit the newly created scratch pad artifact.
- After the scratch pad is created and filled out, STOP and return control to the caller.

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

1. Call the MCP tool `create_artifact` to create a new revision scratch pad artifact for the given target, following the tool-call schema exposed by the current environment.
2. After `create_artifact` returns, infer `scratch_pad_name` from the returned handle (it will look like `scratch://{scratch_pad_name}`), then create and check out a branch:
    - Branch naming: `<target_name>/revision/<scratch_pad_name>` (example: `specman-core/revision/clarify-tokens`).
    - If the branch does not exist yet: `git checkout -b <target_name>/revision/<scratch_pad_name>`.
    - If it already exists: `git checkout <target_name>/revision/<scratch_pad_name>`.
3. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following:
    - Proposed revision outline: the sections/headings affected, and what will change.
    - Draft wording proposals: write candidate replacement/additional paragraphs and constraint statements.
    - Compatibility notes: what existing behavior/contracts must remain true after the revision.
    - Adversarial review: intentionally misread the proposed wording to find ambiguity or loopholes; list every plausible misinterpretation.
    - Questions for the user: for each ambiguity or missing detail, ask a concrete clarifying question instead of guessing.
4. STOP and return control to the caller.

## User Input

- Provide the revision request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
