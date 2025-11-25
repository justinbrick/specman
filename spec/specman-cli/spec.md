---
name: specman-cli
version: "1.0.0"
dependencies:
  - ../specman-core/spec.md
  - ../specman-data-model/spec.md
  - ../specman-templates/spec.md
---

# Specification — SpecMan CLI

The SpecMan CLI defines a command-line binary that orchestrates SpecMan Core capabilities through declarative commands. It standardizes how operators trigger workspace discovery, artifact creation, and safe deletions while remaining agnostic to distribution or PATH management concerns.

## Terminology & References

This document uses the normative keywords defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119). Consumers SHOULD review `specman-core`, `specman-data-model`, and `specman-templates` to understand the lifecycle rules, data entities, and template contracts enforced by this CLI.

## Concepts

### Concept: CLI Invocation Model

- The CLI MUST be executable as a standalone binary; installation, PATH configuration, or shell-integration steps are explicitly out of scope for this specification.
- Every command MUST provide structured stdout/stderr suitable for automation, and SHOULD exit with non-zero codes on validation failures so scripts can detect errors deterministically.
- Commands MUST accept positional arguments and flags that can be scripted without interactive prompts; optional interactive flows MAY exist but MUST have equivalent flag-driven variants.
- The CLI MUST emit human-readable help text describing each command, argument, and related specification reference. A `--help` flag MUST be available on the root command plus every subcommand and subcommand group, and invoking it at any level MUST list the commands available at that scope while the formatting of the help output remains out of scope for this specification.
- Exit statuses MUST map to the POSIX constants defined in [`sysexits.h`](https://man7.org/linux/man-pages/man3/sysexits.h.3head.html); successful executions MUST use `EX_OK`, and failure scenarios MUST choose the closest matching constant (for example `EX_DATAERR` for validation failures) so automation can rely on consistent semantics across commands.

### Concept: Workspace Context Resolution

- On startup, the CLI MUST discover the active SpecMan workspace by scanning the current working directory and its ancestors for the nearest `.specman` folder, mirroring the `Workspace Discovery` concept defined by `specman-core`.
- Callers MAY provide an explicit `--workspace` flag (or environment variable) to override the search path; the CLI MUST validate that the supplied path contains a `.specman` directory and MUST fall back to nearest-ancestor detection when the override is absent or invalid.
- Workspace resolution MUST surface both the workspace root and the `.specman` directory paths to downstream subsystems without recomputing filesystem state per command.
- If no `.specman` folder is found, the CLI MUST fail fast with an actionable error message that includes the search path that was attempted.

### Concept: Lifecycle Command Surface

- The CLI MUST expose create commands for specifications, implementations, and scratch pads, each of which MUST enforce the naming rules defined in the `specman-data-model` and founding specifications.
- Creation commands MUST invoke the dependency mapping and template orchestration behaviors defined by `specman-core`, ensuring that generated artifacts include compliant front matter and section scaffolding.
- Delete commands MUST refuse to proceed when dependency analysis reveals downstream consumers unless the operator explicitly supplies `--force`; forced deletions MUST still print the blocking dependency tree, require explicit confirmation (flag or prompt), and MUST record in the command result that dependencies were overridden.
- All lifecycle commands MUST persist results to the canonical workspace paths (`spec/`, `impl/`, `.specman/scratchpad/`) returned by workspace discovery, and MUST error when filesystem writes fail.

#### Command Catalog

##### `status`

- Purpose: validate the entire workspace graph.
- MUST parse every specification and implementation, invoke the `specman-core` dependency tree builder, and detect invalid references or circular dependencies before completing.
- Exit codes MUST be deterministic: `EX_OK` for a healthy graph, `EX_DATAERR` for failures alongside the artifact identifiers and a concise summary of the missing reference or cycle.

##### `spec` command group

- Scope: operations that exclusively manage specification artifacts located under `spec/`.

###### `spec ls`

- MUST enumerate every specification discovered under `spec/`.
- Output MUST include, at minimum, the specification name and version extracted from front matter and MUST be emitted in a deterministic order (for example lexical by name) so tools can diff outputs reliably.
- MAY apply terminal emphasis to the active version when supported, but the raw text MUST remain parseable without ANSI sequences.

###### `spec new`

- MUST create a new specification using the mandated templates and MUST validate names according to `specman-data-model` before writing to disk.
- Generated files MUST be persisted to `spec/{name}/spec.md`, and the command MUST refuse to overwrite an existing specification unless a future option explicitly allows it.
- The following arguments MUST be honored in the listed precedence/order:

| Argument | Purpose | Default / Notes |
| --- | --- | --- |
| positional-name | Optional positional value immediately after `spec new`; treated as the specification name when `--name` is absent. | `null` |
| `--name <value>` | Explicit specification name; MUST override the positional value when both are present. | `null` |
| `--dependencies <a,b,c>` | Comma-separated dependency locators inserted into the generated front matter. | `[]` |
| `--version <semver>` | Version recorded in front matter. | `1.0.0` |

- All `--dependencies` values MUST be validated for locator support (workspace-relative path or HTTPS URL) before writing them.

##### `impl` command group

- Scope: operations governing implementation artifacts stored under `impl/`.
- Commands in this group MUST reuse workspace discovery results so paths resolve relative to the active SpecMan workspace and MUST enforce the implementation naming constraints defined by `specman-data-model` and the founding specification.

###### `impl ls`

- MUST enumerate every implementation discovered under `impl/` after resolving the workspace root.
- Output MUST include, at minimum, the implementation name, the implementation version, and the targeted specification identifier derived from `spec` front matter (name plus version when available). Additional fields (such as primary language) MAY be included when available, but the required set MUST remain present and parseable without ANSI sequences.
- Results MUST be emitted in a deterministic order (for example, lexical by implementation name) so tooling can diff outputs reliably.
- Exit codes MUST follow the same rules as `status`: `EX_OK` when enumeration succeeds and `EX_DATAERR` (or another `sysexits` value) when parsing failures or workspace violations occur.

###### `impl new`

- MUST create a new implementation using the templates mandated by `specman-templates`, persisting output to `impl/{name}/impl.md` and refusing to overwrite existing implementations unless a future option explicitly allows it.
- MUST require callers to identify the target specification via `--spec`. The flag MUST accept:
  - A short specification name (matching a folder under `spec/`); the CLI MUST resolve this to the workspace-relative path before invoking template rendering.
  - A workspace-relative filesystem path.
  - An HTTPS URL. Unsupported schemes MUST fail fast with `EX_USAGE` and clear remediation guidance.
- MUST require a single implementation name provided either as the positional argument immediately after `impl new` **or** via `--name`. When both are supplied the CLI MUST raise an error; when neither is supplied the CLI MUST fail with a missing-argument error.
- MUST require `--language <identifier@version>` so the resulting front matter can satisfy the data-model implementing-language constraints. The flag MUST NOT have a default value.
- Creation MUST validate that the supplied name satisfies implementation naming rules (≤4 words, hyphenated, includes the implementing language identifier) and MUST invoke template rendering only after resolving all template tokens, including HTML comment directives.

##### `scratch` command group

- Scope: scratch pad lifecycle operations rooted at `.specman/scratchpad/`.
- Commands MUST enforce the scratch pad naming rules (`specman-data-model`), ensure each pad records a valid work type (`feat`, `ref`, or `revision`), and MUST keep the `target` field aligned with a specification or dependency locator.

###### `scratch ls`

- MUST enumerate every scratch pad directory under `.specman/scratchpad/`, including pads created outside this CLI session.
- Output MUST list, at minimum, the scratch pad name (folder slug), work type, and target artifact path/URL. Additional metadata (branch, status) MAY be shown when present, but the required trio MUST remain parseable.
- Results MUST be sorted deterministically (for example, lexical by scratch pad name) and MUST honor `--json` / `--verbose` flags in the same way as other command groups.
- The command MUST surface missing metadata (for example, absent work type) as `EX_DATAERR` while still listing well-formed pads to aid remediation.

###### `scratch new`

- MUST create a new scratch pad using the default or overridden scratch template described in `specman-templates`, placing the result in `.specman/scratchpad/{scratch_name}/scratch.md`.
- MUST require the following arguments:
  - `--target <locator>`: a workspace-relative path or HTTPS URL pointing to the specification or dependency the scratch pad will address. Unsupported schemes MUST raise `EX_USAGE`.
  - `--name <scratch-name>`: a slug meeting the scratch pad naming rules (all lowercase, hyphen separated, ≤4 words). No positional name is accepted for scratch pads to avoid ambiguity.
  - `--type <feat|ref|revision>`: selects the work type; the CLI MUST reject unknown values and MUST populate the `work_type` object accordingly.
- The command MUST persist the scratch pad front matter with the resolved branch name using the `{target_name}/revision|feat|ref/{scratch_name}` pattern and MUST leave template HTML comments intact until satisfied, matching the `specman-templates` governance rules.
- Workspace discovery MUST be used to determine the destination `.specman` folder, and the command MUST fail when the folder is missing rather than attempting to create a workspace implicitly.

### Concept: Data Model Activation

- The CLI MUST bundle a SpecMan data-model implementation (adapter) as an internal library so every installation has a compliant default aligned with the major version of `specman-data-model` declared in this specification.
- The bundled adapter MUST be the only supported adapter; the CLI MUST reject workspace configuration overrides that attempt to register alternative adapters and MUST emit an actionable error that reiterates the bundled-only policy.
- CLI commands MUST serialize entities exactly as defined in the data model before persisting or emitting them, and MUST surface validation errors from the adapter verbatim to the caller.
- If the bundled adapter fails to initialize or becomes incompatible with the workspace data, the CLI MUST fail the command and provide remediation guidance (for example, reinstalling the CLI or aligning workspace data with the supported adapter version).

### Concept: Template Integration & Token Handling

- Creation commands MUST load the appropriate Markdown template from `templates/spec/`, `templates/impl/`, or `templates/scratch/` (or workspace overrides) before rendering artifacts.
- The CLI MUST require callers to supply every declared `{{token}}` before rendering; missing tokens MUST result in descriptive errors that reference the originating template and token name.
- Template rendering MUST respect HTML comment directives embedded in templates and MUST only remove a directive after its instruction has been satisfied or explicitly recorded in the generated artifact.
- The CLI SHOULD cache template metadata (required tokens, scenario type) for the duration of a command invocation to avoid redundant filesystem reads, but MUST NOT cache it across workspaces unless the template version is part of the cache key.
- Workspaces MAY override the default templates by adding plaintext pointer files inside `.specman/templates/` whose filenames are all uppercase and describe the artifact type (for example, `SPEC`, `IMPL`, `SCRATCH`). Each pointer file MUST contain either a workspace-relative path (resolved from the workspace root) or an HTTPS URL to the desired Markdown template.
- When a pointer file exists, the CLI MUST resolve its locator, verify the referenced resource is readable Markdown, and surface an actionable error if the locator is missing, outside the workspace (for filesystem paths), or uses an unsupported scheme.
- When a pointer file is absent, the CLI MUST fall back to the catalog defaults under `templates/spec/`, `templates/impl/`, or `templates/scratch/` to remain aligned with the `specman-templates` specification.
- Pointer files MUST be re-read on each command invocation so operators can change template sources without restarting the CLI, and their resolved locations MUST feed directly into `TemplateRenderPlan` creation.

### Concept: Observability & Error Surfacing

- Each CLI command SHOULD emit structured logs (for example JSON lines) when `--verbose` or `--json` flags are supplied, capturing workspace paths, template locators, and adapter identifiers used during execution.
- Error messages MUST reference the specification section (Concept or Entity) that mandated the failed behavior whenever possible, enabling downstream tooling to triage issues quickly.

## Key Entities

### Entity: CliSession

- Represents a single CLI invocation, including parsed flags, environment overrides, and references to the data-model adapter.
- MUST capture the workspace context, resolved template catalog, and logging preferences for downstream components.
- SHOULD expose helpers to format consistent success/error payloads.

### Entity: WorkspaceContext

- Encapsulates the workspace root, `.specman` directory, detected templates directory, and adapter configuration for the active invocation.
- MUST be derived from the Workspace Context Resolution concept and reused across all subcommands invoked within the same process.
- MAY cache derived paths (spec, impl, scratchpad roots) for efficiency.

### Entity: LifecycleRequest

- Describes a create or delete operation, including artifact type, target name, template locator, dependency tree, and requested flags (`--force`, `--json`, etc.).
- MUST validate names against the data-model naming constraints before dispatching to the adapter.
- SHOULD record rendered template output (for create) or dependency trees (for delete) to support auditing.

### Entity: DeletionPlan

- Captures the dependency analysis for a delete request, including upstream/downstream relationships, whether deletion is permitted, and any required confirmations.
- MUST be produced before any filesystem mutation occurs.
- MUST reference SpecMan Core dependency mapping outputs and annotate whether the current request respects or overrides blocking dependents.

### Entity: TemplateRenderPlan

- Contains the template locator, token map, resolved output path, and any post-processing steps (such as removing satisfied HTML comments).
- MUST ensure every required token is supplied, and MUST record whether default values were injected.
- SHOULD expose dry-run output for tooling that wants to preview generated artifacts.

## Constraints

- Commands MUST be deterministic: identical inputs (workspace, flags, templates) MUST yield identical outputs aside from timestamps or IDs explicitly documented as variable.
- The CLI MUST reject simultaneous create and delete requests within a single invocation to avoid partial state mutations; batching MUST run operations sequentially.
- Configuration files under `.specman/` MAY supply defaults (such as adapter identifiers or template overrides), but command-line flags MUST take precedence.
- The CLI MUST guard against executing outside the detected workspace by refusing to read or write files that resolve beyond the workspace root.
- Extensions or plugins MUST NOT bypass dependency checks or naming validations defined by this specification.

## Additional Notes

- Distribution, install scripts, and binary naming conventions are intentionally unspecified; downstream teams MAY package the CLI for their ecosystems as long as runtime semantics remain compliant.
- Future versions MAY introduce additional command groups (for example, validation or status) provided they reuse the concepts and entities defined here.
- Implementations MAY integrate with credential stores or secrets managers to access remote template catalogs, but such integrations MUST continue to respect the template governance defined in `specman-templates`.
- Persistent audit logging is out of scope for this version because the CLI does not prescribe a storage location for historical records.
