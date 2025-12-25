---
name: specman-core
version: "0.1.0"
dependencies:
  - ref: ../specman-data-model/spec.md
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

!concept-workspace-discovery.requirements:

- The implementation MUST identify the workspace root by scanning the current directory and its ancestors for the nearest `.specman` folder, treating the containing directory as canonical.
- When no `.specman` folder exists along the ancestry chain, the implementation MUST return a descriptive error that callers MAY surface directly to users.
- Workspace discovery utilities MUST expose the absolute path to both the workspace root and the `.specman` directory so downstream services can reference shared metadata without recomputing filesystem state.
- Resolved workspace metadata MUST remain consistent with the [SpecMan Data Model](../specman-data-model/spec.md) rules for SpecMan workspaces and MUST reuse existing data-model entities when emitting structured results.
- Implementations MAY cache the active workspace root for the lifetime of a command invocation, but they MUST revalidate that the `.specman` folder still exists before reusing cached paths.

!concept-workspace-discovery.initialization:

- The implementation MUST expose an initializer that accepts an absolute filesystem path provided by the caller and resolves it to the canonical workspace root and `.specman` directory using the same rules as workspace discovery.
- The initializer MUST accept both workspace-root paths and `.specman` directory paths as valid inputs; in either case it MUST return normalized absolute paths for both the workspace root and `.specman` directory without redundant ancestor search.
- The initializer MUST validate that the supplied path is (or contains) a `.specman` directory; if validation fails, it MUST either create `.specman` (when allowed by the invocation) or return a descriptive error suitable for direct user display, and it MUST NOT fall back to scanning unrelated ancestor paths.
- When creation is requested and a `.specman` directory is absent at the provided root, the initializer MUST create the `.specman` directory at that root, enforce workspace-boundary rules, and then return normalized paths; it MUST NOT create nested `.specman` directories beneath an existing workspace.
- The implementation MUST expose a library-level workspace creator that provisions `.specman` at an explicit path (including required subdirectories such as `scratchpad/` and `cache/` when defined), performs the same validation as the initializer, and keeps the operation idempotent so future workspace-owned files can be added by the implementation rather than by ad-hoc folder creation.
- The initializer MUST reject relative paths and paths that imply nested workspace creation; callers MUST supply the intended workspace root explicitly rather than relying on automatic ascent from arbitrary subpaths.
- The initializer MAY reuse discovery caches only when the cached workspace root matches the normalized result for the supplied path; otherwise it MUST revalidate (and, if needed, create) the `.specman` directory before returning paths.

### Concept: Data Model Backing Implementation

This concept ties runtime behavior to the data model’s authoritative structures.

!concept-data-model-backing-implementation.requirements:

- The implementation MUST persist or retrieve entities exactly as defined in the data model specification.
- Internal storage representations MAY vary, provided they preserve the documented semantics.
- The implementation SHOULD emit data model validation errors that mirror normative constraints from the data model.
- All exposed capabilities MUST operate exclusively on types defined in the [SpecMan Data Model](../specman-data-model/spec.md) and MUST document deterministic input and output expectations.
- Implementations SHOULD maintain backward compatibility for these capabilities within a given major version of this specification.
- Implementations MUST depend on a single major version of the [SpecMan Data Model](../specman-data-model/spec.md) at a time to avoid incompatible schema drift.
- Any serialization emitted by these capabilities MUST validate against the schemas mandated by the data model specification before it is persisted or returned to callers.

### Concept: Dependency Mapping Services

Dependency mapping provides visibility into upstream and downstream relationships across specifications and implementations.

!concept-dependency-mapping-services.requirements:

