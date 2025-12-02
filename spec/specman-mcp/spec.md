---
name: specman-mcp
version: "1.0.0"
dependencies:
  - ref: ../specman-data-model/spec.md
    optional: false
  - ref: ../specman-core/spec.md
    optional: false
  - ref: https://modelcontextprotocol.io/specification
    optional: false
---

# Specification — SpecMan MCP Server

This specification defines the requirements for a Model Context Protocol (MCP) server adapter that exposes every capability furnished by implementations of the [SpecMan Core](../specman-core/spec.md) specification while relying on a compliant STDIN-based MCP transport. Implementers MAY embed any conformant MCP server library—the focus here is the SpecMan-facing contract, not the transport implementation details.

## Terminology & References

This document uses the normative keywords defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119). Readers SHOULD review the [SpecMan Data Model](../specman-data-model/spec.md) to understand workspace entities and MUST familiarize themselves with the current Model Context Protocol guidance at [modelcontextprotocol.io/specification](https://modelcontextprotocol.io/specification). Version negotiation remains an implementation detail; MCP adapters MAY opt into any published MCP revision as long as they negotiate compatibly with connected clients. All capability parity statements inherit requirements from the [SpecMan Core](../specman-core/spec.md) concepts listed below.

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

### Concept: Workspace & Data Governance

MCP calls interact with on-disk workspaces governed by the SpecMan Data Model.

- All file-system interactions initiated through MCP MUST resolve paths via the workspace discovery logic mandated by [SpecMan Core Workspace Discovery](../specman-core/spec.md#concept-workspace-discovery); clients MUST NOT provide absolute paths that escape the workspace root.
- Requests that mutate specifications, implementations, or scratch pads MUST pass through the lifecycle automation rules outlined in [SpecMan Core Lifecycle Automation](../specman-core/spec.md#concept-lifecycle-automation), ensuring templates remain authoritative and dependency checks run before persistence.
- The server MUST enforce SpecMan data invariants before returning success; violations MUST be reported as MCP errors containing the data-model heading that was breached.
- Data returned to MCP clients (e.g., rendered specs, dependency graphs) MUST retain source references so downstream tools can trace each datum back to its origin document within the workspace.

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

### Entity: OperationEnvelope

Encapsulates a single SpecMan action executed via MCP, including deterministic results.

- MUST capture the originating capability descriptor, sanitized inputs, execution timestamps, and resulting artifacts.
- MUST record workspace mutations (created files, updated specs) so downstream auditing or rollback workflows can reason about side effects.
- SHOULD provide a canonical transcript that MCP clients MAY store for provenance, including streamed messages and final status codes.

## Additional Notes

- MCP deployments MAY shard workspaces across multiple processes, but every shard MUST adhere to this specification and expose a single consolidated capability catalog to clients.
- Implementers MAY offer dry-run variants of mutating capabilities so MCP clients can request previews before persisting changes; dry-run responses MUST clearly indicate they are non-persistent.
- Adapters MAY reuse off-the-shelf MCP libraries or frameworks; compliance is measured by the behavior defined in this document, not by re-implementing the protocol stack.
- Because deployments are STDIN-based on local machines, additional network security controls are OPTIONAL; nonetheless, implementers SHOULD ensure logging and locking remain in place to preserve SpecMan Core guarantees.
