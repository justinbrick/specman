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

## Repository

All source, issue tracking, and release notes live in the main GitHub repository:

<https://github.com/justinbrick/specman>

Star the repo or follow along for roadmap updates (including relationship graphing and expanded MCP tool coverage).
