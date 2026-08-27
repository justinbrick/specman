---
name: specman-core
dependencies:
  - ../../docs/founding-spec.md
  - https://spec.commonmark.org/0.31.2/
---

# Specification — SpecMan Core

The SpecMan Core specification defines both the canonical data model and the platform capabilities that guarantee consistent interactions with SpecMan artifacts. Part 1 establishes the foundational data structures — workspaces, project manifests, specifications, implementations, git references, and SpecMan installing — along with their metadata, naming, and layout rules. Part 2 builds on those structures to define the behavioral guarantees implementers MUST honor: workspace discovery, dependency mapping, reference validation, template orchestration, lifecycle automation, structure indexing, metadata mutation, compliance reporting, workspace status, and SpecMan installing.

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

This specification references the [founding specification](../../docs/founding-spec.md) for background and rationale on the topics and entities discussed herein.

---

# Part 1: Data Model

## Concept: SpecMan Workspace

A SpecMan workspace is the directory in which SpecMan tooling can be used. Each workspace corresponds to exactly one git repository containing exactly one SpecMan project (either a specification or an implementation).

### SpecMan Dot Folder

!concept-specman-workspace.dot-folder:

- The SpecMan dot folder MUST be named `.specman` and is used to store tooling state, metadata, and other implementation-specific files that belong to the workspace.
- The presence of a top-level `.specman` directory is the canonical indicator that a directory is a SpecMan workspace root.
- Implementations SHOULD treat the nearest ancestor directory containing a `.specman` folder as the workspace root when tools are invoked from within a subdirectory.
- Tools MAY search parent directories for a `.specman` folder.
- When multiple `.specman` folders are found along the ancestry chain, the nearest one to the current working directory SHOULD be selected as the active workspace root.

!concept-specman-workspace.git-co-location:

- The `.specman/` directory and a `.git/` directory MUST exist at the same directory level.
- Tooling MUST verify that a git repository exists at the workspace root before performing initialization or validation operations.
- If no git repository exists, tooling MUST fail with a descriptive error instructing the user to run `git init` first.
- Tooling MUST NOT support monorepo layouts (multiple `specman.json` files under a single `.git/` directory). Each SpecMan project requires its own dedicated git repository.

## Concept: Specifications

> Reference: [founding specification — Specifications](../../docs/founding-spec.md#specifications)

!concept-specifications.formatting:

- Specifications MUST be written in Markdown.
- Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

### Specification Headings

!concept-specifications.headings.structure:

- Each specification MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).

- Each heading within a specification MUST be unique to the implementation itself.
- Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title).
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

- Each specification lives at the root of its own git repository. There is no requirement for a dedicated `spec/` subdirectory since the repository itself is the specification project.
- The primary specification document MUST be specified by the `spec.index` field in `specman.json` when that field is present.
  - The `spec.index` field is OPTIONAL and, when present, MUST NOT be null.
  - If `spec.index` is absent, tooling MUST NOT assume a default filename such as `spec.md`.
- When `spec.index` is present, tooling MUST scan the referenced document for constraint groups, concepts, entities, and other specification content.

Example:

