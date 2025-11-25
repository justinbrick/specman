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

Source lives under `src/crates/specman-cli`, a binary crate that will be added to the workspace manifest (`src/Cargo.toml`). The layout is organized as:

- `src/main.rs` — entry point that calls `run_cli(std::env::args())`.
- `src/cli.rs` — constructs the `clap::Command` tree, defines shared flags (`--workspace`, `--json`, `--force`, `--help`) and sets `subcommand_required(true)`.
- `src/session.rs` — builds `CliSession` by resolving workspace paths via `specman::workspace::FilesystemWorkspaceLocator`, loading template overrides, and capturing verbosity flags.
- `src/commands/` — modules for `status`, `spec`, `impl`, and `scratch` command groups. Each module contains helper structs (for example, `SpecCommands`, `ImplementationCommands`, `ScratchCommands`) plus typed responses used by the output formatter, covering `ls`, `new`, and `delete` flows as described in the specification.
- `src/output.rs` — houses JSON/text emitters plus sysexit mapping helpers shared across commands.

Every command stores its rendered artifacts inside the canonical workspace directories returned by `WorkspacePaths`, delegating disk writes to `specman::persistence::WorkspacePersistence`. Likewise, deletion flows delegate dependency-tree checks and persistence to the shared library so lifecycle guardrails remain centralized. The CLI crate depends on the `specman` library via a workspace path dependency, so library updates remain in lockstep with the binary.

### Libraries

- **`specman`** (workspace path) supplies `DependencyMapping`, `LifecycleController`, `TemplateEngine`, and workspace discovery utilities that the CLI orchestrates rather than re-implementing.
- **`clap@4`** enforces argument validation, flag parsing, and contextual help output per the CLI specification.
- **`sysexits@0.6`** provides the typed `ExitCode` constants to ensure exit status compliance.
- **`serde_json@1`** converts command responses into deterministic JSON payloads consumed by automation when `--json` or `--verbose` is set.
- **`tracing@0.1`** and `tracing-subscriber@0.3` (optional feature) emit structured logs keyed by command name, workspace root, and template locators, supporting the Observability concept.

## API Surface

The binary exposes a focused API set so other Rust entry points (integration tests, alternative front-ends) can reuse the same orchestration logic without shelling out.

```rust
pub fn run_cli<I, S>(args: I) -> Result<ExitCode, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>;

pub fn build_cli<'a>() -> clap::Command;

pub fn dispatch_command(session: &CliSession, matches: &clap::ArgMatches) -> Result<CommandResult, CliError>;

pub fn emit_result(result: CommandResult, format: OutputFormat) -> Result<ExitCode, CliError>;
```

- `run_cli` bootstraps logging, builds the `clap` tree, parses arguments, resolves the workspace override (if any), and routes to the matching subcommand. It returns an `ExitCode` derived from `sysexits` constants so callers embedding the CLI (for example tests) can assert on deterministic outcomes.
- `build_cli` centralizes the `clap` command tree definition. Subcommands map directly to specification sections: `status`, `spec ls/new/delete`, `impl ls/new/delete`, and `scratch ls/new/delete`. Each command registers `--help` plus `--json`/`--verbose` toggles.
- `dispatch_command` converts parsed matches into typed requests (`LifecycleRequest`, `TemplateRenderPlan`), calls into the `specman` crate for dependency trees or template rendering, and surfaces structured `CommandResult` objects describing stdout payloads and any downstream dependency trees.
- `emit_result` prints human-readable text by default and switches to JSON only when `--json` selects the structured formatter. It mirrors the Observability guidelines by including the governing concept reference in error payloads and maps the command-level outcome to a final `ExitCode`.

Command modules expose thinner helpers to keep unit tests focused. Example (`commands/status.rs`):

```rust
pub fn run_status(session: &CliSession) -> Result<CommandResult, CliError> {
    let tree = session
        .dependency_mapper()
        .dependency_tree(&ArtifactId::Workspace)?;
    Ok(CommandResult::status(tree))
}
```

Here, `run_status` invokes the shared `DependencyMapping` implementation shipped inside the `specman` crate and packages the resulting graph so the formatter can print violations (`EX_DATAERR`) or successes (`EX_OK`). Similar helpers exist for `spec ls`, `spec new`, `impl ls`, `impl new`, `scratch ls`, and `scratch new`, ensuring each command remains a pure function over `CliSession` state.

## Data Models

The CLI crate mirrors key entities from the specification so runtime state remains explicit:

```rust
pub struct CliSession {
    pub workspace: WorkspaceContext,
    pub output: OutputFormat,
    pub verbose: bool,
    pub adapter: Arc<dyn DataModelAdapter>,
    pub lifecycle: Arc<dyn LifecycleController>,
    pub templates: Arc<dyn TemplateEngine>,
}

pub struct WorkspaceContext {
    pub root: PathBuf,
    pub dot_specman: PathBuf,
    pub spec_dir: PathBuf,
    pub impl_dir: PathBuf,
    pub scratchpad_dir: PathBuf,
}

pub struct LifecycleRequest<'a> {
    pub target: ArtifactId,
    pub name: &'a str,
    pub template: TemplateDescriptor,
    pub tokens: TokenMap,
}

pub struct TemplateRenderPlan {
    pub locator: TemplateLocator,
    pub required_tokens: Vec<String>,
    pub resolved_path: PathBuf,
}

pub enum CommandResult {
    Status { tree: DependencyTree },
    SpecList { specs: Vec<SpecSummary> },
    SpecCreated { path: PathBuf },
  SpecDeleted { name: String, dependencies: DependencyTree },
    ImplList { implementations: Vec<ImplSummary> },
    ImplCreated { path: PathBuf },
  ImplDeleted { name: String, dependencies: DependencyTree },
    ScratchList { pads: Vec<ScratchSummary> },
    ScratchCreated { path: PathBuf },
  ScratchDeleted { name: String, dependencies: DependencyTree },
}
```

- `CliSession` holds the resolved workspace state, adapters, and shared services so commands avoid recomputing filesystem metadata.
- `WorkspaceContext` mirrors the `WorkspaceContext` entity defined in the specification and is produced exactly once per invocation via `specman::workspace::FilesystemWorkspaceLocator` or an override path supplied by `--workspace`.
- `LifecycleRequest` packages the arguments for creation commands, including validated names, token maps, and template descriptors derived from the template pointer or default catalog.
- `TemplateRenderPlan` enforces the Template Integration concept by listing required tokens and the resolved output path before any filesystem mutation occurs.
- `CommandResult` enumerates the deterministic payload shapes returned by each command (including delete flows that echo the dependency tree used for confirmation), making it straightforward to serialize responses or inspect them during testing.

## Operational Notes

- **Building & Testing:** Run `cargo build -p specman-cli` and `cargo test -p specman-cli` from `src/`. Integration tests under `src/crates/specman-cli/tests/` spin up temporary workspaces, execute commands via `run_cli`, and assert on exit codes plus filesystem side effects.
- **Installing the Binary:** Run `cargo install --path src/crates/specman-cli` from the repository root (or any directory that can resolve that relative path). The crate now defines a `[[bin]]` target named `specman`, so installation places a `specman` executable on `$PATH` without extra flags while continuing to depend on the path-only `specman` library crate bundled in this workspace.
- **Workspace Resolution:** The `--workspace <path>` flag overrides discovery; absent that flag, the CLI mirrors `specman-core` behavior by scanning ancestors for `.specman`. Invalid overrides fall back to ancestor search and emit `EX_USAGE` errors referencing the Workspace Context Resolution concept.
- **Template Overrides:** Pointer files inside `.specman/templates/{SPEC|IMPL|SCRATCH}` are read on every invocation. Unsupported schemes, unreadable files, or pointers outside the workspace produce `EX_CONFIG` errors that cite [Concept: Template Integration & Token Handling](../../spec/specman-cli/spec.md#concept-template-integration--token-handling).
- **Lifecycle Safeguards:** Delete commands refuse to run when downstream dependencies exist unless `--force` is supplied. Forced deletions still print the blocking dependency tree and annotate results so operators know dependencies were overridden, satisfying the Lifecycle Command Surface rules.
- **Logging & JSON Output:** Human-readable summaries are emitted by default. When `--json` is supplied, the CLI emits newline-delimited JSON records containing the command name, workspace root, adapter id, result payload, and reference to the governing specification section. `--verbose` enables structured `tracing` logs alongside textual summaries. Errors always cite the concept or entity that triggered the failure per the Observability guidance.
- **Sysexits Enforcement:** Success paths use `EX_OK`. Validation issues (naming, missing tokens, dependency failures) map to `EX_DATAERR`. Incorrect CLI usage maps to `EX_USAGE`, filesystem or network issues map to `EX_IOERR`/`EX_OSERR`, and template pointer violations map to `EX_CONFIG`.

This implementation keeps the CLI aligned with `specman-cli` governance while reusing the shared `specman` crate so downstream automation can rely on a single source of truth for workspace, dependency, and template logic.
