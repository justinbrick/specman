---
version: 1.0.0
dependencies:
  - ../../docs/founding-spec.md
---

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

This specification defines the data model for the topics and entities discussed in the [founding specification](../../docs/founding-spec.md) and should be read alongside that document for background and rationale.

## SpecMan Workspace

A SpecMan workspace is the directory in which SpecMan tooling can be used.

### SpecMan Dot Folder

The SpecMan dot folder MUST be named `.specman` and is used to store tooling state, metadata, and other implementation-specific files that belong to the workspace. The presence of a top-level `.specman` directory is the canonical indicator that a directory is a SpecMan workspace root.

Implementations SHOULD treat the nearest ancestor directory containing a `.specman` folder as the workspace root when tools are invoked from within a subdirectory. Tools MAY search parent directories for a `.specman` folder; when multiple `.specman` folders are found along the ancestry chain, the nearest one to the current working directory SHOULD be selected as the active workspace root.

## Scratch Pads

Scratch pads are working documents that track in-progress efforts for SpecMan-aware tooling. Implementations MUST store scratch pads under `.specman/scratch/`. 

Each scratch pad MUST reside in its own subdirectory whose name is all lowercase, uses hyphen separators, contains no more than four words, and MAY include verbs. This will act as the **scratch pad name**.

The primary scratch pad document inside each subdirectory MUST be named `scratch.md`. Scratch pads MAY be deleted when they are no longer being used.

### Target Artifact

A scratch pad MUST have a target artifact in the form of either a specification or an implementation associated with it. This artifact MUST be a relative file path, or a URL if the artifact is external.
These are mutually exclusive, as if an implementation is referenced, then its underlying specification can be implicitly retrieved.

### Work Type

A scratch pad MUST specify its work type, which specifies what kind of actions are being taken. A scratch pad SHOULD only have one work type.
A work type can be one of the following:

- `revision`: a change to the specification
  - If used, the target artifact MUST be a specification. The specification MUST NOT be an external reference.
  - Implies potential refactoring required for all referencing implementations.
  - One or more extra scratch pads MAY be created as a result of a revision.
- `feat`: an introduction of a feature
  - The target artifact MUST be an implementation.
  - SHOULD be used to introduce new functionality to an implementation.
- `ref`: a refactor of an implementation
  - The target artifact MUST be an implementation.
  - Implies potential refactoring required for downstream implementations.


### Git Branches

Scratch pads SHOULD have a Git branch associated with them. A branch MAY be excluded if a Git repository is not present in the SpecMan workspace.

Git branches MUST follow a naming scheme of:
```
{specification_name}/{work_type}/{scratch_pad_name}
```

The meaning of these labels are defined below.

- `specification_name`: the name of the specification 
- `work_type`: the scratch pad work type
- `scratch_pad_name`: the name of the scratch pad

### Scratch Pad Metadata

Scratch pads MUST have front matter metadata to represent the above data.
Frontmatter fields MUST be formatted as below.

- `target`: the target artifact
- `branch`: the git branch

## [Specifications](../../docs/founding-spec.md#specifications)

Specifications MUST be written in Markdown. Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.


### Specification Headings

Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title or any YAML frontmatter).

That heading SHOULD include a reference to RFC 2119 and a short statement indicating how the RFC 2119 normative keywords (for example, MUST, SHOULD, MAY, etc.) are to be interpreted for that document.

Other statements or notes SHOULD be added to this heading regarding referenced documents, but MAY be omitted or relocated under other headings as necessary.

### Specification Layout

Each specification MUST be stored in a folder named after the specification's short name.

- Specification folders MUST be stored in a top level directory named `spec`.
- Specification folders MUST NOT be nested inside other specification folders.
- The base specification document must be located in that folder, under `spec.md`.

Example:

