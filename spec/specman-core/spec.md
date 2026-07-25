---
name: specman-core
version: "3.0.0"
dependencies:
  - ../../docs/founding-spec.md
  - https://spec.commonmark.org/0.31.2/
---

# Specification — SpecMan Core

The SpecMan Core specification defines both the canonical data model and the platform capabilities that guarantee consistent interactions with SpecMan artifacts. Part 1 establishes the foundational data structures — workspaces, scratch pads, specifications, and implementations — along with their metadata, naming, and layout rules. Part 2 builds on those structures to define the behavioral guarantees implementers MUST honor: workspace discovery, dependency mapping, reference validation, template orchestration, lifecycle automation, structure indexing, metadata mutation, compliance reporting, and workspace status.

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

This specification references the [founding specification](../../docs/founding-spec.md) for background and rationale on the topics and entities discussed herein.

---

# Part 1: Data Model

## Concept: SpecMan Workspace

A SpecMan workspace is the directory in which SpecMan tooling can be used.

### SpecMan Dot Folder

!concept-specman-workspace.dot-folder:

- The SpecMan dot folder MUST be named `.specman` and is used to store tooling state, metadata, and other implementation-specific files that belong to the workspace.
- The presence of a top-level `.specman` directory is the canonical indicator that a directory is a SpecMan workspace root.
- Implementations SHOULD treat the nearest ancestor directory containing a `.specman` folder as the workspace root when tools are invoked from within a subdirectory.
- Tools MAY search parent directories for a `.specman` folder.
- When multiple `.specman` folders are found along the ancestry chain, the nearest one to the current working directory SHOULD be selected as the active workspace root.

## Concept: Scratch Pads

Scratch pads are working documents that track in-progress efforts for SpecMan-aware tooling.

!concept-scratch-pads.naming:

- Each scratch pad MUST reside in its own subdirectory whose name is all lowercase, uses hyphen separators, contains no more than four words, and MAY include verbs.
  - This will act as the **scratch pad name**.
- Scratch pads MAY be deleted when they are no longer being used, but MUST first confirm that no other scratch pads declare a dependency on them.

### Scratch Pad Location

!concept-scratch-pads.location:

