---
spec: ../../spec/specman-mcp/spec.md
name: specman-mcp-rust
version: "0.1.0"
location: ../../src/crates/specman-mcp
library:
  name: specman-mcp-adapter@0.1.0
primary_language:
  language: rust@1.91.0
  properties:
    edition: "2024"
    toolchain: stable
  libraries:
    - name: rmcp@0.11.0
    - name: tokio@1
    - name: schemars@0.8
    - name: serde_json@1
    - name: tracing@0.1
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
---

# Implementation — SpecMan MCP Rust Adapter

## Overview

The `specman-mcp-rust` implementation provides the MCP server facade required by [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance) while delegating workspace intelligence to the existing `specman` library documented in [impl/specman-library/impl.md](../specman-library/impl.md). The adapter boots as a STDIN/STDOUT rmcp server, advertises the SpecMan capability catalog, and ensures every tool mirrors the semantics defined in [Concept: SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity). Request payloads and responses reuse the data structures backed by [SpecMan Data Model](../../spec/specman-data-model/spec.md), and all lifecycle actions route through the SpecMan Core controllers described in [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation). Operationally, the adapter emits audit-friendly transcripts, honors the locking guidance from [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution), and exposes derived resource handles that align with [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance).

This document follows the structure mandated by [templates/impl/impl.md](../../templates/impl/impl.md) and explicitly links every section back to the [SpecMan MCP concepts and entities](../../spec/specman-mcp/spec.md#concepts). Downstream tooling can therefore rely on these anchors to build precise traceability graphs between implementation code and the governing specification.

## Implementing Languages

- **Primary — `rust@1.91.0` (edition 2024, stable toolchain):** Rust matches the MSRV already established by the SpecMan workspace and allows the adapter to share crates with `specman-library`. Edition 2024 language features (e.g., `if let` chains, `impl Trait` returns) simplify async rmcp handlers without raising the MSRV beyond what the repo already uses.
    - **`rmcp@0.11.0`** supplies the STDIN/STDOUT Model Context Protocol runtime, giving the adapter a transport with built-in version negotiation and schema publishing that satisfies [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance). “Transport profile” refers to the rmcp transport binding we instantiate—in this case `rmcp::transport::StdioTransport`, which never opens network sockets.
  - **`tokio@1`** powers the async tasks that multiplex MCP tool executions while still confining the process to local STDIN pipes per the specification.
  - **`schemars@0.8`** derives JSON Schema fragments directly from SpecMan Data Model structs so the adapter can embed deterministic schemas inside `SpecManCapabilityDescriptor` instances.
  - **`serde_json@1`** serializes `OperationEnvelope` payloads, streamed deltas, and cached transcripts without bespoke encoders.
  - **`tracing@0.1`** emits the structured lifecycle logs mandated by [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution).

## References

- [spec/specman-core/spec.md](../../spec/specman-core/spec.md) defines the capability contract (workspace discovery, dependency mapping, lifecycle automation, template orchestration, metadata mutation) that every MCP tool must mirror, so the adapter wraps the `specman-library` APIs rather than re-implementing them.
- [spec/specman-data-model/spec.md](../../spec/specman-data-model/spec.md) governs entity serialization for workspaces, dependency graphs, and lifecycle artifacts; all MCP schemas embed these structures to prevent drift.
- [spec/specman-templates/spec.md](../../spec/specman-templates/spec.md) informs the prompt-catalog tools that surface template locators and HTML directive reminders to MCP clients.
- [impl/specman-library/impl.md](../specman-library/impl.md) documents the shared Rust crate that executes SpecMan Core behaviors; the MCP adapter links against this implementation to satisfy [Concept: SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity) without duplicating logic.

## Implementation Details

### Code Location

Source code for the adapter lives under `src/crates/specman-mcp`:

- `src/lib.rs` exports the `SpecmanMcpServer` entry point plus error types shared across handlers.
- `src/server.rs` wires `rmcp::ServerBuilder` to STDIN/STDOUT transports, negotiates protocol versions, and initializes telemetry sinks referenced in [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution).
- `src/capabilities/mod.rs` hosts the `CapabilityRegistry`, mapping SpecMan Core concept identifiers to rmcp tool descriptors and delegating execution to `specman_library` services.
- `src/session.rs` defines `MCPWorkspaceSession`, lock guards, and helper functions that ensure resource handles are normalized before any filesystem operation per [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance).
- `src/telemetry.rs` captures `OperationEnvelope` records, streaming stats, and structured logs that flow back through rmcp notifications.
- `src/prompts.rs` exposes the prompt-catalog tools that resolve template pointer files via `specman_library::templates` and cite [SpecMan Templates](../../spec/specman-templates/spec.md).

### Libraries

- **rmcp@0.11.0:** Provides MCP server runtime, schema registry, and STDIN-compatible transport. The adapter registers all SpecMan tools with deterministic schemas derived from SpecMan Data Model entities.
- **tokio@1:** Supplies the async executor required by rmcp’s streaming API; tasks remain bounded and never spawn network listeners, aligning with the local-only constraint.
- **schemars@0.8:** Generates JSON Schemas for tool inputs/outputs so rmcp clients can validate requests inline.
- **serde_json@1:** Translates SpecMan entities into MCP-friendly JSON payloads without custom encoders.
- **tracing@0.1:** Emits structured logs tagged with workspace root, tool id, and MCP session id, satisfying audit requirements.

## Concept & Entity Breakdown

### Concept: [MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance)

`SpecmanMcpServer` embeds the rmcp STDIN runtime and exposes lifecycle hooks (`initialize`, `shutdown`, `ping`) that forward to SpecMan Core services. Version negotiation relies on rmcp’s handshake but constrains the accepted list to MCP releases vetted against SpecMan payloads. Every tool registers deterministic JSON Schemas derived from SpecMan Data Model structs, and errors raised by SpecMan Core are wrapped in rmcp error frames that preserve the originating concept reference.

#### API Signatures

```rust
pub async fn run(server: SpecmanMcpServer) -> Result<(), AdapterError>;

impl SpecmanMcpServer {
    pub fn new(
        transport: rmcp::StdioTransport,
        registry: Arc<CapabilityRegistry>,
        services: SpecmanServices,
    ) -> Self;

    async fn handle_lifecycle(
        &self,
        event: rmcp::LifecycleEvent,
    ) -> Result<(), AdapterError>;
}
```

- `run` boots the rmcp runtime, advertises supported protocol versions, and registers tool schemas before accepting requests. Failure to negotiate a compatible version returns an MCP error referencing [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance).
- `handle_lifecycle` responds to initialize/shutdown/keep-alive messages and forwards telemetry into the session manager so deterministic execution remains observable.

### Concept: [SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity)

Each SpecMan Core concept is exposed as a tool named `specman.core.<concept_snake_case>` and references the governing heading in `metadata.concept_ref`. Implementation modules convert rmcp invocations into `specman_library` calls, ensuring workspace discovery, dependency graphs, template orchestration, lifecycle automation, and metadata mutation produce identical results to other SpecMan Core consumers (for example, the CLI). The initial release intentionally limits the catalog to specification-mandated concepts; extension tools will be added only when a governing specification requires them.

#### API Signatures

```rust
impl CapabilityRegistry {
    pub fn register_core_capabilities(&mut self, services: &SpecmanServices);

    pub async fn execute(
        &self,
        descriptor_id: &str,
        session: &mut MCPWorkspaceSession,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, AdapterError>;
}
```

- `register_core_capabilities` wires SpecMan Core concepts to rmcp descriptors, attaching JSON Schemas from SpecMan Data Model entities so the registry can reject malformed payloads up front.
- `execute` looks up the descriptor, enforces capability prerequisites (workspace locks, template availability), and returns serialized outputs, guaranteeing parity with the equivalent `specman_library` call.

### Concept: [Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance)

`WorkspaceSessionGuard` ensures every MCP request resolves resource handles through SpecMan Core workspace discovery. Paths that fall outside the detected root or attempt to mutate derived `/dependencies` handles are rejected with descriptive MCP errors. All responses embed source references (artifact paths, template locators) so downstream tools can trace results back to workspace files, satisfying the data-governance requirements of both the SpecMan MCP spec and [Concept: Dependency Mapping Services](../../spec/specman-core/spec.md#concept-dependency-mapping-services).

#### API Signatures

```rust
pub struct WorkspaceSessionGuard {
    locator: Arc<FilesystemWorkspaceLocator>,
}

impl WorkspaceSessionGuard {
    pub fn normalize_handle(
        &self,
        handle: &str,
    ) -> Result<NormalizedHandle, AdapterError>;

    pub fn enforce_mutation_scope(
        &self,
        target: &ArtifactId,
    ) -> Result<(), AdapterError>;
}
```

- `normalize_handle` translates `spec://`, `impl://`, and `scratch://` handles into canonical filesystem paths using `specman_library` utilities. Invalid handles cite [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance).
- `enforce_mutation_scope` verifies that mutating operations originate within the workspace root and rejects attempts against derived `/dependencies` handles.

### Concept: [Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution)

`SessionManager` tracks active MCP sessions, locks resources before invoking lifecycle operations, and emits structured telemetry for every tool call. Concurrent mutations against the same artifact cause either serialized execution (lock acquisition) or rmcp errors that instruct clients to retry later. Heartbeats and progress notifications stream through rmcp channels so long-running operations remain observable.

#### API Signatures

```rust
pub struct SessionManager {
    sessions: DashMap<Uuid, MCPWorkspaceSession>,
}

impl SessionManager {
    pub fn start_session(
        &self,
        init: rmcp::InitializeParams,
    ) -> Result<Uuid, AdapterError>;

    pub fn acquire_lock(
        &self,
        session_id: Uuid,
        artifact: &ArtifactId,
    ) -> Result<LockGuard, AdapterError>;

    pub fn record_progress(
        &self,
        session_id: Uuid,
        message: impl Into<String>,
    );
}
```

- `start_session` negotiates MCP versions, binds the session to a workspace, and records telemetry seeds for auditing.
- `acquire_lock` enforces deterministic execution by serializing conflicting requests; it references [SpecMan Core Deterministic Execution](../../spec/specman-core/spec.md#concept-deterministic-execution) when rejecting overlapping mutations.
- `record_progress` streams heartbeats and progress updates through rmcp’s notification channel, preventing clients from killing the process prematurely.

### Entity: [MCPWorkspaceSession](../../spec/specman-mcp/spec.md#entity-mcpworkspacesession)

Sessions encapsulate the negotiated MCP protocol version, authenticated principal metadata (if provided by rmcp), workspace paths, and lock state. The adapter stores telemetry hooks alongside active tool invocations so structured logs capture the lifecycle transitions mandated by the specification.

#### API Signatures

```rust
impl MCPWorkspaceSession {
    pub fn start(
        protocol_version: String,
        principal: Option<String>,
        workspace: WorkspacePaths,
    ) -> Self;

    pub fn record_tool(&mut self, descriptor_id: &str);

    pub fn finish(&mut self, descriptor_id: &str, outcome: &OperationEnvelope);
}
```

- `start` binds the session to a workspace discovered via SpecMan Core services and stores the negotiated MCP version.
- `record_tool` increments counters used for telemetry and lock tracking.
- `finish` releases locks, captures audit events, and appends the finalized `OperationEnvelope` for later retrieval.

#### Data Model

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MCPWorkspaceSession {
    pub session_id: Uuid,
    pub protocol_version: String,
    pub principal: Option<String>,
    pub workspace: WorkspacePaths,
    pub active_tools: BTreeSet<String>,
    pub locks: BTreeSet<ArtifactId>,
    pub telemetry: SessionTelemetry,
}
```

- `workspace` references the canonical root resolved through [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery).
- `locks` enumerates artifacts currently serialized; emptying the set is required before a session may shut down cleanly.

### Entity: [SpecManCapabilityDescriptor](../../spec/specman-mcp/spec.md#entity-specmancapabilitydescriptor)

Descriptors capture the MCP tool metadata, including links back to the governing SpecMan Core heading, supported version ranges, and JSON Schema fragments that describe inputs/outputs. Optional extensions carry labels so clients recognize non-core features.

#### API Signatures

```rust
impl SpecManCapabilityDescriptor {
    pub fn core(
        id: &str,
        concept_ref: &str,
        spec_version: VersionReq,
        input_schema: Schema,
        output_schema: Schema,
    ) -> Self;

    pub fn with_extension(mut self, extension: CapabilityExtension) -> Self;
}
```

- `core` constructs descriptors for required capabilities, referencing the exact concept heading.
- `with_extension` annotates optional features, citing their owning specification path in `extension.source`.

#### Data Model

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SpecManCapabilityDescriptor {
    pub id: String,
    pub concept_ref: String,
    pub spec_version: VersionReq,
    pub schema_in: Schema,
    pub schema_out: Schema,
    pub extensions: Vec<CapabilityExtension>,
}
```

- `schema_in` and `schema_out` store schemars-generated fragments derived from SpecMan Data Model entities, ensuring clients receive deterministic payload descriptions.
- `extensions` entries MUST set `type: "extension"` and cite the owning specification or implementation path, aligning with the MCP specification’s optional capability guidance.

### Entity: [OperationEnvelope](../../spec/specman-mcp/spec.md#entity-operationenvelope)

Operation envelopes capture sanitized inputs, execution timestamps, resulting artifacts, and streaming transcripts for each MCP tool invocation. They allow clients and operators to audit history or replay deterministic outputs.

#### API Signatures

```rust
impl OperationEnvelope {
    pub fn start(
        capability_id: impl Into<String>,
        inputs: serde_json::Value,
    ) -> Self;

    pub fn record_artifact(&mut self, artifact: ArtifactId, path: PathBuf);

    pub fn finalize(
        &mut self,
        status: OperationStatus,
        transcript: Vec<rmcp::StreamEvent>,
    );

    pub fn into_mcp_response(self) -> rmcp::ToolResponse;
}
```

- `start` stores immutable inputs plus timestamps so envelopes can be correlated with telemetry logs.
- `record_artifact` adds created or mutated artifacts, enabling MCP clients to link outputs back to workspace files.
- `finalize` attaches streamed messages and final status codes before converting the envelope into an rmcp response.

#### Data Model

```rust
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct OperationEnvelope {
    pub capability_id: String,
    pub inputs: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub artifacts: Vec<OperationArtifact>,
    pub transcript: Vec<rmcp::StreamEvent>,
    pub status: OperationStatus,
}
```

- `artifacts` enumerate workspace mutations, satisfying the auditing requirement in [Concept: SpecMan Capability Parity](../../spec/specman-mcp/spec.md#concept-specman-capability-parity).
- `transcript` preserves streamed outputs so MCP clients can replay progress data.

#### Serialization & Storage

- `OperationEnvelope` derives `Serialize`, `Deserialize`, and `JsonSchema`, so every record that leaves the adapter validates against the SpecMan Data Model entities referenced by the governing capability. The schemars output is exported to rmcp’s schema registry, allowing MCP clients to pre-validate responses before attempting to parse them.
- The adapter persists envelopes as newline-delimited JSON (`NDJSON`) under the workspace-owned telemetry file `.specman/logs/specman-mcp.jsonl`, aligning with [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution) and the “no stdout logs” constraint from [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance).
- `OperationStore` appends every finalized envelope atomically by writing to a temporary file and issuing `rename` into place, preventing readers from ever seeing partial JSON. Each entry receives a stable `operation_id` (a UUID stored inside the envelope metadata) so external auditors can correlate rmcp responses with the telemetry record.
- When the log file exceeds 16 MiB or 50,000 envelopes—whichever comes first—the store rotates the artifact into `.specman/logs/specman-mcp.jsonl.{timestamp}` before resuming writes to a fresh file. Rotation metadata (start/end timestamps, record counts) is recorded in a sidecar manifest so CLI tooling can stream or purge logs without parsing every envelope.
- The same `OperationStore` powers the `specman.core.operations.inspect` tool described in [Capability Registry Design](#capability-registry-design); lookups filter by session id, capability id, or time range and stream matching envelopes back over rmcp using the same JSON Schema advertised at descriptor registration time.
- Integration tests serialize mock envelopes, read them back from disk, and assert byte-for-byte determinism to guarantee that the adapter’s append-only strategy remains portable across filesystems.

## Operational Notes

- **Building & Running:** Execute `cargo run -p specman-mcp -- serve` to launch the STDIN server. The process auto-detects the workspace using `specman_library::workspace::FilesystemWorkspaceLocator` and refuses to start when `.specman` is missing, aligning with [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery).
- **Configuration:** Environment variables `SPECMAN_WORKSPACE` (optional override) and `SPECMAN_MCP_PROTOCOLS` (comma-separated MCP version whitelist) tailor workspace selection and version negotiation. Undefined variables default to ancestor discovery plus the MCP versions embedded in the binary.
- **Logging & Telemetry:** Structured logs stream into a workspace-owned file such as `.specman/logs/specman-mcp.jsonl`, keeping stdout/stderr reserved for rmcp protocol frames while still satisfying the audit requirements in [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution). The log path can be overridden via `SPECMAN_MCP_LOG_PATH`.
- **Heartbeats & Progress:** Long-running dependency scans and lifecycle operations emit rmcp `progress` events every 2 seconds. Lack of heartbeats for 10 seconds triggers an rmcp warning frame so clients can retry without tearing down the process.
- **Error Handling:** Capability failures bubble up as rmcp errors whose payload includes the offending concept/entity link plus the `OperationEnvelope` status. Requests that reference out-of-workspace handles return a governance error citing [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance).
- **Prompt Catalog Tools:** `specman.core.prompt_catalog` enumerates specification, implementation, and scratch-pad prompts by reading `.specman/templates` pointer files and `templates/prompts/*.md`. Responses remind callers to respect HTML directives per [SpecMan Templates](../../spec/specman-templates/spec.md#concept-ai-instruction-channel).

## rmcp Capability Inventory

We downloaded `rmcp@0.11.0` from crates.io to catalog the concrete surfaces the adapter will rely on. The table summarizes the features that matter for SpecMan compliance.

| Area | rmcp surface (0.11.0) | Adapter usage |
| --- | --- | --- |
| Tool routing | `#[tool]`, `#[tool_router]`, and `#[tool_handler]` macros generate strongly typed routers plus `ToolRouter<Self>` implementations (see README quick start). | `CapabilityRegistry` does not auto-derive handlers, but the same router traits back our thin façade so each SpecMan tool can be registered with a minimal async function that simply delegates into `specman_library`. |
| Server lifecycle | `ServerHandler` plus `ServiceExt::serve` boot the runtime, publish `ServerInfo`, and expose lifecycle callbacks such as `initialize`, `shutdown`, and `ping`. | `SpecmanMcpServer::run` instantiates a handler that forwards lifecycle events to workspace discovery and telemetry sinks before dispatching to SpecMan capabilities. |
| Schema + descriptors | The crate ships a `model` module with `CallToolResult`, `Content`, `ServerCapabilities`, and `json_schema` helpers behind the `schemars` feature. | We enable the `server` and `schemars` features so every `SpecManCapabilityDescriptor` can embed the canonical JSON Schemas derived from SpecMan Data Model structs before rmcp publishes them to clients. |
| Transport | `transport::stdio`, `transport::child_process::TokioChildProcess`, and the generalized `Transport` trait cover STDIN/STDOUT, child-process pipes, async read/write, and streamable HTTP. | The adapter only enables `transport-io` and `transport-child-process` so the server binds to STDIN/STDOUT while clients (integration tests) can launch it as a child process. |
| Notifications | `NotificationContext`, `RequestContext`, and `Peer` provide progress/logging streams plus cancellation hooks. | `OperationEnvelope` builders tap into these contexts to emit MCP-compliant `progress` and `logging_message` events, keeping SpecMan lifecycle automation observable without leaking stdout writes. |
| Feature flags | `Cargo.toml` shows `server` includes `transport-async-rw` and `schemars`, while `macros` pulls in `rmcp-macros`. | `specman-mcp-rust` enables `server`, `macros`, and `schemars` so both the registry and integration tests share one dependency graph. Optional client-side flags remain disabled to keep binary size small. |

### STDIN/STDOUT transport proof

- The README quick-start sample constructs `Counter::new().serve(stdio())`, demonstrating that `ServiceExt::serve` accepts the STDIN transport directly.
- `rmcp::transport::stdio()` (defined in `src/transport/io.rs`) returns the concrete `(tokio::io::Stdin, tokio::io::Stdout)` pair, so no adapter-specific plumbing is required to satisfy [Concept: MCP Transport Compliance](../../spec/specman-mcp/spec.md#concept-mcp-transport-compliance).
- `src/transport.rs` lists STDIO as a standard server transport alongside the child-process client, reinforcing that deterministic local pipes remain a first-class runtime profile.
- The `server` feature automatically enables `transport-io`, so compiling the adapter with `features = ["server"]` guarantees the STDIN transport stays available even when we do not opt into unrelated HTTP features.

## Capability Registry Design

`CapabilityRegistry` owns the MCP descriptor catalog and the executors that bridge rmcp payloads into `specman_library` services. Each registry entry carries the descriptor plus an async closure that receives the decoded payload, a mutable `MCPWorkspaceSession`, and the shared `SpecmanServices` bundle.

```rust
pub struct CapabilityRegistry {
    entries: BTreeMap<String, RegisteredCapability>,
}

struct RegisteredCapability {
    descriptor: SpecManCapabilityDescriptor,
    executor: CapabilityExecutor,
}

type CapabilityExecutor = Arc<
    dyn Fn(
            &mut MCPWorkspaceSession,
            serde_json::Value,
            &SpecmanServices,
        ) -> CapabilityFuture + Send + Sync,
>;
```

`SpecmanServices` exposes the exact `specman_library` primitives the registry needs—no business logic is reimplemented in the adapter layer.

```rust
pub struct SpecmanServices {
    pub locator: Arc<FilesystemWorkspaceLocator>,
    pub dependencies: Arc<FilesystemDependencyMapper<FilesystemWorkspaceLocator>>,
    pub lifecycle: Arc<dyn LifecycleController>,
    pub metadata: Arc<MetadataMutator<FilesystemWorkspaceLocator>>,
    pub persistence: Arc<WorkspacePersistence<FilesystemWorkspaceLocator>>,
    pub template_engine: Arc<dyn TemplateEngine>,
}
```

- The locator satisfies [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery).
- Dependency mapper, lifecycle controller, metadata mutator, and persistence helpers come directly from `specman_library` and already embed SpecMan Data Model entities.
- The template engine is the existing `MarkdownTemplateEngine`, which can expose pointer-backed prompts plus rendered previews without diverging from SpecMan Templates guidance.

`register_core_capabilities` iterates over the rows below, building deterministic descriptors via `schemars::schema_for!` on each referenced struct. Identifiers follow `specman.core.<concept_snake_case>.<action>` and every descriptor cites the governing heading inside `concept_ref`.

| Tool ID | Concept reference | Inputs (schema) | Outputs (schema) | `specman_library` binding | Notes |
| --- | --- | --- | --- | --- | --- |
| `specman.core.workspace.describe` | [Concept: Workspace Discovery](../../spec/specman-core/spec.md#concept-workspace-discovery) | `WorkspaceSelector` (optional override path + environment hints) | `WorkspacePaths` | `FilesystemWorkspaceLocator::workspace()` | Returns canonical root plus `.specman` directory, ensuring every follow-on capability shares the same workspace. |
| `specman.core.workspace.list_artifacts` | [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance) | `ArtifactFilter` (resource scheme + optional glob) | `Vec<ArtifactSummary>` | `WorkspacePaths::{spec_dir, impl_dir, scratchpad_dir}` + `ArtifactFrontMatter` parser | Lists `spec://`, `impl://`, and `scratch://` handles together with normalized identifiers and locations. |
| `specman.core.dependency.tree` | [Concept: Dependency Mapping Services](../../spec/specman-core/spec.md#concept-dependency-mapping-services) | `ArtifactLocatorRequest` (accepts filesystem paths, HTTPS URLs, or resource handles) | `DependencyTree` | `FilesystemDependencyMapper::dependency_tree_from_locator()` | Fulfills the spec requirement to expose upstream/downstream/aggregate trees plus `/dependencies` handle support. |
| `specman.core.dependency.slice` | [Concept: Dependency Mapping Services](../../spec/specman-core/spec.md#concept-dependency-mapping-services) | `DependencySliceRequest` (`mode: upstream|downstream`, locator) | `Vec<DependencyEdge>` | `DependencyMapping::{upstream, downstream}` | Returns partial graphs so MCP clients can draw targeted impact visualizations without downloading the full tree. |
| `specman.core.templates.catalog` | [Concept: Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration) | `TemplateCatalogRequest` (scenario + optional pointer override) | `Vec<TemplateDescriptor>` | `TemplateEngine` + pointer resolution helpers already used by lifecycle automation | Enumerates spec/impl/scratch templates and embeds required tokens so MCP clients can prompt operators interactively. |
| `specman.core.templates.pointer_update` | [Concept: Template Orchestration](../../spec/specman-core/spec.md#concept-template-orchestration) | `PointerMutationRequest` (locator, pointer name, action) | `PointerMutationResult` | Workspace-scoped pointer helpers in `specman_library::templates` | Allows MCP operators to set, clear, or inspect `.specman/templates/{SPEC,IMPL,SCRATCH}` pointers without leaving the session. |
| `specman.core.prompt_catalog` | [SpecMan Templates — Concept: AI Instruction Channel](../../spec/specman-templates/spec.md#concept-ai-instruction-channel) | `PromptCatalogRequest` (artifact type) | `Vec<PromptDescriptor>` | Reads `templates/prompts/*.md` via the existing template engine and returns metadata + HTML directives | Required by the MCP spec to expose prompt catalogs with template citations. |
| `specman.core.lifecycle.plan_creation` | [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation) | `CreationRequest` | `CreationPlan` | `LifecycleController::plan_creation()` | Produces deterministic plans (rendered templates + dependency guardrails) before mutating the workspace. |
| `specman.core.lifecycle.create` | [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation) | `CreationExecutionRequest` (plan id + tokens + dry-run flag) | `OperationEnvelope` | `LifecycleController::plan_creation()` + `WorkspacePersistence::persist()` | Applies rendered templates, records artifacts in the envelope, and streams progress updates through rmcp. |
| `specman.core.lifecycle.plan_deletion` | [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation) | `DeletionRequest` | `DeletionPlan` | `LifecycleController::plan_deletion()` | Surfaces blocking dependency trees so MCP clients can warn operators before deleting artifacts. |
| `specman.core.lifecycle.delete` | [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation) | `DeletionExecutionRequest` (plan id + force flag) | `OperationEnvelope` | `LifecycleController::execute_deletion()` | Serializes destructive mutations by routing through the lifecycle controller plus lock guard described below. |
| `specman.core.lifecycle.scratchpad` | [Concept: Lifecycle Automation](../../spec/specman-core/spec.md#concept-lifecycle-automation) | `ScratchPadPlanRequest` (profile id, tokens) | `ScratchPadPlan` | `LifecycleController::plan_scratchpad()` | Keeps scratch-pad creation aligned with SpecMan Data Model work-type rules enforced by `specman_library`. |
| `specman.core.metadata.describe` | [Concept: Data Model Backing Implementation](../../spec/specman-core/spec.md#concept-data-model-backing-implementation) | `ArtifactLocatorRequest` | `ArtifactFrontMatter` | `MetadataMutator` (read-only path) | Returns the typed YAML front matter for any artifact so MCP clients can display current metadata without editing the file manually. |
| `specman.core.metadata.patch` | [Concept: Metadata Mutation](../../spec/specman-core/spec.md#concept-metadata-mutation) | `MetadataMutationRequest` | `MetadataMutationResult` | `MetadataMutator::mutate()` | Adds or removes dependencies/references, enforces workspace boundaries, and (optionally) persists the updated artifact. |
| `specman.core.operations.inspect` | [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution) + [Entity: OperationEnvelope](../../spec/specman-mcp/spec.md#entity-operationenvelope) | `OperationQuery` (session id + optional capability id) | `Vec<OperationEnvelope>` | Adapter-local `OperationStore` backed by structured telemetry on disk | Exposes audit history through MCP so supervising agents can fetch prior envelopes without scraping log files. |

Each executor deserializes payloads using the schema recorded in the descriptor. Validation failures return rmcp errors whose payload references the governing concept. Successful executions emit an `OperationEnvelope`, persist it to `.specman/logs/specman-mcp.jsonl`, and publish the resulting value back to rmcp.

## MCPWorkspaceSession Guard Prototype

`SessionManager` now produces a `WorkspaceSessionGuard` for every tool invocation. The guard enforces workspace normalization, lock coordination, and telemetry updates before allowing the executor to touch any `specman_library` service.

### Structures

```rust
pub struct SessionManager {
    sessions: DashMap<Uuid, SessionState>,
    locks: LockTable,
    telemetry: TelemetrySink,
}

struct SessionState {
    session: MCPWorkspaceSession,
    workspace: WorkspacePaths,
}

pub struct WorkspaceSessionGuard<'a> {
    session: &'a mut MCPWorkspaceSession,
    services: &'a SpecmanServices,
    lock_scope: LockScope,
    envelope: OperationEnvelopeBuilder,
}
```

- `LockTable` is a `DashMap<ArtifactId, LockOwner>` that serializes write-intent calls while still allowing concurrent read-only tools (for example, dependency lookups) to proceed.
- `OperationEnvelopeBuilder` accumulates streamed messages, artifacts, and timestamps until the executor finalizes the result.
- `TelemetrySink` appends JSON lines to `.specman/logs/specman-mcp.jsonl` and mirrors summaries through rmcp logging notifications, satisfying [Concept: Session Safety & Deterministic Execution](../../spec/specman-mcp/spec.md#concept-session-safety--deterministic-execution).

### Flow per tool invocation

1. `CapabilityRegistry::execute` locates the descriptor and allocates an `OperationEnvelopeBuilder` seeded with the capability id plus raw inputs.
2. `SessionManager::with_session` retrieves (or creates) the `MCPWorkspaceSession`, resolves the workspace via `SpecmanServices::locator`, and constructs a `WorkspaceSessionGuard`.
3. The guard normalizes every resource handle in the payload using `specman_library::dependency::ArtifactLocator::from_reference`, ensuring [Concept: Workspace & Data Governance](../../spec/specman-mcp/spec.md#concept-workspace--data-governance) is enforced uniformly.
4. Based on the descriptor metadata, the guard derives a list of `CapabilityLockIntent` values:
   - `Read(ArtifactId)` for inspection tools (workspace describe, dependency tree, metadata describe).
   - `Write(ArtifactId)` for lifecycle mutations, metadata patches, and pointer updates.
   - `WorkspaceWide` for pointer editing or configuration changes that affect global state.
5. The guard acquires the necessary locks via `LockTable::acquire`, which blocks until no incompatible intent is active. Lock owners are keyed by `(session_id, capability_id)` so a crashed client can be force-cleaned during session teardown.
6. The executor runs against the `SpecmanServices` bundle. For example, the create tool calls `LifecycleController::plan_creation` followed by `WorkspacePersistence::persist`, while metadata mutations go through `MetadataMutator::mutate` with `persist = true` when the request demands it.
7. During execution, helpers call `WorkspaceSessionGuard::record_progress(message)` to emit rmcp `progress` notifications and append transcripts to the envelope.
8. On success, the guard finalizes the envelope, releases locks, writes telemetry, and returns the rmcp tool response. On failure, it still releases locks and emits an MCP error whose payload includes the same envelope metadata for auditing.

### Integration points with `specman_library`

- `LifecycleController` already implements dependency guarding, but the guard adds a higher-level mutex so concurrent MCP sessions cannot interleave conflicting writes before `LifecycleController` performs its own checks.
- `WorkspacePersistence` handles filesystem writes/removals; the guard simply tracks the resulting `ArtifactId` + `PathBuf` so they appear inside the envelope.
- `MetadataMutator` enforces YAML-front-matter invariants and workspace boundaries; the guard supplements the error with the MCP concept reference to keep responses spec-compliant.
- Dependency mapping, prompt catalog resolution, and template rendering all reuse the existing SpecMan services, so the guard’s only role is to ensure they observe the same workspace and share read locks when necessary.

- Session shutdown refuses to complete while `locks` is non-empty, ensuring callers release or cancel every outstanding operation before terminating.

Together, these guard mechanics guarantee that every MCP request respects SpecMan Core invariants, produces auditable `OperationEnvelope` records, and never escapes the negotiated workspace.
