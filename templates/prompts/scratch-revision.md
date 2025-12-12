# Scratch Pad Prompt — Revision

You are applying user suggestions to revise the specification referenced by {{target_path}} using a scratch pad.

Before proceeding, satisfy these reading prerequisites:

- Review `spec/specman-data-model/spec.md` and every dependency it lists (notably `docs/founding-spec.md`) so revision notes stay aligned with the canonical rules.
- Open {{target_path}} and read each dependency from its front matter to understand all upstream specifications driving the revision.

Read the following dependencies before continuing:

{{context}}

Steps:

1. {{artifact_name_or_request}}
2. Copy `templates/scratch/scratch.md`, retain HTML comments until fulfilled, and update the front matter with `target: {{target_path}}` and `work_type: { revision: { revised_headings: [...] } }`, filling `revised_headings` from the revision details in the User Input section.
3. {{branch_name_or_request}}
4. Use Context plus Scope & Goals to summarize the requested changes, impacted headings, and acceptance criteria based on the User Input section, keeping the prose concise and decision-ready while grounding in the provided context and dependencies when relevant.
5. Run a conflict audit before drafting changes: scan {{target_path}} and every dependency for statements that could contradict the proposed update, capture each conflict with the original quote, heading, requirement level, and a clarifying question, then flag whether the new change should override, amend, or respect the existing statement. Add each conflict as a task (inline or in `tasks.md`) and restate the full set in your chat response as one consolidated decision block.
6. Track supporting analysis in Notes, record firm Decisions with rationale, and convert remaining work into Tasks with handoff-ready Next Steps, then ensure the result complies with `spec/specman-data-model/spec.md` requirements for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning the pad in `.specman/scratchpad/<scratch-name>/scratch.md`.

## User Input

- Provide the revision request and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
