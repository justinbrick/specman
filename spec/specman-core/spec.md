---
name: specman-core
version: "0.1.0"
dependencies:
  - ref: ../specman-data-model/spec.md
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
- Implementations MUST depend on a single major version of the [SpecMan Data Model](../specman-data-model/spec.md) at a time to avoid incompatible schema drift.
- Any serialization emitted by these capabilities MUST validate against the schemas mandated by the data model specification before it is persisted or returned to callers.

### Concept: Dependency Mapping Services

Dependency mapping provides visibility into upstream and downstream relationships across specifications and implementations.

- The implementation MUST construct dependency trees that enumerate upstream providers, downstream consumers, and full transitive relationships.
- Dependency lookups MUST return results in upstream, downstream, and aggregate forms to support targeted impact analysis.
- Tree traversal APIs SHOULD expose both hierarchical and flattened views to accommodate varied client needs.
- Implementations MUST expose a callable dependency-tree builder that accepts a filesystem path or HTTPS URL pointing to either a specification or implementation artifact and normalizes that locator relative to the active workspace root before traversal begins.
- The tree builder MUST parse YAML front matter (when present) for dependencies or references, recursively resolve each upstream artifact, and continue traversal until the graph is fully explored or a cycle is encountered.
- Resolvers MUST support filesystem paths (absolute or workspace-relative), HTTPS URLs that point to Markdown specifications or implementations, and SpecMan resource handles expressed as `spec://{artifact}`, `impl://{artifact}`, or `scratch://{artifact}`. Each handle MUST be normalized through workspace discovery before traversal begins and MUST resolve to canonical artifact identifiers defined by the SpecMan Data Model.
- Requests that supply locator schemes outside of filesystem, HTTPS, or the SpecMan resource handles (`spec://`, `impl://`, `scratch://`) MUST fail fast with a descriptive error that directs callers to use the supported schemes instead of attempting implicit rewrites.
- Requests that reference targets outside of the detected workspace MUST fail with an error that explains the workspace boundary violation.
- Cycle detection MUST terminate traversal immediately and return a descriptive error that includes the partial tree gathered so far so callers can remediate invalid dependency graphs.
- When a referenced dependency or implementation lacks front matter metadata, or when the dependency resolves to HTML or other plaintext without metadata, the tree builder MUST still add the artifact to the dependency set using the best available identifier (path or URL) and annotate the entry to indicate metadata was unavailable.

### Concept: Template Orchestration

Template orchestration governs how reusable content is discovered and rendered.

