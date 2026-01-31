use std::fs;
use std::io::Write;
use std::path::Path;

use futures::channel::mpsc;
use rmcp::model::{
    ClientRequest, ListResourceTemplatesRequest, ReadResourceRequest, ReadResourceRequestParams,
    ResourceContents, ServerResult,
};
use rmcp::{RoleClient, RoleServer, serve_client, serve_server};
use rmcp::service::{ServiceError, RunningService};
use specman_mcp::SpecmanMcpServer;
use tempfile::TempDir;

struct TestWorkspace {
    _temp: TempDir,
    _server: RunningService<RoleServer, SpecmanMcpServer>,
    client: RunningService<RoleClient, ()>,
}

impl TestWorkspace {
    async fn create() -> Result<Self, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        create_workspace_files(temp.path())?;

        let server = SpecmanMcpServer::new_with_root(temp.path())?;

        // Wire an in-memory duplex transport between a RoleClient and RoleServer.
        // client Tx == server Rx, and server Tx == client Rx.
        let (client_to_server_tx, server_incoming_rx) =
            mpsc::unbounded::<rmcp::service::TxJsonRpcMessage<RoleClient>>();
        let (server_to_client_tx, client_incoming_rx) =
            mpsc::unbounded::<rmcp::service::TxJsonRpcMessage<RoleServer>>();

        let server_transport = (server_to_client_tx, server_incoming_rx);
        let client_transport = (client_to_server_tx, client_incoming_rx);

        // Start both halves. The client performs initialization handshake.
        let server_task = tokio::spawn(async move { serve_server(server, server_transport).await });
        let client_task = tokio::spawn(async move { serve_client((), client_transport).await });

        // Ensure both sides are up. If either side fails initialization, surface it.
        let running_server = server_task
            .await
            .map_err(|e| format!("server task join failed: {e}"))??;

        let client = client_task
            .await
            .map_err(|e| format!("client task join failed: {e}"))??;

        Ok(Self {
            _temp: temp,
            _server: running_server,
            client,
        })
    }

    async fn list_templates(&self) -> Result<Vec<String>, ServiceError> {
        let result = self
            .client
            .send_request(ClientRequest::ListResourceTemplatesRequest(
                ListResourceTemplatesRequest::default(),
            ))
            .await?;

        match result {
            ServerResult::ListResourceTemplatesResult(r) => Ok(r
                .resource_templates
                .into_iter()
                .map(|t| t.raw.uri_template)
                .collect()),
            _other => Err(ServiceError::UnexpectedResponse),
        }
    }

    async fn read_text_resource(&self, uri: &str) -> Result<(String, String), ServiceError> {
        let request = ReadResourceRequest::new(ReadResourceRequestParams {
            uri: uri.to_string(),
            meta: None,
        });

        let result = self
            .client
            .send_request(ClientRequest::ReadResourceRequest(request))
            .await?;

        match result {
            ServerResult::ReadResourceResult(r) => {
                let first = r
                    .contents
                    .into_iter()
                    .next()
                    .ok_or(ServiceError::UnexpectedResponse)?;

                match first {
                    ResourceContents::TextResourceContents {
                        mime_type, text, ..
                    } => Ok((mime_type.unwrap_or_default(), text)),
                    _ => Err(ServiceError::UnexpectedResponse),
                }
            }
            _ => Err(ServiceError::UnexpectedResponse),
        }
    }
}

#[tokio::test]
async fn templates_include_constraints_templates() -> Result<(), Box<dyn std::error::Error>> {
    // This replaces the old unit-test-only coverage by asserting that templates
    // are discoverable via the MCP protocol method `resources/templates/list`.
    let ws = TestWorkspace::create().await?;

    let uris = ws.list_templates().await?;

    for expected in [
        "spec://{artifact}/constraints",
        "spec://{artifact}/constraints/{constraint_id}",
    ] {
        assert!(uris.contains(&expected.to_string()), "missing {expected}");
    }

    Ok(())
}

