---
artifact_type: implementation
inputs:
  - implementation_name
  - target_spec_path
  - source_location
  - language_inventory
  - reference_items
success_criteria:
  - front_matter_complete
  - references_typed
  - html_comments_preserved
  - data_model_validated
---

# Prompt — Implementation Authoring

Use this blueprint when guiding an AI system to draft a SpecMan implementation document.

```
You are documenting the implementation "{{implementation_name}}" that realizes the specification at {{target_spec_path}}.

Inputs you have:
- Source location: {{source_location}}
- Implementing languages: {{language_inventory}}
- References: {{reference_items}}

Instructions:
1. Copy the canonical Markdown template from `templates/impl/impl.md` and use it as the base document.
2. State that you will honor and preserve every HTML comment directive in the template, removing them only after their guidance is satisfied.
3. Update the YAML front matter: set `spec` to {{target_spec_path}}, populate `location`, `library` (if applicable), `primary_language`, optional `secondary_languages`, and replace the placeholder `references` list with the provided items.
4. Fill the Overview, Implementing Languages, References, Implementation Details, API Surface, Data Models, and Operational Notes sections with concise, human-readable explanations that link back to specification concepts when relevant.
5. Document API signatures inside fenced code blocks, providing explanatory text for inputs, outputs, and side effects.
6. Reference any dependent specifications or implementations inline where appropriate.

Validation Checklist:
- Confirm compliance with the Implementations requirements described in `spec/specman-data-model/spec.md` (sections: Implementations, Implementing Language, References, APIs, Implementation Metadata).
- Ensure metadata fields (paths, names, versions) are accurate and relative to the workspace when possible.
- Verify that all HTML comment directives remain satisfied.

Deliverable:
- Return the completed Markdown implementation document, suitable for placement at `impl/<implementation-name>/impl.md`.
```