- Templates MUST declare substitution tokens using double braces (`{{token_name}}`), and rendering engines MUST refuse to materialize output until every declared token is supplied.
- Template consumers MUST accept locator inputs expressed as absolute filesystem paths, workspace-relative paths rooted at the discovered workspace, HTTPS URLs, or packaged-default identifiers bundled with the runtime.
- When creating specifications, implementations, or scratch pads, the orchestrator MUST search for workspace-managed overrides under `.specman/templates/` in the following order: (1) artifact-specific Markdown files (for example `.specman/templates/spec.md`, `.specman/templates/impl.md`, or `.specman/templates/scratch.md` plus any nested directories the workspace defines), (2) uppercase pointer files (`SPEC`, `IMPL`, `SCRATCH`) whose contents resolve to workspace-relative paths or HTTPS URLs, and (3) packaged defaults embedded with the SpecMan Core runtime. Packaged defaults MUST be versioned with the runtime, remain read-only, and MAY be delivered via resources compiled into the binary or co-located artifacts inside the packaged application.
- Implementations MUST expose pointer-file lifecycle helpers for every artifact profile so callers can add new `SPEC`, `IMPL`, or `SCRATCH` pointer files, update (set) their target locators, or remove them without editing the filesystem manually.
- Pointer update operations MUST persist uppercase pointer files under `.specman/templates/`, enforce the same locator validation rules defined for runtime resolution (workspace-bound filesystem paths and reachable HTTPS Markdown), and MUST refresh any `.specman/cache/templates/` entries referencing the affected locator before signaling success.
- Pointer removal operations MUST delete the targeted pointer file, purge cached remote content that referenced it, and MUST document the resulting fallback search order so clients know which template source will be used next. When the removal would leave the orchestration layer without any valid template source, the helper MUST fail with a descriptive error instead of leaving an invalid pointer state.
- Pointer-file lifecycle helpers MUST surface structured success and failure results—including validation errors or fallback descriptions—so CLI layers and APIs can relay operator-facing guidance without re-parsing filesystem state.
- Pointer files MUST be re-read on every invocation so workspace changes take effect without restarting tooling. Implementations MUST validate that filesystem locators remain inside the workspace root and that HTTPS locators are reachable plaintext Markdown before rendering.
- When a pointer file references an HTTPS resource, the fetched Markdown MUST be cached under `.specman/cache/templates/` using deterministic filenames (for example, hashing the URL). Cache entries MUST store the downloaded content verbatim together with the source locator and last-refresh metadata, and they MUST be reused for subsequent invocations until the pointer file content or remote resource changes.
- Template orchestration MUST refresh cached remote content whenever the pointer file changes or the remote server signals a new version (for example via `ETag` or `Last-Modified`). If refresh attempts fail, tooling MUST fall back to the last known-good cache entry before reverting to packaged defaults.
- Template rendering workflows MUST preserve HTML comment directives present in the source templates until each directive is satisfied. After fulfilling a directive, tooling MAY remove or replace the associated comment but MUST NOT drop unsatisfied instructions.
- Special-purpose template functions SHOULD exist for common scenarios such as creating specifications, implementations, and scratch pads together with their work-type variants.
- Template metadata (required tokens, locator provenance, cache path) MAY be cached for the duration of a command invocation but MUST include the workspace root and template version in the cache key. Tooling MUST NOT reuse metadata caches across different workspaces unless both the template version and workspace identifier match.

### Concept: Deterministic Execution

Deterministic execution codifies behavioral guarantees so downstream consumers can rely on predictable, side-effect-aware APIs.

- Consumers MUST treat all SpecMan Core functions as pure unless the documentation explicitly calls out side effects; implementers MUST document any deviations before release.
- Breaking changes to function signatures or observable behaviors MUST trigger a major version increment of this specification so dependent tooling can coordinate adoption.

### Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications, implementations, and scratch pads.

