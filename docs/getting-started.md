# Getting Started with SpecMan

This guide walks through the minimum steps required to stand up a SpecMan workspace, install the CLI, and start producing deterministic specifications, implementations, and scratch pads. It assumes you share SpecMan's bias toward behavior-first engineering and want nothing to do with approval theater or Speckit's vague prompts.

## 1. Prerequisites

- **Rust toolchain:** Install Rust 1.91 (or newer) with `rustup`. The workspace targets edition 2024 for both the core library and CLI.
- **Cargo workspace checkout:** Clone this repository or vendor the `specman` crates into your own monorepo. All paths below assume the root is `<repo>/` and the Rust sources live under `<repo>/src/`.
- **Deterministic shell:** Run commands from Linux or another POSIX-friendly environment so path separators and `sysexits` codes behave as specified.

## 2. Install the CLI

Install the published CLI from crates.io:

```bash
cargo install specman-cli
```

This produces a `specman` binary on your `$PATH`.

For contributors working from a repository checkout:

```bash
cd src
cargo install --path crates/specman-cli
```

The binary is intentionally self-contained: it bundles the SpecMan data-model adapter, dependency mapper, and template engine so every run behaves the same no matter where it executes.

## 3. Initialize a Workspace

SpecMan treats the nearest ancestor with a `.specman/` folder as the workspace root. To bootstrap a new workspace:

1. Create a `.specman/` directory at the repository root.
2. Copy the curated templates if they are not already present:
   - `templates/spec/spec.md`
   - `templates/impl/impl.md`
   - `templates/scratch/scratch.md`
   - Prompt catalog under `templates/prompts/`
3. Optionally add `.specman/templates/{SPEC,IMPL,SCRATCH}` pointer files if you plan to override the default templates with workspace-specific versions.

The workspace layout should now resemble:

```
<repo>/
  .specman/
    scratchpad/
  spec/
  impl/
  templates/
```

### Manage Template Pointers with the CLI

Once the workspace exists, prefer the `specman template` command group over manual edits when you need to override or remove template pointers:

```bash
# Point specification scaffolding at a workspace-specific template
specman template set --kind spec --locator templates/spec/custom.md

# Revert the implementation pointer so the CLI falls back to overrides or embedded defaults
specman template remove --kind impl
```

Both subcommands validate locators (workspace-relative paths or HTTPS URLs), acquire the filesystem locks required by SpecMan Core, refresh cached remote templates, and print provenance metadata describing which tier (override, pointer file, embedded default) will be used on the next `spec`, `impl`, or `scratch` command. If you remove a pointer, the CLI rewrites the embedded fallback cache immediately so subsequent runs stay deterministic.

## 4. Author Specifications, Implementations, and Scratch Pads

1. **Create a specification**

   ```bash
   specman spec new --name workspace-lifecycle --version 1.0.0 --dependencies ../spec/specman-core/spec.md
   ```

   This renders `spec/workspace-lifecycle/spec.md` from the specification template, enforcing front-matter fields from the [SpecMan Data Model](../spec/specman-data-model/spec.md).
2. **Create an implementation**

   ```bash
   specman impl new --spec spec/workspace-lifecycle/spec.md --name workspace-lifecycle-rust --language rust@1.91.0
   ```

   The CLI resolves template tokens, verifies naming rules, and persists the output under `impl/`.
3. **Track changes with scratch pads**

   ```bash
   specman scratch new --name lifecycle-telemetry --target spec/workspace-lifecycle/spec.md --type revision
   ```

   Scratch pads live under `.specman/scratchpad/` and must include front matter describing target, branch, and work type. The CLI currently supports `--type feat|ref|revision`; the data model also allows `draft`/`fix` for other generators.

Each command honors `--workspace <path>` overrides, emits deterministic stdout/stderr, and exits using `sysexits` codes so scripts or CI pipelines can react programmatically.

## 5. Validate the Workspace

Run `specman status` to parse every specification and implementation, build the dependency tree, and surface missing references or cycles before changes land:

```bash
specman status
```

Expect `EX_OK` on success or `EX_DATAERR` with actionable diagnostics when something violates the rules.

## 6. Inspect Dependency Trees

Use the read-only `dependencies` subcommands to visualize upstream or downstream relationships without editing artifacts. Each command accepts the artifact slug (folder name) and honors mutually exclusive `--downstream|--upstream|--all` flags, defaulting to downstream when no flag is provided. Include `--json` to mirror the same `DependencyTree` payloads emitted by `specman status`.

```bash
# Downstream-only view for a specification
specman spec dependencies specman-cli --downstream

# Upstream dependencies for an implementation
specman impl dependencies specman-cli-rust --upstream

# Combined upstream/downstream view for a scratch pad
specman scratch dependencies cli-deps-subcmd --all
```

The CLI validates slugs against workspace contents, reuses SpecMan Core's dependency mapper, and exits with `EX_USAGE` when multiple direction flags are combined.

## 7. Keep Automation Aligned

- **Templates stay authoritative:** Never edit a template's HTML comments unless the directive has been satisfied. They act as guardrails for AI systems and humans alike.
- **Use scratch pads for real work:** They are not diary entries—they capture the analysis, questions, and tasks needed to modify a spec or implementation. Delete them only when downstream pads no longer depend on them.
- **Reject bureaucracy:** If a task has nothing to do with defining or implementing behavior, it does not belong in SpecMan. The toolchain exists to replace Speckit's meandering prompt soup with clear, testable artifacts.

## Next Steps

- Dive into the [About SpecMan](./about.md) document for the philosophical backdrop.
- Review the specifications: [Data Model](../spec/specman-data-model/spec.md), [Core](../spec/specman-core/spec.md), and [CLI](../spec/specman-cli/spec.md).
- Explore the concrete implementations under `impl/` to see how the Rust crates map to each specification.
