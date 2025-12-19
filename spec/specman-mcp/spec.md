---
name: specman-mcp
version: "1.0.0"
dependencies:
  - ref: ../specman-data-model/spec.md
    optional: false
  - ref: ../specman-core/spec.md
    optional: false
  - ref: https://modelcontextprotocol.io/docs/learn/architecture
    optional: false
---

# Specification — SpecMan MCP Server

This specification defines the requirements for a Model Context Protocol (MCP) server adapter that exposes every capability furnished by implementations of the [SpecMan Core](../specman-core/spec.md) specification while relying on a compliant STDIN-based MCP transport. Implementers MAY embed any conformant MCP server library—the focus here is the SpecMan-facing contract, not the transport implementation details.

## Terminology & References

This document uses the normative keywords defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119). Readers SHOULD review the [SpecMan Data Model](../specman-data-model/spec.md) to understand workspace entities and MUST familiarize themselves with the current Model Context Protocol guidance at [modelcontextprotocol.io/docs/learn/architecture](https://modelcontextprotocol.io/docs/learn/architecture). Version negotiation remains an implementation detail; MCP adapters MAY opt into any published MCP revision as long as they negotiate compatibly with connected clients. All capability parity statements inherit requirements from the [SpecMan Core](../specman-core/spec.md) concepts listed below.

## Concepts

### Concept: MCP Transport Compliance

The MCP server adapter sits on top of a STDIN/STDOUT MCP runtime that brokers SpecMan workflows for local agents.

- The adapter MUST implement MCP initialization, tool enumeration, and version negotiation flows as described in the official MCP specification, yet it MUST NOT mandate a specific MCP revision; instead it MUST advertise the versions it supports and honor the MCP version-negotiation handshake.
- Implementations MUST run as STDIN-based MCP servers intended for local invocation so tooling operates on the user’s machine without exposing network-accessible endpoints.
- Every MCP tool exposed by the adapter MUST include deterministic parameter schemas that mirror the entities defined in the [SpecMan Data Model](../specman-data-model/spec.md); schema drift is NOT permitted.
- The adapter MUST surface lifecycle hooks (initialize, shutdown, keep-alive) so MCP clients can coordinate long-running SpecMan tasks without bypassing the MCP lifecycle described in [SpecMan Core Deterministic Execution](../specman-core/spec.md#concept-deterministic-execution).
- Streaming outputs, partial results, and tool errors MUST follow the MCP framing rules; when SpecMan Core would emit structured errors, the MCP transport MUST encapsulate them as MCP-compliant error payloads without losing error codes or references.

### Concept: SpecMan Capability Parity

This concept ensures that every capability delivered by a SpecMan Core-compliant implementation is reachable through MCP tools with identical semantics, regardless of which MCP runtime library hosts the adapter.

- For each concept defined in [SpecMan Core](../specman-core/spec.md#concepts), the MCP adapter MUST expose at least one tool whose behavior, inputs, and outputs align with the originating concept’s constraints (for example, workspace discovery, dependency mapping, template orchestration, lifecycle automation, metadata mutation).
- When a SpecMan Core implementation ships additional optional or experimental capabilities, the adapter MAY surface them via extension tools, but it MUST clearly label each tool with the governing specification or implementation path so clients can opt into or ignore the capability.
- The adapter MUST act as a pure façade: it MUST delegate to an underlying SpecMan Core implementation or library rather than re-defining the business logic within the MCP layer.
- Capability descriptors MUST include a stable identifier (`specman.core.<concept_snake_case>`) and version metadata so MCP clients can bind to specific SpecMan Core releases.
- If an underlying SpecMan Core capability is temporarily unavailable, the MCP adapter MUST return an MCP error that cites the impacted concept and RECOMMENDED remediation (e.g., re-run once workspace lock clears) instead of silently degrading behavior.
- The adapter MUST provide MCP tools that enumerate specifications, implementations, and scratch pads as resource handles using the `spec://{artifact}`, `impl://{artifact}`, and `scratch://{artifact}` schemes defined in [SpecMan Core Dependency Mapping Services](../specman-core/spec.md#concept-dependency-mapping-services). At a minimum, adapters MUST expose list and describe tools for each artifact class, and each response MUST serialize entities using the [SpecMan Data Model](../specman-data-model/spec.md).
- Dependency graph tooling MUST accept `<scheme>://{artifact}/dependencies` inputs and return upstream/downstream trees powered by the SpecMan Core dependency mapping services. `/dependencies` handles are read-only aliases whose responses MUST include the same structure and error semantics as invoking the dependency tree builder directly.
- The adapter MUST surface prompt-catalog tools that return authoring prompts for creating and modifying specifications, implementations, and scratch pads. Each prompt response MUST conform to [Concept: Prompt Catalog](#concept-prompt-catalog), cite the effective template resolved by SpecMan Core, declare the intended work type (for scratch pads), and remind clients to honor HTML comment directives.
- The adapter MUST provide lifecycle tools that execute the prompted create or modify flows for specs, implementations, and scratch pads. These tools MUST call into SpecMan Core lifecycle automation, enforce naming and metadata constraints from the SpecMan Data Model, and emit MCP errors when persistence or validation fails.

#### Required Tool: `create_artifact`

To make artifact creation consistently automatable across MCP clients, compliant adapters MUST expose a lifecycle tool named `create_artifact`.

- The adapter MUST expose an MCP tool named `create_artifact` that creates SpecMan artifacts (specifications, implementations, scratch pads) by delegating to [SpecMan Core Lifecycle Automation](../specman-core/spec.md#concept-lifecycle-automation).
- The tool MUST support creating each artifact class:
  - specifications (`spec/{name}/spec.md`)
  - implementations (`impl/{name}/impl.md`)
  - scratch pads (`.specman/scratchpad/{name}/scratch.md`) for every supported scratch work type (`draft`, `revision`, `feat`, `ref`, `fix`).
- The tool MUST accept inputs sufficient to populate all REQUIRED values in the selected artifact template and to write a data-model-compliant YAML front matter block for the addressed artifact kind.
  - For scratch pads, this includes allowing callers to supply the work type object (including `revised_headings` / `refactored_headings` / `fixed_headings` as applicable) and the persisted `target` value.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the [SpecMan Data Model](../specman-data-model/spec.md) before persisting any files.
- The tool MUST normalize any locator handles provided as inputs (for example `spec://{artifact}` / `impl://{artifact}` / `scratch://{artifact}`) into canonical workspace-relative paths before writing artifact content, including scratch pad front matter `target`.
- The tool MUST honor template governance requirements from [SpecMan Core Template Orchestration](../specman-core/spec.md#concept-template-orchestration): templates MUST be applied as the source of truth, HTML comment directives MUST be preserved until their guidance is satisfied, and required template substitutions MUST be validated.
- The tool MUST return a deterministic result payload describing what was created. At minimum it MUST include the created artifact identifier(s) and canonical workspace-relative path(s). Implementations SHOULD also include the effective template locator/provenance used.

##### Input Schema Requirements

Because MCP requires explicit tool schemas, `create_artifact` MUST publish a deterministic parameter schema; however, the specific input *shape* is implementation-defined.

- The adapter MUST document the `create_artifact` input schema it exposes, and it MUST be deterministic across releases except where versioned as a breaking change.
- The schema MUST allow callers to provide enough information to:
  - select the artifact class to create (specification vs implementation vs scratch pad)
  - rely on the server to resolve the effective template via workspace template pointer files and scenario selection (callers MUST NOT provide template locator overrides)
  - supply every required value needed to fully render the selected template (including any required template substitutions)
  - for scratch pads, select the scratch pad work type variant and provide any required work-type-specific metadata (for example revised/refactored/fixed heading fragments)
  - control persistence behavior when such options are supported
- When the schema accepts template-substitution inputs, the adapter MUST NOT permit substitutions for tokens outside the set governed by [SpecMan Core Template Orchestration](../specman-core/spec.md#concept-template-orchestration), including the token-contract constraints defined there.

### Concept: Prompt Catalog

Prompt catalog tooling defines how MCP clients obtain deterministic prompts for artifact creation and modification.

!concept-prompt-catalog.responses:

- Prompt-catalog tools MUST emit prompts that clearly identify the artifact class and, for scratch pads, the selected work type.
- Prompts MUST instruct operators or downstream AI systems to review the target specification and all of its dependencies before authoring changes and MUST remind them to preserve HTML comment directives until satisfied.
- Each prompt response MUST cite the effective template source resolved via SpecMan Core template orchestration (workspace overrides first, then packaged defaults) so clients know which scaffold is authoritative.

!concept-prompt-catalog.scope:

- Prompt catalog governance applies exclusively to MCP prompt- and resource-oriented surfaces. CLI documentation MUST NOT expose prompt templates directly; CLI usage relies on the same SpecMan Core lifecycle automation without surfacing prompt text.
- Prompt catalog responses MAY tailor wording for specific MCP scenarios, but they MUST remain deterministic for a given template/version combination.

### Concept: Workspace & Data Governance

MCP calls interact with on-disk workspaces governed by the SpecMan Data Model.

- All file-system interactions initiated through MCP MUST resolve paths via the workspace discovery logic mandated by [SpecMan Core Workspace Discovery](../specman-core/spec.md#concept-workspace-discovery); clients MUST NOT provide absolute paths that escape the workspace root.
- Requests that mutate specifications, implementations, or scratch pads MUST pass through the lifecycle automation rules outlined in [SpecMan Core Lifecycle Automation](../specman-core/spec.md#concept-lifecycle-automation), ensuring templates remain authoritative and dependency checks run before persistence.
- The server MUST enforce SpecMan data invariants before returning success; violations MUST be reported as MCP errors containing the data-model heading that was breached.
- Data returned to MCP clients (e.g., rendered specs, dependency graphs) MUST retain source references so downstream tools can trace each datum back to its origin document within the workspace.
- Resource handles resolved via `spec://`, `impl://`, or `scratch://` MUST be normalized through workspace discovery, bound to canonical artifact paths, and rejected when they refer to artifacts outside the active workspace. Normalized handles MUST retain stable identifiers so MCP clients can reuse them across sessions.
- `/dependencies` handles MUST be treated as derived read-only locators whose responses are generated exclusively by dependency mapping services; mutation attempts against these handles MUST fail with an MCP error explaining that only query operations are supported.
- Prompt catalog and lifecycle tools MUST reference template locators resolved via SpecMan Core template orchestration (workspace pointer files first, then packaged defaults), validate that supplied names comply with the [founding specification](../../docs/founding-spec.md), and document any workspace mutations in the lifecycle tool results.

### Concept: Session Safety & Deterministic Execution

Remote execution must stay predictable and observable even though deployments are expected to be local STDIN-based processes.

- Each MCP session MUST bind to a single user-controlled process context; external authentication requirements are out of scope because local operators already possess the necessary permissions to launch the binary.
- The adapter MUST still emit an audit-friendly transcript (for example, structured logs) capturing requested capabilities, targeted workspaces, and resulting artifact paths so CLI wrappers or supervising tools can review activity.
- Concurrent requests targeting the same artifact MUST honor locking semantics consistent with [SpecMan Core Deterministic Execution](../specman-core/spec.md#concept-deterministic-execution); when conflicts occur, the adapter MUST serialize operations or fail fast with an actionable error.
- Long-running operations MUST provide heartbeat or progress notifications using MCP streams so clients can detect stalls without terminating the workspace process abruptly.

## Key Entities

### Entity: MCPWorkspaceSession

Represents a negotiated MCP session bound to a single SpecMan workspace.

- MUST store the agreed MCP protocol version, authenticated principal metadata, and workspace root path derived from workspace discovery.
- MUST track active tool invocations plus their locks so conflicting operations can be rejected deterministically.
- SHOULD expose telemetry hooks (structured logs or events) that mirror session lifecycle transitions (initialize, tool call, shutdown).

### Entity: SpecManCapabilityDescriptor

Defines the MCP tool metadata for each SpecMan Core capability.

- MUST include fields for `id`, `concept_ref` (link to the governing SpecMan Core heading), supported SpecMan Core version range, and optional extension metadata.
- MUST embed JSON Schema fragments that match the SpecMan Data Model serialization for the capability’s inputs/outputs.
- MAY reference implementation-specific extensions, but those entries MUST carry a `type: extension` label and cite the owning specification or implementation path.

## Additional Notes

- MCP deployments MAY shard workspaces across multiple processes, but every shard MUST adhere to this specification and expose a single consolidated capability catalog to clients.
- Implementers MAY offer read-only planning tools as separate capabilities so MCP clients can request previews before persisting changes; preview responses MUST clearly indicate they are non-persistent.
- Adapters MAY reuse off-the-shelf MCP libraries or frameworks; compliance is measured by the behavior defined in this document, not by re-implementing the protocol stack.
- Because deployments are STDIN-based on local machines, additional network security controls are OPTIONAL; nonetheless, implementers SHOULD ensure logging and locking remain in place to preserve SpecMan Core guarantees.
- MCP adapters SHOULD document the mapping between resource handles and human-readable artifact names so that clients can prompt users before invoking lifecycle operations.
- The `/dependencies` suffix is RESERVED for MCP adapters and MUST NOT be repurposed for mutation flows or non-dependency data; adapters MAY introduce additional read-only suffixes in future revisions provided they extend the resource-handle schema consistently.