- [workspace](#concept-specman-workspace)/
  - spec.md
  - specman.json  (contains `"spec": { "index": "spec.md" }`)

### Standalone Specifications

> ![NOTE] Standalone specifications are experimental, and may not be added to the non-draft version.

!concept-specifications.standalone.requirements:

- A specification MAY NOT require a reference to an implementation to be used.
  - For example, when a specification defines usage in a common format that can be used without requiring explicit implementation details (e.g. CLI commands)
- When a specification does not require an implementation, this SHOULD be indicated by the absence of any `impl` implementations referencing it in the dependency graph. Tooling MAY infer standalone status from the lack of referencing implementations.

### Specification Dependencies

> Reference: [founding specification — Dependencies](../../docs/founding-spec.md#dependencies)

!concept-specifications.dependencies:

- Dependencies MUST be either another specification or an external resource that contains documentation detailing a specification.
  - If the dependency is an external resource, it MUST be available in a plaintext format, in such a way that it could be read through a code editor.
  - Tooling MAY omit processing external dependencies outside of presenting the content if they are not formatted in markdown.
- Specifications MUST NOT declare implementations as dependencies. Referencing an implementation would leak technical details into the specification layer and violates the separation between requirements and execution.
- Each dependency on another SpecMan specification MUST be expressed as a [GitReference](#entity-gitreference) in the `spec.references` array of `specman.json`.
- Dependencies on external (non-SpecMan) resources MAY be expressed as URLs in the Markdown body or as additional fields in `specman.json`.

If a concept or key entity is referenced from one of the dependencies, it SHOULD be marked with an [inline link](https://spec.commonmark.org/0.31.2/#inline-link).

### Specification Metadata

!concept-specifications.metadata.project-manifest:

- Specification project-level metadata MUST be stored in a `specman.json` file at the repository root, as defined in [Concept: Project Manifest (specman.json)](#concept-project-manifest-specmanjson).
- The `specman.json` file MUST contain a `spec` object (not `impl`).
- Specification Markdown documents (`spec.md`) contain the specification body. All project-level metadata fields defined by this specification MUST reside in `specman.json`.
- Tooling that parses specifications MUST read project metadata from `specman.json`, not from Markdown frontmatter.

!concept-specifications.metadata.versioning:

- SpecMan does NOT define an explicit version field in project metadata.
- Authors MUST use git tags to mark versions of their specifications.
- Tooling MAY read git tags to determine artifact versions for display or compatibility purposes, but MUST NOT require a version field in `specman.json`.

## Concept: Implementations

> Reference: [founding specification — Implementation](../../docs/founding-spec.md#implementation)

!concept-implementations.formatting:

- Implementations MUST be authored as Markdown documents to support consistent rendering, review, and automated processing.
- Implementations MUST contain human-readable content.

### Specification Coverage

!concept-implementations.specification-coverage.requirements:

- Each implementation MUST declare the specifications it implements via the `impl.implements` array in `specman.json`, as defined in [Entity: ImplProject](#entity-implproject).
- Each entry in `implements` MUST contain a [GitReference](#entity-gitreference) pointing to a specification repository.
- When a core specification references other specifications, the implementation MUST either implement the referenced specifications itself (by adding them to `implements`) or determine whether compliant implementations already exist. If such an implementation exists, it SHOULD be referenced via `utilizes` instead of reinventing it.
- Specifications included in the `implements` list MUST be intended for implementation. Specifications needed only for background context SHOULD remain in the specification dependency graph rather than the implementation's `implements`.

### Implementation Headings

!concept-implementations.headings.structure:

- Each implementation MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).

- A heading SHOULD be a link if it is a direct reference to a specification concept or key entity.
- If multiple concepts or key entities are related, they SHOULD be linked directly under the heading in an unordered list that provides inline links to the concepts / entities.

### Implementation Layout

!concept-implementations.layout.filesystem:

- Each implementation lives at the root of its own git repository. There is no requirement for a dedicated `impl/` subdirectory since the repository itself is the implementation project.
- The base implementation document MUST be stored under `impl.md` at the repository root.
- Related documents MAY be stored inside the repository.
  - Related documents MUST be human-readable files, with no binary representation. (e.g. markdown, json, yml)

Example:

- [workspace](#concept-specman-workspace)/
  - impl.md
  - specman.json

### Implementation References

> Reference: [founding specification — References](../../docs/founding-spec.md#references)

!concept-implementations.references.model:

- Implementations declare relationships to other artifacts through the `impl.implements` and `impl.utilizes` arrays in `specman.json`.
- `implements`: specifications this implementation targets. Each entry MUST be an [ImplementsEntry](#entity-implementsentry) containing a [GitReference](#entity-gitreference) that points to a specification.
- `utilizes`: other implementations this implementation depends on. Each entry MUST be a [UtilizesEntry](#entity-utilizesentry) containing a [GitReference](#entity-gitreference) that points to an implementation.

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

!concept-implementations.metadata.project-manifest:

- Implementation project-level metadata MUST be stored in a `specman.json` file at the repository root, as defined in [Concept: Project Manifest (specman.json)](#concept-project-manifest-specmanjson).
- The `specman.json` file MUST contain an `impl` object (not `spec`).
- Implementation Markdown documents (`impl.md`) contain the implementation body. All project-level metadata fields defined by this specification MUST reside in `specman.json`.
- Tooling that parses implementations MUST read project metadata from `specman.json`, not from Markdown frontmatter.

!concept-implementations.metadata.versioning:

- SpecMan does NOT define an explicit version field in project metadata.
- Authors MUST use git tags to mark versions of their implementations.
- Tooling MAY read git tags to determine artifact versions for display or compatibility purposes, but MUST NOT require a version field in `specman.json`.

---

---

## Concept: Project Manifest (specman.json)

The project manifest is a JSON file named `specman.json` that lives at the root of a SpecMan-governed git repository. It is the single authoritative source for all project-level metadata.

!concept-project-manifest.location:

- The `specman.json` file MUST reside at the repository root (the same directory that contains the `.git/` and `.specman/` directories).
- The `specman.json` file MUST NOT be added to any `.gitignore` file within the repository.
- Implementations that perform initialization MUST ensure a git repository exists at the same level as the `specman.json` file they create or validate.
  - If no git repository exists, the implementation MUST fail with a descriptive error instructing the user to run `git init` first.

!concept-project-manifest.formatting:

- The `specman.json` file MUST be valid JSON as defined by RFC 8259.
- The `specman.json` file MUST be UTF-8 encoded.
- The `specman.json` file SHOULD include a top-level `$schema` field referencing the canonical SpecMan JSON Schema. The schema is defined in [`specman.schema.json`](specman.schema.json), colocated with this specification.
- The canonical `$schema` value is the URL for [this schema](specman.schema.json).

!concept-project-manifest.mutual-exclusion:

- A `specman.json` file MUST contain exactly one of `spec` or `impl` at the top level.
- If `spec` is present, `impl` MUST be absent (not present as a key).
- If `impl` is present, `spec` MUST be absent (not present as a key).
- Neither `spec` nor `impl` may be `null`.

!concept-project-manifest.versioning:

- SpecMan does NOT define an explicit version field in project metadata.
- Authors MUST use git tags to mark versions of their specifications and implementations.
- Tooling MAY read git tags to determine artifact versions for display or compatibility purposes, but MUST NOT require a version field in `specman.json`.

## Entity: GitReference

A GitReference identifies a SpecMan artifact (specification or implementation) via its git repository.

!entity-gitreference.fields:

- `alias` (string, REQUIRED): A local name for the referenced artifact. This is the handle used to refer to the dependency in local path resolution and symlink naming. The alias SHOULD be stable across versions of the referencing project.
- `url` (string, REQUIRED): The git remote URL. MUST be either an SSH git link (e.g., `git@gitservice.com:owner/repo.git`) or an HTTPS git link (e.g., `https://gitservice.com/owner/repo.git`). The URL MUST be usable as an argument to `git clone`.
- `ref` (string, REQUIRED): A reference to a specific commit. The value is either a full commit hash or a git tag name. Branches are never consulted; the implementation resolves tags to commit hashes before any clone or checkout operation. If a tag name is supplied, implementations MUST resolve it to its commit hash via `git ls-remote` before use.

!entity-gitreference.tag-resolution:

- If `ref` is a tag name (i.e., it is not a 40-character hex string), implementations MUST resolve it to a commit hash using `git ls-remote --tags {url} {ref}` before performing any clone.
- If tag resolution fails, the implementation MUST fail with a descriptive error.

!entity-gitreference.identity-matching:

- For the purpose of determining whether two `GitReference` values refer to the same artifact, implementations MUST compare the git host and path extracted from `url` (e.g., `example.com/owner/repo`), ignoring the scheme (SSH vs HTTPS) and the `.git` suffix.

## Entity: SpecManProject

The top-level structure of a `specman.json` file.

!entity-specmanproject.fields:

- `$schema` (string, OPTIONAL): A relative file URL pointing to the canonical SpecMan JSON Schema (`specman.schema.json`, colocated with this specification). When present, implementers SHOULD validate the document against this schema before processing.
- `name` (string, REQUIRED): The project name. MAY be any UTF-8 string. This is a free-form human-readable name.
- `description` (string, OPTIONAL): A human-readable description of the project.
- `tags` (array of string, OPTIONAL): Tags for categorization. MAY be absent or an empty array, but MUST NOT be null.
- `spec` (object, OPTIONAL): Present if and only if this project is a specification. See [Entity: SpecProject](#entity-specproject).
- `impl` (object, OPTIONAL): Present if and only if this project is an implementation. See [Entity: ImplProject](#entity-implproject).

!entity-specmanproject.mutual-exclusion:

- Exactly one of `spec` or `impl` MUST be present.
- The other MUST be absent (not present as a key).
- Neither `spec` nor `impl` may be `null`.

## Entity: SpecProject

Represents a specification project within `specman.json`.

!entity-specproject.fields:

- `index` (string, OPTIONAL): The workspace-relative path to the main Markdown document for this specification. When present, the value MUST NOT be null and MUST reference an existing file; this is the document that SpecMan tooling scans for constraint groups, concepts, entities, and other specification content. When absent, tooling MUST NOT assume a default filename. For now, this MUST be the only document scanned for constraint groups.
- `references` (array of GitReference, OPTIONAL): A list of dependencies on other specifications. Each entry MUST be a [GitReference](#entity-gitreference). This field MAY be absent or an empty array, but MUST NOT be null.

## Entity: ImplProject

Represents an implementation project within `specman.json`.

!entity-implproject.fields:

- `implements` (array of ImplementsEntry, REQUIRED): The specifications this implementation targets. MUST contain at least one entry.
- `utilizes` (array of UtilizesEntry, OPTIONAL): Other implementations this implementation depends on. MAY be absent or an empty array, but MUST NOT be null.

## Entity: ImplementsEntry

Describes one specification being implemented.

!entity-implementsentry.fields:

- `ref` (GitReference, REQUIRED): The specification being implemented. The referenced artifact MUST be a specification.
- `constraints` (array of string, OPTIONAL): A list of regex patterns. Each constraint group identifier from the target specification is tested against the full dot-delimited constraint group identifier (e.g., `concept-slug.category`). If a constraint group matches at least one regex, it MUST be included in compliance checking. If no regex matches a constraint group, that constraint group MUST NOT be tracked by specman tooling. If this field is absent or an empty array, implementations MUST treat it as functionally equivalent to `[".*"]` (i.e., all constraint groups are included). This field MUST NOT be null.

## Entity: UtilizesEntry

Describes one implementation being utilized.

!entity-utilizesentry.fields:

- `ref` (GitReference, REQUIRED): The implementation being utilized. The referenced artifact MUST be an implementation.
- `ignore_constraints` (array of string, OPTIONAL): A list of regex patterns. When evaluating compliance, constraint groups from the utilized implementation that match any of these patterns MUST be filtered out and ignored. If the utilized implementation references the same specification as the current implementation but with a different git reference (determined by matching git host and path from `url`, not by name), implementations MUST warn the user during every explicit schema validation or upon pulling dependencies. This field MAY be absent or an empty array, but MUST NOT be null.

# Part 2: Core Behaviors

## Concept: Workspace Discovery

Workspace discovery ensures every SpecMan-aware tool can deterministically locate the active workspace root and its `.specman` directory from any starting location.

!concept-workspace-discovery.requirements:

- The implementation MUST identify the workspace root by scanning the current directory and its ancestors for the nearest `.specman` folder, treating the containing directory as canonical.
- The workspace root MUST also contain a `.git/` directory at the same level as `.specman/`. If `.specman/` is found but `.git/` is absent at the same level, the implementation MUST return a descriptive error.
- When no `.specman` folder exists along the ancestry chain, the implementation MUST return a descriptive error that callers MAY surface directly to users.
- Workspace discovery utilities MUST expose the absolute path to both the workspace root and the `.specman` directory so downstream services can reference shared metadata without recomputing filesystem state.
- Resolved workspace metadata MUST remain consistent with the data model rules for SpecMan workspaces (see [Concept: SpecMan Workspace](#concept-specman-workspace)) and MUST reuse existing data-model entities when emitting structured results.
- Implementations MAY cache the active workspace root for the lifetime of a command invocation, but they MUST revalidate that the `.specman` folder still exists before reusing cached paths.

!concept-workspace-discovery.initialization:

- The implementation MUST expose an initializer that accepts an absolute filesystem path provided by the caller and resolves it to the canonical workspace root and `.specman` directory using the same rules as workspace discovery.
- The initializer MUST accept both workspace-root paths and `.specman` directory paths as valid inputs; in either case it MUST return normalized absolute paths for both the workspace root and `.specman` directory without redundant ancestor search.
- The initializer MUST validate that the supplied path is (or contains) a `.specman` directory; if validation fails, it MUST either create `.specman` (when allowed by the invocation) or return a descriptive error suitable for direct user display, and it MUST NOT fall back to scanning unrelated ancestor paths.
- When creation is requested and a `.specman` directory is absent at the provided root, the initializer MUST create the `.specman` directory at that root, enforce workspace-boundary rules, and then return normalized paths; it MUST NOT create nested `.specman` directories beneath an existing workspace.
- The implementation MUST expose a library-level workspace creator that provisions `.specman` at an explicit path (including required subdirectories such as `ref/` and `cache/` when defined), performs the same validation as the initializer, and keeps the operation idempotent so future workspace-owned files can be added by the implementation rather than by ad-hoc folder creation.
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
- Implementations MUST expose a callable dependency-tree builder that accepts a path to a `specman.json` file, a git reference, or an alias that resolves through the global reference cache, and normalizes that locator before traversal begins.
- The tree builder MUST parse `specman.json` for `spec.references` (for specifications) or `impl.implements` and `impl.utilizes` (for implementations), recursively resolve each upstream artifact, and continue traversal until the graph is fully explored or a cycle is encountered.
- Resolvers MUST resolve git references by looking them up in the local project's `.specman/ref/` symlink directory first, then in the global cache at `$HOME/.specman/ref/`. If a reference is not found locally, the resolver MAY trigger a clone via [SpecMan Installing](#concept-specman-installing).
- Cycle detection MUST terminate traversal immediately and return a descriptive error that includes the partial tree gathered so far so callers can remediate invalid dependency graphs.
- When a referenced dependency or implementation lacks a `specman.json`, or when the dependency cannot be resolved, the tree builder MUST still add the artifact to the dependency set using the best available identifier and annotate the entry to indicate metadata was unavailable.

## Concept: Reference Validation

Reference validation ensures that references embedded in Markdown artifacts — particularly link destinations in inline links — can be validated deterministically against the workspace filesystem and external HTTPS resources. This enables tooling to detect broken links early, prevent invalid relationship graphs, and provide actionable diagnostics to authors.

!concept-reference-validation.requirements:

- The implementation MUST expose a callable reference-validation capability that accepts a locator to a Markdown artifact and returns structured validation results.
  - Artifact locators MUST use filesystem paths or HTTPS URLs.
- The validator MUST parse Markdown using CommonMark-compatible rules to identify links and their destinations, including:
  - inline links (`[text](destination)`),
  - full/collapsed/shortcut reference links resolved through link reference definitions, and
  - autolinks (`<https://example.com>`).
- The validator MUST NOT validate image destinations (`![alt](destination)`) as references.
- For every discovered link destination, the validator MUST classify the destination as one of:
  - workspace-filesystem (a filesystem path that resolves inside the active workspace),
  - HTTPS URL, or
  - unsupported/unknown.
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
- When creating specifications or implementations, the orchestrator MUST search for workspace-managed overrides under `.specman/templates/` in the following order: (1) artifact-specific Markdown files (for example `.specman/templates/spec.md` and `.specman/templates/impl.md` plus any nested directories the workspace defines), (2) uppercase pointer files (`SPEC` and `IMPL`) whose contents resolve to workspace-relative paths or HTTPS URLs, and (3) packaged defaults embedded with the SpecMan Core runtime. Packaged defaults MUST be versioned with the runtime, remain read-only, and MAY be delivered via resources compiled into the binary or co-located artifacts inside the packaged application.
- Implementations MUST expose pointer-file lifecycle helpers for every artifact profile so callers can add new `SPEC` or `IMPL` pointer files, update (set) their target locators, or remove them without editing the filesystem manually.
- Pointer update operations MUST persist uppercase pointer files under `.specman/templates/`, enforce the same locator validation rules defined for runtime resolution (workspace-bound filesystem paths and reachable HTTPS Markdown), and MUST refresh any `.specman/cache/templates/` entries referencing the affected locator before signaling success.
- Pointer removal operations MUST delete the targeted pointer file, purge cached remote content that referenced it, and MUST document the resulting fallback search order so clients know which template source will be used next. When the removal would leave the orchestration layer without any valid template source, the helper MUST fail with a descriptive error instead of leaving an invalid pointer state.
- Pointer-file lifecycle helpers MUST surface structured success and failure results — including validation errors or fallback descriptions — so CLI layers and APIs can relay operator-facing guidance without re-parsing filesystem state.
- Pointer files MUST be re-read on every invocation so workspace changes take effect without restarting tooling. Implementations MUST validate that filesystem locators remain inside the workspace root and that HTTPS locators are reachable plaintext Markdown before rendering.
- When a pointer file references an HTTPS resource, the fetched Markdown MUST be cached under `.specman/cache/templates/` using deterministic filenames (for example, hashing the URL). Cache entries MUST store the downloaded content verbatim together with the source locator and last-refresh metadata, and they MUST be reused for subsequent invocations until the pointer file content or remote resource changes.
- Template orchestration MUST refresh cached remote content whenever the pointer file changes or the remote server signals a new version (for example via `ETag` or `Last-Modified`). If refresh attempts fail, tooling MUST fall back to the last known-good cache entry before reverting to packaged defaults.
- Template rendering workflows MUST preserve HTML comment directives present in the source templates until each directive is satisfied. After fulfilling a directive, tooling MAY remove or replace the associated comment but MUST NOT drop unsatisfied instructions.
- Special-purpose template functions SHOULD exist for common scenarios such as creating specifications and implementations.
- Template metadata (required tokens, locator provenance, cache path) MAY be cached for the duration of a command invocation but MUST include the workspace root and template version in the cache key. Tooling MUST NOT reuse metadata caches across different workspaces unless both the template version and workspace identifier match.

!concept-template-orchestration.ai-instruction-directives:

- Template guidance for automated agents MUST be conveyed inside HTML comments (`<!-- ... -->`) that sit adjacent to the mutable region they govern and MUST NOT leak into rendered Markdown.
- Rendering engines MUST preserve HTML instruction comments until each directive is satisfied; if a directive cannot be satisfied, tooling MUST fail the operation rather than silently dropping the comment.

!concept-template-orchestration.token-contract:

- The effective template descriptor defines a closed token set; lifecycle or MCP clients MUST reject substitutions for tokens that are not declared by the descriptor.
- Token substitution covers Markdown body content only. Project metadata (`specman.json` for specs/impls) MUST be produced or mutated by lifecycle workflows after template rendering, not by injecting `{{token}}` placeholders.
- When callers supply token data, the implementation MUST surface validation errors verbatim whenever a required token is missing or an unknown token is supplied.

## Concept: Deterministic Execution

Deterministic execution codifies behavioral guarantees so downstream consumers can rely on predictable, side-effect-aware APIs.

!concept-deterministic-execution.requirements:

- Consumers MUST treat all SpecMan Core functions as pure unless the documentation explicitly calls out side effects; implementers MUST document any deviations before release.
- Breaking changes to function signatures or observable behaviors MUST trigger a major version increment of this specification so dependent tooling can coordinate adoption.

## Concept: Lifecycle Automation

Lifecycle automation standardizes creation and deletion workflows for specifications and implementations.

!concept-lifecycle-automation.requirements:

- Automated creation flows MUST require an associated template locator and MUST validate that required tokens are supplied.
- Lifecycle operations MUST enforce template usage for all new specifications and implementations so generated artifacts remain data-model compliant.
- Implementations MUST expose user-facing deletion workflows for specifications and implementations so that every artifact type can be removed with the same rigor applied to creation.
- Creation tooling MUST cover both artifact types (specifications and implementations) and MUST enforce the naming and metadata rules defined by Part 1 of this specification and the [founding specification](../../docs/founding-spec.md).
- Creation workflows MUST persist generated Markdown artifacts and supporting metadata into the canonical workspace locations (the path specified by `spec.index` for specifications and `impl.md` at the repository root for implementations) using the paths returned by workspace discovery.
- When a pointer file downloads content from an HTTPS locator, Lifecycle automation MUST route the rendered template through the `.specman/cache/templates/` store before writing artifacts so repeated invocations reuse the cached copy unless the pointer or upstream content changes.
- Persistence helpers MUST write the rendered template output (with all required tokens populated) together with its project metadata; persisting additional representations of entities, concepts, or other runtime data structures is out of scope for this specification.
- Lifecycle automation MUST provide direct integrations with the metadata mutation capabilities described in [Concept: Metadata Mutation](#concept-metadata-mutation).
- Deletion workflows MUST reuse dependency mapping services, refuse to proceed when dependent artifacts exist, and MUST return a dependency tree describing all impacted consumers whenever a removal is blocked.
- Deletion workflows MUST ensure the targeted artifact and any associated metadata are removed from their canonical workspace locations once safety checks pass.
- Lifecycle controllers MUST expose a persistence interface that can round-trip newly created artifacts back onto disk and SHOULD surface explicit errors if the filesystem write fails so callers can remediate workspace permissions.

!concept-lifecycle-automation.metadata-generation:

- Creation workflows MUST generate or merge a `specman.json` file after template rendering so that every artifact persists the metadata mandated by Part 1 and governing specifications.
- Templates MUST NOT embed `specman.json` content; lifecycle automation and metadata mutation workflows are the authoritative mechanisms for writing and updating metadata.
- Metadata mutation helpers MUST update the `specman.json` in-place and MUST continue to enforce the workspace-boundary and git-reference validation rules defined elsewhere in this specification.

## Concept: SpecMan Structure

Creating a structure which maps the SpecMan data model allows consumers to read markdown content when given identifiers for concepts, key entities, or constraints.

### Structure Indexing

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

Metadata mutation ensures `specman.json` for specifications and implementations can be updated without rewriting unrelated content.

!concept-metadata-mutation.requirements:

- Implementations MUST expose metadata mutation interfaces that accept a structured update object corresponding to the artifact type (specification or implementation).
- For specifications and implementations, the update object MUST operate on the `specman.json` file.
- The update object MUST strictly define the fields eligible for mutation on that artifact type; it MUST NOT allow arbitrary key-value insertion.
- Mutation operations MUST apply updates by merging the provided structure into the existing metadata (Partial Update semantics):
  - If a field is omitted from the update object, the existing value MUST remain unchanged.
  - Scalar fields (strings, numbers, booleans) present in the update object MUST replace the existing values.
- For list-valued fields, the update object MUST provide dedicated properties to add or remove items without requiring the caller to provide the full list:
  - The interface MUST support `add_{field}` properties to append unique items to the list.
  - The interface MUST support `remove_{field}` properties to remove items from the list.
  - The interface MAY support the base field name to perform a full replacement (set) of the list.
- Implementations MUST NOT require callers to construct a list of abstract "operation" commands (e.g., `{"op": "add", ...}`). Instead, the API surface MUST be a strongly-typed or schema-validated structure.
- Metadata mutation helpers MUST reuse git-reference resolution and workspace-boundary enforcement rules before applying edits.
- Metadata mutation operations MUST persist the updated `specman.json` and MUST return the full updated document.

!concept-metadata-mutation.scope.supported-fields:

- Metadata mutation MUST be supported for specification and implementation artifacts.
- For specifications, metadata mutation MUST support updating `name`, `description`, `tags`, and adding/removing entries in the `spec.references` list.
- For implementations, metadata mutation MUST support updating `name`, `description`, `tags`, and adding/removing entries in the `impl.implements` and `impl.utilizes` lists.

## Concept: Validation Anchors

Validation anchors provide a mechanism to enforce and verify that an implementation adheres to the requirements defined in its backing specification.

!concept-validation-anchors.definition:

- A validation anchor MUST be a text marker embedded within the source code or related files of an implementation.
- Validation anchors MUST reference a specific constraint group defined in the specification.
- Implementations SHOULD use validation anchors to explicitly demonstrate compliance with specification constraints.

## Concept: Validation Scanning

Validation scanning defines how tooling discovers anchors within an implementation.

!concept-validation-scanning.scope:

- Tooling MUST resolve the implementation's source code root as the workspace root (the directory containing `.specman/` and `specman.json`).
- The scanner MUST recursively traverse the source directory to identify files for analysis.

!concept-validation-scanning.filtering:

- The scanner MUST respect standard `.gitignore` rules found in the implementation's source tree; ignored files MUST NOT be scanned.
- The scanner MUST only analyze files containing text content (e.g., encoded in UTF-8 or ASCII).
- The scanner MUST skip binary files.

## Concept: Compliance Reporting

Compliance reporting exposes the relationship between specification constraints and implementation anchors.

!concept-compliance-reporting.interface:

- Implementations MUST provide an interface or surface to generate compliance reports.
- The reporting tool MUST resolve the target specification(s) from the implementation's `specman.json` `impl.implements` array.
- The tool MUST extract all constraint groups from the resolved specification(s) only. Constraint groups from transitive specification dependencies MUST NOT be included in compliance evaluation.
- The tool MUST scan the implementation's source location (the workspace root) for validation tags.
- The reporting tool MUST scope structural indexing to the implementation, its governing specification(s), and those specification dependencies; unrelated artifacts MUST be ignored and MUST NOT cause compliance report failures.

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

- The implementation MUST expose a workspace status capability that scans specifications and implementations within the active workspace.
- The status check MUST accept a configuration (e.g., flags or options) to enable or disable specific validation categories. Supported categories MUST include at least:
  - `structure`: Validates `specman.json` and basic artifact validity.
  - `references`: Validates inline links, dependencies, and external URLs.
  - `cycles`: Validates the dependency graph for cycles.
  - `compliance`: Validates implementation anchor coverage.
- Implementations SHOULD enable all validation categories by default unless explicitly disabled by the user.
- When `structure` validation is enabled, the status check MUST validate that every scanned artifact has a valid `specman.json` conforming to this specification.
- When `references` validation is enabled, the status check MUST perform reference validation on all artifacts, ensuring that:
  - All inline links to workspace files resolve to existing files.
  - All inline links to HTTP(S) resources are valid URLs (connectivity checks MAY be optional or configurable).
  - All artifact dependencies (specification `references`) and implementation `implements`/`utilizes` resolve to existing, valid artifacts.
- When `cycles` validation is enabled, the status check MUST construct the full dependency graph and verify that no cyclic dependencies exist between specifications.
- When `compliance` validation is enabled, the status check MUST verify compliance coverage for every implementation:
  - It MUST extract all constraint groups from the implementation's governing specification(s) only; constraint groups from transitive specification dependencies MUST NOT be included.
  - It MUST scan the implementation's source code for validation anchors.
  - It MUST report a failure if any mandatory constraint group lacks a corresponding validation anchor.
- The status check MUST return an aggregated report containing:
  - A primary section for Specifications and Implementations, determining the global pass/fail status.
  - A comprehensive list of all validation errors, warnings, and missing compliance anchors, grouped by artifact.
- The status check SHOULD NOT halt on the first error; it MUST attempt to collect all discoverable errors in the workspace.

## Concept: SpecMan Installing

SpecMan Installing is the process of resolving all git-referenced dependencies for a project and making them available locally. It delegates all git operations to the user's `git` CLI, piggybacking on the user's existing authentication and configuration.

!concept-specman-installing.git-delegation:

- Implementations MUST use the user's `git` command for all remote operations (clone, fetch, ls-remote).
- Implementations MUST NOT implement their own git transport or authentication.
- Implementations MUST respect the user's existing git configuration (credentials, SSH keys, `.gitconfig` settings).

!concept-specman-installing.recursive-resolution:

- Installing MUST recursively resolve all transitively referenced artifacts.
- The recursion depth MUST NOT exceed 10 levels.
- If recursion exceeds 10 levels, the install MUST fail with a descriptive error listing the dependency chain that caused the overflow.

!concept-specman-installing.symlink-structure:

- After resolving references, implementations MUST create symlinks under the project's `.specman/ref/` directory.
- Each symlink name MUST be the `alias` from the corresponding [GitReference](#entity-gitreference).
- Each symlink MUST point to the corresponding directory in the global reference cache (`$HOME/.specman/ref/{ref_name}`).
- If a symlink with the same alias already exists, implementations MUST update it to point to the resolved reference.

## Concept: Global Reference Cache

SpecMan maintains a global cache of cloned references under the user's home directory to avoid redundant clones across projects.

!concept-global-reference-cache.location:

- The global cache root MUST be `$HOME/.specman/ref/`.
- Each cloned artifact MUST be stored under `$HOME/.specman/ref/{ref_name}/`.

!concept-global-reference-cache.naming:

- The reference directory name (`ref_name`) MUST be computed as: `{host}_{path}_{hash}`, where:
  - `host` is the domain of the git server (e.g., `github.com`).
  - `path` is the repository path with the `.git` suffix stripped (e.g., `owner/repo`).
  - `hash` is the full git commit hash of the resolved reference.
- All three components MUST be joined with underscore (`_`) separators.
- Example: for `https://github.com/owner/spec-repo.git` at commit `abc123def456...`, the ref_name is `github.com_owner/spec-repo_abc123def456...`.

!concept-global-reference-cache.clone-rules:

- Before cloning, implementations MUST resolve any tag reference to a commit hash (see [Entity: GitReference](#entity-gitreference)).
- Implementations MUST clone the repository into the global cache using `git clone`.
- After cloning, implementations MUST checkout the exact commit hash.
- Each unique `(host, path, hash)` tuple MUST produce exactly one cache directory. Implementations MUST check for an existing directory before cloning and reuse it if present.

!concept-global-reference-cache.cleanup:

- The global cache MAY accumulate unused entries over time.
- Implementations MAY provide a cleanup command that removes cache entries not referenced by any known project.
- Implementations MUST NOT automatically delete cache entries without explicit user action.


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

- MUST record the locator URI or absolute path and the intended template scenario (specification or implementation).
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

Controller responsible for enforcing lifecycle policies across specifications and implementations.

!entity-lifecyclecontroller.requirements:

- MUST orchestrate create and delete operations for every artifact type, delegating to dependency mapping and templating subsystems.
- MUST terminate deletion attempts that would orphan dependents and MUST return the blocking dependency tree to the caller.
- MUST expose deletion entry points that mirror creation workflows so operators have symmetrical controls for specifications and implementations.
- SHOULD integrate auditing hooks that capture lifecycle events for compliance tracking.
- MUST surface explicit errors when filesystem persistence fails (for example, permissions or missing directories) so callers can remediate issues without corrupting the workspace.

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

- `structure` (boolean, required): Enable structure validation (`specman.json`). Default: `true`.
- `references` (boolean, required): Enable reference validation. Default: `true`.
- `cycles` (boolean, required): Enable dependency cycle detection. Default: `true`.
- `compliance` (boolean, required): Enable compliance checks (anchors). Default: `true`.

## Entity: WorkspaceStatusReport

The aggregated result of a workspace status check.

!entity-workspacestatusreport.schema:

- `global_status` (string, enum: `Pass` | `Fail`): Overall workspace status.
- `spec_impl_status` (string, enum: `Pass` | `Fail`): Status for specification and implementation artifacts.
- `artifacts` (map<ArtifactId, ArtifactStatus>): Detailed status per artifact.
- `cycle_errors` (array<string>): Global errors related to dependency cycles.
- `structure_errors` (array<string>): Global errors related to workspace structure.
- `artifact_count` (integer): Total number of artifacts processed.

## Entity: ArtifactStatus

Validation results for a single artifact.

!entity-artifactstatus.schema:

- `structure_errors` (array<string>): Errors related to `specman.json`.
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
