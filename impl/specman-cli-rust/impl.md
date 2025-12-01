---
spec: ../../spec/specman-cli/spec.md
name: specman-cli-rust
version: "0.1.0"
location: ../../src/crates/specman-cli
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
    toolchain: stable
  libraries:
    - name: clap@4
    - name: sysexits@0.6
    - name: serde_json@1
    - name: tracing@0.1
secondary_languages: []
references:
  - ref: ../../spec/specman-core/spec.md
    type: specification
    optional: false
  - ref: ../../spec/specman-data-model/spec.md
    type: specification
    optional: false
  - ref: ../../spec/specman-templates/spec.md
    type: specification
    optional: false
  - ref: ../specman-library/impl.md
    type: implementation
    optional: false
---

# Implementation — SpecMan CLI Rust Binary

## Overview

The `specman-cli-rust` implementation delivers the command-line interface defined in [`spec/specman-cli/spec.md`](../../spec/specman-cli/spec.md). It packages the existing `specman` workspace services into an operator-friendly binary that honors the CLI Invocation Model, Workspace Context Resolution, Lifecycle Command Surface, Template Integration, and Observability concepts. The binary coordinates workspace discovery, dependency validation, template rendering, and scratch-pad orchestration so practitioners can create, list, or delete specifications, implementations, and scratch pads without writing code. Each command surfaces structured stdout/stderr, uses POSIX `sysexits`, and references the governing specification section in every actionable error, enabling automation and human users to triage results quickly. The CLI remains a thin wrapper over the shared `specman` library so there is a single source of truth for workspace, dependency, and template behavior.

## Implementing Languages

