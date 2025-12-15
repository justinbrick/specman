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

### Concept: Template Governance

The template catalog provides the canonical scaffolding for specifications, implementations, and scratch pads referenced by SpecMan tooling.

- The template catalog MUST define one Markdown template for each artifact type mandated by the SpecMan data model: specification, implementation, and scratch pad.
- Each template MUST guarantee that the headings, sections, and required metadata needed for data-model compliance are present by default.
- Template updates MUST remain backward compatible with previously generated artifacts unless the `specman-data-model` specification introduces conflicting requirements.
- Implementation templates MUST enumerate every concept and entity from their governing specification under dedicated headings that link back to the source spec and include localized API signatures (and entity data models) so relationship graphs can be derived from the Markdown itself.

### Concept: AI Instruction Channel

Template guidance for automated agents is conveyed exclusively through non-rendered comment blocks so that static normative text remains immutable.

- Instructions for AI or other tooling MUST be expressed inside HTML comments (`<!-- ... -->`) and MUST NOT appear in rendered Markdown content.
- Instruction comments MUST clearly identify the responsibility of the consumer (for example, "AI TODO" or "Tooling MUST").
- Instruction comments MUST be placed adjacent to the mutable region they govern and MUST be omitted from sections that are intentionally static.

### Concept: Prompt Catalog

The template catalog includes prompt blueprints that equip AI systems to create compliant artifacts for common workflows.

- The prompt catalog MUST expose one prompt template for generating a specification, one prompt template for generating an implementation, and four prompt templates for generating scratch pads (discovery, execution, synthesis, and fix).
- Prompt templates MUST direct the AI to honor template instructions, preserve HTML comment guidance, and reference the `specman-data-model` specification when validating output.
- Scratch pad prompt templates MUST target distinct scenarios: discovery (early research), execution (active task tracking), and synthesis (handoff or retrospective work).

### Concept: Template Token Contract

The template catalog and its prompts share a fixed set of interpolation tokens that MUST expand consistently across specifications, implementations, and scratch pads. Templates and prompts MUST NOT introduce tokens outside the enumerated set.

Tokens ending in `_or_request` MUST expand to a complete instruction string on their own; prompt templates MUST place these tokens as standalone steps and MUST NOT wrap them with extra prose that repeats or adds to the instruction.

#### Token `{{output_name}}`

- Specification, implementation, and scratch pad **templates** MUST resolve this token to their artifact name (front matter `name` or folder name) following the naming rules in the [founding specification](../../docs/founding-spec.md#specification-name) and [Scratch Pads](../specman-data-model/spec.md#scratch-pads).
- Prompt templates MUST NOT render or request `{{output_name}}`; prompts MUST gather artifact names via `{{artifact_name_or_request}}` and MAY derive the eventual output name from that input outside the prompt body.

#### Token `{{context}}`

- Specification prompts MUST expand this token to list all dependencies declared for the specification so authors review upstream material before editing.
- Implementation prompts MUST expand this token to include every referenced specification or implementation from the `references` front matter, preserving traceability to upstream artifacts.
- Scratch pad prompts MUST expand this token to enumerate the dependency chain for the targeted artifact (specification or implementation) so revision work begins with complete prerequisites.

#### Token `{{dependencies}}`

- Specification templates MUST map this token to the `dependencies` list recorded in their front matter and MUST NOT omit required dependencies when rendering instructions.
- Implementation templates MUST translate this token into the structured `references` metadata, mirroring how the data model represents upstream artifacts for implementations.
- Scratch pad templates MUST expand this token to the dependency chain inferred from the scratch pad target so work logs reflect the same upstream set referenced in `{{context}}`.

#### Token `{{target_path}}`

- Scratch pad prompts and templates MUST treat this token as REQUIRED when present.
- The value of `{{target_path}}` MAY be any locator string that a human or AI can use with a client (for example: a workspace-relative path, an HTTPS URL, or a client-only handle like `spec://{artifact}` / `impl://{artifact}` / `scratch://{artifact}`).
- When rendering a scratch pad **artifact**, clients MUST ensure the scratch pad front matter `target` field conforms to the [SpecMan Data Model target artifact rules](../specman-data-model/spec.md#target-artifact) (workspace-relative path or HTTPS URL). If `{{target_path}}` was provided as a handle, the client MUST normalize it to a canonical workspace-relative path before writing the artifact.

#### Token `{{artifact_name_or_request}}`

- Scratch pad prompts MUST place this token as its own numbered step; the token text itself captures the provided artifact (scratch pad) name or asks the reader for one when absent. Prompt bodies MUST NOT add separate instructions about naming beyond the token.
- Specification and implementation prompts MAY accept this token for consistency but are not required to surface it when the artifact name is already implied by context.
- When the caller does not provide a value, the token MUST emit guidance asking for a name that satisfies the relevant naming rules from the founding specification or scratch pad constraints; when provided, it MUST echo the supplied name.

#### Token `{{branch_name_or_request}}`

- Scratch pad prompts and templates MUST treat this token as OPTIONAL input and place it as its own numbered step. The token text is self-contained and MUST NOT be wrapped with additional branch-naming instructions. The token MUST expand to instructions:
  - When a branch name is provided, the token text MUST instruct the reader to check out that branch.
  - When a branch name is not provided, the token text MUST instruct the reader to generate a branch name that follows `{target_name}/{work_type}/{scratch_pad_name}` and show an example string rather than auto-populating.
- When supplied, the value MUST comply with the branch naming scheme defined in the [SpecMan Data Model](../specman-data-model/spec.md#git-branches).

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

- Scratch pad prompt templates MUST specify their work type (`feat`, `ref`, `revision`, or `fix`).
- Scratch pad prompt templates MUST instruct the AI to include front matter capturing target artifact, work type, and branch information required by the data model specification.
- Scratch pad prompt templates SHOULD suggest section layouts suited to their scenario (for example, notes for feat/revision, checklist for ref/feat, and summary for revision).

## Constraints

- Template files MUST retain HTML comment instructions verbatim when copied; downstream automation MAY delete comment blocks only after satisfying their directives.
- Specification, implementation, and scratch pad templates MUST NOT remove or reword the RFC 2119 guidance included in their static sections.
- Prompt templates MUST instruct AI systems to acknowledge and respect any HTML comment instructions present in the target template before generating content.

## Additional Notes

Future iterations MAY extend the prompt catalog with additional scenarios as new workflow patterns emerge. Implementations consuming these templates SHOULD version their prompt integrations to remain aligned with the template catalog.