- Automated creation flows MUST require an associated template locator and MUST validate that required tokens are supplied.
- Lifecycle operations MUST enforce template usage for all new specifications, implementations, and scratch pads so generated artifacts remain data-model compliant.
- Implementations MUST expose user-facing deletion workflows for specifications, implementations, and scratch pads so that every artifact type can be removed with the same rigor applied to creation.
- Creation tooling MUST cover all three artifact types (specifications, implementations, scratch pads) and MUST enforce the naming and metadata rules defined by the [SpecMan Data Model](../specman-data-model/spec.md) and [founding specification](../../docs/founding-spec.md).
- Creation workflows MUST persist generated Markdown artifacts and supporting metadata into the canonical workspace locations (`spec/{name}/spec.md`, `impl/{name}/impl.md`, `.specman/scratchpad/{slug}/scratch.md`) using the paths returned by workspace discovery.
- When a pointer file downloads content from an HTTPS locator, Lifecycle automation MUST route the rendered template through the `.specman/cache/templates/` store before writing artifacts so repeated invocations reuse the cached copy unless the pointer or upstream content changes.
- Persistence helpers MUST write the rendered template output (with all required tokens populated) together with its front matter or metadata; persisting additional representations of entities, concepts, or other runtime data structures is out of scope for this specification.
- Lifecycle automation MUST provide direct integrations with the metadata mutation capabilities described in [Concept: Metadata Mutation](#concept-metadata-mutation).
- Deletion workflows MUST reuse dependency mapping services, refuse to proceed when dependent artifacts exist, and MUST return a dependency tree describing all impacted consumers whenever a removal is blocked.
- Deletion workflows MUST ensure the targeted artifact and any associated metadata or scratch pad directories are removed from their canonical workspace locations once safety checks pass.
- Scratch pad creation workflows MUST offer selectable profiles aligned with defined scratch pad types and MUST leverage corresponding templates.
- Lifecycle controllers MUST expose a persistence interface that can round-trip newly created artifacts back onto disk and SHOULD surface explicit errors if the filesystem write fails so callers can remediate workspace permissions.

### Concept: Metadata Mutation

Metadata mutation ensures YAML front matter for specifications and implementations can be updated without rewriting the surrounding Markdown content.

- Implementations MUST expose metadata mutation helpers that accept a filesystem path or HTTPS URL to a specification or implementation, merge updated values into the YAML front matter, and leave the Markdown body unchanged.
- Metadata mutation helpers MUST support adding dependencies or references by artifact locator and MUST be idempotent when the supplied locator already exists in the corresponding list.
- Callers MUST be able to choose whether metadata mutation helpers immediately persist the updated artifact to disk or return the full document content; when returning content, the helpers MUST emit the complete file with differences limited to the front matter block.
- Metadata mutation helpers MUST reuse the locator normalization, workspace-boundary enforcement, and supported-scheme validation rules defined for dependency traversal before applying edits.
- Metadata mutation operations MUST reuse the dependency traversal validation flow (workspace boundary enforcement, supported locator schemes, YAML parsing guarantees) before applying edits to any artifact.
- Metadata mutation operations MUST rewrite only the YAML front matter block and MUST either persist the updated artifact to its canonical path or return the full document with body content unchanged.
- When metadata mutation operations add dependencies or references, they MUST treat the addition as idempotent, leaving the artifact untouched if the locator already exists.

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
- When a cached remote template is used, the descriptor MUST record the cache file path and validator metadata supplied by the associated `TemplateCache` entry.

### Entity: TemplateCache

Cache store that retains remote template content referenced by pointer files.

- MUST persist downloads inside `.specman/cache/templates/` using deterministic filenames derived from the source locator.
- MUST record the original locator, retrieval timestamp, and any validator metadata (for example `ETag`) so Template Orchestration can determine staleness before reuse.
- SHOULD expose purge and refresh helpers so lifecycle controllers can invalidate entries when pointer files change or when users request a clean refresh.

### Entity: LifecycleController

Controller responsible for enforcing lifecycle policies across specifications, implementations, and scratch pads.

- MUST orchestrate create and delete operations for every artifact type, delegating to dependency mapping and templating subsystems.
- MUST terminate deletion attempts that would orphan dependents and MUST return the blocking dependency tree to the caller.
- MUST expose deletion entry points that mirror creation workflows so operators have symmetrical controls for specifications, implementations, and scratch pads.
- SHOULD integrate auditing hooks that capture lifecycle events for compliance tracking.
- MUST surface explicit errors when filesystem persistence fails (for example, permissions or missing directories) so callers can remediate issues without corrupting the workspace.

### Entity: ScratchPadProfile

Defines the characteristics and template linkages for scratch pad variants.

- MUST enumerate available scratch pad types alongside their required templates.
- SHOULD expose optional configuration fields to tailor scratch pad content to team workflows.
- MAY reuse `TemplateDescriptor` instances to avoid duplication across related profiles.

## Additional Notes

- Migration guides MAY accompany minor releases to help downstream integrators adopt new optional capabilities.
- Implementers MAY provide caching or indexing strategies for dependency trees when doing so preserves freshness guarantees.
- Template repositories SHOULD be discoverable through configuration so administrators CAN extend or swap template sources without code changes.
- Scratch pad workflows MAY integrate with collaboration tooling (e.g., team workspaces) to streamline drafting phases.
