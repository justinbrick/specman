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

Scratch pads SHOULD have a Git branch associated with them. A branch MAY be excluded if a Git repository is not present in the same directory as the SpecMan workspace.

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


### Workspace Layout

A SpecMan workspace SHOULD contain the following top-level folders:

- `spec`: contains all specifications.
- `impl`: contains all implementations.


## [Specifications](../../docs/founding-spec.md#specifications)

Specifications MUST be written in Markdown. Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title or any YAML frontmatter). That heading SHOULD include a reference to RFC 2119 and a short statement indicating how the RFC 2119 normative keywords (for example, MUST, SHOULD, MAY, etc.) are to be interpreted for that document.

Each specification MUST be stored in a folder named after the specification's short name. The specification document MUST be located in that folder at a path of `spec.md`.

Specification folders MUST NOT be nested inside other specification folders.

### Standalone Specifications

> [!NOTE] Standalone specifications are experimental, and may not be added to the non-draft version.

A specification MAY NOT require a reference to an implementation to be used. For example, when a specification defines usage in a common format that can be used without requiring explicit implementation details (e.g. CLI commands) 

When a specification does not require an implementation, this SHOULD be recorded in the spec's top-of-file YAML frontmatter using a boolean field named `requires_implementation`. If `requires_implementation` is omitted, implementations and tooling MUST treat the value as `true` by default.

### Dependencies

Each [dependency](../../docs/founding-spec.md#dependencies) item MUST be represented as one of the following forms:

- A string: a local file path or a URL to another specification document.
- An object with two fields:
  - `ref` (string): a local file path or a URL pointing to the dependency.
  - `optional` (boolean): when true, indicates this dependency is optional.

Processors SHOULD accept both a single string and an object form and SHOULD treat unspecified `optional` as `false` by default.

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

Implementations SHOULD be authored as Markdown documents to support consistent rendering, review, and automated processing.
Implementation documents MUST declare their frontmatter fields, including a `spec` field referencing the implemented specification as either a relative file path string or a URL.

### Implementation Layout

Implementations MUST reside in the workspace root `impl` directory. Each implementation MUST be stored in its own subdirectory within `impl`, and the implementation document inside that folder MUST be named `spec.md`.

Implementation folders MUST NOT be nested inside other implementation folders; each implementation must live directly under `impl` in its own sibling directory.

### Implementing Language

Implementation documents MUST declare their implementing languages by using YAML frontmatter fields. When additional languages are used, the frontmatter MAY include a `secondary_languages` field listing them; this field MAY be omitted when no secondary languages are present.

### References

Implementation frontmatter MAY declare a `references` field capturing external artifacts relied upon by the implementation. This field is functionally equivalent to [specification dependencies](#dependencies) but MUST be expressed exclusively as a list of objects. Each object MUST include a `ref` string identifying the target and a `type` string whose value MUST be either `implementation` or `specification`. Scalar entries MUST NOT appear in the `references` list.

### Libraries

Implementation documents SHOULD declare library metadata in their YAML frontmatter using a `libraries` field. The `libraries` field MUST list acceptable version-formatted strings as defined in the founding specification.

### APIs

### Implementation Metadata

Implementations MUST specify YAML frontmatter at the top of the document.