#[tokio::test]
async fn constraints_index_routes_and_returns_expected_json()
-> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    let (mime, text) = ws.read_text_resource("spec://testspec/constraints").await?;
    assert_eq!(mime, "application/json");

    let value: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(value["artifact"], "spec://testspec");

    let items = value["constraints"]
        .as_array()
        .expect("constraints should be a JSON array");

    // Deterministic ordering (document order) is validated by checking index positions.
    assert_eq!(items.len(), 5, "expected five constraints");
    assert_eq!(items[0]["constraint_id"], "concept-test.group");
    assert_eq!(items[0]["identifier_line"], "!concept-test.group:");
    assert_eq!(
        items[0]["uri"],
        "spec://testspec/constraints/concept-test.group"
    );

    assert_eq!(items[1]["constraint_id"], "concept-test.other");
    assert_eq!(items[2]["constraint_id"], "concept-a.group");
    assert_eq!(items[3]["constraint_id"], "concept-a.group.b");
    assert_eq!(items[4]["constraint_id"], "concept-Case.Group");

    Ok(())
}

#[tokio::test]
async fn constraints_index_trailing_slash_routes_to_index() -> Result<(), Box<dyn std::error::Error>>
{
    let ws = TestWorkspace::create().await?;
    let (mime, _text) = ws
        .read_text_resource("spec://testspec/constraints/")
        .await?;
    assert_eq!(mime, "application/json");
    Ok(())
}

#[tokio::test]
async fn constraints_index_empty_spec_returns_empty_list() -> Result<(), Box<dyn std::error::Error>>
{
    let ws = TestWorkspace::create().await?;
    let (mime, text) = ws.read_text_resource("spec://empty/constraints").await?;
    assert_eq!(mime, "application/json");

    let value: serde_json::Value = serde_json::from_str(&text)?;
    let items = value["constraints"]
        .as_array()
        .expect("constraints should be a JSON array");
    assert!(items.is_empty());

    Ok(())
}

