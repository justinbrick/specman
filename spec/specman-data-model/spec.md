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

### Workspace Layout

A SpecMan workspace SHOULD contain the following top-level folders:

- `spec`: contains all specifications.
- `impl`: contains all implementations.


## Specifications
Specifications MUST be written in Markdown. Implementations and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

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

### Implementation Name

### Implementing Language

### References

#### Libraries

#### Data Models

#### APIs
