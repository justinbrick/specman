# Scratch Pad Prompt — Revision

## Scope

Your task is to create the scratch pad artifact and then fill it out with a concrete revision plan.

- Do NOT modify the specification yet.
- Do NOT edit code.
- Only edit the newly created scratch pad artifact.
- After the scratch pad is created and filled out, STOP and return control to the caller.

You are applying user suggestions to revise the specification referenced by {{target_path}} using a scratch pad.

## Request

The user wants to make the following revision:

{{request}}

If the request above is missing any of the information you need (which sections/headings change and why, or new constraints), you MUST ask the user clarifying questions before proceeding. Continue prompting until you have enough to proceed.

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

## Steps

1. If any prerequisite information is unclear, prompt the user for clarification and wait for their answer.
2. Open the created scratch pad artifact (use the returned handle/path) and fill it out with the following:
    - Proposed revision outline: the sections/headings affected, and what will change.
    - Draft wording proposals: write candidate replacement/additional paragraphs and constraint statements.
    - Compatibility notes: what existing behavior/contracts must remain true after the revision.
    - Adversarial review: intentionally misread the proposed wording to find ambiguity or loopholes; list every plausible misinterpretation.
    - Questions for the user: for each ambiguity or missing detail, ask a concrete clarifying question instead of guessing.
3. STOP and return control to the caller.

## Tool Calls

Only once the user has answered every clarifying question and no unclear areas remain, call the MCP tool `create_revision` to create a new revision scratch pad artifact for the given target, following the tool-call schema exposed by the current environment. Then fill it out per the steps above.
