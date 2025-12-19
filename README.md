# Specification Manager

Specification Manager is a CLI tool that helps AI agents and teams create, read, and follow clear, code-friendly specifications. It favors concise, implementable constraints over task-tracking or corporate jargon.

## About SpecMan

SpecMan exists because tools like GitHub's Speckit optimize for shareholder optics, mushy prompts, and approvals rather than for engineers who need executable behaviors. SpecMan rejects that path and keeps everything in-repo, deterministic, and auditable:

- **Data-first contracts:** The [SpecMan Data Model](spec/specman-data-model/spec.md) nails down YAML schemas for workspaces, specs, implementations, and scratch pads so nothing depends on vibes.
- **Deterministic platform services:** [SpecMan Core](spec/specman-core/spec.md) handles workspace discovery, dependency trees, lifecycle automation, and metadata mutation so commands behave the same everywhere.
- **Template & prompt governance:** [SpecMan Core](spec/specman-core/spec.md#concept-template-orchestration) defines the template orchestration rules and HTML-guarded scaffolds that force AI systems to satisfy every directive instead of hand-waving.
- **Operator-focused CLI:** The [SpecMan CLI](spec/specman-cli/spec.md) code paths (implemented in `impl/specman-cli-rust/impl.md`) prioritize spec authors and implementers, not bureaucrats.

Read the extended background in `docs/about.md` if you want the full manifesto.

## Testing

Run repository tests from the `src` directory to exercise filesystem + HTTPS traversal paths, cycle detection, and metadata fallbacks:

```bash
cd src
cargo test -p specman
```

CI should also run `cargo fmt` and `cargo clippy` to keep formatting and lint gates aligned with Rust 1.91.

## Documentation Index

- `docs/about.md` — philosophy, goals, and why SpecMan replaces Speckit's poor prompting.
- `docs/getting-started.md` — installation, workspace setup, CLI walkthrough.
- `docs/lifecycle-ergonomics.md` — Dec 2025 lifecycle refactor notes (façade API, structured errors, CLI/MCP alignment).
