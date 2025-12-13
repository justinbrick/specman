---
spec: ../../spec/specman-mcp/spec.md
name: specman-mcp-rust
version: "0.2.0"
location: ../../src/crates/specman-mcp
library:
  name: specman-mcp-server@0.2.0
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
  libraries:
    - name: rmcp@latest
    - name: specman-library@2.0.0
    - name: schemars
    - name: serde_json
secondary_languages: []
references:
  - ref: ../../spec/specman-core/spec.md
    type: specification
    optional: false
  - ref: ../../spec/specman-data-model/spec.md
    type: specification
    optional: false
  - ref: ../../spec/specman-templates/spec.md
    type: specification
    optional: false
  - ref: ../specman-library/impl.md
    type: implementation
    optional: false
  - ref: https://modelcontextprotocol.io/docs/learn/architecture
    type: specification
    optional: false
---

# Implementation — SpecMan MCP Rust Adapter

## Overview

This adapter implements the [SpecMan MCP Server](../../spec/specman-mcp/spec.md) by projecting SpecMan Core capabilities into MCP tools over a STDIN transport. The runtime uses the `rmcp` crate for lifecycle negotiation and framing, delegates capability logic to the shared `specman-library`, and preserves data-model fidelity for every request and response. Version negotiation and tool schemas adhere to [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance), while capability mapping aligns with [Concept: SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity) and [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance).

## Implementing Languages

