---
version: 1.0.0
dependencies:
  - ../../docs/founding-spec.md
  - https://spec.commonmark.org/0.31.2/
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

Scratch pads are working documents that track in-progress efforts for SpecMan-aware tooling. 

- Each scratch pad MUST reside in its own subdirectory whose name is all lowercase, uses hyphen separators, contains no more than four words, and MAY include verbs.
  - This will act as the **scratch pad name**.
- Scratch pads MAY be deleted when they are no longer being used.

### Scratch Pad Location

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

A scratch pad MUST have a target artifact associated with it.
- The artifact MUST be either a specification or dependency. 
  - These are mutually exclusive, as if an implementation is referenced, then its underlying specification can be implicitly retrieved.
- This artifact MUST be a relative file path, or a URL if the artifact is external.


### Scratch Pad Content

There MUST be specific content included inside of a scratch pad, for readability sake.
- A scratch pad MUST contain a notes section.
  - This is to allow for any AI to resume from little to no context.
- A scratch pad SHOULD have a tasks file.
  - The tasks file will serve as a list of tasks to be completed before the the scratch pad may be considered completed.
  - If present, the tasks file MUST be located under the directory containing the `scratch.md` file, and MUST be labelled `tasks.md`.

### Work Type

A scratch pad MUST specify its work type, which specifies what kind of actions are being taken. 
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



### Git Branches

Scratch pads SHOULD have a Git branch associated with them. A branch MAY be excluded if a Git repository is not present in the SpecMan workspace.

Git branches MUST follow a naming scheme of:

```
{target_name}/{work_type}/{scratch_pad_name}
```

The meaning of these labels are defined below.

- `target_name`: the name of the target artifact
- `work_type`: the scratch pad [work type](#work-type)
- `scratch_pad_name`: the name of the scratch pad

### Scratch Pad Metadata

Scratch pads MUST have front matter metadata to represent the above data.
Frontmatter fields MUST be formatted as below.

- `target`: the target artifact
- `branch`: the git branch
  - this field MAY be omitted if there is no Git workspace.
- `work_type`: the object representing the work type
  - `draft|revision|feat|ref`: a field on the object representing the work type.

## [Specifications](../../docs/founding-spec.md#specifications)

Specifications MUST be written in Markdown. Compliant specifications and contributors SHOULD author and publish specification documents using the Markdown format so they can be rendered, reviewed, and processed consistently by tooling.

### Specification Headings

Each specification MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).
- Each heading within a specification MUST be unique to the implementation itself. 
- Specifications SHOULD include a top-level heading titled "Terminology & References" placed near the top of the file (immediately below the main title or any YAML frontmatter).
  - This heading SHOULD include a reference to RFC 2119 and a short statement indicating how the RFC 2119 normative keywords (for example, MUST, SHOULD, MAY, etc.) are to be interpreted for that document.
  - Other statements or notes SHOULD be added to this heading regarding referenced documents, but MAY be omitted or relocated under other headings as necessary.

### Specification [Concepts](../../docs/founding-spec.md#concepts) and [Entities](../../docs/founding-spec.md#key-entities)

- Each concept or key entity SHOULD have its own [heading](#specification-headings).

### Specification Layout

Each specification MUST be stored in a folder designated specifically for that specification.

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

## [Implementations](../../docs/founding-spec.md#implementation)

- Implementations MUST be authored as Markdown documents to support consistent rendering, review, and automated processing.
- Implementations MUST contain human-readable content.

### Implementation Headings

Each implementation MUST categorize their content into [headings](https://spec.commonmark.org/0.31.2/#atx-headings).
- A heading SHOULD be a link if it is a direct reference to a specification concept or key entity.
- If multiple concepts or key entities are related, they SHOULD be linked directly under the heading in an unordered list that provides inline links to the concepts / entities.

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
  - each library in this list MAY be formed as an object if additional metadata is required.
    - if it is an object, the library reference MUST be stored under the field `name`.

### Implementation Locators

Implementation details are usually associated with code, or optionally as a library. As a result, details are required to properly reference implementations within other implementations if needed.
- Implementation code MAY be referenced as a path, relative to the [Specman workspace](#specman-workspace).
  - The code location MUST lead to a folder.
- If the implementation has a library, it SHOULD be listed in the implementation.

### [References](../../docs/founding-spec.md#references)

Implementations MAY reference external artifacts relied upon by the implementation. This is functionally equivalent to [specification dependencies](#dependencies), but MUST be expressed exclusively as a list of objects. 

These objects MUST adhere to the listed fields below.

- `ref`: local path or URL to target artifact
- `type`: the type of artifact. MUST be one of ("implementation", "specification").
- `optional`: a boolean value indicating whether this reference is optional.


### [APIs](../../docs/founding-spec.md#apis)

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

Implementations MUST specify YAML frontmatter at the top of the document.
The frontmatter fields MUST be formatted as listed below.

- `spec`: a local path or URL to the target specification
- `name`: the [implementation name](../../docs/founding-spec.md#implementation-name)
  - if this field is omitted, processors MUST use the parent directory as the implementation name.
- `location`: the location of the source code as defined in [implementation locators](#implementation-locators)
  - this field MAY be omitted if the implementation does not have an available source.
- `library`: the name of the library defined by this implementation, if one is available.
  - this MUST take the shape of a string or object, as defined by the `libraries` field in [the implementing language model](#implementing-language)
  - this field MAY be omitted if there is no library available.
- `primary_language`: the primary [`language`](#implementing-language)
- `secondary_languages`: a list of [`language`](#implementing-language)
  - this field MAY be omitted if no secondary languages are present.

Example:

```yaml
---
spec: ../path/to/spec.md
name: implementation-name
version: "1.0.0"
location: ../path/to/code
library:
  name: implementation-library@1.0.0
  extra_data: 5
primary_language:
  language: lang
  properties:
    lang-property: a
  libraries:
    - name: library@1.0.0
      extra_data: 5
---
```