---
spec: "../../spec/specman-core/spec.md"
name: markdown-templates
version: "0.2.0"
location: "templates"
primary_language:
  language: markdown@0.31.2
---

# Implementation — Markdown Templates

This implementation provides a curated set of specification, implementation, and scratch pad Markdown templates kept in the `templates/` workspace directory. Each template aligns with the SpecMan data model defined in `spec/specman-data-model/spec.md`.

## Included Items

- [`templates/spec/spec.md`](../../templates/spec/spec.md) — specification scaffold with inline concept/entity constraint guidance baked directly into each heading.
- [`templates/impl/impl.md`](../../templates/impl/impl.md) — implementation scaffold that embeds per-section constraint reminders, adds a Concept & Entity Breakdown with per-heading API signatures, and requires inline links back to governing spec fragments.
- [`templates/scratch/scratch.md`](../../templates/scratch/scratch.md) — general scratch pad scaffold with structured sections for context, notes, and follow-up actions across feat/ref/revision work.
- [`templates/scratch/fix.md`](../../templates/scratch/fix.md) — fix-focused scratch pad scaffold that captures defect context, reproduction steps, and `fixed_headings` metadata.
- [`templates/prompts/spec.md`](../../templates/prompts/spec.md) — specification authoring prompt aligned with template requirements.
- [`templates/prompts/impl.md`](../../templates/prompts/impl.md) — implementation authoring prompt that reinforces metadata and reference expectations.
- [`templates/prompts/scratch-feat.md`](../../templates/prompts/scratch-feat.md) — feature execution scratch pad prompt.
- [`templates/prompts/scratch-revision.md`](../../templates/prompts/scratch-revision.md) — revision synthesis scratch pad prompt.
- [`templates/prompts/scratch-ref.md`](../../templates/prompts/scratch-ref.md) — refactor discovery scratch pad prompt.
- [`templates/prompts/scratch-fix.md`](../../templates/prompts/scratch-fix.md) — fix remediation scratch pad prompt.

## Prompt Templates

The prompt catalog in `templates/prompts/` equips automation with starting points that enforce SpecMan norms. Each prompt instructs the AI to copy its companion Markdown template verbatim, preserve HTML comment directives, and validate the result against `spec/specman-data-model/spec.md` before completion. Scratch pad prompts further call out the appropriate `work_type` object shape and scenario-specific sections so their artifacts remain consistent across discovery, execution, synthesis, and fix workflows.

Interactive guardrails ensure upstream requirements stay visible:

- `templates/prompts/spec.md` now performs an adversarial requirement review, intentionally misinterpreting inputs to surface ambiguities and returning the resulting issues/questions to the user before finalizing prose.
- `templates/prompts/impl.md` requires authors to enumerate every concept/entity from the governing specification, link directly to those headings, and nest API signatures plus data models beneath each subsection so relationship graphs can be inferred automatically.
- `templates/prompts/scratch-revision.md` introduces a conflict audit that enumerates existing statements (with headings and RFC 2119 levels) that might contradict the requested revision so authors can explicitly confirm overrides or alignments, and records each conflict as a task while reiterating the whole set in the chat response.
- `templates/prompts/scratch-ref.md`, `templates/prompts/scratch-feat.md`, and `templates/prompts/scratch-fix.md` require an "Entity & Concept" breakdown that inventories every affected module/API, outlines the planned change, and produces a staged plan future scratch pads can execute.
- `templates/prompts/scratch-ref.md`, `templates/prompts/scratch-feat.md`, and `templates/prompts/scratch-fix.md` add numbered steps that verify every affected structure or function has updated code comments describing what changed and why; experiments are explicitly covered so exploratory branches stay documented. These prompts also require teams to track unresolved questions (tasks or inline bullets) **and** repeat the entire question list in their chat response alongside the batched decision block so reviewers can respond in one pass.
- `templates/prompts/scratch-fix.md` additionally forces authors to capture reproduction steps, impact assessment, and `fixed_headings` alignment so defect coverage is explicit.
- Scratch pad prompts now require the `{{artifact_name_or_request}}` token, which is itself the entire instruction for providing or requesting the scratch pad name; it must appear as its own numbered step and must not be surrounded with extra naming prose. Specification and implementation prompts leave artifact-name confirmation to their own context. Prompt templates MUST NOT render `{{output_name}}`; only non-prompt templates resolve that token.
- The branch token was renamed to `{{branch_name_or_request}}` and is used as a standalone step whose expansion carries the instructions: check out the provided branch when present, or ask the reader to generate a compliant `{target_name}/{work_type}/{scratch_pad_name}` branch (with an example) when absent.

## Specification Template

Authors MAY copy `templates/spec/spec.md` into a new spec directory under `spec/` (for example `spec/new-feature/spec.md`). Replace the placeholder front matter with the canonical `name`, semantic `version`, and dependency list. The template pre-populates a "Terminology & References" section with the RFC 2119 guidance that specs SHOULD include, and each concept/entity heading now embeds its own constraint checklist so requirements stay co-located with the owning section.

## Implementation Template

Copy `templates/impl/impl.md` into the target implementation folder (for example `impl/new-feature/impl.md`). Update the front matter to point at the governing specification, execution location, and implementing languages. The `references` YAML block illustrates the required shape (`ref`, `type`, `optional`) and SHOULD be replaced with concrete entries if the implementation depends on other specs or implementations. Every major section now reminds authors to capture the constraints or guarantees relevant to that slice of the system, and the new Concept & Entity Breakdown section enforces one heading per specification concept/entity—each heading includes an inline link, localized constraints, API signatures, and (for entities) the supporting data model so relationship graphs can be derived from the Markdown alone.

## Scratch Pad Template

Scratch pads SHOULD start from `templates/scratch/scratch.md` for feat/ref/revision work, and `templates/scratch/fix.md` for implementation fixes. Place the copied file in `.specman/scratchpad/<scratch-name>/scratch.md`, adjust the `target` path, optionally set `branch`, and populate the `work_type` object (`draft`, `revision`, `feat`, `ref`, or `fix`) per the data model. The body sections help track context, notes, decisions, and next steps, call out the code-comment audit, and include an explicit reminder to link a companion `tasks.md` when present. The fix template adds dedicated sections for reproduction steps, impact assessment, and `fixed_headings` coverage so defects remain traceable to their governing specifications.

## Instantiating Templates

1. Create the destination directory (`spec/<spec-name>/`, `impl/<impl-name>/`, or `.specman/scratchpad/<scratch-name>/`).
2. Copy the appropriate template file from `templates/` into the new directory and rename it if necessary.
3. Replace placeholder metadata and headings with project-specific content, ensuring requirement keywords follow RFC 2119 semantics.
4. Review linked dependencies and references to confirm they exist and that headings remain unique within the document.

## Maintenance Notes

- Update these templates if the SpecMan data model or founding spec adds new required metadata.
- Consider adding automated linting that verifies front matter fields and required headings when new specs or implementations are introduced.