- Primary — `rust@1.91.0` (edition 2024): selected for parity with existing SpecMan crates and to reuse the `specman-library`. `rmcp` supplies MCP server primitives, while `schemars` and `serde_json` surface JSON Schemas and serialization mandated by [SpecMan Data Model](../../spec/specman-data-model/spec.md#implementations). Secondary languages: none.

## References

- [spec/specman-core/spec.md](../../spec/specman-core/spec.md) — governs dependency mapping, lifecycle automation, and metadata mutation behaviors that this adapter exposes via MCP tools.
- [spec/specman-data-model/spec.md](../../spec/specman-data-model/spec.md) — defines artifact identifiers, workspace rules, and schema invariants mirrored in MCP tool input/output payloads.
- [spec/specman-templates/spec.md](../../spec/specman-templates/spec.md) — informs prompt catalog exposure and template pointer handling referenced by lifecycle tools.
- [impl/specman-library/impl.md](../specman-library/impl.md) — reused Rust crate providing workspace discovery, dependency traversal, lifecycle, and schema derivation.
- [MCP architecture overview](https://modelcontextprotocol.io/docs/learn/architecture) — external MCP guidance for initialization, tool/resource primitives, streaming, and notifications.

## Implementation Details

### Code Location

Source code resides under `src/crates/specman-mcp`.

- `src/lib.rs` defines `SpecmanMcpServer` (the MCP handler) plus `run_stdio_server()`.
- `src/bin/specman-mcp.rs` is the binary entry point that runs the server over stdio.

### Libraries

- `rmcp@latest` — MCP server runtime used for STDIN lifecycle, tool/resource/prompt registration, streaming responses, and notifications.
- `specman-library@2.0.0` — shared SpecMan Core implementation supplying workspace discovery, dependency mapping, lifecycle automation, metadata mutation, and schema derivation.
- `schemars` and `serde_json` — generate and serialize JSON Schemas for MCP tool parameters and outputs tied to SpecMan Data Model entities.

## Concept & Entity Breakdown

### Concept: [MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance)

The adapter runs as a STDIN/STDOUT MCP server using `rmcp`, advertising supported MCP protocol versions and honoring initialization, shutdown, and keep-alive flows. Tool schemas mirror SpecMan Data Model entities; lifecycle hooks surface errors as MCP-compliant payloads without losing SpecMan error codes or references.

#### API Signatures — Transport

```rust
pub async fn run_stdio_server() -> Result<(), ServerInitializeError>;

impl SpecmanMcpServer {
  pub fn new() -> Result<Self, SpecmanError>;
  pub fn new_with_root(root: impl Into<PathBuf>) -> Result<Self, SpecmanError>;
  pub async fn run_stdio(self) -> Result<(), ServerInitializeError>;
}
```

- `run_stdio_server` builds the handler, serves it over `rmcp`’s stdio transport, and blocks until the peer closes the transport.
- `new_with_root` allows hosts/tests to pin workspace discovery to a specific directory.

### Concept: [SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity)

The adapter exposes a focused subset of SpecMan functionality as MCP tools/prompts, prioritizing safe inspection (workspace discovery/inventory) and deterministic prompt generation.

#### API Signatures — Capability Parity

```rust
// Tools and prompts are registered via rmcp's `#[tool_router]` / `#[prompt_router]` macros.
// Each handler is a method on `SpecmanMcpServer` annotated with `#[tool(...)]` or `#[prompt(...)]`.
```

- Current tool surface: `workspace_discovery`, `workspace_inventory`.
- Current prompt surface: `feat`, `ref`, `revision`, `fix` (scratch pad prompt templates).

### Concept: [Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance)

All filesystem access flows through SpecMan workspace discovery, and resource handles (`spec://`, `impl://`, `scratch://`) are normalized before use. Dependency graph queries and `/dependencies` handles are read-only and return SpecMan Data Model representations. Mutating operations reuse lifecycle automation with dependency checks.

#### API Signatures — Governance

```rust
fn artifact_path(id: &ArtifactId, workspace: &WorkspacePaths) -> PathBuf;
fn artifact_handle(summary: &ArtifactSummary) -> String;
```

- Handles use the `spec://`, `impl://`, and `scratch://` schemes and are always emitted in normalized form.
- Paths returned to MCP clients are workspace-resolved and never allow escaping outside the discovered root.

### Concept: [Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution)

Sessions bind to single workspaces, maintain locks for mutating operations, and stream progress via MCP notifications. Conflicts serialize operations or fail fast with actionable errors aligned to SpecMan Core deterministic execution rules.

#### API Signatures — Session Safety

```rust
pub async fn with_session_lock<F, T>(
    session: &MCPWorkspaceSession,
    target: &ArtifactId,
    op: F,
) -> Result<T, McpError>
where
    F: FnOnce() -> Result<T, McpError>;

pub fn audit_event(session: &MCPWorkspaceSession, envelope: &OperationEnvelope);
```

- `with_session_lock` enforces single-writer semantics per artifact; rejects conflicting calls.
- `audit_event` records structured telemetry (capability id, artifacts, durations) for replay and provenance.

### Entity: [MCPWorkspaceSession](../../spec/specman-mcp/spec.md#entity-mcpworkspacesession)

Tracks a negotiated MCP session bound to a workspace, including protocol version, principal metadata, and active locks. Telemetry hooks emit structured logs for lifecycle transitions.

#### API Signatures — MCPWorkspaceSession

```rust
pub fn session_identity(&self) -> &SessionIdentity;

pub fn register_lock(&self, artifact: &ArtifactId) -> Result<(), McpError>;
```

#### Data Model — MCPWorkspaceSession

```rust
pub struct MCPWorkspaceSession {
    pub protocol_version: Version,
    pub principal: Principal,
    pub workspace: WorkspacePaths,
    pub active_tools: BTreeSet<String>,
    pub locks: Mutex<BTreeSet<ArtifactId>>,
    pub telemetry: TelemetrySink,
}
```

- Invariants: `workspace` comes from workspace discovery; locks guard mutating operations; telemetry sink must not drop lifecycle events.

### Entity: [SpecManCapabilityDescriptor](../../spec/specman-mcp/spec.md#entity-specmancapabilitydescriptor)

Defines MCP tool metadata for each SpecMan Core concept. Includes concept reference, supported SpecMan Core version range, and JSON Schemas for inputs/outputs; extension metadata marked `type: extension` cites owning spec/implementation.

#### API Signatures — SpecManCapabilityDescriptor

```rust
pub fn descriptor_for(concept: &str, schema: Schema) -> SpecManCapabilityDescriptor;
```

#### Data Model — SpecManCapabilityDescriptor

```rust
pub struct SpecManCapabilityDescriptor {
    pub id: String,
    pub concept_ref: String,
    pub core_versions: VersionRange,
    pub input_schema: Schema,
    pub output_schema: Schema,
    pub extensions: Vec<ExtensionMetadata>,
}
```

- Invariants: `id` uses `specman.core.<concept_snake_case>`; schemas align with SpecMan Data Model entities; extensions clearly labeled.

### Entity: [OperationEnvelope](../../spec/specman-mcp/spec.md#entity-operationenvelope)

Encapsulates a SpecMan action executed via MCP, capturing capability id, sanitized inputs, execution timestamps, artifacts, streamed messages, and final status for provenance.

#### API Signatures — OperationEnvelope

```rust
pub fn record_operation(
    descriptor: &SpecManCapabilityDescriptor,
    inputs: serde_json::Value,
    result: serde_json::Value,
    artifacts: Vec<ArtifactId>,
    logs: Vec<LogEntry>,
) -> OperationEnvelope;
```

#### Data Model — OperationEnvelope

```rust
pub struct OperationEnvelope {
    pub capability_id: String,
    pub concept_ref: String,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
    pub artifacts: Vec<ArtifactId>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub transcript: Vec<LogEntry>,
    pub status: OperationStatus,
}
```

- Invariants: inputs are sanitized and schema-validated; transcript preserves ordering; status encodes success, partial, or error with SpecMan error codes retained.

## Operational Notes

- Build/run: `cargo build -p specman-mcp` (once the crate exists). Run the STDIN server via `cargo run -p specman-mcp` to start the `rmcp` stdio transport.
- Transport: Only STDIN/STDOUT transport is supported per [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance); advertise supported MCP versions and honor negotiation before exposing tools. No additional MCP primitives beyond tools/resources/prompts are exposed at this stage.
- Integration: All tool handlers call into `specman-library` for workspace discovery, dependency mapping, lifecycle automation, and metadata mutation, preserving SpecMan Core invariants.
- Observability: Emit structured telemetry (capability id, workspace root, artifact paths, durations) for each `OperationEnvelope`. Logging should note conflict handling and dependency checks.
- Concurrency: Use per-artifact locks to serialize mutating operations; read-only operations can proceed concurrently but still validate workspace resolution.
