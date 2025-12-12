# Scratch Pad Prompt — Refactor

You are gathering refactor discovery insights in a scratch pad whose name (`{{output_name}}`) you must infer from the supplied inputs, keeping it compliant with `spec/specman-data-model/spec.md` naming rules while focusing on implementation {{target_path}}.

Before any analysis, complete these reading prerequisites:

- Study `spec/specman-data-model/spec.md` along with its dependencies (including `docs/founding-spec.md`) so the refactor aligns with current guidance.
- Read {{target_path}}, its governing specification, and every dependency/reference declared in those files to ensure the refactor considers all touchpoints.

Read the following dependencies before continuing:
{{context}}

User-provided input:
{{arguments}}

Steps:

1. Create or refine a short, lowercase, hyphenated scratch pad name (≤4 words) that captures the refactor focus, ensuring it satisfies SpecMan naming constraints, and apply it to `{{output_name}}` moving forward.
2. Copy `templates/scratch/scratch.md`, retain HTML comments until satisfied, and set the front matter with `target: {{target_path}}` and `work_type: { ref: { refactored_headings: [...] } }`, filling `refactored_headings` from the refactor focus described in the User Input section.
3. {{branch_name_or_request}}
4. Capture the current architecture, issues, and constraints in Context plus Scope & Goals, grounding the plan in the provided context and dependencies where applicable, then seed Notes with findings from the User Input section (code links, experiments, and risks).
5. Build an "Entity & Concept Decomposition" section: enumerate every entity, concept, module, or API touched inside the target and its governing spec, outline the planned refactor for each (what changes, why, downstream effect, validation, open questions), and convert this breakdown into a staged implementation plan for downstream execution.
6. Add a dedicated step that confirms every structure or function touched by the refactor has its code comments updated (or newly added) to describe what changed and why. Experimental branches are not exempt—experiments MUST keep these comments current too.
7. Record emerging decisions, candidate tasks, and follow-up experiments so the refactor plan stays actionable. Capture each open question inside the scratch pad (tasks or inline bullets) and restate the full list in your chat response so the user can answer every question alongside the batched decision block in one reply.
8. Verify the scratch pad follows `spec/specman-data-model/spec.md` guidance for Scratch Pads, Work Type, Scratch Pad Content, and Git Branches before returning `.specman/scratchpad/{{output_name}}/scratch.md`.

## User Input

- Provide the refactor guidance here. Keep this section at the bottom so user input stays isolated from the prompt structure.
