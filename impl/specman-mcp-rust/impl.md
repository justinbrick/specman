---
spec: ../../spec/specman-mcp/spec.md
name: specman-mcp-rust
version: 2.0.0
location: ../../src/crates/specman-mcp
references:
- ref: ../specman-library/impl.md
  type: implementation
  optional: false
---

# Implementation — SpecMan MCP Rust Adapter

## Overview

This adapter implements the [SpecMan MCP Server](../../spec/specman-mcp/spec.md) by projecting SpecMan Core capabilities into MCP tools over a STDIN transport. The runtime uses the `rmcp` crate for lifecycle negotiation and framing, delegates capability logic to the shared `specman-library`, and preserves data-model fidelity for every request and response. Version negotiation and tool schemas adhere to [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance), while capability mapping aligns with [Concept: SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity) and [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace-data-governance). The binary accepts an optional `--workspace <path>` argument to pin workspace discovery to a specific root; when omitted it defaults to the current working directory.

## Implementation Stack

The adapter is implemented in **Rust (2024 Edition)** (1.91.0) and reuses the `specman-library`.

- **`rmcp@latest`**: MCP server primitives.
- **`schemars`**: JSON Schema generation for tool parameters.
- **`serde_json`**: Serialization mandated by SpecMan Data Model.

## References

