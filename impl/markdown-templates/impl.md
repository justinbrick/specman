---
spec: "../../spec/specman-templates/spec.md"
name: markdown-templates
version: "0.1.0"
location: "templates"
primary_language:
  language: markdown@0.31.2
---

# Implementation — Markdown Templates

This implementation provides a curated set of specification, implementation, and scratch pad Markdown templates kept in the `templates/` workspace directory. Each template aligns with the SpecMan data model defined in `spec/specman-data-model/spec.md`.

## Included Items
- [`templates/spec/spec.md`](../../templates/spec/spec.md) — specification scaffold with required metadata and section placeholders.
- [`templates/impl/impl.md`](../../templates/impl/impl.md) — implementation scaffold including metadata, reference guidance, and API documentation slots.
- [`templates/scratch/scratch.md`](../../templates/scratch/scratch.md) — scratch pad scaffold with structured sections for context, notes, and follow-up actions.
- [`templates/prompts/spec.md`](../../templates/prompts/spec.md) — specification authoring prompt aligned with template requirements.
- [`templates/prompts/impl.md`](../../templates/prompts/impl.md) — implementation authoring prompt that reinforces metadata and reference expectations.
- [`templates/prompts/scratch-feat.md`](../../templates/prompts/scratch-feat.md) — feature execution scratch pad prompt.
- [`templates/prompts/scratch-revision.md`](../../templates/prompts/scratch-revision.md) — revision synthesis scratch pad prompt.
- [`templates/prompts/scratch-ref.md`](../../templates/prompts/scratch-ref.md) — refactor discovery scratch pad prompt.

## Prompt Templates

The prompt catalog in `templates/prompts/` equips automation with starting points that enforce SpecMan norms. Each prompt instructs the AI to copy its companion Markdown template verbatim, preserve HTML comment directives, and validate the result against `spec/specman-data-model/spec.md` before completion. Scratch pad prompts further call out the appropriate `work_type` object shape and scenario-specific sections so their artifacts remain consistent across discovery, execution, and synthesis workflows.

## Specification Template

Authors MAY copy `templates/spec/spec.md` into a new spec directory under `spec/` (for example `spec/new-feature/spec.md`). Replace the placeholder front matter with the canonical `name`, semantic `version`, and dependency list. The template pre-populates a "Terminology & References" section with the RFC 2119 guidance that specs SHOULD include. Subsequent headings provide space to document concepts, entities, and cross-cutting constraints while maintaining unique headings, as required by the data model.

## Implementation Template

Copy `templates/impl/impl.md` into the target implementation folder (for example `impl/new-feature/impl.md`). Update the front matter to point at the governing specification, execution location, and implementing languages. The `references` YAML block illustrates the required shape (`ref`, `type`, `optional`) and SHOULD be replaced with concrete entries if the implementation depends on other specs or implementations. The remaining sections prompt authors to describe code locations, libraries, exposed APIs (with fenced code blocks for signatures), and operational considerations.

## Scratch Pad Template

Scratch pads SHOULD start from `templates/scratch/scratch.md`. Place the copied file in `.specman/scratchpad/<scratch-name>/scratch.md`, adjust the `target` path, optionally set `branch`, and populate the `work_type` object (one of `draft`, `revision`, `feat`, or `ref`) per the data model. The body sections help track context, notes, decisions, and next steps, and include an explicit reminder to link a companion `tasks.md` when present.

## Instantiating Templates

1. Create the destination directory (`spec/<spec-name>/`, `impl/<impl-name>/`, or `.specman/scratchpad/<scratch-name>/`).
2. Copy the appropriate template file from `templates/` into the new directory and rename it if necessary.
3. Replace placeholder metadata and headings with project-specific content, ensuring requirement keywords follow RFC 2119 semantics.
4. Review linked dependencies and references to confirm they exist and that headings remain unique within the document.

## Maintenance Notes

- Update these templates if the SpecMan data model or founding spec adds new required metadata.
- Consider adding automated linting that verifies front matter fields and required headings when new specs or implementations are introduced.
