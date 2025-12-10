# Suggested Commands for Development

From repository root (recommended):

- Build workspace:

```bash
cd src
cargo build
```

- Run tests for the core crate:

```bash
cd src
cargo test -p specman
```

- Install the CLI (produces `specman` on `$PATH`):

```bash
cd src
cargo install --path crates/specman-cli
```

- Formatting & linting (CI gates):

```bash
cd src
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

- Run the CLI (after install or from `target/debug`):

```bash
specman status
```

- Workspace / template bootstrap (documented in `docs/getting-started.md`): create a `.specman/` folder, copy template pointers under `templates/`, then use `specman template` commands to manage pointers.

- CI recommendations: run `cargo fmt`, `cargo clippy`, `cargo test` for changed crates; respect deterministic output and `sysexits` exit codes for actionable failures.