- The implementation MUST construct dependency trees that enumerate upstream providers, downstream consumers, and full transitive relationships.
- Dependency lookups MUST return results in upstream, downstream, and aggregate forms to support targeted impact analysis.
- Tree traversal APIs SHOULD expose both hierarchical and flattened views to accommodate varied client needs.
- Implementations MUST expose a callable dependency-tree builder that accepts a filesystem path or HTTPS URL pointing to either a specification or implementation artifact and normalizes that locator relative to the active workspace root before traversal begins.
- The tree builder MUST parse YAML front matter (when present) for dependencies or references, recursively resolve each upstream artifact, and continue traversal until the graph is fully explored or a cycle is encountered.
- Resolvers MUST support filesystem paths (absolute or workspace-relative), HTTPS URLs that point to Markdown specifications or implementations, and SpecMan resource handles expressed as `spec://{artifact}`, `impl://{artifact}`, or `scratch://{artifact}`. Handle semantics and normalization MUST follow the locator scheme rules defined in the [SpecMan Data Model](../specman-data-model/spec.md#locator-schemes), including workspace discovery before traversal begins and resolution to canonical artifact identifiers.
- Requests that supply locator schemes outside of filesystem, HTTPS, or the SpecMan resource handles (`spec://`, `impl://`, `scratch://`) MUST fail fast with a descriptive error that directs callers to use the supported schemes instead of attempting implicit rewrites.
- Requests that reference targets outside of the detected workspace MUST fail with an error that explains the workspace boundary violation.
- Cycle detection MUST terminate traversal immediately and return a descriptive error that includes the partial tree gathered so far so callers can remediate invalid dependency graphs.
- When a referenced dependency or implementation lacks front matter metadata, or when the dependency resolves to HTML or other plaintext without metadata, the tree builder MUST still add the artifact to the dependency set using the best available identifier (path or URL) and annotate the entry to indicate metadata was unavailable.

### Concept: Reference Validation

Reference validation ensures that references embedded in Markdown artifacts—particularly link destinations in inline links—can be validated deterministically against the workspace filesystem and external HTTPS resources. This enables tooling to detect broken links early, prevent invalid relationship graphs, and provide actionable diagnostics to authors.

!concept-reference-validation.requirements:

- The implementation MUST expose a callable reference-validation capability that accepts a locator to a Markdown artifact and returns structured validation results.
  - Artifact locators MAY use filesystem paths, HTTPS URLs, or SpecMan resource handles (`spec://{artifact}` / `impl://{artifact}` / `scratch://{artifact}`) as input to the validator.
- The validator MUST parse Markdown using CommonMark-compatible rules to identify links and their destinations, including:
  - inline links (`[text](destination)`),
  - full/collapsed/shortcut reference links resolved through link reference definitions, and
  - autolinks (`<https://example.com>`).
- The validator MUST NOT validate image destinations (`![alt](destination)`) as references.
- For every discovered link destination, the validator MUST classify the destination as one of:
  - workspace-filesystem (a filesystem path that resolves inside the active workspace),
  - HTTPS URL, or
  - unsupported/unknown.
- SpecMan resource handles (`spec://{artifact}`, `impl://{artifact}`, `scratch://{artifact}`) are client-side identifiers and MUST NOT be stored as Markdown link destinations inside SpecMan artifacts.
  - If such a handle is encountered in a Markdown link destination, the validator MUST report it as invalid and MUST NOT attempt to resolve or “validate” the target.
- If a destination uses a scheme outside the supported set for Markdown references (workspace-filesystem paths and HTTPS URLs), the validator MUST report it as invalid and MUST NOT attempt implicit rewrites.
- When validating filesystem destinations, the validator MUST resolve them relative to the source artifact’s directory.
  - The validator MUST normalize the resolved path and MUST enforce workspace-boundary rules (it MUST NOT allow traversal outside the workspace root after normalization).
  - The validator SHOULD additionally enforce the workspace boundary using canonicalized paths (for example resolving symlinks/junctions) when the platform and permissions allow.
- When validating HTTPS destinations, the validator MUST at minimum validate that the destination is a well-formed HTTPS URL.
  - The validator SHOULD support an optional reachability check mode (for example `HEAD`/`GET`).
  - When reachability mode is enabled, the validator MUST treat HTTPS redirects (3xx) as success.
  - When reachability mode is enabled, the validator MUST NOT treat timeouts as validation failures; it SHOULD instead emit a non-fatal diagnostic indicating the check could not be completed.
- When a destination contains a fragment component (for example `./doc.md#some-heading`) and the destination resolves to Markdown, the validator MUST validate that the fragment refers to an existing heading slug as defined by the SpecMan Data Model’s heading-slug algorithm (see [Concept: Markdown Slugs](../specman-data-model/spec.md#concept-markdown-slugs)).
- Validation results MUST be deterministic for a fixed set of inputs and a fixed validation mode.
- Validation results MUST include, for each failure, enough context for callers to surface a helpful message (at minimum: source artifact locator, link destination, and source range information when available).

!concept-reference-validation.results.contract:

- The validator MUST return a structured result that includes:
  - a list of discovered references (or a count),
  - a list of validation errors, and
  - an overall status indicating success/failure.
- The validator MUST return the complete list of validation errors discovered in the processed artifact and MUST NOT fail fast on the first invalid reference.
- Errors SHOULD be grouped by type (unsupported scheme, workspace boundary violation, missing file, unreachable HTTPS, unknown fragment).
- The validator MUST NOT mutate the validated artifacts.

### Concept: Template Orchestration

Template orchestration governs how reusable content is discovered and rendered.

!concept-template-orchestration.requirements:

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

!concept-template-orchestration.ai-instruction-directives:

- Template guidance for automated agents MUST be conveyed inside HTML comments (`<!-- ... -->`) that sit adjacent to the mutable region they govern and MUST NOT leak into rendered Markdown.
- Rendering engines MUST preserve HTML instruction comments until each directive is satisfied; if a directive cannot be satisfied, tooling MUST fail the operation rather than silently dropping the comment.

!concept-template-orchestration.token-contract:

- The effective template descriptor defines a closed token set; lifecycle or MCP clients MUST reject substitutions for tokens that are not declared by the descriptor.
- Token substitution covers Markdown body content only. YAML front matter MUST be produced or mutated by lifecycle workflows after template rendering, not by injecting `{{token}}` placeholders inside front matter.
- When callers supply token data, the implementation MUST surface validation errors verbatim whenever a required token is missing or an unknown token is supplied.

### Concept: Deterministic Execution

Deterministic execution codifies behavioral guarantees so downstream consumers can rely on predictable, side-effect-aware APIs.

!concept-deterministic-execution.requirements:

- Consumers MUST treat all SpecMan Core functions as pure unless the documentation explicitly calls out side effects; implementers MUST document any deviations before release.
- Breaking changes to function signatures or observable behaviors MUST trigger a major version increment of this specification so dependent tooling can coordinate adoption.

### Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications, implementations, and scratch pads.

!concept-lifecycle-automation.requirements:

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

!concept-lifecycle-automation.frontmatter-generation:

- Creation workflows MUST generate or merge YAML front matter after template rendering so that every artifact persists the metadata mandated by the SpecMan Data Model and governing specifications.
- Templates MUST NOT embed YAML front matter; lifecycle automation and metadata mutation workflows are the authoritative mechanisms for writing and updating metadata.
- Metadata mutation helpers MUST update the YAML front matter in-place without rewriting the Markdown body and MUST continue to enforce the workspace-boundary and locator-validation rules defined elsewhere in this specification.

### Concept: SpecMan Structure

Creating a structure which maps the SpecMan data model allows consumers to read markdown content when given identifiers for concepts, key entities, or constraints.

#### Structure Indexing

To make sure that entities can be easily searched, implementations MUST index documents that are stored or referenced inside of the workspace.

!concept-specman-structure.indexing.collection:

- Implementations MUST index all markdown documents
- HTML documents MAY optionally be indexed.

!concept-specman-structure.indexing.headings:

- Each heading and their content MUST be indexed.
  - The content of the heading shall be defined as any markdown content, subheadings, or constraint groups located underneath this heading.

!concept-specman-structure.indexing.constraints:

- Each constraint group MUST be indexed.

!concept-specman-structure.indexing.relationships:

Relationships provide a way to construct a relationship graph by parsing the content of an entity, and finding inline links to include content inside of it.

- Headings MUST have a mapped relationship to the implementation or specification which it is stored in.
- Headings MUST have a mapped relationship to other headings that have been referenced via inline links inside of the heading content.
- Constraint groups MUST have a mapped relationship to the heading who's slug may be discovered by the first part of the constraint group.
  - If a heading can not be matched via slug to the first group, then a relationship MUST be indexed to the nearest heading which contains the constraint group.

!concept-specman-structure.referencing.validation:

- Implementations that index relationships from inline links MUST provide a method to validate the referenced destinations and report any invalid references.
- Relationship indexing MUST NOT silently drop invalid references; it MUST either record them as invalid with diagnostics or fail the indexing operation with a descriptive error.
- Validation of inline-link destinations used for relationships MUST reuse the locator normalization and workspace-boundary rules described elsewhere in this specification.

## Structure Discovery

Discovery allows for consumers of implementations to find the markdown content of a related item by using identifiers.

!concept-specman-structure.discovery.identifiers:

- Implementations MUST provide methods for enumerating the available identifiers of [heading slugs][heading slugs] and [constraint groups][constraint groups].

!concept-specman-structure.discovery.rendering:

Rendering the markdown content allows for readers to properly understand all possible related context.

- Implementations MUST return markdown content when provided with a heading slug.
  - The content inside of the heading slug must return content under any related headings that have been referenced via inline link, in the order of which the inline links were referenced.
  - Implementations MUST ensure that referenced headings content is not duplicated, so that it may only appear once when rendering the markdown content and its related content.
- Implementations MUST return markdown content when provided with a constraint group identifier.
  - The rendered content MUST contain the content of the heading which the constraint group has an active relationship to.

### Concept: Metadata Mutation

Metadata mutation ensures YAML front matter for specifications, implementations, and scratch pads can be updated without rewriting the surrounding Markdown content.

!concept-metadata-mutation.requirements:

- Implementations MUST expose metadata mutation helpers that accept a filesystem path or HTTPS URL to a specification or implementation (and a filesystem path to a scratch pad), merge updated values into the YAML front matter, and leave the Markdown body unchanged.
- Callers MUST be able to choose whether metadata mutation helpers immediately persist the updated artifact to disk or return the full document content; when returning content, the helpers MUST emit the complete file with differences limited to the front matter block.
- Metadata mutation helpers MUST reuse the locator normalization, workspace-boundary enforcement, and supported-scheme validation rules defined for dependency traversal before applying edits.
- Metadata mutation operations MUST reuse the dependency traversal validation flow (workspace boundary enforcement, supported locator schemes, YAML parsing guarantees) before applying edits to any artifact.
- Metadata mutation operations MUST rewrite only the YAML front matter block and MUST either persist the updated artifact to its canonical path or return the full document with body content unchanged.
- Metadata mutation helpers MUST support adding dependencies or references by artifact locator and MUST be idempotent when the supplied locator already exists in the corresponding list.
- Metadata mutation helpers MUST support list-valued field updates via an explicit ops-based mutation surface (for example add/remove operations), and MUST be idempotent where applicable.

!concept-metadata-mutation.scope.supported-fields:

- Metadata mutation MUST be supported for specification, implementation, and scratch pad artifacts.
- For specifications, metadata mutation MUST support updating the `version` field and adding/removing entries in the `dependencies` list.
- For implementations, metadata mutation MUST support updating the `version` field, updating language fields, and adding/removing entries in the `references` list.
- For scratch pads, metadata mutation MUST support updating any YAML front matter fields except `target`.
  - Scratch pad `target` MUST be treated as immutable; attempts to change it MUST fail with a descriptive error.

## Key Entities

### Entity: DataModelAdapter

Adapter responsible for translating runtime interactions to persisted data model instances.

!entity-datamodeladapter.requirements:

- MUST ensure transformations honor data model invariants.
- SHOULD provide observability hooks for auditing cross-cutting behaviors.
- MAY cache read-mostly projections when it does not compromise consistency guarantees.

### Entity: DependencyTree

Aggregated representation of upstream and downstream relationships for a given artifact.

!entity-dependencytree.requirements:

- MUST capture root artifact metadata together with its direct and transitive dependencies.
- MUST expose traversal helpers to retrieve upstream-only, downstream-only, or combined views.
- SHOULD provide serialization compatible with the [SpecMan Data Model](../specman-data-model/spec.md) for interchange.

### Entity: TemplateDescriptor

Metadata describing how templates are located and rendered.

!entity-templatedescriptor.requirements:

- MUST record the locator URI or absolute path and the intended template scenario (specification, implementation, scratch pad, or derivative work type).
- SHOULD list required substitution tokens so callers MAY validate inputs before rendering.
- MAY reference helper functions that provide contextual data during template expansion.
- When a cached remote template is used, the descriptor MUST record the cache file path and validator metadata supplied by the associated `TemplateCache` entry.

### Entity: TemplateCache

Cache store that retains remote template content referenced by pointer files.

!entity-templatecache.requirements:

- MUST persist downloads inside `.specman/cache/templates/` using deterministic filenames derived from the source locator.
- MUST record the original locator, retrieval timestamp, and any validator metadata (for example `ETag`) so Template Orchestration can determine staleness before reuse.
- SHOULD expose purge and refresh helpers so lifecycle controllers can invalidate entries when pointer files change or when users request a clean refresh.

### Entity: LifecycleController

Controller responsible for enforcing lifecycle policies across specifications, implementations, and scratch pads.

!entity-lifecyclecontroller.requirements:

- MUST orchestrate create and delete operations for every artifact type, delegating to dependency mapping and templating subsystems.
- MUST terminate deletion attempts that would orphan dependents and MUST return the blocking dependency tree to the caller.
- MUST expose deletion entry points that mirror creation workflows so operators have symmetrical controls for specifications, implementations, and scratch pads.
- SHOULD integrate auditing hooks that capture lifecycle events for compliance tracking.
- MUST surface explicit errors when filesystem persistence fails (for example, permissions or missing directories) so callers can remediate issues without corrupting the workspace.

### Entity: ScratchPadProfile

Defines the characteristics and template linkages for scratch pad variants.

!entity-scratchpadprofile.requirements:

- MUST enumerate available scratch pad types alongside their required templates.
- SHOULD expose optional configuration fields to tailor scratch pad content to team workflows.
- MAY reuse `TemplateDescriptor` instances to avoid duplication across related profiles.

## Additional Notes

Migration guides MAY accompany minor releases to help downstream integrators adopt new optional capabilities.

Implementers MAY provide caching or indexing strategies for dependency trees when doing so preserves freshness guarantees.

Template repositories SHOULD be discoverable through configuration so administrators CAN extend or swap template sources without code changes.

Scratch pad workflows MAY integrate with collaboration tooling (e.g., team workspaces) to streamline drafting phases.

[heading slugs]: ../specman-data-model/spec.md#concept-markdown-slugs
[constraint groups]: ../specman-data-model/spec.md#constraint-groups
