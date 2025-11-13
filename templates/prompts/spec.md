---
artifact_type: specification
inputs:
  - spec_name
  - summary
  - dependency_list
success_criteria:
  - front_matter_complete
  - headings_unique
  - html_comments_preserved
  - data_model_validated
---

# Prompt — Specification Authoring

Use this blueprint when requesting an AI system to produce a SpecMan-compliant specification.

```
You are creating a SpecMan specification titled "{{spec_name}}".

Inputs you have:
- Summary: {{summary}}
- Dependencies: {{dependency_list}}

Instructions:
1. Load the canonical template located at `templates/spec/spec.md` and copy it verbatim as the starting point.
2. Explicitly acknowledge that all HTML comment directives in the template will be preserved and satisfied before removing any of them.
3. Update the YAML front matter with the provided `name`, semantic `version`, and dependency list. Retain the reference to `spec/specman-data-model/spec.md` unless a superseding official version is supplied.
4. Keep the "Terminology & References" section and ensure the RFC 2119 statement remains intact.
5. Populate Concepts, Key Entities, Constraints, and Additional Notes with content aligned to the supplied summary and dependencies. Maintain unique headings per the SpecMan data model.
6. Cross-reference related specifications or entities using inline links when possible.

Validation Checklist:
- Confirm the artifact satisfies the structural and metadata rules in `spec/specman-data-model/spec.md` (sections: Specifications, Dependencies, Specification Metadata).
- Verify that normative keywords follow RFC 2119 semantics.
- Ensure all HTML comment directives have either been fulfilled or intentionally retained.

Deliverable:
- Return the completed Markdown specification, ready to place under `spec/<spec-name>/spec.md`.
```
