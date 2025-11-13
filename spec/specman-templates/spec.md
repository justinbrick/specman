---
name: specman-templates
version: "1.0.0"
dependencies:
  - ref: https://raw.githubusercontent.com/jbrickley-tcs/specman/refs/heads/main/spec/specman-data-model/spec.md
    optional: false
---

# Specification — SpecMan Templates

This specification defines the authoritative template catalog for SpecMan workspaces so that tooling and AI systems can generate Markdown artifacts aligned with the SpecMan data model.

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

Readers SHOULD review the `specman-data-model` specification declared in this document's dependencies to understand how the templates reinforce the underlying data requirements.

## Concepts

### Concept: template-governance

The template catalog provides the canonical scaffolding for specifications, implementations, and scratch pads referenced by SpecMan tooling.

- The template catalog MUST define one Markdown template for each artifact type mandated by the SpecMan data model: specification, implementation, and scratch pad.
- Each template MUST guarantee that the headings, sections, and required metadata needed for data-model compliance are present by default.
- Template updates MUST remain backward compatible with previously generated artifacts unless the `specman-data-model` specification introduces conflicting requirements.

### Concept: ai-instruction-channel

Template guidance for automated agents is conveyed exclusively through non-rendered comment blocks so that static normative text remains immutable.

- Instructions for AI or other tooling MUST be expressed inside HTML comments (`<!-- ... -->`) and MUST NOT appear in rendered Markdown content.
- Instruction comments MUST clearly identify the responsibility of the consumer (for example, "AI TODO" or "Tooling MUST").
- Instruction comments MUST be placed adjacent to the mutable region they govern and MUST be omitted from sections that are intentionally static.

### Concept: prompt-catalog

The template catalog includes prompt blueprints that equip AI systems to create compliant artifacts for common workflows.

- The prompt catalog MUST expose one prompt template for generating a specification, one prompt template for generating an implementation, and three prompt templates for generating scratch pads.
- Prompt templates MUST direct the AI to honor template instructions, preserve HTML comment guidance, and reference the `specman-data-model` specification when validating output.
- Scratch pad prompt templates MUST target distinct scenarios: discovery (early research), execution (active task tracking), and synthesis (handoff or retrospective work).

## Key Entities

### Entity: markdown-template

A Markdown template is a reusable document skeleton that enforces structure for a target artifact type.

- Each Markdown template MUST contain YAML front matter fields required by the `specman-data-model` specification and MUST reference dependency stubs when applicable.
- Each Markdown template MUST provide placeholder headings that mirror the canonical structure defined by the data model.
- HTML comment instructions in a Markdown template MUST describe how AI or automation SHOULD populate mutable sections without altering static normative text.

### Entity: prompt-template

A prompt template is a Markdown document or snippet that instructs an AI system to generate a compliant artifact.

- Prompt templates MUST declare the expected artifact type, required inputs, and success criteria.
- Prompt templates MUST embed validation reminders referencing the `specman-data-model` specification.
- Prompt templates MAY instruct the AI to gather or confirm contextual metadata before authoring content.

### Entity: scratch-pad-prompt

A scratch pad prompt describes how an AI system should assemble a scratch pad tailored to a specific work scenario.

- Scratch pad prompt templates MUST specify their work type (`feat`, `ref`, or `revision`).
- Scratch pad prompt templates MUST instruct the AI to include front matter capturing target artifact, work type, and branch information required by the data model specification.
- Scratch pad prompt templates SHOULD suggest section layouts suited to their scenario (for example, notes for feat/revision, checklist for ref/feat, and summary for revision).

## Constraints

- Markdown templates for specifications MUST reside in `templates/spec/`, implementation templates MUST reside in `templates/impl/`, and scratch pad templates MUST reside in `templates/scratch/`.
- Prompt templates MUST be stored under `templates/prompts/` with filenames that match their artifact type or scenario.
- Template files MUST retain HTML comment instructions verbatim when copied; downstream automation MAY delete comment blocks only after satisfying their directives.
- Specification, implementation, and scratch pad templates MUST NOT remove or reword the RFC 2119 guidance included in their static sections.
- Prompt templates MUST instruct AI systems to acknowledge and respect any HTML comment instructions present in the target template before generating content.

## Additional Notes

Future iterations MAY extend the prompt catalog with additional scenarios as new workflow patterns emerge. Implementations consuming these templates SHOULD version their prompt integrations to remain aligned with the template catalog.
