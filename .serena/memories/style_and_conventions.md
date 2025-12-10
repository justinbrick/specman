# Style & Conventions

- Language & toolchain: Rust (target Rust 1.91+, edition 2024). Use `rustup` to manage the toolchain.
- Formatting & linting: Run `cargo fmt` and `cargo clippy` in CI and locally. Keep code compatible with Rust 1.91.
- Workspace layout: Rust sources under `src/` and a Cargo workspace defined in `src/Cargo.toml` (members: `crates/specman`, `crates/specman-cli`, `crates/specman-mcp`). Documentation and templates live at the repo root in `docs/`, `templates/`, `spec/`, and `impl/`.
- Templates & front-matter: Templates use HTML guard comments; front-matter is YAML that must satisfy the SpecMan Data Model. Prefer the CLI `specman template` commands to change pointers.
- CLI behavior: Commands exit with `sysexits` codes; outputs are deterministic for CI reliability. Prefer idempotent, filesystem-safe operations.
- Naming & API conventions: Follow Rust idioms (snake_case for functions, CamelCase for types, module organization by responsibility). Keep public APIs documented in crate-level docs.

If you want, I can expand this with concrete lint rules, CI commands, or examples from the codebase.