- Each scratch pad MUST be stored in it's own folder.
- Scratch pad folders MUST NOT be nested within eachother.
- Each scratch pad folder MUST be stored in a root folder, `scratchpad`.
  - This root folder MUST be located under the [Specman dot folder](#specman-dot-folder).
- The primary scratch pad document inside each subdirectory MUST be named `scratch.md`.
- Each scratch pad folder MAY contain various other documents or files, to assist in making changes.

Example:

- .specman/
  - scratchpad/
    - scratch-pad-name/
      - scratch.md

### Target Artifact

!concept-scratch-pads.target-artifact:

- A scratch pad MUST have a target artifact associated with it.

- The artifact MUST be either a specification or an implementation.
- This artifact MUST be a relative file path, or a URL if the artifact is external.

### Scratch Pad Dependencies

!concept-scratch-pads.dependencies:

- Scratch pads MAY declare dependencies on other scratch pads when the downstream work requires the upstream analysis (for example, a refactor scratch pad depending on a revision scratch pad).
- Scratch pad dependencies MUST reference other scratch pads only; specifications and implementations continue to be expressed through the `target` field.
- A scratch pad MUST NOT be deleted while another scratch pad depends on it.

### Scratch Pad Content

!concept-scratch-pads.content:

- There MUST be specific content included inside of a scratch pad, for readability sake.

- A scratch pad MUST contain a notes section.
  - This is to allow for any AI to resume from little to no context.
- A scratch pad SHOULD have a tasks file.
  - The tasks file will serve as a list of tasks to be completed before the the scratch pad may be considered completed.
  - If present, the tasks file MUST be located under the directory containing the `scratch.md` file, and MUST be labelled `tasks.md`.

### Work Type

!concept-scratch-pads.work-type:

- A scratch pad MUST specify its work type, which specifies what kind of actions are being taken.

- A scratch pad MUST only have one work type.
- Work types MUST be represented as objects, to store data unique to the work type.
  - If the work type does not have any data, it SHOULD be represented as an empty object.

A work type can be one of the following:

- `revision`: a change to the specification
  - The target artifact MUST be a specification. The specification MUST NOT be an external reference.
  - Implies potential refactoring required for all referencing implementations.
  - One or more extra scratch pads MAY be created as a result of a revision.
- `feat`: an introduction of a feature
  - The target artifact MUST be an implementation.
  - SHOULD be used to introduce new functionality via implementations.
- `ref`: a refactor of an implementation
  - The target artifact MUST be an implementation.
  - Implies potential refactoring required for downstream implementations.
- `fix`: a correction applied to an implementation to address defects without modifying specifications
  - The target artifact MUST be an implementation and MUST NOT be a specification or external reference.
  - SHOULD be used when the implementation needs remediation (bug fixes, defects) independent of specification updates.

### Scratch Pad Metadata

!concept-scratch-pads.metadata:

- Scratch pads MUST have front matter metadata to represent the above data.
- Frontmatter fields MUST be formatted as below.

- `target`: the target artifact
- `work_type`: the object representing the work type
  - `revision|feat|ref|fix`: a field on the object representing the work type.
- `dependencies`: a list of [dependencies](#scratch-pad-dependencies).
  - this field MAY be omitted if this scratch pad does not depend on other scratch pads.

!concept-scratch-pads.metadata.unknown-fields:

- Front matter parsers MUST silently ignore any YAML front matter fields that are not defined by the governing specification.
- Unknown fields MUST NOT cause parse errors, warnings, or validation failures.
- This rule applies to specifications, implementations, and scratch pads alike.

### Dependency Graph Integrity

!concept-scratch-pads.dependency-graph-integrity.requirements:

- The combined dependency graph spanning specifications, implementations, and scratch pads MUST remain acyclic.
- Tooling SHOULD validate the dependency graph whenever artifacts are added or updated, and MUST reject or flag any change that would introduce a cycle.
- Authors SHOULD restructure work or adjust dependencies to remove cycles before publishing updates.

## Concept: Specifications

> Reference: [founding specification — Specifications](../../docs/founding-spec.md#specifications)

!concept-specifications.formatting:

- Specifications MUST be written in Markdown.
- Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

### Specification Headings

!concept-specifications.headings.structure:

- Each specification MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).

- Each heading within a specification MUST be unique to the implementation itself.
- Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title or any YAML frontmatter).
  - This heading SHOULD include a reference to RFC 2119 and a short statement indicating how the RFC 2119 normative keywords (for example, MUST, SHOULD, MAY, etc.) are to be interpreted for that document.
  - Other statements or notes SHOULD be added to this heading regarding referenced documents, but MAY be omitted or relocated under other headings as necessary.

#### Concept: Markdown Slugs

CommonMark does not define fragment identifiers for headings. SpecMan defines a deterministic heading-slug algorithm so tooling can reliably generate, resolve, and validate intra-document links.

!concept-markdown-slugs.formatting:

- A heading slug MUST be derived from the heading's plain-text title by applying the following steps in order:
  - Inline content handling: the heading's inline content (as defined by CommonMark) MUST be converted to plain text by stripping formatting while preserving the rendered text (for example, `**bold**` becomes `bold`, inline code backticks are removed, and links contribute their link text).
  - Normalization: the plain-text title MUST be Unicode-normalized using NFKD.
  - Case: the title MUST be lowercased using Unicode case folding.
  - Separator mapping: any Unicode whitespace characters MUST be treated as spaces.
  - Character filtering: any character that is not a Unicode letter (`\p{L}`), Unicode number (`\p{N}`), space, or hyphen (`-`) MUST be removed.
  - Hyphenation: contiguous spaces MUST be replaced by a single hyphen (`-`).
  - Cleanup: contiguous hyphens MUST be collapsed to a single hyphen, and leading/trailing hyphens MUST be removed.
- If the resulting slug is empty, tooling MUST treat the slug as invalid and MUST surface a descriptive error.
- Duplicate disambiguation: within a single Markdown document, if multiple headings produce the same base slug, tooling MUST disambiguate later occurrences by appending a hyphen and a monotonically increasing integer suffix starting at `-1` (for example, `overview`, `overview-1`, `overview-2`).

Tooling MAY implement additional compatibility layers for specific renderers, but when SpecMan tooling generates or validates intra-document links it MUST use the algorithm defined above.

### Specification Concepts and Entities

> Reference: [founding specification — Concepts](../../docs/founding-spec.md#concepts) and [Key Entities](../../docs/founding-spec.md#key-entities)

!concept-specifications.concepts-and-entities.structure:

- Each concept or key entity SHOULD have its own [heading](#specification-headings).

#### Concept: Concept & Entity Headings

Due to the loose nature of Markdown and the lack of built-in heading typing in CommonMark, SpecMan uses a prefix convention so tooling can deterministically identify which headings represent concepts and key entities.

!concept-concept-entity-headings.structure:

- Headings that represent concepts MUST begin with the literal prefix `Concept:` followed by a space and a human-readable name.
- Headings that represent key entities MUST begin with the literal prefix `Entity:` followed by a space and a human-readable name.
- Tooling that parses or renders specifications MUST identify concept/entity headings using the prefixes above by default.
  - To support multilingual or organization-specific conventions, tooling MUST provide configuration options that allow the concept and entity prefixes to be customized.

#### Concept: Constraints

Specifications express requirements as *constraints* using RFC 2119 keywords. SpecMan adds a lightweight, tool-friendly convention for identifying and linking to constraint groups within a concept or entity section.

##### Constraint Content

!concept-constraints.content:

- Each constraint section MUST be associated with exactly one concept or key entity.
  - The association MUST be expressed by the first group in the constraint identifier (the constrained concept/entity heading slug).
- Constraints SHOULD appear under the heading for the concept or key entity they constrain.
  - Constraints MAY appear under subheadings, provided those subheadings are nested under the constrained concept/entity heading.
  - Constraints MAY alternatively appear in a standalone "Constraints" section, provided every constraint section uses a constraint identifier whose first group names the constrained concept/entity heading slug.
- Constraint statements SHOULD be expressed as list items in an unordered list.
- A constraint section MAY include additional Markdown content (paragraphs, code blocks, tables, etc.) directly under the constraint identifier line.
  - Tooling that extracts constraints MUST treat this content as part of the constraint section.
- Additional clauses for a constraint SHOULD be expressed as nested list items immediately under the parent constraint statement.

##### Constraint Groups

Constraint identifiers are made up of one or more *groups*.

!concept-constraints.groups.formatting:

- A group MUST be plain text that satisfies the character filtering rules defined by [Concept: Markdown Slugs](#concept-markdown-slugs).
- A group set MUST be represented as two or more groups delimited by a period (`.`).

!concept-constraints.groups.ordering:

- There MUST be at least two groups in a group set.
  - The first group MAY be the heading slug of the concept/entity heading being constrained.
    - If the first group matches a heading slug, tooling MAY use this match to associate the constraint group with the heading.
  - The second group MUST be a short, human-chosen category name that distinguishes the constraint set (for example `formatting`, `ordering`, `referencing`).
  - Additional groups MAY be appended for further categorization.

##### Constraint Identifier Lines

A *constraint section* is signaled by a single line preceding its constraint list.

!concept-constraints.identifiers.formatting:

- A constraint section MUST start with an exclamation mark (`!`), followed immediately by the group set, and MUST end with a colon (`:`).
- The constraint identifier line MUST be the only content on its line.
- Within a single document, each group set used in a constraint identifier line MUST be unique.
- A constraint section MUST be treated as ended when either:
  - A new constraint identifier line is encountered, or
  - A heading (any ATX heading) is encountered.

Example:

```markdown
!example-markdown-slugs.formatting:
- Headings MUST be converted to plain text before slugging.
  - Tooling SHOULD preserve the rendered text.
```

##### HTML Generation and Referencing

Constraint identifiers are not part of standard Markdown heading/link semantics; they are an additional convention for SpecMan-aware tooling.

!concept-constraints.identifiers.generation:

- When generating HTML, an HTML generator that chooses to support constraint identifiers SHOULD:
  - Attach an HTML `id` equal to the group set (without the leading `!` and trailing `:`) to an element that appears immediately before the associated constraint content.
  - Omit the raw constraint identifier line from rendered HTML.

!concept-constraints.identifiers.referencing:

- When linking to constraints:
  - If the HTML output is constraint-aware (IDs exist), clients MAY link directly to `#{group_set}`.
  - If the HTML output is not constraint-aware, clients MAY support a query-style selector `x-constraint={group_set}` and use it to locate the first matching constraint identifier line in the source text.
    - Clients SHOULD accept both `?x-constraint={group_set}#{heading_slug}` and `#{heading_slug}?x-constraint={group_set}` forms for robustness.

### Specification Layout

!concept-specifications.layout.filesystem:

- Each specification MUST be stored in a folder designated specifically for that specification.

- Specification folders MUST be stored in a top level directory named `spec`.
- Specification folders MUST NOT be nested inside other specification folders.
- The base specification document must be located in that folder, under `spec.md`.

Example:

- [workspace](#concept-specman-workspace)/
  - spec/
    - {spec_name}/
      - spec.md

### Standalone Specifications

> ![NOTE] Standalone specifications are experimental, and may not be added to the non-draft version.

!concept-specifications.standalone.requirements:

- A specification MAY NOT require a reference to an implementation to be used.
  - For example, when a specification defines usage in a common format that can be used without requiring explicit implementation details (e.g. CLI commands)
- When a specification does not require an implementation, this SHOULD be recorded in the spec's top-of-file YAML frontmatter using a boolean field named `requires_implementation`.
  - If `requires_implementation` is omitted, implementations and tooling MUST treat the value as `true` by default.

### Specification Dependencies

> Reference: [founding specification — Dependencies](../../docs/founding-spec.md#dependencies)

!concept-specifications.dependencies:

- Dependencies MUST be either another specification or an external resource that contains documentation detailing a specification.
  - If the dependency is an external resource, it MUST be available in a plaintext format, in such a way that it could be read through a code editor.
  - Tooling MAY omit processing external dependencies outside of presenting the content if they are not formatted in markdown.
- Specifications MUST NOT declare implementations as dependencies. Referencing an implementation would leak technical details into the specification layer and violates the separation between requirements and execution.
- Each dependency item MUST be represented as one of the following forms:
  - A string: a local file path or a URL to another specification document.
  - An object with two fields:
    - `ref` (string): a local file path or a URL pointing to the dependency.
    - `optional` (boolean): when true, indicates this dependency is optional.

If a concept or key entity is referenced from one of the dependencies, it SHOULD be marked with an [inline link](https://spec.commonmark.org/0.31.2/#inline-link).

### Specification Metadata

!concept-specifications.metadata.frontmatter:

- Specifications SHOULD have front-matter at the beginning of the document to declare the above data.
- The frontmatter fields MUST be formatted as listed below.

- `name`: the [specification name](../../docs/founding-spec.md#specification-name)
  - if this field is omitted, processors MUST use the parent directory as the name.
- `version`: the [specification version](../../docs/founding-spec.md#specification-version)
- `dependencies`: a list of [`dependency`](#specification-dependencies)

Example:

```yaml
---
name: spec-name
version: "1.0.0"
dependencies:
  - ../other-spec.md
  - https://example.com/specs/founding-spec.md
  - ref: ../maybe-optional.md
    optional: true
---
```

## Concept: Implementations

> Reference: [founding specification — Implementation](../../docs/founding-spec.md#implementation)

!concept-implementations.formatting:

- Implementations MUST be authored as Markdown documents to support consistent rendering, review, and automated processing.
- Implementations MUST contain human-readable content.

### Specification Coverage

!concept-implementations.specification-coverage.requirements:

- Each implementation MUST declare exactly one core specification that it implements. This contract is represented by the REQUIRED `spec` field in the implementation's front matter.
- Implementations MAY implement multiple specifications. Every additional specification MUST be listed in the implementation `references` array with `type: specification`, and each entry MUST correspond to functionality the implementation actively plans to deliver.
- When a core specification references other specifications, the implementation MUST either implement the referenced specifications itself or determine whether compliant implementations already exist. If such an implementation exists, it SHOULD be referenced and reused as the implementation model instead of reinventing it.
- Specifications included in the implementation references list MUST be intended for implementation. Specifications needed only for background context SHOULD remain in the specification dependency graph rather than the implementation's references.

### Implementation Headings

!concept-implementations.headings.structure:

- Each implementation MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).

- A heading SHOULD be a link if it is a direct reference to a specification concept or key entity.
- If multiple concepts or key entities are related, they SHOULD be linked directly under the heading in an unordered list that provides inline links to the concepts / entities.

### Implementation Layout

!concept-implementations.layout.filesystem:

- Implementation documents MUST be stored in folders.

- Implementation folders MUST be stored in a parent folder named `impl`.
- The root implementation folder MUST be inside of a SpecMan workspace.
- The base implementation document MUST be stored under `impl.md`.
- Related documents MAY be stored inside of the implementation folder.
  - Related documents MUST be human-readable files, with no binary representation. (e.g. markdown, json, yml)

Example:

- [workspace](#concept-specman-workspace)/
  - impl/
    - {impl_name}/
      - impl.md

### Implementation Locators

!concept-implementations.locators.model:

Implementation locators describe where implementation code lives and how it is published.

- The `location` front-matter field MUST point to the root folder of the implementation's code. It MAY be a workspace-relative path or a URL, and MUST remain inside the detected workspace when a workspace exists.
- These locators are distinct from SpecMan locator schemes (`spec://`, `impl://`, `scratch://`); see [Locator Schemes](#locator-schemes) for scheme semantics.

### Locator Schemes

!concept-implementations.locator-schemes.resolution:

SpecMan locator schemes provide canonical handles for specifications, implementations, and scratch pads.

- Supported schemes MUST be `spec://{artifact}`, `impl://{artifact}`, and `scratch://{artifact}`. Each handle identifies the canonical artifact and MUST be unique within a workspace.
- Locator handles MUST be treated as **client inputs and client-facing identifiers**, not artifact content.
  - Specifications, implementations, and scratch pads MUST NOT contain `spec://` / `impl://` / `scratch://` handles in front matter, metadata, or body content.
  - Artifact-to-artifact references stored inside files MUST be expressed as workspace-relative paths or HTTPS URLs.
  - Clients (for example the CLI, MCP adapters, or APIs) MAY accept locator handles as user input and MAY emit locator handles in responses, but they MUST normalize handles to canonical paths before persisting anything into an artifact.
- Resolution rules:
  - `spec://{artifact}` MUST resolve to `spec/{artifact}/spec.md` under the workspace root.
  - `impl://{artifact}` MUST resolve to `impl/{artifact}/impl.md` under the workspace root.
  - `scratch://{artifact}` MUST resolve to `.specman/scratchpad/{artifact}/scratch.md` under the workspace root.
- Clients MUST resolve locator handles using the active workspace root (via workspace discovery or an explicit workspace context).
- If a workspace root cannot be inferred or the resolved path would fall outside the workspace boundary, resolution MUST fail with a descriptive error instead of guessing.

### Implementation References

> Reference: [founding specification — References](../../docs/founding-spec.md#references)

!concept-implementations.references.model:

- Implementations MAY reference external artifacts relied upon by the implementation. This is functionally equivalent to [specification dependencies](#specification-dependencies), but MUST be expressed exclusively as a list of objects.
- These objects MUST adhere to the listed fields below.

- `ref`: local path or URL to target artifact
- `type`: the type of artifact. MUST be one of ("implementation", "specification").
- `optional`: a boolean value indicating whether this reference is optional.

### Implementation APIs

> Reference: [founding specification — APIs](../../docs/founding-spec.md#apis)

!concept-implementations.apis.documentation:

- APIs SHOULD have documentation clearly identifying what the code does.
  - Documentation SHOULD focus on the "what" and the "why," rather than the "how."
- APIs signatures MUST be contained inside of a [fenced code block](https://spec.commonmark.org/0.31.2/#fenced-code-blocks).
  - If the implementation language of the code block has language code, it should be provided in the info string of the code block.
- Each API listed SHOULD contain an inline link to corresponding concepts or key entities, if used.
- If creating API information for a key entity, the structure of the entity MUST be included.
  - The structure of an entity MAY be in either markdown or the code of the implementing language.
    - When using markdown, the format SHOULD be an unordered list using [code spans](https://spec.commonmark.org/0.31.2/#code-spans).
    - When using code, the example SHOULD only show the bare structure - the fields of a structure, and nothing more.
  - The structure MUST define the data type for each field.

### Implementation Metadata

!concept-implementations.metadata.frontmatter:

- Implementations MUST specify YAML frontmatter at the top of the document.
- The frontmatter fields MUST be formatted as listed below.

- `spec`: a local path or URL to the target specification
- `name`: the [implementation name](../../docs/founding-spec.md#implementation-name)
  - if this field is omitted, processors MUST use the parent directory as the implementation name.
- `location`: the location of the source code as defined in [implementation locators](#implementation-locators)

Example:

```yaml
---
spec: ../path/to/spec.md
name: implementation-name
version: "1.0.0"
location: ../path/to/code
---
```

---

# Part 2: Core Behaviors

## Concept: Workspace Discovery

Workspace discovery ensures every SpecMan-aware tool can deterministically locate the active workspace root and its `.specman` directory from any starting location.

!concept-workspace-discovery.requirements:

- The implementation MUST identify the workspace root by scanning the current directory and its ancestors for the nearest `.specman` folder, treating the containing directory as canonical.
- When no `.specman` folder exists along the ancestry chain, the implementation MUST return a descriptive error that callers MAY surface directly to users.
- Workspace discovery utilities MUST expose the absolute path to both the workspace root and the `.specman` directory so downstream services can reference shared metadata without recomputing filesystem state.
- Resolved workspace metadata MUST remain consistent with the data model rules for SpecMan workspaces (see [Concept: SpecMan Workspace](#concept-specman-workspace)) and MUST reuse existing data-model entities when emitting structured results.
- Implementations MAY cache the active workspace root for the lifetime of a command invocation, but they MUST revalidate that the `.specman` folder still exists before reusing cached paths.

!concept-workspace-discovery.initialization:

- The implementation MUST expose an initializer that accepts an absolute filesystem path provided by the caller and resolves it to the canonical workspace root and `.specman` directory using the same rules as workspace discovery.
- The initializer MUST accept both workspace-root paths and `.specman` directory paths as valid inputs; in either case it MUST return normalized absolute paths for both the workspace root and `.specman` directory without redundant ancestor search.
- The initializer MUST validate that the supplied path is (or contains) a `.specman` directory; if validation fails, it MUST either create `.specman` (when allowed by the invocation) or return a descriptive error suitable for direct user display, and it MUST NOT fall back to scanning unrelated ancestor paths.
- When creation is requested and a `.specman` directory is absent at the provided root, the initializer MUST create the `.specman` directory at that root, enforce workspace-boundary rules, and then return normalized paths; it MUST NOT create nested `.specman` directories beneath an existing workspace.
- The implementation MUST expose a library-level workspace creator that provisions `.specman` at an explicit path (including required subdirectories such as `scratchpad/` and `cache/` when defined), performs the same validation as the initializer, and keeps the operation idempotent so future workspace-owned files can be added by the implementation rather than by ad-hoc folder creation.
- The initializer MUST reject relative paths and paths that imply nested workspace creation; callers MUST supply the intended workspace root explicitly rather than relying on automatic ascent from arbitrary subpaths.
- The initializer MAY reuse discovery caches only when the cached workspace root matches the normalized result for the supplied path; otherwise it MUST revalidate (and, if needed, create) the `.specman` directory before returning paths.

## Concept: Data Model Backing Implementation

This concept ties runtime behavior to the data model's authoritative structures.

!concept-data-model-backing-implementation.requirements:

- The implementation MUST persist or retrieve entities exactly as defined in Part 1 of this specification.
- Internal storage representations MAY vary, provided they preserve the documented semantics.
- The implementation SHOULD emit data model validation errors that mirror normative constraints from Part 1.
- All exposed capabilities MUST operate exclusively on types defined in this specification and MUST document deterministic input and output expectations.
- Implementations SHOULD maintain backward compatibility for these capabilities within a given major version of this specification.
- Implementations MUST depend on a single major version of this specification at a time to avoid incompatible schema drift.
- Any serialization emitted by these capabilities MUST validate against the schemas mandated by this specification before it is persisted or returned to callers.

## Concept: Dependency Mapping Services

Dependency mapping provides visibility into upstream and downstream relationships across specifications and implementations.

!concept-dependency-mapping-services.requirements:

- The implementation MUST construct dependency trees that enumerate upstream providers, downstream consumers, and full transitive relationships.
- Dependency lookups MUST return results in upstream, downstream, and aggregate forms to support targeted impact analysis.
- Tree traversal APIs SHOULD expose both hierarchical and flattened views to accommodate varied client needs.
- Implementations MUST expose a callable dependency-tree builder that accepts a filesystem path or HTTPS URL pointing to either a specification or implementation artifact and normalizes that locator relative to the active workspace root before traversal begins.
- The tree builder MUST parse YAML front matter (when present) for dependencies or references, recursively resolve each upstream artifact, and continue traversal until the graph is fully explored or a cycle is encountered.
- Resolvers MUST support filesystem paths (absolute or workspace-relative), HTTPS URLs that point to Markdown specifications or implementations, and SpecMan resource handles expressed as `spec://{artifact}`, `impl://{artifact}`, or `scratch://{artifact}`. Handle semantics and normalization MUST follow the locator scheme rules defined in [Locator Schemes](#locator-schemes), including workspace discovery before traversal begins and resolution to canonical artifact identifiers.
- Requests that supply locator schemes outside of filesystem, HTTPS, or the SpecMan resource handles (`spec://`, `impl://`, `scratch://`) MUST fail fast with a descriptive error that directs callers to use the supported schemes instead of attempting implicit rewrites.
- Requests that reference targets outside of the detected workspace MUST fail with an error that explains the workspace boundary violation.
- Cycle detection MUST terminate traversal immediately and return a descriptive error that includes the partial tree gathered so far so callers can remediate invalid dependency graphs.
- When a referenced dependency or implementation lacks front matter metadata, or when the dependency resolves to HTML or other plaintext without metadata, the tree builder MUST still add the artifact to the dependency set using the best available identifier (path or URL) and annotate the entry to indicate metadata was unavailable.

## Concept: Reference Validation

Reference validation ensures that references embedded in Markdown artifacts — particularly link destinations in inline links — can be validated deterministically against the workspace filesystem and external HTTPS resources. This enables tooling to detect broken links early, prevent invalid relationship graphs, and provide actionable diagnostics to authors.

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
  - If such a handle is encountered in a Markdown link destination, the validator MUST report it as invalid and MUST NOT attempt to resolve or "validate" the target.
- If a destination uses a scheme outside the supported set for Markdown references (workspace-filesystem paths and HTTPS URLs), the validator MUST report it as invalid and MUST NOT attempt implicit rewrites.
- When validating filesystem destinations, the validator MUST resolve them relative to the source artifact's directory.
  - The validator MUST normalize the resolved path and MUST enforce workspace-boundary rules (it MUST NOT allow traversal outside the workspace root after normalization).
  - The validator SHOULD additionally enforce the workspace boundary using canonicalized paths (for example resolving symlinks/junctions) when the platform and permissions allow.
- When validating HTTPS destinations, the validator MUST at minimum validate that the destination is a well-formed HTTPS URL.
  - The validator SHOULD support an optional reachability check mode (for example `HEAD`/`GET`).
  - When reachability mode is enabled, the validator MUST treat HTTPS redirects (3xx) as success.
  - When reachability mode is enabled, the validator MUST NOT treat timeouts as validation failures; it SHOULD instead emit a non-fatal diagnostic indicating the check could not be completed.
- When a destination contains a fragment component (for example `./doc.md#some-heading`) and the destination resolves to Markdown, the validator MUST validate that the fragment refers to an existing heading slug as defined by [Concept: Markdown Slugs](#concept-markdown-slugs).
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

## Concept: Template Orchestration

Template orchestration governs how reusable content is discovered and rendered.

!concept-template-orchestration.requirements:

- Templates MUST declare substitution tokens using double braces (`{{token_name}}`), and rendering engines MUST refuse to materialize output until every declared token is supplied.
- Template consumers MUST accept locator inputs expressed as absolute filesystem paths, workspace-relative paths rooted at the discovered workspace, HTTPS URLs, or packaged-default identifiers bundled with the runtime.
- When creating specifications, implementations, or scratch pads, the orchestrator MUST search for workspace-managed overrides under `.specman/templates/` in the following order: (1) artifact-specific Markdown files (for example `.specman/templates/spec.md`, `.specman/templates/impl.md`, or `.specman/templates/scratch.md` plus any nested directories the workspace defines), (2) uppercase pointer files (`SPEC`, `IMPL`, `SCRATCH`) whose contents resolve to workspace-relative paths or HTTPS URLs, and (3) packaged defaults embedded with the SpecMan Core runtime. Packaged defaults MUST be versioned with the runtime, remain read-only, and MAY be delivered via resources compiled into the binary or co-located artifacts inside the packaged application.
- Implementations MUST expose pointer-file lifecycle helpers for every artifact profile so callers can add new `SPEC`, `IMPL`, or `SCRATCH` pointer files, update (set) their target locators, or remove them without editing the filesystem manually.
- Pointer update operations MUST persist uppercase pointer files under `.specman/templates/`, enforce the same locator validation rules defined for runtime resolution (workspace-bound filesystem paths and reachable HTTPS Markdown), and MUST refresh any `.specman/cache/templates/` entries referencing the affected locator before signaling success.
- Pointer removal operations MUST delete the targeted pointer file, purge cached remote content that referenced it, and MUST document the resulting fallback search order so clients know which template source will be used next. When the removal would leave the orchestration layer without any valid template source, the helper MUST fail with a descriptive error instead of leaving an invalid pointer state.
- Pointer-file lifecycle helpers MUST surface structured success and failure results — including validation errors or fallback descriptions — so CLI layers and APIs can relay operator-facing guidance without re-parsing filesystem state.
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

## Concept: Deterministic Execution

Deterministic execution codifies behavioral guarantees so downstream consumers can rely on predictable, side-effect-aware APIs.

!concept-deterministic-execution.requirements:

- Consumers MUST treat all SpecMan Core functions as pure unless the documentation explicitly calls out side effects; implementers MUST document any deviations before release.
- Breaking changes to function signatures or observable behaviors MUST trigger a major version increment of this specification so dependent tooling can coordinate adoption.

## Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications, implementations, and scratch pads.

!concept-lifecycle-automation.requirements:

- Automated creation flows MUST require an associated template locator and MUST validate that required tokens are supplied.
- Lifecycle operations MUST enforce template usage for all new specifications, implementations, and scratch pads so generated artifacts remain data-model compliant.
- Implementations MUST expose user-facing deletion workflows for specifications, implementations, and scratch pads so that every artifact type can be removed with the same rigor applied to creation.
- Creation tooling MUST cover all three artifact types (specifications, implementations, scratch pads) and MUST enforce the naming and metadata rules defined by Part 1 of this specification and the [founding specification](../../docs/founding-spec.md).
- Creation workflows MUST persist generated Markdown artifacts and supporting metadata into the canonical workspace locations (`spec/{name}/spec.md`, `impl/{name}/impl.md`, `.specman/scratchpad/{slug}/scratch.md`) using the paths returned by workspace discovery.
- When a pointer file downloads content from an HTTPS locator, Lifecycle automation MUST route the rendered template through the `.specman/cache/templates/` store before writing artifacts so repeated invocations reuse the cached copy unless the pointer or upstream content changes.
- Persistence helpers MUST write the rendered template output (with all required tokens populated) together with its front matter or metadata; persisting additional representations of entities, concepts, or other runtime data structures is out of scope for this specification.
- Lifecycle automation MUST provide direct integrations with the metadata mutation capabilities described in [Concept: Metadata Mutation](#concept-metadata-mutation).
- Deletion workflows MUST reuse dependency mapping services, refuse to proceed when dependent artifacts exist, and MUST return a dependency tree describing all impacted consumers whenever a removal is blocked.
- Deletion workflows MUST ensure the targeted artifact and any associated metadata or scratch pad directories are removed from their canonical workspace locations once safety checks pass.
- Scratch pad creation workflows MUST offer selectable profiles aligned with defined scratch pad types and MUST leverage corresponding templates.
- Lifecycle controllers MUST expose a persistence interface that can round-trip newly created artifacts back onto disk and SHOULD surface explicit errors if the filesystem write fails so callers can remediate workspace permissions.

!concept-lifecycle-automation.frontmatter-generation:

- Creation workflows MUST generate or merge YAML front matter after template rendering so that every artifact persists the metadata mandated by Part 1 and governing specifications.
- Templates MUST NOT embed YAML front matter; lifecycle automation and metadata mutation workflows are the authoritative mechanisms for writing and updating metadata.
- Metadata mutation helpers MUST update the YAML front matter in-place without rewriting the Markdown body and MUST continue to enforce the workspace-boundary and locator-validation rules defined elsewhere in this specification.

## Concept: SpecMan Structure

Creating a structure which maps the SpecMan data model allows consumers to read markdown content when given identifiers for concepts, key entities, or constraints.

### Structure Indexing

To make sure that entities can be easily searched, implementations MUST index documents that are stored or referenced inside of the workspace.

!concept-specman-structure.indexing.collection:

- Implementations MUST index all markdown documents
- HTML documents MAY optionally be indexed.
- Scratch pad markdown artifacts MUST be included in structure indexing and validation outputs.

!concept-specman-structure.indexing.headings:

- Each heading and their content MUST be indexed.
  - The content of the heading shall be defined as any markdown content, subheadings, or constraint groups located underneath this heading.

!concept-specman-structure.indexing.constraints:

- Each constraint group MUST be indexed.

!concept-specman-structure.indexing.relationships:

Relationships provide a way to construct a relationship graph by parsing the content of an entity, and finding inline links to include content inside of it.

- Headings MUST have a mapped relationship to the implementation or specification which it is stored in.
- Headings MUST have a mapped relationship to other headings that have been referenced via inline links inside of the heading content.
- Constraint groups MAY have a mapped relationship to the heading whose slug may be discovered by the first part of the constraint group.
  - If a heading can not be matched via slug to the first group, then a relationship MUST be indexed to the nearest containing heading which contains the constraint group.

!concept-specman-structure.referencing.validation:

- Implementations that index relationships from inline links MUST provide a method to validate the referenced destinations and report any invalid references.
- Relationship indexing MUST NOT silently drop invalid references; it MUST either record them as invalid with diagnostics or fail the indexing operation with a descriptive error.
- Validation of inline-link destinations used for relationships MUST reuse the locator normalization and workspace-boundary rules described elsewhere in this specification.

### Structure Discovery

Discovery allows for consumers of implementations to find the markdown content of a related item by using identifiers.

!concept-specman-structure.discovery.identifiers:

- Implementations MUST provide methods for enumerating the available identifiers of [heading slugs](#concept-markdown-slugs) and [constraint groups](#constraint-groups).

!concept-specman-structure.discovery.rendering:

Rendering the markdown content allows for readers to properly understand all possible related context.

- Implementations MUST return markdown content when provided with a heading slug.
  - The content inside of the heading slug must return content under any related headings that have been referenced via inline link, in the order of which the inline links were referenced.
  - Implementations MUST ensure that referenced headings content is not duplicated, so that it may only appear once when rendering the markdown content and its related content.
  - Implementations MUST detect cycles during recursive rendering and MUST NOT enter a cycle.
  - Implementations MUST enforce a maximum recursion depth of 50 levels when following referenced headings.
- Implementations MUST return markdown content when provided with a constraint group identifier.
  - The rendered content MUST contain the content of the heading which the constraint group has an active relationship to, which MUST recursively include content from inline-linked headings subject to the heading-rendering rules above.

## Concept: Metadata Mutation

Metadata mutation ensures YAML front matter for specifications, implementations, and scratch pads can be updated without rewriting the surrounding Markdown content.

!concept-metadata-mutation.requirements:

- Implementations MUST expose metadata mutation interfaces that accept a structured update object corresponding to the artifact type (specification, implementation, or scratch pad).
- The update object MUST strictly define the fields eligible for mutation on that artifact type; it MUST NOT allow arbitrary key-value insertion into the front matter.
- Mutation operations MUST apply updates by merging the provided structure into the existing front matter (Partial Update semantics):
  - If a field is omitted from the update object, the existing value in the front matter MUST remain unchanged.
  - Scalar fields (strings, numbers, booleans) present in the update object MUST replace the existing values.
- For list-valued fields (such as dependencies or references), the update object MUST provide dedicated properties to add or remove items without requiring the caller to provide the full list:
  - The interface MUST support `add_{field}` properties (e.g., `add_dependencies`) to append unique items to the list.
  - The interface MUST support `remove_{field}` properties (e.g., `remove_dependencies`) to remove items from the list.
  - The interface MAY support the base field name (e.g., `dependencies`) to perform a full replacement (set) of the list.
- Implementations MUST NOT require callers to construct a list of abstract "operation" commands (e.g., `{"op": "add", ...}`). Instead, the API surface MUST be a strongly-typed or schema-validated structure.
- Metadata mutation helpers MUST reuse the locator normalization, workspace-boundary enforcement, and supported-scheme validation rules defined for dependency traversal before applying edits.
- Metadata mutation operations MUST rewrite only the YAML front matter block and MUST either persist the updated artifact to its canonical path or return the full document with body content unchanged.

!concept-metadata-mutation.scope.supported-fields:

- Metadata mutation MUST be supported for specification, implementation, and scratch pad artifacts.
- For specifications, metadata mutation MUST support updating the `version` field and adding/removing entries in the `dependencies` list.
- For implementations, metadata mutation MUST support updating the `version` field, updating language fields, and adding/removing entries in the `references` list.
- For scratch pads, metadata mutation MUST support updating any YAML front matter fields except `target`.
  - Scratch pad `target` MUST be treated as immutable; attempts to change it MUST fail with a descriptive error.

## Concept: Validation Anchors

Validation anchors provide a mechanism to enforce and verify that an implementation adheres to the requirements defined in its backing specification.

!concept-validation-anchors.definition:

- A validation anchor MUST be a text marker embedded within the source code or related files of an implementation.
- Validation anchors MUST reference a specific constraint group defined in the specification.
- Implementations SHOULD use validation anchors to explicitly demonstrate compliance with specification constraints.

## Concept: Validation Scanning

Validation scanning defines how tooling discovers anchors within an implementation.

!concept-validation-scanning.scope:

- Tooling MUST resolve the implementation's source code root using the `location` field defined in the implementation metadata.
- The scanner MUST recursively traverse the source directory to identify files for analysis.

!concept-validation-scanning.filtering:

- The scanner MUST respect standard `.gitignore` rules found in the implementation's source tree; ignored files MUST NOT be scanned.
- The scanner MUST only analyze files containing text content (e.g., encoded in UTF-8 or ASCII).
- The scanner MUST skip binary files.

## Concept: Compliance Reporting

Compliance reporting exposes the relationship between specification constraints and implementation anchors.

!concept-compliance-reporting.interface:

- Implementations MUST provide an interface or surface to generate compliance reports.
- The reporting tool MUST resolve the target specification from the implementation's `spec` metadata.
- The tool MUST extract all constraint groups from the resolved specification and its transitive specification dependencies.
- The tool MUST scan the implementation's source location for validation tags.
- The reporting tool MUST scope structural indexing to the implementation, its governing specification, and those specification dependencies; unrelated workspace artifacts (including scratch pads) MUST be ignored and MUST NOT cause compliance report failures.

!concept-compliance-reporting.coverage:

- The report MUST calculate coverage by mapping found validation tags to specification constraint groups.
- A constraint group MUST be considered "covered" if at least one validation tag referencing its identifier is found.
- The report MUST identify the file paths and line numbers of all discovered validation tags.
- The report SHOULD warn about "orphaned" tags that reference non-existent constraint groups.

!concept-compliance-reporting.semantics:

- The presence of a validation tag implies that the implementation logic satisfying the referenced constraint group is present at or near the tag's location.
- If multiple tags reference the same constraint group, tooling MUST report all locations, acknowledging that verification may be distributed across multiple files (e.g., specification tests vs. unit tests).
- Authors SHOULD place validation tags at the top of a file if that file is dedicated to testing the referenced constraint group.

## Concept: Workspace Status

Workspace status provides a holistic health check of the workspace, aggregating validation results across all managed artifacts to ensure structural integrity and compliance coverage.

!concept-workspace-status.requirements:

- The implementation MUST expose a workspace status capability that scans specifications, implementations, and scratch pads within the active workspace.
- The status check MUST accept a configuration (e.g., flags or options) to enable or disable specific validation categories. Supported categories MUST include at least:
  - `structure`: Validates YAML front matter and basic artifact validity.
  - `references`: Validates inline links, dependencies, and external URLs.
  - `cycles`: Validates the dependency graph for cycles.
  - `compliance`: Validates implementation anchor coverage.
  - `scratchpads`: Includes scratch pads in the validation set.
- Implementations SHOULD enable all validation categories by default unless explicitly disabled by the user.
- When `structure` validation is enabled, the status check MUST validate that every scanned artifact has valid YAML front matter conforming to this specification.
- When `references` validation is enabled, the status check MUST perform reference validation on all artifacts, ensuring that:
  - All inline links to workspace files resolve to existing files.
  - All inline links to HTTP(S) resources are valid URLs (connectivity checks MAY be optional or configurable).
  - All artifact dependencies (in specifications) and references (in implementations) resolve to existing, valid artifacts.
  - The `location` path in implementation metadata resolves to an existing directory on the filesystem.
- When `cycles` validation is enabled, the status check MUST construct the full dependency graph and verify that no cyclic dependencies exist between specifications.
- When `compliance` validation is enabled, the status check MUST verify compliance coverage for every implementation:
  - It MUST extract all constraint groups from the implementation's governing specification (and transitive dependencies).
  - It MUST scan the implementation's source code for validation anchors.
  - It MUST report a failure if any mandatory constraint group lacks a corresponding validation anchor.
- When `scratchpads` validation is enabled, the status check MUST apply the other enabled checks to scratch pad artifacts; if disabled, scratch pads MUST be ignored.
- The status check MUST return an aggregated report containing:
  - A primary section for Specifications and Implementations, determining the global pass/fail status.
  - A secondary, distinct section for Scratch Pad validation results (if enabled), which MUST NOT affect the global pass/fail status of the workspace.
  - A comprehensive list of all validation errors, warnings, and missing compliance anchors, grouped by artifact.
- The status check SHOULD NOT halt on the first error; it MUST attempt to collect all discoverable errors in the workspace.

---

# Part 3: Key Entities

## Entity: DataModelAdapter

Adapter responsible for translating runtime interactions to persisted data model instances.

!entity-datamodeladapter.requirements:

- MUST ensure transformations honor data model invariants.
- SHOULD provide observability hooks for auditing cross-cutting behaviors.
- MAY cache read-mostly projections when it does not compromise consistency guarantees.

## Entity: DependencyTree

Aggregated representation of upstream and downstream relationships for a given artifact.

!entity-dependencytree.requirements:

- MUST capture root artifact metadata together with its direct and transitive dependencies.
- MUST expose traversal helpers to retrieve upstream-only, downstream-only, or combined views.
- SHOULD provide serialization compatible with this specification for interchange.

## Entity: TemplateDescriptor

Metadata describing how templates are located and rendered.

!entity-templatedescriptor.requirements:

- MUST record the locator URI or absolute path and the intended template scenario (specification, implementation, scratch pad, or derivative work type).
- SHOULD list required substitution tokens so callers MAY validate inputs before rendering.
- MAY reference helper functions that provide contextual data during template expansion.
- When a cached remote template is used, the descriptor MUST record the cache file path and validator metadata supplied by the associated `TemplateCache` entry.

## Entity: TemplateCache

Cache store that retains remote template content referenced by pointer files.

!entity-templatecache.requirements:

- MUST persist downloads inside `.specman/cache/templates/` using deterministic filenames derived from the source locator.
- MUST record the original locator, retrieval timestamp, and any validator metadata (for example `ETag`) so Template Orchestration can determine staleness before reuse.
- SHOULD expose purge and refresh helpers so lifecycle controllers can invalidate entries when pointer files change or when users request a clean refresh.

## Entity: LifecycleController

Controller responsible for enforcing lifecycle policies across specifications, implementations, and scratch pads.

!entity-lifecyclecontroller.requirements:

- MUST orchestrate create and delete operations for every artifact type, delegating to dependency mapping and templating subsystems.
- MUST terminate deletion attempts that would orphan dependents and MUST return the blocking dependency tree to the caller.
- MUST expose deletion entry points that mirror creation workflows so operators have symmetrical controls for specifications, implementations, and scratch pads.
- SHOULD integrate auditing hooks that capture lifecycle events for compliance tracking.
- MUST surface explicit errors when filesystem persistence fails (for example, permissions or missing directories) so callers can remediate issues without corrupting the workspace.

## Entity: ScratchPadProfile

Defines the characteristics and template linkages for scratch pad variants.

!entity-scratchpadprofile.requirements:

- MUST enumerate available scratch pad types alongside their required templates.
- SHOULD expose optional configuration fields to tailor scratch pad content to team workflows.
- MAY reuse `TemplateDescriptor` instances to avoid duplication across related profiles.

## Entity: Validation Tag

A validation tag is the concrete syntax used to define a validation anchor.

!entity-validation-tag.syntax:

- A validation tag MUST be enclosed in square brackets (`[]`).
- A validation tag MUST start with the case-insensitive keyword `ENSURES`, followed by a colon (`:`).
- The tag MUST contain a valid constraint group identifier (e.g., `concept-slug.category`).
- The tag MAY contain an optional type specification, separated from the identifier by a colon (`:`).
- Whitespace chars (spaces, tabs) defined by Unicode MAY be included around the keyword, colons, identifier, and type for readability; tooling MUST ignore this whitespace during parsing.

!entity-validation-tag.types:

- The validation tag type determines the nature of the verification.
- Supported types MUST be:
  - `TEST`: Represents an automated unit or integration test (Default).
  - `CHECK`: Represents a logical check within the code (e.g., conditional logic, pattern matching).
  - `MANUAL`: Represents a manual assertion or non-automated verification.
- If the type is omitted, tooling MUST treat the tag as type `TEST`.

## Entity: WorkspaceStatusConfig

Configuration object for the workspace status capability.

!entity-workspacestatusconfig.schema:

- `structure` (boolean, required): Enable structure validation (front matter, versioning). Default: `true`.
- `references` (boolean, required): Enable reference validation. Default: `true`.
- `cycles` (boolean, required): Enable dependency cycle detection. Default: `true`.
- `compliance` (boolean, required): Enable compliance checks (anchors). Default: `true`.
- `scratchpads` (boolean, required): Include scratch pads in validation. Default: `true`.

## Entity: WorkspaceStatusReport

The aggregated result of a workspace status check.

!entity-workspacestatusreport.schema:

- `global_status` (string, enum: `Pass` | `Fail`): Overall workspace status.
- `spec_impl_status` (string, enum: `Pass` | `Fail`): Status for specification and implementation artifacts.
- `scratchpad_status` (string, enum: `Pass` | `Fail`): Status for scratch pad artifacts.
- `artifacts` (map<ArtifactId, ArtifactStatus>): Detailed status per artifact.
- `cycle_errors` (array<string>): Global errors related to dependency cycles.
- `structure_errors` (array<string>): Global errors related to workspace structure.
- `artifact_count` (integer): Total number of artifacts processed.

## Entity: ArtifactStatus

Validation results for a single artifact.

!entity-artifactstatus.schema:

- `structure_errors` (array<string>): Errors related to file structure or front matter.
- `reference_errors` (array<ReferenceValidationIssue>): Errors from reference validation.
- `compliance_missing` (array<string>): Missing compliance constraints (implementations only).
- `compliance_orphans` (array<ValidationTag>): Orphaned compliance tags (implementations only).

## Entity: ReferenceValidationOptions

Configuration options for reference validation.

!entity-referencevalidationoptions.schema:

- `https` (object): HTTPS validation options.
  - `mode` (string, enum: `check-syntax` | `check-reachability`): Validation mode.
- `transitive` (object): Transitive traversal options.
  - `enabled` (boolean): Whether to validate linked documents.
  - `max_documents` (integer): Maximum traversal depth/count.

---

## Additional Notes

Migration guides MAY accompany minor releases to help downstream integrators adopt new optional capabilities.

Implementers MAY provide caching or indexing strategies for dependency trees when doing so preserves freshness guarantees.

Template repositories SHOULD be discoverable through configuration so administrators CAN extend or swap template sources without code changes.

Scratch pad workflows MAY integrate with collaboration tooling (e.g., team workspaces) to streamline drafting phases.