- [workspace](#specman-workspace)/
  - spec/
    - {spec_name}/
      - spec.md

### Standalone Specifications

> ![NOTE] Standalone specifications are experimental, and may not be added to the non-draft version.

A specification MAY NOT require a reference to an implementation to be used. For example, when a specification defines usage in a common format that can be used without requiring explicit implementation details (e.g. CLI commands) 

When a specification does not require an implementation, this SHOULD be recorded in the spec's top-of-file YAML frontmatter using a boolean field named `requires_implementation`. If `requires_implementation` is omitted, implementations and tooling MUST treat the value as `true` by default.

### [Dependencies](../../docs/founding-spec.md#dependencies)

- Dependencies MUST be either another specification or an external resource that contains documentation detailing a specification.
  - If the dependency is an external resource, it MUST be available in a plaintext format, in such a way that it could be read through a code editor.
  - Tooling MAY omit processing external dependencies outside of presenting the content if they are not formatted in markdown.
- Each dependency item MUST be represented as one of the following forms:
  - A string: a local file path or a URL to another specification document.
  - An object with two fields:
    - `ref` (string): a local file path or a URL pointing to the dependency.
    - `optional` (boolean): when true, indicates this dependency is optional.

If a concept or key entity is referenced from one of the dependencies, it SHOULD be marked with an [inline link](https://spec.commonmark.org/0.31.2/#inline-link).


### Specification Metadata

Specifications should have front-matter at the beginning of the document to declare the above data.
The frontmatter fields MUST be formatted as listed below.

- `version`: the [specification version](../../docs/founding-spec.md#specification-version)
- `dependencies`: a list of [`dependency`](#dependencies)

Example:

```yaml
---
version: "1.0.0"
dependencies:
  - ../other-spec.md
  - https://example.com/specs/founding-spec.md
  - ref: ../maybe-optional.md
    optional: true
---
```

## [Implementations](../../docs/founding-spec.md#implementation)

Implementations MUST be authored as Markdown documents to support consistent rendering, review, and automated processing.
Implementation documents MUST declare their frontmatter fields, including a `spec` field referencing the implemented specification as either a relative file path string or a URL.

### Implementation Layout

Implementation documents MUST be stored in folders.

- Implementation folders MUST be stored in a parent folder named `impl`.
- The root implementation folder MUST be inside of a SpecMan workspace.
- The base implementation document MUST be stored under `impl.md`.
- Related documents MAY be stored inside of the implementation folder.
  - Related documents MUST be human-readable files, with no binary representation. (e.g. markdown, json, yml)

Example:
- [workspace](#specman-workspace)/
  - impl/
    - {impl_name}/
      - impl.md

### [Implementing Language](../../docs/founding-spec.md#implementing-language)

Each implementing language MUST be formatted as an object.

These objects MUST adhere to the listed fields below.

- `language`: a string in the format of `language_identifier@language_version`
- `properties`: a map of values to specify language-specific properties.
  - this field MAY be omitted if defaults can be assumed or the language has no configurable properties
- `libraries`: a list of strings to identify [used libraries](../../docs/founding-spec.md#libraries)
  - this field MAY be omitted if no libraries outside of the language-specific standard library are being used.

### [References](../../docs/founding-spec.md#references)

Implementations MAY reference capturing external artifacts relied upon by the implementation. This is functionally equivalent to [specification dependencies](#dependencies), but MUST be expressed exclusively as a list of objects. 

These objects MUST adhere to the listed fields below.

- `ref`: local path or URL to target artifact
- `type`: the type of artifact. MUST be one of ("implementation", "specification").
- `optional`: a boolean value indicating whether this reference is optional.


### [APIs](../../docs/founding-spec.md#apis)

Implementations MUST group API stubs by concept or key entity.


### Implementation Metadata

Implementations MUST specify YAML frontmatter at the top of the document.
The frontmatter fields MUST be formatted as listed below.

- `spec`: a local path or URL to the target specification
- `primary_language`: the primary [`language`](#implementing-language)
- `secondary_languages`: a list of [`language`](#implementing-language)
  - this field MAY be omitted if no secondary languages are present.
