---
name: specman-mcp
version: 2.0.0
dependencies:
- ref: ../specman-core/spec.md
  optional: false
- ref: https://modelcontextprotocol.io/docs/learn/architecture
  optional: false
---

# Specification — SpecMan MCP Server

This specification defines the requirements for a Model Context Protocol (MCP) server adapter that exposes every capability furnished by implementations of the [SpecMan Core](../specman-core/spec.md) specification while relying on a compliant STDIN-based MCP transport. Implementers MAY embed any conformant MCP server library—the focus here is the SpecMan-facing contract, not the transport implementation details.

## Terminology & References

This document uses the normative keywords defined in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119). Readers SHOULD review the [SpecMan Core](../specman-core/spec.md) specification to understand workspace entities and MUST familiarize themselves with the current Model Context Protocol guidance at [modelcontextprotocol.io/docs/learn/architecture](https://modelcontextprotocol.io/docs/learn/architecture). Version negotiation remains an implementation detail; MCP adapters MAY opt into any published MCP revision as long as they negotiate compatibly with connected clients. All capability parity statements inherit requirements from the [SpecMan Core](../specman-core/spec.md) concepts listed below.

## Concepts

### Concept: MCP Transport Compliance

The MCP server adapter sits on top of a STDIN/STDOUT MCP runtime that brokers SpecMan workflows for local agents.

- The adapter MUST implement MCP initialization, tool enumeration, and version negotiation flows as described in the official MCP specification, yet it MUST NOT mandate a specific MCP revision; instead it MUST advertise the versions it supports and honor the MCP version-negotiation handshake.
- Implementations MUST run as STDIN-based MCP servers intended for local invocation so tooling operates on the user’s machine without exposing network-accessible endpoints.
- Every MCP tool exposed by the adapter MUST include deterministic parameter schemas that mirror the entities defined in the [SpecMan Core](../specman-core/spec.md) specification; schema drift is NOT permitted.
- The adapter MUST surface lifecycle hooks (initialize, shutdown, keep-alive) so MCP clients can coordinate long-running SpecMan tasks without bypassing the MCP lifecycle described in [SpecMan Core Deterministic Execution](../specman-core/spec.md#concept-deterministic-execution).
- Streaming outputs, partial results, and tool errors MUST follow the MCP framing rules; when SpecMan Core would emit structured errors, the MCP transport MUST encapsulate them as MCP-compliant error payloads without losing error codes or references.

### Concept: MCP Auto-Completion

MCP clients need deterministic completion responses for SpecMan-facing tool/resource/prompt arguments so they can discover valid identifiers without bespoke workspace scans.

!concept-mcp-auto-completion.requirements:

- Implementations MUST provide completion responses for MCP surfaces that accept artifact identifiers, including tools, resources, and prompt arguments.
- Completion suggestions defined by this specification MUST include only `spec://{artifact}` and `impl://{artifact}` handles when a handle-valued argument is being completed.
- Completion responses defined by this specification MUST NOT suggest `scratch://` handles.
- Completion filtering MUST evaluate candidate text using the completion value that would be inserted into the target argument.
- For this concept, normalization MUST be: trim leading and trailing whitespace, then apply Unicode-aware case folding.
- Completion filtering MUST use a deterministic `fuzzy` matching mode.
- In `fuzzy` mode, a candidate MUST be retained according to a deterministic fuzzy-matching algorithm that is documented by the implementation.
- Mode selection MUST be deterministic. When a completion surface accepts an explicit mode selector, omitted mode values MUST default to `fuzzy`; when a surface does not accept an explicit mode selector, the implementation MUST document and apply a fixed mode for that surface.
- Completion responses MUST be deterministic for the same workspace state, source input text, and selected mode, and MUST preserve stable ordering for equivalent candidates.
- If the normalized source text is empty, implementations MUST return the full candidate set scoped by existing handle/type constraints.
- Completion candidates MUST be sourced from the active workspace discovered by SpecMan Core workspace discovery and MUST respect workspace-boundary rules.

!concept-mcp-auto-completion.validation:

- Suggestions representing locators or references MUST pass the same locator-scheme and workspace-boundary validation rules used by dependency and reference validation.
- Implementations MUST NOT suggest unsupported destination schemes.
- Implementations MUST evaluate matching after normalization and before final ordering is emitted.
- Implementations MUST keep filtering deterministic for a fixed workspace state, fixed input text, and fixed matching mode.
- Implementations MUST define deterministic tie-breaking so equivalent fuzzy scores preserve stable ordering expectations.
- When completion indexes or metadata are malformed, adapters MUST return partial suggestions for valid segments and MUST emit warning diagnostics through MCP-supported warning logging mechanisms.
- Degraded completion warnings MUST NOT be encoded as MCP transport errors unless the adapter cannot produce any valid suggestion.

!concept-mcp-auto-completion.performance:

- Implementations SHOULD cache completion indexes for the duration of a request and MAY cache across requests when cache keys include workspace root and content fingerprinting.
- Cached completion data MUST be invalidated when source artifacts change.

### Concept: SpecMan Capability Parity

This concept ensures that every capability delivered by a SpecMan Core-compliant implementation is reachable through MCP tools with identical semantics, regardless of which MCP runtime library hosts the adapter.

!concept-specman-capability-parity.completion:

- MCP tools and resources that accept artifact identifiers MUST provide completion hints/responses aligned to their accepted handle scope.
- Resource completion for derived resource suffixes (for example `/constraints`, `/dependencies`, `/compliance`) MUST only suggest suffixes valid for the selected base artifact type.
- Tool/resource completion responses covered by this specification MUST share a consistent completion index so deterministic behavior is preserved across MCP surfaces.
- Tool/resource/prompt completion responses covered by this specification MUST share the same normalization, mode-selection, and filtering semantics defined by [Concept: MCP Auto-Completion](#concept-mcp-auto-completion).

- For each concept defined in [SpecMan Core](../specman-core/spec.md#concepts), the MCP adapter MUST expose at least one tool whose behavior, inputs, and outputs align with the originating concept’s constraints (for example, workspace discovery, dependency mapping, template orchestration, lifecycle automation, metadata mutation).
- When a SpecMan Core implementation ships additional optional or experimental capabilities, the adapter MAY surface them via extension tools, but it MUST clearly label each tool with the governing specification or implementation path so clients can opt into or ignore the capability.
- The adapter MUST act as a pure façade: it MUST delegate to an underlying SpecMan Core implementation or library rather than re-defining the business logic within the MCP layer.
- Capability descriptors MUST include a stable identifier (`specman.core.<concept_snake_case>`) and version metadata so MCP clients can bind to specific SpecMan Core releases.
- If an underlying SpecMan Core capability is temporarily unavailable, the MCP adapter MUST return an MCP error that cites the impacted concept and RECOMMENDED remediation (e.g., re-run once workspace lock clears) instead of silently degrading behavior.
- The adapter MUST provide MCP tools that enumerate specifications, implementations, and scratch pads as resource handles using the `spec://{artifact}`, `impl://{artifact}`, and `scratch://{artifact}` schemes defined in [SpecMan Core Dependency Mapping Services](../specman-core/spec.md#concept-dependency-mapping-services). At a minimum, adapters MUST expose list and describe tools for each artifact class, and each response MUST serialize entities using the data model defined in [SpecMan Core](../specman-core/spec.md).
- Dependency graph tooling MUST accept `<scheme>://{artifact}/dependencies` inputs and return upstream/downstream trees powered by the SpecMan Core dependency mapping services. `/dependencies` handles are read-only aliases whose responses MUST include the same structure and error semantics as invoking the dependency tree builder directly.
- The adapter MUST surface prompt-catalog tools that return authoring prompts for creating and modifying specifications, implementations, and scratch pads. Each prompt response MUST conform to [Concept: Prompt Catalog](#concept-prompt-catalog), cite the effective template resolved by SpecMan Core, declare the intended work type (for scratch pads), and remind clients to honor HTML comment directives.
- The adapter MUST provide lifecycle tools that execute the prompted create or modify flows for specs, implementations, and scratch pads. These tools MUST call into SpecMan Core lifecycle automation, enforce naming and metadata constraints from the SpecMan Data Model, and emit MCP errors when persistence or validation fails. MCP lifecycle tools MUST NOT perform sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_specification`

To allow MCP clients to create specification artifacts deterministically, compliant adapters MUST expose a lifecycle tool named `create_specification`.

!concept-specman-capability-parity.tooling.create-specification:

- The adapter MUST expose an MCP tool named `create_specification` that creates a specification artifact (`spec/{name}/spec.md`) by delegating to SpecMan Core lifecycle automation.
- The tool MUST accept a `name` parameter (the specification slug) and a `title` parameter (the human-readable title).
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration: templates MUST be applied as the source of truth, HTML comment directives MUST be preserved until satisfied, and required template substitutions MUST be validated.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`spec://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_implementation`

!concept-specman-capability-parity.tooling.create-implementation:

- The adapter MUST expose an MCP tool named `create_implementation` that creates an implementation artifact (`impl/{name}/impl.md`).
- The tool MUST accept a `name` parameter and a `target` parameter that resolves to a specification artifact.
- The tool MUST validate that `target` resolves to an existing specification before persisting.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`impl://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_revision`

!concept-specman-capability-parity.tooling.create-revision:

- The adapter MUST expose an MCP tool named `create_revision` that creates a revision scratch pad.
- The tool's description MUST indicate that it creates a scratch pad (a planning document for a specification revision), not a modification to the specification itself.
- The tool MUST accept a `name` parameter and a `target` parameter that MUST resolve to a specification artifact.
- The scratch pad work type MUST be set to `revision`.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`scratch://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_feature`

!concept-specman-capability-parity.tooling.create-feature:

- The adapter MUST expose an MCP tool named `create_feature` that creates a feature scratch pad.
- The tool's description MUST indicate that it creates a scratch pad (a planning document for introducing a feature), not a feature implementation itself.
- The tool MUST accept a `name` parameter and a `target` parameter that MUST resolve to an implementation artifact.
- The scratch pad work type MUST be set to `feat`.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`scratch://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_refactor`

!concept-specman-capability-parity.tooling.create-refactor:

- The adapter MUST expose an MCP tool named `create_refactor` that creates a refactor scratch pad.
- The tool's description MUST indicate that it creates a scratch pad (a planning document for refactoring).
- The tool MUST accept a `name` parameter and a `target` parameter that MUST resolve to an implementation artifact.
- The scratch pad work type MUST be set to `ref`.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`scratch://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `create_fix`

!concept-specman-capability-parity.tooling.create-fix:

- The adapter MUST expose an MCP tool named `create_fix` that creates a fix scratch pad.
- The tool's description MUST indicate that it creates a scratch pad (a planning document for applying a fix).
- The tool MUST accept a `name` parameter and a `target` parameter that MUST resolve to an implementation artifact.
- The scratch pad work type MUST be set to `fix`.
- The tool MUST enforce naming, metadata, and workspace-boundary constraints from the SpecMan Data Model before persisting.
- The tool MUST honor template governance requirements from SpecMan Core template orchestration.
- The tool MUST return a deterministic result payload including the created artifact identifier, canonical handle (`scratch://{name}`), and workspace-relative path.
- The tool MUST NOT perform MCP sampling or elicitation; all inputs MUST be supplied directly by the caller.

#### Required Tool: `update_specification`

To allow MCP clients to update specification metadata deterministically without rewriting Markdown bodies, compliant adapters MUST expose a lifecycle tool named `update_specification`.

!concept-specman-capability-parity.tooling.update-specification:

- The adapter MUST expose an MCP tool named `update_specification`.
- The tool MUST update YAML front matter metadata for specification artifacts and MUST leave the Markdown body unchanged.
- The tool MUST delegate to the underlying SpecMan Core implementation's metadata mutation capabilities (see [Concept: Metadata Mutation](../specman-core/spec.md#concept-metadata-mutation)).
- The tool MUST accept a `locator` identifying the target specification and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path, HTTPS URL, or a SpecMan locator handle (`spec://{artifact}`) as the locator input.
  - If a SpecMan handle is supplied, the adapter MUST normalize it to a canonical workspace-relative path before applying any update.
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `requires_implementation`, `dependencies`.
  - For list-valued fields, removals MUST be expressible via explicit remove ops, and additions MUST be idempotent.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `update_implementation`

!concept-specman-capability-parity.tooling.update-implementation:

- The adapter MUST expose an MCP tool named `update_implementation`.
- The tool MUST update YAML front matter metadata for implementation artifacts and MUST leave the Markdown body unchanged.
- The tool MUST accept a `locator` identifying the target implementation and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path, HTTPS URL, or a SpecMan locator handle (`impl://{artifact}`).
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `spec`, `location`, `references`, `dependencies`.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `update_revision`

!concept-specman-capability-parity.tooling.update-revision:

- The adapter MUST expose an MCP tool named `update_revision`.
- The tool's description MUST indicate that it updates a revision scratch pad (a planning document).
- The tool MUST update YAML front matter metadata for revision scratch pad artifacts and MUST leave the Markdown body unchanged.
- The tool MUST accept a `locator` identifying the target revision scratch pad and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path or a SpecMan locator handle (`scratch://{artifact}`). HTTPS locators MUST NOT be accepted (scratch pads are workspace-local only).
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `dependencies`.
- The `target` field MUST NOT be accepted as input; scratch pad target is immutable.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `update_feature`

!concept-specman-capability-parity.tooling.update-feature:

- The adapter MUST expose an MCP tool named `update_feature`.
- The tool's description MUST indicate that it updates a feature scratch pad (a planning document).
- The tool MUST update YAML front matter metadata for feature scratch pad artifacts and MUST leave the Markdown body unchanged.
- The tool MUST accept a `locator` identifying the target feature scratch pad and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path or a SpecMan locator handle (`scratch://{artifact}`). HTTPS locators MUST NOT be accepted (scratch pads are workspace-local only).
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `dependencies`.
- The `target` field MUST NOT be accepted as input; scratch pad target is immutable.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `update_refactor`

!concept-specman-capability-parity.tooling.update-refactor:

- The adapter MUST expose an MCP tool named `update_refactor`.
- The tool's description MUST indicate that it updates a refactor scratch pad (a planning document).
- The tool MUST update YAML front matter metadata for refactor scratch pad artifacts and MUST leave the Markdown body unchanged.
- The tool MUST accept a `locator` identifying the target refactor scratch pad and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path or a SpecMan locator handle (`scratch://{artifact}`). HTTPS locators MUST NOT be accepted (scratch pads are workspace-local only).
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `dependencies`.
- The `target` field MUST NOT be accepted as input; scratch pad target is immutable.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `update_fix`

!concept-specman-capability-parity.tooling.update-fix:

- The adapter MUST expose an MCP tool named `update_fix`.
- The tool's description MUST indicate that it updates a fix scratch pad (a planning document).
- The tool MUST update YAML front matter metadata for fix scratch pad artifacts and MUST leave the Markdown body unchanged.
- The tool MUST accept a `locator` identifying the target fix scratch pad and a `mode` switch (`persist` | `preview`).
  - Callers MAY supply a filesystem path or a SpecMan locator handle (`scratch://{artifact}`). HTTPS locators MUST NOT be accepted (scratch pads are workspace-local only).
- Supported mutation fields: `name`, `title`, `description`, `version`, `tags`, `dependencies`.
- The `target` field MUST NOT be accepted as input; scratch pad target is immutable.
- The tool MUST support a persistence mode switch: `persist` writes to disk; `preview` returns the updated document without writing.

#### Required Tool: `get_workspace_status`

To provide visibility into workspace health and compliance, compliant adapters MUST expose a diagnostic tool named `get_workspace_status`.

!concept-specman-capability-parity.tooling.get-workspace-status:

- The adapter MUST expose an MCP tool named `get_workspace_status`.
- The tool MUST delegate to the underlying SpecMan Core workspace status capability (see [SpecMan Core Workspace Status](../specman-core/spec.md#concept-workspace-status)).
- The tool MUST accept an optional boolean parameter `http` (default: `true`) to control external reachability checks.
  - The tool MUST map this parameter to the `WorkspaceStatusConfig` (defined in [Entity: WorkspaceStatusConfig](../specman-core/spec.md#entity-workspacestatusconfig)) as follows:
    - If `http` is `true` (or omitted), HTTP reference validation MUST be enabled (e.g., using `Reachability` mode).
    - If `http` is `false`, HTTP reference validation MUST be disabled (e.g., using `SyntaxOnly` mode), while other reference checks MUST remain enabled.
- The tool MUST return a structured validation report conforming to [Entity: WorkspaceStatusReport](../specman-core/spec.md#entity-workspacestatusreport) defined in SpecMan Core.
  - Scratch pad validation SHOULD be included if supported by the underlying implementation's default configuration, or EXPLICITLY enabled if required to match specific user intent (defaults are implementation-defined but MUST include spec/impl validation).
- The tool MUST NOT treat validation failures (e.g., broken links, missing coverage) as MCP protocol errors; it MUST return the report successfully so clients can analyze the failures.

### Concept: Prompt Catalog

Prompt catalog tooling defines how MCP clients obtain deterministic prompts for artifact creation and modification.

!concept-prompt-catalog.responses:

- Prompt-catalog tools MUST emit prompts that clearly identify the artifact class and, for scratch pads, the selected work type.
- Prompts MUST instruct operators or downstream AI systems to review the target specification and all of its dependencies before authoring changes and MUST remind them to preserve HTML comment directives until satisfied.
- Each prompt response MUST cite the effective template source resolved via SpecMan Core template orchestration (workspace overrides first, then packaged defaults) so clients know which scaffold is authoritative.

!concept-prompt-catalog.scope:

- Prompt catalog governance applies exclusively to MCP prompt- and resource-oriented surfaces. CLI documentation MUST NOT expose prompt templates directly; CLI usage relies on the same SpecMan Core lifecycle automation without surfacing prompt text.
- Prompt catalog responses MAY tailor wording for specific MCP scenarios, but they MUST remain deterministic for a given template/version combination.

!concept-prompt-catalog.argument-completion:

- Prompt arguments for `create_revision` MUST auto-complete only specification targets and MUST NOT suggest implementation handles.
- Prompt arguments for `create_feature`, `create_refactor`, and `create_fix` MUST auto-complete only implementation targets and MUST NOT suggest specification handles.
- Where prompt arguments accept handle values, suggestions MUST resolve to canonical `spec://` or `impl://` handles, while human-readable labels MAY be provided as auxiliary metadata.

!concept-prompt-catalog.migration-prompts:

- MCP adapters MUST expose a deterministic “migration” prompt in the prompt catalog that instructs migrating non-SpecMan code into SpecMan artifacts.
- The migration prompt MUST direct the operator/agent to create a new scratch pad for the target specification before any analysis, using lifecycle automation and the canonical scratch pad locations defined in the SpecMan Data Model.
- The migration prompt MUST enumerate and sequence explicit phases as a checklist: (1) enumerate source files to be scanned; (2) read the codebase and extract candidate concepts, entities, and constraints; (3) draft or update the specification from those findings; (4) generate implementation documentation after the specification draft is produced.

!concept-prompt-catalog.compliance-prompts:

- MCP adapters MUST expose a deterministic `compliance` prompt in the prompt catalog.
- The prompt MUST accept an implementation artifact identifier as input.
- The prompt MUST instruct the agent to retrieve the compliance report from `impl://{artifact}/compliance`.
- The prompt MUST instruct the agent that if any constraints are NOT ensured (covered) by the implementation:
    1. Create a `ref` (refactor) scratch pad for the implementation.
    2. Detail all constraint groups that are not assured in the scratch pad.
    3. Provide instructions in the scratch pad on how to create the necessary tests or checks.
    4. Provide instructions in the scratch pad on how to add the required [Validation Tags](../specman-core/spec.md#entity-validation-tag) to confirm compliance in the next iteration.

### Concept: Constraint Resources

Constraint resources allow MCP clients to discover and read constraint groups defined inside specifications through MCP resources, without requiring clients to parse Markdown.

!concept-constraint-resources.resources.templates:

- The adapter MUST expose MCP resource templates that allow clients to read constraints for a specification artifact using the `spec://` locator scheme.
- The adapter MUST support a resource at `spec://{artifact}/constraints` that returns a list of constraints defined in the referenced specification.
- The adapter MUST support a resource at `spec://{artifact}/constraints/{constraint_id}` that returns the constraint content for the identified constraint.
- These resources MUST be read-only derived locators.

!concept-constraint-resources.identifiers.constraint-id:

- `constraint_id` MUST be the constraint group set (the substring between the leading `!` and the trailing `:` in a constraint identifier line).
  - Example: for the line `!concept-prompt-catalog.responses:`, the `constraint_id` is `concept-prompt-catalog.responses`.
- `constraint_id` values MUST be treated as case-sensitive identifiers.
- If a `constraint_id` is not found, the adapter MUST return an MCP error that includes the containing artifact and the missing `constraint_id`.

!concept-constraint-resources.scope.schemes:

- Constraint resources MUST be exposed only for specification artifacts.
  - `spec://{artifact}/constraints` and `spec://{artifact}/constraints/{constraint_id}` are the only supported constraint resource locators.
  - `impl://.../constraints` and `scratch://.../constraints` MUST NOT be exposed.

!concept-constraint-resources.responses.index:

- Reading `spec://{artifact}/constraints` MUST return a deterministic list of constraints.
- Each list entry MUST include at minimum:
  - `constraint_id`
  - `identifier_line` (the literal identifier line as it appears in the document, including the leading `!` and trailing `:`)
  - `uri` (the canonical resource URI for reading that constraint via `.../constraints/{constraint_id}`)

!concept-constraint-resources.responses.read:

- Reading `spec://{artifact}/constraints/{constraint_id}` MUST return the Markdown content of that constraint group.
- The returned content MUST include the identifier line and all constraint statements belonging to that group.
- The adapter MUST NOT return unrelated constraints that merely share a prefix; matching MUST be exact on `constraint_id`.

### Concept: Compliance Resources

Compliance resources allow MCP clients to retrieve the implementation compliance status for a specific implementation artifact, leveraging the compliance reporting capabilities defined in SpecMan Core.

!concept-compliance-resources.resources.location:

- The adapter MUST expose a compliance resource at `impl://{artifact}/compliance`.
- The resource MUST return the compliance report for the identified implementation `artifact` as generated by [SpecMan Core Compliance Reporting](../specman-core/spec.md#concept-compliance-reporting).
- If the identified artifact is not an implementation or does not support compliance reporting, the adapter MUST return an MCP error.
- The compliance resource MUST include constraint coverage for the governing specification and its transitive specification dependencies.
- The compliance resource MUST ignore unrelated workspace artifacts (including scratch pads); malformed unrelated artifacts MUST NOT surface as errors in compliance responses.

!concept-compliance-resources.scope.schemes:

- Compliance resources MUST be exposed only for implementation artifacts.
- `spec://.../compliance` and `scratch://.../compliance` MUST NOT be exposed.

### Concept: Workspace & Data Governance

MCP calls interact with on-disk workspaces governed by the SpecMan Data Model.

- All file-system interactions initiated through MCP MUST resolve paths via the workspace discovery logic mandated by [SpecMan Core Workspace Discovery](../specman-core/spec.md#concept-workspace-discovery); clients MUST NOT provide absolute paths that escape the workspace root. MCP server binaries MUST accept a `--workspace <path>` argument that pins workspace discovery to the provided root; when omitted, implementations MUST default to the current working directory.
- Requests that mutate specifications, implementations, or scratch pads MUST pass through the lifecycle automation rules outlined in [SpecMan Core Lifecycle Automation](../specman-core/spec.md#concept-lifecycle-automation), ensuring templates remain authoritative and dependency checks run before persistence.
- The server MUST enforce SpecMan data invariants before returning success; violations MUST be reported as MCP errors containing the data-model heading that was breached.
- Data returned to MCP clients (e.g., rendered specs, dependency graphs) MUST retain source references so downstream tools can trace each datum back to its origin document within the workspace.
- Resource handles resolved via `spec://`, `impl://`, or `scratch://` MUST be normalized through workspace discovery, bound to canonical artifact paths, and rejected when they refer to artifacts outside the active workspace. Normalized handles MUST retain stable identifiers so MCP clients can reuse them across sessions.
- `/dependencies` handles MUST be treated as derived read-only locators whose responses are generated exclusively by dependency mapping services; mutation attempts against these handles MUST fail with an MCP error explaining that only query operations are supported.
- `/constraints` handles MUST be treated as derived read-only locators whose responses are generated exclusively by structure discovery services; mutation attempts against these handles MUST fail with an MCP error explaining that only query operations are supported.
- Prompt catalog and lifecycle tools MUST reference template locators resolved via SpecMan Core template orchestration (workspace pointer files first, then packaged defaults) and validate that supplied names comply with the [founding specification](../../docs/founding-spec.md). MCP lifecycle tools MUST NOT perform sampling or elicitation; all inputs MUST be supplied directly by the caller.

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
- MUST include completion capability metadata for MCP surfaces that support completion, including accepted handle kinds and whether degraded completion warnings are emitted via MCP warning logging.
- MUST embed JSON Schema fragments that match the SpecMan Data Model serialization for the capability's inputs/outputs.
- Each dedicated lifecycle tool (`create_specification`, `create_implementation`, `create_revision`, `create_feature`, `create_refactor`, `create_fix`, `update_specification`, `update_implementation`, `update_revision`, `update_feature`, `update_refactor`, `update_fix`) MUST have its own capability descriptor.
- MAY reference implementation-specific extensions, but those entries MUST carry a `type: extension` label and cite the owning specification or implementation path.

## Additional Notes

- MCP deployments MAY shard workspaces across multiple processes, but every shard MUST adhere to this specification and expose a single consolidated capability catalog to clients.
- Implementers MAY offer read-only planning tools as separate capabilities so MCP clients can request previews before persisting changes; preview responses MUST clearly indicate they are non-persistent.
- Adapters MAY reuse off-the-shelf MCP libraries or frameworks; compliance is measured by the behavior defined in this document, not by re-implementing the protocol stack.
- Because deployments are STDIN-based on local machines, additional network security controls are OPTIONAL; nonetheless, implementers SHOULD ensure logging and locking remain in place to preserve SpecMan Core guarantees.
- MCP adapters SHOULD document the mapping between resource handles and human-readable artifact names so that clients can prompt users before invoking lifecycle operations.
- The `/dependencies` and `/constraints` suffixes are RESERVED for MCP adapters and MUST NOT be repurposed for mutation flows or unrelated data; adapters MAY introduce additional read-only suffixes in future revisions provided they extend the resource-handle schema consistently.