#[tokio::test]
async fn constraints_rejects_non_spec_scheme() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    let err = ws
        .read_text_resource("impl://testimpl/constraints")
        .await
        .err()
        .expect("expected an error");

    match err {
        ServiceError::McpError(ed) => {
            assert!(
                ed.message.contains("only available") || ed.message.contains("spec://"),
                "unexpected error message: {ed:?}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn constraints_double_slash_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    let err = ws
        .read_text_resource("spec://testspec/constraints//")
        .await
        .err()
        .expect("expected an error");

    match err {
        ServiceError::McpError(ed) => {
            assert!(
                ed.message.contains("double slash") || ed.message.contains("malformed"),
                "unexpected error message: {ed:?}"
            );
        }
        other => panic!("unexpected error type: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn constraint_content_returns_context() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    let (mime, text) = ws
        .read_text_resource("spec://testspec/constraints/concept-test.group")
        .await?;
    assert_eq!(mime, "text/markdown");

    assert!(text.contains("!concept-test.group:"));
    assert!(text.contains("- MUST be indexable"));

    // Now includes other groups' identifier lines or statements because they share the heading.
    assert!(text.contains("!concept-test.other:"));
    assert!(text.contains("- MUST be discoverable"));

    Ok(())
}

#[tokio::test]
async fn constraint_content_includes_additional_markdown() -> Result<(), Box<dyn std::error::Error>>
{
    let ws = TestWorkspace::create().await?;

    let (_mime, text) = ws
        .read_text_resource("spec://testspec/constraints/concept-a.group")
        .await?;

    assert!(text.contains("!concept-a.group:"));
    assert!(text.contains("- MUST be short"));
    assert!(
        text.contains("Additional context paragraph"),
        "additional markdown content must be included"
    );

    // Includes next constraint group because they share the heading.
    assert!(text.contains("!concept-a.group.b:"));

    Ok(())
}

#[tokio::test]
async fn constraint_content_missing_constraint_errors_with_context()
-> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    let err = ws
        .read_text_resource("spec://testspec/constraints/concept-test.missing")
        .await
        .err()
        .expect("expected an error");

    match err {
        ServiceError::McpError(ed) => {
            assert!(ed.message.contains("concept-test.missing"), "{ed:?}");
            assert!(ed.message.contains("spec/testspec/spec.md"), "{ed:?}");
        }
        other => panic!("unexpected error type: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn constraint_id_is_case_sensitive() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    // Spec contains `concept-Case.Group` (mixed case).
    let err = ws
        .read_text_resource("spec://testspec/constraints/concept-case.group")
        .await
        .err()
        .expect("expected an error");

    match err {
        ServiceError::McpError(ed) => {
            assert!(ed.message.contains("not found"), "{ed:?}");
        }
        other => panic!("unexpected error type: {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn constraint_id_malformed_variants_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    for (uri, expected_substring) in [
        (
            "spec://testspec/constraints/!concept-test.group",
            "must not include leading",
        ),
        (
            "spec://testspec/constraints/concept-test.group:",
            "must not include leading",
        ),
        (
            "spec://testspec/constraints/concept/test.group",
            "must not include '/'",
        ),
        (
            "spec://testspec/constraints/concept test.group",
            "must not include whitespace",
        ),
    ] {
        let err = ws
            .read_text_resource(uri)
            .await
            .err()
            .expect("expected an error");

        match err {
            ServiceError::McpError(ed) => {
                assert!(
                    ed.message.contains(expected_substring) || ed.message.contains("constraint_id"),
                    "unexpected error message for {uri}: {ed:?}"
                );
            }
            other => panic!("unexpected error type for {uri}: {other:?}"),
        }
    }

    Ok(())
}

fn create_workspace_files(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let spec_dir = root.join("spec/testspec");
    let empty_spec_dir = root.join("spec/empty");
    let impl_dir = root.join("impl/testimpl");

    let template_dir = root.join("templates");
    let dot_specman_templates = root.join(".specman/templates");

    fs::create_dir_all(&spec_dir)?;
    fs::create_dir_all(&empty_spec_dir)?;
    fs::create_dir_all(&impl_dir)?;
    fs::create_dir_all(&template_dir)?;
    fs::create_dir_all(&dot_specman_templates)?;

    fs::write(
        spec_dir.join("spec.md"),
        r"---
name: testspec
version: '0.1.0'
dependencies: []
---

# Spec Body

## Constraints

!concept-test.group:
- MUST be indexable

!concept-test.other:
- MUST be discoverable

!concept-a.group:
- MUST be short

Additional context paragraph.

!concept-a.group.b:
- MUST not collide

!concept-Case.Group:
- MUST be case sensitive
",
    )?;

    fs::write(
        empty_spec_dir.join("spec.md"),
        r"---
name: empty
version: '0.1.0'
dependencies: []
---

# Empty Spec
",
    )?;

    fs::write(
        impl_dir.join("impl.md"),
        r"---
spec: spec://testspec
name: testimpl
version: '0.1.0'
---

# Impl Body
",
    )?;

    // Provide tokenized templates so create/update flows don't fall back to embedded defaults.
    fs::write(
        template_dir.join("impl.md"),
        r"---
spec: {{target_path}}
name: {{output_name}}
version: '0.1.0'
---

# Impl — {{output_name}}
",
    )?;

    fs::write(
        template_dir.join("scratch.md"),
        r"---
target: {{target_path}}
branch: main
work_type:
    {{work_type_kind}}: {}
---

# Scratch Pad — {{output_name}}
",
    )?;

    fs::write(dot_specman_templates.join("IMPL"), "templates/impl.md\n")?;
    fs::write(
        dot_specman_templates.join("SCRATCH"),
        "templates/scratch.md\n",
    )?;

    // Minimal scratch artifact to keep workspace inventory logic happy.
    let scratch_dir = root.join(".specman/scratchpad/testscratch");
    fs::create_dir_all(&scratch_dir)?;
    let scratch_content = r"---
target: impl://testimpl
branch: main
work_type:
  feat: {}
---

# Scratch Body
";
    let mut scratch_file = fs::File::create(scratch_dir.join("scratch.md"))?;
    scratch_file.write_all(scratch_content.as_bytes())?;

    Ok(())
}
