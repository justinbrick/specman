name: specman-core
version: "0.1.0"
dependencies:
  - ref: https://raw.githubusercontent.com/jbrickley-tcs/specman/refs/heads/main/spec/specman-data-model/spec.md
    optional: false
  - ref: ../specman-templates/spec.md
    optional: false
---

<!-- Template directives from templates/spec/spec.md were preserved and fulfilled prior to removal. -->

# Specification — SpecMan Core

The SpecMan Core specification defines the platform capabilities that guarantee consistent interactions with the [SpecMan Data Model](../specman-data-model/spec.md). It focuses on the behaviors and governance rules implementers MUST honor so downstream specifications MAY rely on a stable, versioned integration experience independent of any concrete delivery mechanism.

## Terminology & References

This document uses the normative keywords defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119). Consumers SHOULD review the [SpecMan Data Model](../specman-data-model/spec.md) for canonical entity definitions and serialization rules reused throughout this specification.

## Concepts

### Concept: Data Model Backing Implementation

This concept ties runtime behavior to the data model’s authoritative structures.

- The implementation MUST persist or retrieve entities exactly as defined in the data model specification.
- Internal storage representations MAY vary, provided they preserve the documented semantics.
- The implementation SHOULD emit data model validation errors that mirror normative constraints from the data model.
- All exposed capabilities MUST operate exclusively on types defined in the [SpecMan Data Model](../specman-data-model/spec.md) and MUST document deterministic input and output expectations.
- Implementations SHOULD maintain backward compatibility for these capabilities within a given major version of this specification.

### Concept: Dependency Mapping Services

Dependency mapping provides visibility into upstream and downstream relationships across specifications and implementations.

- The implementation MUST construct dependency trees that enumerate upstream providers, downstream consumers, and full transitive relationships.
- Dependency lookups MUST return results in upstream, downstream, and aggregate forms to support targeted impact analysis.
- Tree traversal APIs SHOULD expose both hierarchical and flattened views to accommodate varied client needs.

### Concept: Template Orchestration

Template orchestration governs how reusable content is discovered and rendered.

- Templates MUST declare substitution tokens using double braces (`{{token_name}}`).
- The system MUST accept template locators as absolute filesystem paths or HTTPS URLs targeting Markdown resources.
- Special-purpose template functions SHOULD exist for common scenarios such as creating specifications, implementations, and scratch pads together with their work-type variants.
- The runtime MUST NOT hardcode template content; it MUST resolve templates at runtime via the provided locator.

### Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications, implementations, and scratch pads.

- Automated creation flows MUST require an associated template locator and MUST validate that required tokens are supplied.
- Deletion workflows MUST refuse to proceed when dependent artifacts exist and MUST return a dependency tree describing all impacted consumers.
- Scratch pad creation SHOULD support selectable profiles aligned with defined scratch pad types and MUST leverage corresponding templates.

## Key Entities

### Entity: DataModelAdapter

Adapter responsible for translating runtime interactions to persisted data model instances.

- MUST ensure transformations honor data model invariants.
- SHOULD provide observability hooks for auditing cross-cutting behaviors.
- MAY cache read-mostly projections when it does not compromise consistency guarantees.

### Entity: DependencyTree

Aggregated representation of upstream and downstream relationships for a given artifact.

- MUST capture root artifact metadata together with its direct and transitive dependencies.
- MUST expose traversal helpers to retrieve upstream-only, downstream-only, or combined views.
- SHOULD provide serialization compatible with the [SpecMan Data Model](../specman-data-model/spec.md) for interchange.

### Entity: TemplateDescriptor

Metadata describing how templates are located and rendered.

- MUST record the locator URI or absolute path and the intended template scenario (specification, implementation, scratch pad, or derivative work type).
- SHOULD list required substitution tokens so callers MAY validate inputs before rendering.
- MAY reference helper functions that provide contextual data during template expansion.

### Entity: LifecycleController

Controller responsible for enforcing lifecycle policies across specifications and implementations.

- MUST orchestrate create and delete operations, delegating to dependency mapping and templating subsystems.
- MUST terminate deletion attempts that would orphan dependents and MUST return the blocking dependency tree to the caller.
- SHOULD integrate auditing hooks that capture lifecycle events for compliance tracking.

### Entity: ScratchPadProfile

Defines the characteristics and template linkages for scratch pad variants.

- MUST enumerate available scratch pad types alongside their required templates.
- SHOULD expose optional configuration fields to tailor scratch pad content to team workflows.
- MAY reuse `TemplateDescriptor` instances to avoid duplication across related profiles.

## Constraints

- This implementation MUST depend on a single major version of the [SpecMan Data Model](../specman-data-model/spec.md) at any given time.
- Consumers MUST treat all functions as pure unless explicitly documented otherwise.
- Any serialization emitted here MUST validate against the schemas mandated by the data model specification.
- Breaking changes to function signatures or behaviors MUST trigger a major version increment of this specification.
- Dependency inspection APIs MUST produce results that include upstream, downstream, and full dependency sets for any supported artifact.
- Template rendering routines MUST require callers to supply all `{{}}` token values before materialization.
- Lifecycle operations MUST enforce template usage for new specifications, implementations, and scratch pads.
- Scratch pad creation workflows MUST offer selectable profiles and MUST apply the template associated with the chosen profile.
- Deletion workflows MUST fail when dependencies exist and MUST include the complete dependency tree in the failure response.

## Additional Notes

- Migration guides MAY accompany minor releases to help downstream integrators adopt new optional capabilities.
- Implementers MAY provide caching or indexing strategies for dependency trees when doing so preserves freshness guarantees.
- Template repositories SHOULD be discoverable through configuration so administrators CAN extend or swap template sources without code changes.
- Scratch pad workflows MAY integrate with collaboration tooling (e.g., team workspaces) to streamline drafting phases.
