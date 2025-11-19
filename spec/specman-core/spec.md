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

### Concept: Workspace Discovery

Workspace discovery ensures every SpecMan-aware tool can deterministically locate the active workspace root and its `.specman` directory from any starting location.

- The implementation MUST identify the workspace root by scanning the current directory and its ancestors for the nearest `.specman` folder, treating the containing directory as canonical.
- When no `.specman` folder exists along the ancestry chain, the implementation MUST return a descriptive error that callers MAY surface directly to users.
- Workspace discovery utilities MUST expose the absolute path to both the workspace root and the `.specman` directory so downstream services can reference shared metadata without recomputing filesystem state.
- Resolved workspace metadata MUST remain consistent with the [SpecMan Data Model](../specman-data-model/spec.md) rules for SpecMan workspaces and MUST reuse existing data-model entities when emitting structured results.
- Implementations MAY cache the active workspace root for the lifetime of a command invocation, but they MUST revalidate that the `.specman` folder still exists before reusing cached paths.

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
- Dependency tree construction MUST accept a target path pointing to either a specification or implementation Markdown artifact and MUST normalize that path relative to the active workspace root before traversal begins.
- Resolvers MUST support filesystem paths (absolute or workspace-relative) and HTTPS URLs that point to Markdown specifications or implementations and MUST reuse workspace discovery results to normalize those inputs.
- Requests that supply unsupported locator schemes MUST fail fast with a descriptive error that directs callers to use filesystem or HTTPS references instead of attempting implicit rewrites.
- Requests that reference targets outside of the detected workspace MUST fail with an error that explains the workspace boundary violation.

### Concept: Template Orchestration

Template orchestration governs how reusable content is discovered and rendered.

- Templates MUST declare substitution tokens using double braces (`{{token_name}}`).
- The system MUST accept template locators as absolute filesystem paths or HTTPS URLs targeting Markdown resources.
- Special-purpose template functions SHOULD exist for common scenarios such as creating specifications, implementations, and scratch pads together with their work-type variants.
- The runtime MUST NOT hardcode template content; it MUST resolve templates at runtime via the provided locator.

### Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications, implementations, and scratch pads.

- Automated creation flows MUST require an associated template locator and MUST validate that required tokens are supplied.
- Creation tooling MUST cover all three artifact types (specifications, implementations, scratch pads) and MUST enforce the naming and metadata rules defined by the [SpecMan Data Model](../specman-data-model/spec.md) and [founding specification](../../docs/founding-spec.md).
- Creation workflows MUST persist generated Markdown artifacts and supporting metadata into the canonical workspace locations (`spec/{name}/spec.md`, `impl/{name}/impl.md`, `.specman/scratchpad/{slug}/scratch.md`) using the paths returned by workspace discovery.
- Persistence helpers MUST write the rendered template output (with all required tokens populated) together with its front matter or metadata; persisting additional representations of entities, concepts, or other runtime data structures is out of scope for this specification.
- Deletion workflows MUST refuse to proceed when dependent artifacts exist and MUST return a dependency tree describing all impacted consumers.
- Scratch pad creation SHOULD support selectable profiles aligned with defined scratch pad types and MUST leverage corresponding templates.
- Lifecycle controllers MUST expose a persistence interface that can round-trip newly created artifacts back onto disk and SHOULD surface explicit errors if the filesystem write fails so callers can remediate workspace permissions.

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
- Workspace discovery routines MUST stop processing and return an error when no `.specman` folder exists in the current directory ancestry.
- Workspace discovery helpers MUST return absolute, normalized paths for both the workspace root and `.specman` folder, and MUST revalidate cached paths before reuse.
- Template rendering routines MUST require callers to supply all `{{}}` token values before materialization.
- Lifecycle operations MUST enforce template usage for new specifications, implementations, and scratch pads.
- Scratch pad creation workflows MUST offer selectable profiles and MUST apply the template associated with the chosen profile.
- Deletion workflows MUST fail when dependencies exist and MUST include the complete dependency tree in the failure response.
- Dependency traversal MUST reject target paths that fall outside the active workspace root and MUST describe the violation.
- Dependency traversal inputs MUST be limited to filesystem paths (absolute or workspace-relative) and HTTPS URLs pointing to Markdown specifications or implementations; unsupported schemes MUST trigger descriptive errors that instruct callers to use supported locators.
- Creation workflows for specifications, implementations, and scratch pads MUST persist the rendered template artifacts (with populated tokens and required front matter) into the canonical workspace directories defined by the [SpecMan Data Model](../specman-data-model/spec.md), leveraging workspace discovery results to determine destinations; persisting additional entity or concept data structures is out of scope for this specification.
- Lifecycle controllers MUST surface errors when filesystem persistence fails (for example, permissions, missing directories) so callers can remediate without corrupting the workspace.

## Additional Notes

- Migration guides MAY accompany minor releases to help downstream integrators adopt new optional capabilities.
- Implementers MAY provide caching or indexing strategies for dependency trees when doing so preserves freshness guarantees.
- Template repositories SHOULD be discoverable through configuration so administrators CAN extend or swap template sources without code changes.
- Scratch pad workflows MAY integrate with collaboration tooling (e.g., team workspaces) to streamline drafting phases.
