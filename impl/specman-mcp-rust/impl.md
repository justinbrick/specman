---
spec: ../../spec/specman-mcp/spec.md
name: specman-mcp-rust
version: "0.1.0"
location: ../../src/crates/specman-mcp
library:
  name: specman-mcp-server@0.1.0
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
  libraries:
    - name: rmcp@latest
    - name: specman-library@0.1.0
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

Source code resides (or will reside) under `src/crates/specman-mcp`, a library/binary crate that binds `rmcp` session handling to SpecMan Core services from `specman-library`. The crate exposes a STDIN-focused entry point (for example, `main.rs` wiring `run_stdio_server`) plus modules for capability registration, tool dispatch, and schema publishing.

### Libraries

- `rmcp@latest` — MCP server runtime used for STDIN lifecycle, tool/resource/prompt registration, streaming responses, and notifications.
- `specman-library@0.1.0` — shared SpecMan Core implementation supplying workspace discovery, dependency mapping, lifecycle automation, metadata mutation, and schema derivation.
- `schemars` and `serde_json` — generate and serialize JSON Schemas for MCP tool parameters and outputs tied to SpecMan Data Model entities.

## Concept & Entity Breakdown

### Concept: [MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance)

The adapter runs as a STDIN/STDOUT MCP server using `rmcp`, advertising supported MCP protocol versions and honoring initialization, shutdown, and keep-alive flows. Tool schemas mirror SpecMan Data Model entities; lifecycle hooks surface errors as MCP-compliant payloads without losing SpecMan error codes or references.

#### API Signatures — Transport

```rust
pub async fn run_stdio_server(supported_versions: &[Version]) -> Result<(), McpError>;

pub async fn initialize_session(
    transport: &mut McpTransport,
    workspace: Arc<dyn WorkspaceLocator>,
    capabilities: Vec<SpecManCapabilityDescriptor>,
) -> Result<MCPWorkspaceSession, McpError>;
```

- `run_stdio_server` wires `rmcp` STDIN transport, advertises protocol versions, and blocks until shutdown. Invariants: only STDIN transport; version negotiation must succeed before tool exposure.
- `initialize_session` performs MCP handshake, binds a workspace (via SpecMan Core discovery), and returns session state used by tool handlers; errors propagate as MCP initialization failures with SpecMan references.

### Concept: [SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity)

Every SpecMan Core capability is exposed as an MCP tool with stable identifiers (`specman.core.<concept_snake_case>`). Tool handlers delegate to `specman-library` functions for workspace discovery, dependency mapping, lifecycle automation, metadata mutation, template orchestration, and prompt catalog access. Optional/experimental capabilities are labeled with their source spec/implementation.

#### API Signatures — Capability Parity

```rust
pub fn capability_catalog(core: &SpecmanCore) -> Vec<SpecManCapabilityDescriptor>;

pub async fn handle_tool_call(
    session: &MCPWorkspaceSession,
    request: ToolCallRequest,
) -> Result<OperationEnvelope, McpError>;
```

- `capability_catalog` assembles descriptors for all SpecMan Core concepts, including JSON Schemas derived from the data model.
- `handle_tool_call` dispatches to SpecMan Core implementations, enforcing concept-specific invariants and returning an `OperationEnvelope` that records results, errors, and artifact paths.

### Concept: [Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance)

All filesystem access flows through SpecMan workspace discovery, and resource handles (`spec://`, `impl://`, `scratch://`) are normalized before use. Dependency graph queries and `/dependencies` handles are read-only and return SpecMan Data Model representations. Mutating operations reuse lifecycle automation with dependency checks.

#### API Signatures — Governance

```rust
pub fn resolve_handle(handle: &str, workspace: &WorkspacePaths) -> Result<ArtifactId, McpError>;

pub async fn describe_dependencies(
    mapper: &impl DependencyMapping,
    root: &ArtifactId,
) -> Result<DependencyTree, McpError>;
```

- `resolve_handle` rejects workspace escapes and normalizes resource handles to canonical artifact identifiers.
- `describe_dependencies` returns upstream/downstream trees; mutation handlers must consult this before writes and propagate errors when blocking dependents exist.

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
