# SpecMan — Project Overview

- Purpose: SpecMan is a CLI and library for creating, validating, and managing executable software specifications, templates, and implementations. It emphasizes deterministic, in-repo, data-first contracts and operator-focused tooling.
- Tech stack: Rust (workspace, edition 2024). Key crates: `specman` (runtime library), `specman-cli` (operator CLI), `specman-mcp` (MCP server work in progress).
- Repo layout (important paths):
  - `src/` — Rust workspace and crates
  - `src/crates/specman` — core runtime crate
  - `spec/`, `impl/`, `templates/` — specs, implementations, and templates content
  - `docs/` — human documentation (`getting-started.md`, `about.md`)
  - `.specman/` — workspace configuration (used by CLI)
- Entrypoints & artifacts: the CLI installs as `specman` (via `cargo install --path crates/specman-cli`) and exposes commands like `specman status`, `specman spec new`, `specman impl new`, `specman template`.
- Determinism rules: CLI outputs are deterministic; commands return POSIX `sysexits` codes and template files use HTML-guarded scaffolds and front-matter validation.

References: README.md, docs/getting-started.md, src/crates/specman/README.md
