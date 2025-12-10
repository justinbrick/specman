# SpecMan Runtime Library

SpecMan is the runtime foundation for authoring and automating software specifications. It powers dependency mapping, templating, lifecycle workflows, and other orchestration primitives that keep complex specs consistent and reproducible.

## What It Does

- Builds metadata and dependency graphs for spec documents and templates
- Validates and normalizes front matter to keep specs structured
- Provides persistence helpers so higher-level tools can store and retrieve spec state
- Supplies lifecycle utilities (ingest, render, export) that downstream clients such as the CLI or MCP server consume

## Getting Started

Add the crate to your workspace:

```toml
specman = { git = "https://github.com/justinbrick/specman", package = "specman" }
```

Then import the APIs you need:

```rust
use specman::workspace::Workspace;
use specman::dependency_tree::DependencyTree;
```

The crate exposes modules for adapters, metadata, templates, workspace management, and more. See the source files for detailed documentation.

## Repository

All source, issue tracking, and release notes live in the main GitHub repository:

<https://github.com/justinbrick/specman>

Star the repo or follow along for roadmap updates such as the upcoming MCP server and relationship graphing capabilities.
