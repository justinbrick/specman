---
dependencies:
  - ../../docs/founding-spec.md
---

## Terminology & References

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in RFC 2119.

This specification defines the data model for the topics and entities discussed in the [founding specification](../../docs/founding-spec.md) and should be read alongside that document for background and rationale.

## SpecMan Workspace

A SpecMan workspace is the directory or folder that contains the SpecMan dot folder.

The SpecMan dot folder MUST be named `.specman` and is used to store tooling state, metadata, and other implementation-specific files that belong to the workspace. The presence of a top-level `.specman` directory is the canonical indicator that a directory is a SpecMan workspace root.

Implementations SHOULD treat the nearest ancestor directory containing a `.specman` folder as the workspace root when tools are invoked from within a subdirectory. Tools MAY search parent directories for a `.specman` folder; when multiple `.specman` folders are found along the ancestry chain, the nearest one to the current working directory SHOULD be selected as the active workspace root.

The `.specman` folder's exact contents and layout are implementation-defined, but SHOULD be documented by the implementation and kept small and machine-readable to support discovery, migration, and tooling interoperability.

### Scratch Pads

Scratch pads are working documents that track in-progress efforts for SpecMan-aware tooling. Implementations MUST store scratch pads under `.specman/scratch/`. Each scratch pad MUST reside in its own subdirectory whose name is all lowercase, uses hyphen separators, contains no more than four words, and MAY include verbs. The primary scratch pad document inside each subdirectory MUST be named `scratch.md`. Implementations SHOULD delete scratch pad directories once they are no longer needed.

### Workspace Layout

A SpecMan workspace SHOULD contain the following top-level folders:

- `spec`: contains all specifications.
- `impl`: contains all implementations.


## Specifications
Specifications MUST be written in Markdown. Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

A specification MAY not require an implementation. When a specification does not require an implementation this SHOULD be recorded in the spec's top-of-file YAML frontmatter using a boolean field named `requires_implementation`. If `requires_implementation` is omitted, implementations and tooling MUST treat the value as `true` by default.

Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title or any YAML frontmatter). That heading SHOULD include a reference to RFC 2119 and a short statement indicating how the RFC 2119 normative keywords (for example, MUST, SHOULD, MAY, etc.) are to be interpreted for that document.

Each specification MUST be stored in a folder named after the specification's short name. The specification document MUST be located in that folder at a path of `spec.md`.

Specification folders MUST NOT be nested inside other specification folders.

### Dependencies

Dependencies SHOULD be declared in a top-of-file YAML frontmatter header named `dependencies` so tools can discover and resolve them before parsing the Markdown body.

Each dependency item in the `dependencies` header MAY be one of the following forms:

- A string: a local file path or a URL to another specification document.
- An object with two fields:
  - `ref` (string): a local file path or a URL pointing to the dependency.
  - `optional` (boolean): when true, indicates this dependency is optional.

The header may therefore contain a simple list mixing string entries and object entries. Example forms:

```yaml
---
dependencies:
  - ../other-spec.md
  - https://example.com/specs/founding-spec.md
  - ref: ../maybe-optional.md
    optional: true
---
```

Processors SHOULD accept both a single string and an object form and SHOULD treat unspecified `optional` as `false` by default.

## Implementation

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