- **Primary — `rust@1.91.0` (edition 2024):** Rust provides memory safety, strong typing, and straightforward integration with the existing `specman` crate. Edition 2024 features (let-else, `impl Trait` return types, `clippy::future_not_send` fixes) keep the codebase modern while staying aligned with the repository MSRV. The binary uses `tokio::main`-free synchronous execution to keep startup deterministic and minimize dependencies.
  - **`clap@4`** drives the CLI surface with declarative subcommand definitions, ensuring every command exposes `--help` and honors required flag validation per [Concept: CLI Invocation Model](../../spec/specman-cli/spec.md#concept-cli-invocation-model).
  - **`sysexits@0.6`** maps domain errors to the mandated exit codes (for example `EX_OK`, `EX_DATAERR`, `EX_USAGE`, `EX_CANTCREAT`).
  - **`serde_json@1`** serializes structured outputs when `--json` or `--verbose` flags are enabled, matching the Observability requirements.
  - **`tracing@0.1`** emits contextual logs (workspace root, template locators, adapter identifiers) without coupling to a specific logging backend.
- **Secondary languages:** none. Shell wrappers and scripts simply call the compiled binary.

## References

- [`spec/specman-core/spec.md`](../../spec/specman-core/spec.md) defines the dependency mapping, lifecycle automation, and template orchestration APIs that the CLI invokes through the `specman` crate.
- [`spec/specman-data-model/spec.md`](../../spec/specman-data-model/spec.md) governs workspace metadata, implementing-language objects, reference schemas, and data-model validation enforced before any command mutates files.
- [`spec/specman-templates/spec.md`](../../spec/specman-templates/spec.md) drives template pointer resolution plus HTML directive handling for creation commands.
- [`impl/specman-library/impl.md`](../specman-library/impl.md) documents the Rust library crate bundled by this CLI. The binary links directly against that crate to reuse workspace discovery, dependency tree building, template rendering, and persistence helpers.

## Implementation Details

### Code Location

Source lives under `src/crates/specman-cli`, a binary crate listed in the workspace manifest (`src/Cargo.toml`). The layout is organized as:

- `src/main.rs` — entry point that calls `run_cli(std::env::args())`.
- `src/cli.rs` — constructs the `clap::Command` tree, defines shared flags (`--workspace`, `--json`, `--force`, `--help`) and sets `subcommand_required(true)`.
- `src/context.rs` — builds `CliSession` by resolving workspace paths via `specman::workspace::FilesystemWorkspaceLocator`, loading template overrides, and capturing verbosity flags.
- `src/commands/` — modules for `status`, `spec`, `impl`, and `scratch` command groups. Each module contains helper structs plus typed responses used by the output formatter, covering `ls`, `new`, `delete`, and `dependencies` flows as described in the specification.
- `src/formatter.rs` — houses JSON/text emitters plus sysexit mapping helpers shared across commands.

Every command stores its rendered artifacts inside the canonical workspace directories returned by `WorkspacePaths`, delegating disk writes to `specman::persistence::WorkspacePersistence`. Likewise, deletion flows delegate dependency-tree checks and persistence to the shared library so lifecycle guardrails remain centralized. The CLI crate depends on the `specman` library via a workspace path dependency, so library updates remain in lockstep with the binary.

## Concept Coverage

### Concept: CLI Invocation Model ([spec/specman-cli/spec.md#concept-cli-invocation-model](../../spec/specman-cli/spec.md#concept-cli-invocation-model))

`run_cli` and `build_cli` enforce the SpecMan CLI grammar: global flags are parsed once, each subcommand exposes `--help`, and all exit codes map to `sysexits`. The surface that operators script against is summarized below.

| Root Command | Subcommands | Module | Notes |
| --- | --- | --- | --- |
| `status` | _n/a_ | [`commands/status.rs`](../../src/crates/specman-cli/src/commands/status.rs) | Validates the full workspace graph before exiting with `EX_OK`/`EX_DATAERR`. |
| `spec` | `ls`, `new`, `delete`, `dependencies` | [`commands/spec.rs`](../../src/crates/specman-cli/src/commands/spec.rs) | Manages artifacts under `spec/`; honors `--name`, `--dependencies`, `--version`, and delete `--force`. |
| `impl` | `ls`, `new`, `delete`, `dependencies` | [`commands/implementation.rs`](../../src/crates/specman-cli/src/commands/implementation.rs) | Mirrors the specification flows while requiring `--spec` and `--language` on creation. |
| `scratch` | `ls`, `new`, `delete`, `dependencies` | [`commands/scratch.rs`](../../src/crates/specman-cli/src/commands/scratch.rs) | Operates on `.specman/scratchpad`, enforcing naming, `--target`, and work-type validation. |

```rust
pub fn run_cli<I, S>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone;

fn build_cli() -> Command;
```

The CLI never shells out; instead, `run_cli` wires `Verbosity` flags into `emit_result`, so consumers embedding the crate can capture `ExitCode` outcomes directly.

### Concept: Workspace Context Resolution ([spec/specman-cli/spec.md#concept-workspace-context-resolution](../../spec/specman-cli/spec.md#concept-workspace-context-resolution))

`CliSession::bootstrap` performs workspace discovery using `FilesystemWorkspaceLocator`, honors the optional `--workspace` override, and fans the result out through `WorkspacePaths`. The locator revalidates cached paths to guarantee `.specman` still exists, preventing stale roots during long-running sessions.

```rust
pub fn bootstrap(
    workspace_override: Option<String>,
    verbosity: Verbosity,
) -> Result<CliSession, CliError>;
```

Each command receives the same `CliSession`, so downstream helpers (dependency mapper, persistence layer, lifecycle controller) reuse canonical paths without re-reading the filesystem.

### Concept: Lifecycle Command Surface ([spec/specman-cli/spec.md#concept-lifecycle-command-surface](../../spec/specman-cli/spec.md#concept-lifecycle-command-surface))

The four command groups expose symmetrical create/list/delete/dependencies flows, all of which reuse the shared lifecycle controller to guard against unsafe deletions. Direction flags are centralized in [`commands/dependencies.rs`](../../src/crates/specman-cli/src/commands/dependencies.rs), keeping UX consistent across artifact types and ensuring the CLI rejects conflicting `--upstream`/`--downstream`/`--all` combinations with `EX_USAGE`. `CommandResult::DependencyTree` reuses the same serialization regardless of artifact type, so ASCII output, JSON output, and downstream deletes all share the exact tree that `plan_deletion` produced.

```
Downstream: 2 edge(s)
    spec specman-cli@1.0.0
    ├── impl specman-cli-rust@0.1.0
    └── scratch cli-entity-map
```

Delete commands always print at least one dependency-tree example like the above before honoring `--force`, satisfying the agreed-upon documentation detail level.

### Concept: Data Model Activation ([spec/specman-cli/spec.md#concept-data-model-activation](../../spec/specman-cli/spec.md#concept-data-model-activation))

`CliSession` wires the `specman` library’s `FilesystemDependencyMapper`, `WorkspacePersistence`, `MarkdownTemplateEngine`, `InMemoryAdapter`, and `DefaultLifecycleController` together so every CLI action goes through the same adapter stack. The binary depends on:

- `specman` — canonical dependency mapping, lifecycle automation, data-model persistence.
- `clap@4` — declarative CLI definition with help output for every command.
- `sysexits@0.6` — typed POSIX codes surfaced via `ExitStatus`.
- `serde_json@1` — JSON serialization for `CommandResult` when `--json` is set.
- `tracing@0.1`/`tracing-subscriber@0.3` — structured logs keyed by workspace context and template locators.

### Concept: Template Integration & Token Handling ([spec/specman-cli/spec.md#concept-template-integration--token-handling](../../spec/specman-cli/spec.md#concept-template-integration--token-handling))

Workspace pointer files under `.specman/templates/{SPEC,IMPL,SCRATCH}` override the bundled catalog. `TemplateCatalog::descriptor` resolves those locators, after which `MarkdownTemplateEngine::render` enforces token completeness and preserves HTML directives until the generated artifact records how each instruction was satisfied.

```rust
let descriptor = session.templates.descriptor(TemplateKind::Specification)?;
let mut rendered = session
    .template_engine
    .render(&descriptor, &TokenMap::new())?;
```

Tokens sourced from CLI flags (for example, `--dependencies` or `--language`) feed directly into the `TokenMap`, so template errors cite the originating concept and template path.

### Concept: Observability & Error Surfacing ([spec/specman-cli/spec.md#concept-observability--error-surfacing](../../spec/specman-cli/spec.md#concept-observability--error-surfacing))

`emit_result` emits deterministic text for humans and newline-delimited JSON for automation, while `tracing` logs capture workspace roots, adapter identifiers, and template locators whenever `--verbose` is toggled. Errors reference the governing concept (for example, Workspace Context Resolution when discovery fails) and bubble the correct `sysexits` value via `ExitStatus`.

```rust
pub fn emit_result(result: CommandResult, format: OutputFormat) -> Result<ExitCode, CliError>;
```

Sample JSON emission:

```json
{"type":"spec_list","specs":[{"name":"specman-cli","version":"1.0.0","path":"spec/specman-cli/spec.md"}]}
```

Structured logs accompany verbose output so operators can correlate stdout with telemetry sinks.

## Entity Coverage

### Entity: CliSession ([spec/specman-cli/spec.md#entity-clisession](../../spec/specman-cli/spec.md#entity-clisession))

`CliSession` is defined in [`src/crates/specman-cli/src/context.rs`](../../src/crates/specman-cli/src/context.rs) and packages every service the CLI needs for a single invocation. The struct mirrors the spec entity by caching workspace paths, adapters, and template metadata so commands remain pure functions over shared state.

```rust
pub struct CliSession {
    pub workspace_paths: WorkspacePaths,
    pub dependency_mapper: Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
    pub persistence: Arc<WorkspacePersistence<Arc<FilesystemWorkspaceLocator>>>,
    pub template_engine: Arc<MarkdownTemplateEngine>,
    pub templates: TemplateCatalog,
    pub lifecycle: Arc<DefaultLifecycleController<
        Arc<FilesystemDependencyMapper<Arc<FilesystemWorkspaceLocator>>>,
        Arc<MarkdownTemplateEngine>,
        Arc<InMemoryAdapter>,
    >>,
    pub verbosity: Verbosity,
}
```

The `bootstrap` constructor validates the workspace override, initializes adapters, and logs the resolved root when `--verbose` is set.

### Entity: WorkspaceContext ([spec/specman-cli/spec.md#entity-workspacecontext](../../spec/specman-cli/spec.md#entity-workspacecontext))

The CLI relies on `specman::workspace::WorkspacePaths` to fulfill the `WorkspaceContext` entity. `WorkspacePaths` guarantees canonical directories for `spec/`, `impl/`, and `.specman/scratchpad`, and it surfaces accessor methods that the CLI uses when reading or writing artifacts.

```rust
pub struct WorkspacePaths {
    root: PathBuf,
    dot_specman: PathBuf,
}

impl WorkspacePaths {
    pub fn root(&self) -> &Path;
    pub fn dot_specman(&self) -> &Path;
    pub fn spec_dir(&self) -> PathBuf;
    pub fn impl_dir(&self) -> PathBuf;
    pub fn scratchpad_dir(&self) -> PathBuf;
}
```

Because `CliSession` holds a single `WorkspacePaths` instance, every command automatically respects Workspace Context Resolution rules even when invoked from nested directories.

### Entity: LifecycleRequest ([spec/specman-cli/spec.md#entity-lifecyclerequest](../../spec/specman-cli/spec.md#entity-lifecyclerequest))

The spec’s `LifecycleRequest` maps directly to `specman::lifecycle::CreationRequest`. Each create command (spec/impl/scratch) populates this struct with the resolved `ArtifactId`, template descriptor, and token map prior to invoking the lifecycle controller.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CreationRequest {
    pub target: ArtifactId,
    pub template: TemplateDescriptor,
    pub tokens: TokenMap,
}
```

Tokens originate from CLI arguments (`--dependencies`, `--language`, scratch work-type metadata), so the resulting `CreationRequest` captures both the artifact identity and the template inputs mandated by the specification.

### Entity: DeletionPlan ([spec/specman-cli/spec.md#entity-deletionplan](../../spec/specman-cli/spec.md#entity-deletionplan))

Forced and non-forced deletions rely on `specman::lifecycle::DeletionPlan`, which records whether downstream artifacts block removal and caches the dependency tree used for user confirmations.

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeletionPlan {
    pub dependencies: DependencyTree,
    pub blocked: bool,
}
```

`commands/spec.rs`, `commands/implementation.rs`, and `commands/scratch.rs` all call `plan_deletion` before invoking `execute_deletion`, ensuring the CLI refuses to remove artifacts when `blocked` is true unless `--force` is provided.

### Entity: TemplateRenderPlan ([spec/specman-cli/spec.md#entity-templaterenderplan](../../spec/specman-cli/spec.md#entity-templaterenderplan))

While the `specman` crate does not expose a struct literally named `TemplateRenderPlan`, the CLI satisfies the entity by combining a `TemplateDescriptor`, the `TemplateCatalog`, and the persistence layer before touching disk:

```rust
let descriptor = session.templates.descriptor(template_kind)?;
let rendered = session
    .template_engine
    .render(&descriptor, &token_map)?;
let persisted = session
    .persistence
    .persist(&artifact, &rendered)?;
```

This trio enforces the same guarantees described in the specification: every required token is validated before rendering, HTML directives stay intact until satisfied, and the resolved output path is computed prior to persistence so callers know exactly where artifacts land.

## Operational Notes

- **Building & Testing:** Run `cargo build -p specman-cli` and `cargo test -p specman-cli` from `src/`. Integration tests under `src/crates/specman-cli/tests/` spin up temporary workspaces, execute commands via `run_cli`, and assert on exit codes plus filesystem side effects.
- **Installing the Binary:** Run `cargo install --path src/crates/specman-cli` from the repository root (or any directory that can resolve that relative path). The crate now defines a `[[bin]]` target named `specman`, so installation places a `specman` executable on `$PATH` without extra flags while continuing to depend on the path-only `specman` library crate bundled in this workspace.
- **Workspace Resolution:** The `--workspace <path>` flag overrides discovery; absent that flag, the CLI mirrors `specman-core` behavior by scanning ancestors for `.specman`. Invalid overrides fall back to ancestor search and emit `EX_USAGE` errors referencing the Workspace Context Resolution concept.
- **Template Overrides:** Pointer files inside `.specman/templates/{SPEC|IMPL|SCRATCH}` are read on every invocation. Unsupported schemes, unreadable files, or pointers outside the workspace produce `EX_CONFIG` errors that cite [Concept: Template Integration & Token Handling](../../spec/specman-cli/spec.md#concept-template-integration--token-handling).
- **Dependency Inspection:** `spec dependencies`, `impl dependencies`, and `scratch dependencies` are read-only subcommands that reuse the shared `DependencyTree` formatter. Direction flags restrict traversal (`--downstream` default, `--upstream`, or combined `--all`), and the CLI rejects conflicting flags so scripts can rely on deterministic `sysexits` codes.
- **Lifecycle Safeguards:** Delete commands refuse to run when downstream dependencies exist unless `--force` is supplied. Forced deletions still print the blocking dependency tree and annotate results so operators know dependencies were overridden, satisfying the Lifecycle Command Surface rules.
- **Logging & JSON Output:** Human-readable summaries are emitted by default. When `--json` is supplied, the CLI emits newline-delimited JSON records containing the command name, workspace root, adapter id, result payload, and reference to the governing specification section. `--verbose` enables structured `tracing` logs alongside textual summaries. Errors always cite the concept or entity that triggered the failure per the Observability guidance.
- **Sysexits Enforcement:** Success paths use `EX_OK`. Validation issues (naming, missing tokens, dependency failures) map to `EX_DATAERR`. Incorrect CLI usage maps to `EX_USAGE`, filesystem or network issues map to `EX_IOERR`/`EX_OSERR`, and template pointer violations map to `EX_CONFIG`.


This implementation keeps the CLI aligned with `specman-cli` governance while reusing the shared `specman` crate so downstream automation can rely on a single source of truth for workspace, dependency, and template logic.

