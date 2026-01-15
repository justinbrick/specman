---
version: 1.1.0
dependencies:
  - ../../docs/founding-spec.md
  - https://spec.commonmark.org/0.31.2/
---

# SpecMan Data Model

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

This specification defines the data model for the topics and entities discussed in the [founding specification](../../docs/founding-spec.md) and should be read alongside that document for background and rationale.

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

- `draft`: create an initial specification
  - The target artifact MUST be a specification. The specification MUST NOT be an external reference.
- `revision`: a change to the specification
  - The target artifact MUST be a specification. The specification MUST NOT be an external reference.
  - Implies potential refactoring required for all referencing implementations.
  - One or more extra scratch pads MAY be created as a result of a revision.
  - The object representation of this work type MUST follow this form:
    - `revised_headings`: a list of headings that have been revised.
      - each revised heading MUST be represented as a markdown fragment that exists within the specification
- `feat`: an introduction of a feature
  - The target artifact MUST be an implementation.
  - SHOULD be used to introduce new functionality via implementations.
- `ref`: a refactor of an implementation
  - The target artifact MUST be an implementation.
  - Implies potential refactoring required for downstream implementations.
  - The object representation of this work type MUST follow this form:
    - `refactored_headings`: a list of headings that have been refactored.
      - each refactored heading MUST be represented as a markdown fragment that exists within the specification
- `fix`: a correction applied to an implementation to address defects without modifying specifications
  - The target artifact MUST be an implementation and MUST NOT be a specification or external reference.
  - SHOULD be used when the implementation needs remediation (bug fixes, defects) independent of specification updates.
  - The object representation of this work type MUST follow this form:
    - `fixed_headings`: a list of headings for concepts or entities impacted by the fix.
      - each fixed heading MUST be represented as a markdown fragment that exists within the implementation's referenced specifications.

### Git Branches

!concept-scratch-pads.git-branches:

- Scratch pads SHOULD have a Git branch associated with them.
- A branch MAY be excluded if a Git repository is not present in the SpecMan workspace.
- Git branches MUST follow a naming scheme of: `{target_name}/{work_type}/{scratch_pad_name}`

The meaning of these labels are defined below.

- `target_name`: the name of the target artifact
- `work_type`: the scratch pad [work type](#work-type)
- `scratch_pad_name`: the name of the scratch pad

### Scratch Pad Metadata

!concept-scratch-pads.metadata:

- Scratch pads MUST have front matter metadata to represent the above data.
- Frontmatter fields MUST be formatted as below.

- `target`: the target artifact
- `branch`: the git branch
  - this field MAY be omitted if there is no Git workspace.
- `work_type`: the object representing the work type
  - `draft|revision|feat|ref|fix`: a field on the object representing the work type.
- `dependencies`: a list of [dependencies](#scratch-pad-dependencies).
  - this field MAY be omitted if this scratch pad does not depend on other scratch pads.

### Dependency Graph Integrity

!concept-scratch-pads.dependency-graph-integrity.requirements:

- The combined dependency graph spanning specifications, implementations, and scratch pads MUST remain acyclic.
- Tooling SHOULD validate the dependency graph whenever artifacts are added or updated, and MUST reject or flag any change that would introduce a cycle.
- Authors SHOULD restructure work or adjust dependencies to remove cycles before publishing updates.

## Concept: [Specifications](../../docs/founding-spec.md#specifications)

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

### Specification [Concepts](../../docs/founding-spec.md#concepts) and [Entities](../../docs/founding-spec.md#key-entities)

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

### [Dependencies](../../docs/founding-spec.md#dependencies)

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
- `dependencies`: a list of [`dependency`](#dependencies)

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

## Concept: [Implementations](../../docs/founding-spec.md#implementation)

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

- The `location` front-matter field MUST point to the root folder of the implementation’s code. It MAY be a workspace-relative path or a URL, and MUST remain inside the detected workspace when a workspace exists.
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

### [References](../../docs/founding-spec.md#references)

!concept-implementations.references.model:

- Implementations MAY reference external artifacts relied upon by the implementation. This is functionally equivalent to [specification dependencies](#dependencies), but MUST be expressed exclusively as a list of objects.
- These objects MUST adhere to the listed fields below.

- `ref`: local path or URL to target artifact
- `type`: the type of artifact. MUST be one of ("implementation", "specification").
- `optional`: a boolean value indicating whether this reference is optional.

### [APIs](../../docs/founding-spec.md#apis)

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
