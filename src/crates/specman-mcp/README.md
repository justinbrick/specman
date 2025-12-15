# SpecMan MCP Server

`specman-mcp` exposes a subset of SpecMan capabilities over the Model Context Protocol (MCP), using a stdio transport.

## What It Provides

- **Tools**
  - `create_artifact` — create a specification, implementation, or scratch pad from a `CreateRequest`
- **Prompts**
  - `feat`, `ref`, `revision`, `fix` — generate deterministic scratch-pad prompts from the embedded templates

## Running

Install from crates.io:

```bash
cargo install specman-mcp
```

Run the server:

```bash
specman-mcp
```

Dev/testing from a repository checkout:

```bash
cd src
cargo run -p specman-mcp --bin specman-mcp
```

This process speaks MCP over stdio; run it under an MCP-capable host.

## Notes

- Prompt outputs are tested for determinism (stable example values and ordering).
- Lifecycle mutations are intentionally limited today; the CLI remains the primary interface for create/delete workflows.
