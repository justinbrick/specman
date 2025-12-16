# Scratch Pad Prompt — Feature

You are capturing active feature execution in a scratch pad, targeting implementation {{target_path}}.

Before you begin, complete these reading prerequisites:

- Open {{target_path}}, the specification it references, and every dependency/reference enumerated in that implementation so you understand upstream and downstream contracts.

Read the following dependencies before continuing:

{{context}}

Steps:

1. {{artifact_name_or_request}}
2. {{branch_name_or_request}}
3. Call the MCP tool `create_artifact` to create the Scratch Pad, ensuring `target` is set to {{target_path}} and `work_type` is `feat`.

4. Summarize the feature goals from the User Input section inside Context plus Scope & Goals, grounding the work in the provided context and dependencies where applicable, and convert the planned tasks into a concise checklist that links to `tasks.md` when present.
5. Add an "Entity & Concept Plan" subsection that inventories every relevant entity, concept, module, or API referenced by the target and its governing spec, describing current behavior, the intended feature delta, dependencies impacted, validation checkpoints, and open questions so downstream implementers have a playbook.
6. Insert a numbered step that explicitly confirms every structure or function touched by the feature has updated (or newly added) code comments describing the change and rationale. Call out that experimental codepaths are not exempt—experiments MUST keep these comments current as well.
7. Use Notes to log discoveries and blockers, capture decisions as they occur, and track each unresolved question inside the scratch pad (tasks or inline bullets). Reiterate the full open-question list in your chat response alongside the batched decision block so the user can answer everything in one reply, then record immediate follow-ups in Next Steps.
8. Confirm compliance with SpecMan Data Model requirements for Scratch Pads, Work Type, Git Branches, and Scratch Pad Metadata before returning the result.

## User Input

- Provide the feature requirements and any constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