- [spec/specman-core/spec.md](../../spec/specman-core/spec.md) — governs dependency mapping, lifecycle automation, and metadata mutation behaviors that this adapter exposes via MCP tools.
- [spec/specman-core/spec.md](../../spec/specman-core/spec.md) — defines artifact identifiers, workspace rules, and schema invariants mirrored in MCP tool input/output payloads.
- [impl/specman-library/impl.md](../specman-library/impl.md) — reused Rust crate providing workspace discovery, dependency traversal, lifecycle, and schema derivation.
- [MCP architecture overview](https://modelcontextprotocol.io/docs/learn/architecture) — external MCP guidance for initialization, tool/resource primitives, streaming, and notifications.

## Implementation Details

### Code Location

Source code resides under `src/crates/specman-mcp`.

- `src/lib.rs` defines `SpecmanMcpServer` (the MCP handler) plus `run_stdio_server()`.
- `src/bin/specman-mcp.rs` is the binary entry point that runs the server over stdio.

### Libraries

- `rmcp@latest` — MCP server runtime used for STDIN lifecycle, tool/resource/prompt registration, streaming responses, and notifications.
- `specman-library@2.1.1` — shared SpecMan Core implementation supplying workspace discovery, dependency mapping, lifecycle automation, metadata mutation, and schema derivation.
- `schemars` and `serde_json` — generate and serialize JSON Schemas for MCP tool parameters and outputs tied to SpecMan Data Model entities.

## Concept & Entity Breakdown

### Concept: [MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance)

The adapter runs as a STDIN/STDOUT MCP server using `rmcp`, advertising supported MCP protocol versions and honoring initialization, shutdown, and keep-alive flows. Tool schemas mirror SpecMan Data Model entities; lifecycle hooks surface errors as MCP-compliant payloads without losing SpecMan error codes or references.

#### API Signatures — Transport

```rust
pub async fn run_stdio_server() -> Result<(), ServerInitializeError>;
pub async fn run_stdio_server_with_root(
  workspace_root: Option<PathBuf>,
) -> Result<(), ServerInitializeError>;

impl SpecmanMcpServer {
  pub fn new() -> Result<Self, SpecmanError>;
  pub fn new_with_root(root: impl Into<PathBuf>) -> Result<Self, SpecmanError>;
  pub async fn run_stdio(self) -> Result<(), ServerInitializeError>;
}
```

- `run_stdio_server_with_root` accepts an optional workspace root (defaulting to the current working directory), builds the handler, serves it over `rmcp`’s stdio transport, and blocks until the peer closes the transport.
- `run_stdio_server` is a convenience wrapper that defaults to the current working directory.
- `new_with_root` allows hosts/tests to pin workspace discovery to a specific directory.

### Concept: [SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity)

The adapter exposes a focused subset of SpecMan functionality as MCP tools/prompts, prioritizing deterministic prompt generation and safe, workspace-bound mutations.

#### API Signatures — Capability Parity

```rust
// Tools and prompts are registered via rmcp's `#[tool_router]` / `#[prompt_router]` macros.
// Each handler is a method on `SpecmanMcpServer` annotated with `#[tool(...)]` or `#[prompt(...)]`.
```

- Current tool surface (12 tools): `create_specification`, `create_implementation`, `create_revision`, `create_feature`, `create_refactor`, `create_fix`, `update_specification`, `update_implementation`, `update_revision`, `update_feature`, `update_refactor`, `update_fix`.
- All tools use flat input structs with `#[derive(JsonSchema)]` — no handcrafted schemas, no tagged unions.
- MCP sampling and elicitation are NOT used; all inputs are supplied directly by the caller.
- Current prompt surface: `feat`, `ref`, `revision`, `fix`, `spec`, `impl`, `migration`, `compliance`.
  - Prompts are identifier-driven: each prompt accepts exactly one required argument identifying the target artifact (a `target`/`spec`/`implementation` locator) or, for `spec`, the name of the new specification to create, and the rendered prompt instructs the agent to immediately query the user for what they want to accomplish rather than accepting free-text intent as an argument. There is no trailing free-form user-input section; the agent keeps querying the user until the request is unambiguous before running lifecycle tool calls.
  - `migration` renders deterministic guidance to create the target specification via lifecycle automation, then create a revision scratch pad for that spec before running the four mandated migration phases (enumerate sources, extract findings, draft/update specification, generate implementation documentation).
  - `compliance` instructs the agent to retrieve `impl://{artifact}/compliance` and handle any missing constraints.

### Concept: [Compliance Resources](../../spec/specman-mcp/spec.md#concept-compliance-resources)

The adapter exposes compliance reports for implementations, leveraging `specman-library` validation.

- Resource: `impl://{artifact}/compliance`
- Content: JSON-serialized `ComplianceReport` struct (coverage, missing requirements, orphans).
- Error handling: Returns specific errors if the implementation has no upstream specification or multiple (ambiguous) specifications.

### Concept: [Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace-data-governance)

All filesystem access flows through SpecMan workspace discovery, and resource handles (`spec://`, `impl://`, `scratch://`) are normalized before use. Dependency graph queries and `/dependencies` handles are read-only and return SpecMan Data Model representations. Mutating operations reuse lifecycle automation with dependency checks.

#### API Signatures — Governance

```rust
fn artifact_path(id: &ArtifactId, workspace: &WorkspacePaths) -> PathBuf;
fn artifact_handle(summary: &ArtifactSummary) -> String;

// Create tools — each delegates to SpecMan Core lifecycle automation.
async fn create_specification(Parameters(args): Parameters<CreateSpecificationArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;
async fn create_implementation(Parameters(args): Parameters<CreateImplementationArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;
async fn create_revision(Parameters(args): Parameters<CreateScratchPadArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;
async fn create_feature(Parameters(args): Parameters<CreateScratchPadArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;
async fn create_refactor(Parameters(args): Parameters<CreateScratchPadArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;
async fn create_fix(Parameters(args): Parameters<CreateScratchPadArgs>)
  -> Result<Json<CreateArtifactResult>, McpError>;

// Update tools — each delegates to SpecMan Core metadata mutation.
// Update tools mirror the create tool surface (6 create + 6 update).
async fn update_specification(Parameters(args): Parameters<UpdateSpecificationArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;
async fn update_implementation(Parameters(args): Parameters<UpdateImplementationArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;
async fn update_revision(Parameters(args): Parameters<UpdateScratchPadArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;
async fn update_feature(Parameters(args): Parameters<UpdateScratchPadArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;
async fn update_refactor(Parameters(args): Parameters<UpdateScratchPadArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;
async fn update_fix(Parameters(args): Parameters<UpdateScratchPadArgs>)
  -> Result<Json<UpdateArtifactResult>, McpError>;

// All input types use flat structs with #[derive(JsonSchema)].
// No handcrafted schemas, no tagged unions, no sampling/elicitation.

// Semantics:
// - Only YAML front matter changes; the Markdown body is preserved byte-for-byte.
// - Scratch pad `target` is immutable; scratch update tools do not accept `target`.
// - HTTPS locators are preview-only for spec/impl updates; scratch updates reject HTTPS.
```

- Handles use the `spec://`, `impl://`, and `scratch://` schemes and are always emitted in normalized form.
- Scratch pad `target` locators are normalized to canonical workspace-relative paths before persisting.
- Implementation targets are normalized to canonical `spec://...` handles before persisting.
- Paths returned to MCP clients are canonical workspace-relative paths and never allow escaping outside the discovered root.
- For `create_artifact` implementation targets, the adapter normalizes the input into a canonical `spec://...` handle before persisting so dependency resolution is base-path independent.
- Paths returned to MCP clients are canonical workspace-relative paths and never allow escaping outside the discovered root.

### Concept: [Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety-deterministic-execution)

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

pub fn audit_event(session: &MCPWorkspaceSession, capability_id: &str, artifacts: &[ArtifactId]);
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

## Operational Notes

- Build/run: `cargo build -p specman-mcp` (once the crate exists). Run the STDIN server via `cargo run -p specman-mcp` to start the `rmcp` stdio transport.
- Transport: Only STDIN/STDOUT transport is supported per [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance); advertise supported MCP versions and honor negotiation before exposing tools. No additional MCP primitives beyond tools/resources/prompts are exposed at this stage.
- Integration: All tool handlers call into `specman-library` for workspace discovery, dependency mapping, lifecycle automation, and metadata mutation, preserving SpecMan Core invariants.
- Observability: Emit structured telemetry (capability id, workspace root, artifact paths, durations) for each tool invocation. Logging should note conflict handling and dependency checks.
- Concurrency: Use per-artifact locks to serialize mutating operations; read-only operations can proceed concurrently but still validate workspace resolution.
