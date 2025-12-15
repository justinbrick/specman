# Scratch Pad Prompt — Refactor

You are gathering refactor discovery insights in a scratch pad focusing on implementation {{target_path}}.

Before any analysis, complete these reading prerequisites:

- Study the SpecMan Data Model specification and its dependencies (including the Founding Spec) so the refactor aligns with current guidance.
- Read {{target_path}}, its governing specification, and every dependency/reference declared in those files to ensure the refactor considers all touchpoints.

Read the following dependencies before continuing:

{{context}}

Steps:

1. Decide the scratch pad name (lowercase, hyphenated, and no more than 4 words) unless already provided by your environment.
2. Create and check out the git branch derived from the scratch pad rules (or use the branch name provided by your environment).
3. Call the MCP tool `create_artifact` to create the Scratch Pad, ensuring:
    - `target` is set to {{target_path}}.
    - `work_type` is `ref` and `refactored_headings` is populated from the refactor focus described in the User Input section.
4. Capture the current architecture, issues, and constraints in Context plus Scope & Goals, grounding the plan in the provided context and dependencies where applicable, then seed Notes with findings from the User Input section (code links, experiments, and risks).
5. Build an "Entity & Concept Decomposition" section: enumerate every entity, concept, module, or API touched inside the target and its governing spec, outline the planned refactor for each (what changes, why, downstream effect, validation, open questions), and convert this breakdown into a staged implementation plan for downstream execution.
6. Add a dedicated step that confirms every structure or function touched by the refactor has its code comments updated (or newly added) to describe what changed and why. Experimental branches are not exempt—experiments MUST keep these comments current too.
7. Record emerging decisions, candidate tasks, and follow-up experiments so the refactor plan stays actionable. Capture each open question inside the scratch pad (tasks or inline bullets) and restate the full list in your chat response so the user can answer every question alongside the batched decision block in one reply.
8. Verify the scratch pad follows SpecMan Data Model guidance for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning the result.

## User Input

- Provide the refactor guidance here. Keep this section at the bottom so user input stays isolated from the prompt structure.
