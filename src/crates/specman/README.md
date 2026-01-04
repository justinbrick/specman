# SpecMan Runtime Library

SpecMan is the runtime foundation for authoring and automating software specifications. It powers workspace discovery, dependency mapping, templating, and lifecycle workflows that keep specs consistent and reproducible.

## What It Does

- Discovers workspace roots and canonical directories (`WorkspaceLocator`, `WorkspacePaths`)
- Builds dependency graphs for specifications/implementations/scratch pads (`DependencyTree`)
- Resolves templates (embedded defaults + workspace pointer files) via `TemplateCatalog`
- Renders Markdown templates deterministically (`MarkdownTemplateEngine`)
- Provides lifecycle orchestration via the high-level `Specman` façade (plan/create/delete)
- Exposes structured lifecycle failures (`SpecmanError::Lifecycle(LifecycleError)`) so callers can branch programmatically

## Getting Started

Add the published crate to your `Cargo.toml`:

```toml
specman = "2"
```

Or with Cargo:

```bash
cargo add specman@2
```

The ergonomic entrypoint is `DefaultSpecman`, which wires the default filesystem-backed stack:

```rust
use specman::DefaultSpecman;

let specman = DefaultSpecman::from_current_dir()?;
# Ok::<(), specman::SpecmanError>(())
```

For automation, match lifecycle errors directly instead of parsing strings:

```rust
use specman::error::LifecycleError;
use specman::SpecmanError;

fn classify(err: SpecmanError) {
 match err {
  SpecmanError::Lifecycle(LifecycleError::DeletionBlocked { .. }) => {
   // downstream dependents exist
  }
  SpecmanError::Lifecycle(LifecycleError::PlanTargetMismatch { .. }) => {
   // a stale plan was supplied for a different artifact
  }
  _ => {}
 }
}
```

## Reference Validation

Use the reference validator when you need to sanity-check Markdown link destinations inside a workspace. Typical flow:

```rust
use specman::{
    ReferenceValidationStatus, ReferenceValidator, ValidationMode, discover_workspace,
};

let workspace = discover_workspace(".")?; // or WorkspaceDiscovery::initialize(start)

// Default mode: reachability on, fragments on, transitive traversal on (64 doc limit).
let validator = ReferenceValidator::new(&workspace);
let report = validator.validate("spec/my-spec/spec.md")?;

if report.status == ReferenceValidationStatus::Failure {
    for issue in &report.issues {
        eprintln!("{:?}: {}", issue.kind, issue.message);
    }
}

// To tweak behavior (e.g., disable fragment checks or reachability):
let mut mode = ValidationMode::default();
mode.resolve_fragments = false; // skip heading-fragment verification
mode.reachability = specman::ReachabilityPolicy::Disabled; // syntax-only for https://
let report = ReferenceValidator::with_mode(&workspace, mode).validate("impl/foo/impl.md")?;
```

Intricate details to be aware of:

- Root locators may be workspace paths or SpecMan handles (`spec://{name}`, `impl://{name}`, `scratch://{slug}`); handles found **inside** Markdown links are rejected as `DisallowedHandle`.
- HTTPS reachability is on by default. Redirects count as success; timeouts surface as `Diagnostic`, while 4xx responses become `Error` and fail the run.
- Filesystem targets are resolved relative to the source document, canonicalized, and must stay inside the workspace root; missing files still count as `WorkspaceBoundary` or `FileMissing` errors even if they never existed.
- Fragment validation uses SpecMan’s slug rules (NFKD + lowercase + punctuation filtering + suffix `-1`, `-2`, ...). Cross-document fragments are validated transitively by default up to 64 Markdown documents.
- Images are ignored; inline links, reference-style links, and autolinks are validated. Unresolved reference identifiers are reported as errors.
- Results are deterministic: issues are reported in source order and the report carries both `discovered` references and `issues` for consumers.

## Repository

All source, issue tracking, and release notes live in the main GitHub repository:

<https://github.com/justinbrick/specman>

Star the repo or follow along for roadmap updates (including relationship graphing and expanded MCP tool coverage).
