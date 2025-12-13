# SpecMan CLI

The SpecMan CLI exposes the SpecMan runtime features (dependency mapping, templating, lifecycle automation) through a command-line binary for managing SpecMan workspaces.

## Features

- Workspace bootstrapping and template hydration
- Front matter validation and normalization
- Dependency inspection and graph queries
- Lifecycle helpers for create/delete flows (via the `specman` façade)

## Installation

Install via Cargo:

```bash
cargo install specman-cli
```

## Usage

For the authoritative list of commands, semantics, and exit-code guarantees, see the CLI specification:

<https://github.com/justinbrick/specman/blob/main/spec/specman-cli/spec.md>

You can also run `specman --help` to list available commands and `specman <command> --help` for per-command usage.

## Repository

Development and roadmap updates live in the main SpecMan repository:

<https://github.com/justinbrick/specman>
