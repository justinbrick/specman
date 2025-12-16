# Scratch Pad Prompt — Fix

You are documenting an implementation fix inside a scratch pad focusing on implementation {{target_path}}.

Before making changes, complete these reading prerequisites:

- Read {{target_path}}, its governing specification, and every dependency/reference those files declare so you understand every impacted contract.

Read the following dependencies before continuing:

{{context}}

Steps:

1. {{artifact_name_or_request}}
2. {{branch_name_or_request}}
3. Call the MCP tool `create_artifact` to create the Scratch Pad, ensuring:
    - `target` is set to {{target_path}}.
    - `work_type` is `fix` and `fixed_headings` lists each specification heading the fix touches based on the User Input section.
4. Populate the Context, Defect Summary, Reproduction & Evidence, Impact Assessment, and Fix Scope & Goals sections with concrete findings that link back to specs, code, failing tests, or experiments, grounding in the provided context and dependencies where helpful and drawing inputs from the User Input section.
5. Add an "Entity & Concept Remediation Plan" subsection that inventories every concept, entity, module, or API affected inside the target (and its governing spec), explaining what changes, why it fixes the defect, downstream effects, validation steps, and any open questions. Convert this breakdown into a staged execution plan so downstream scratch pads can pick up the work verbatim.
6. Use the Code Comment Updates section to confirm that every function, structure, or configuration touched by the fix has refreshed comments describing what changed and why. Experiments MUST keep these notes up to date as well.
7. Track discoveries in Notes, capture decisions with rationale, and record each open question inside the scratch pad (tasks or inline bullets). Reiterate the entire open-question list in your chat response alongside the batched decision block so stakeholders can answer everything in one reply, then log concrete follow-ups in Next Steps.
8. Verify the scratch pad complies with SpecMan Data Model requirements for Scratch Pads, Work Type (fix), Git Branches, and Metadata before returning the result.

## User Input

- Provide the fix details, reproduction clues, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
