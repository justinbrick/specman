use std::fs;
use std::path::Path;

use futures::channel::mpsc;
use rmcp::model::{
    ClientRequest, ReadResourceRequest, ReadResourceRequestParams, ResourceContents,
    ServerResult,
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
            ServerResult::ReadResourceResult(result) => {
                let content = result.contents.into_iter().next().ok_or_else(|| {
                    ServiceError::McpError(rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "No content returned".into(),
                        data: None,
                    })
                })?;

                match content {
                    ResourceContents::TextResourceContents {
                        text, mime_type, ..
                    } => Ok((mime_type.unwrap_or_default(), text)),
                    _ => Err(ServiceError::McpError(rmcp::model::ErrorData {
                        code: rmcp::model::ErrorCode::INVALID_PARAMS,
                        message: "Expected text content".into(),
                        data: None,
                    })),
                }
            }
            _ => Err(ServiceError::McpError(rmcp::model::ErrorData {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                message: "Unexpected result type".into(),
                data: None,
            })),
        }
    }
}

fn create_workspace_files(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let spec_dir = root.join("spec/testspec");
    let impl_dir = root.join("impl/testimpl");
    let impl_src_dir = impl_dir.join("src");
    let dot_specman = root.join(".specman");

    fs::create_dir_all(&spec_dir)?;
    fs::create_dir_all(&impl_dir)?;
    fs::create_dir_all(&impl_src_dir)?;
    fs::create_dir_all(&dot_specman)?;

    fs::write(
        spec_dir.join("spec.md"),
        r"---
name: testspec
version: '0.1.0'
dependencies: []
---
# Spec Body
!concept-test.group:
- MUST be indexable
!concept-test.missed:
- MUST be missed
",
    )?;

    fs::write(
        impl_dir.join("impl.md"),
        r"---
name: testimpl
version: '0.1.0'
spec: spec://testspec
---
# Impl Body
",
    )?;

    // Add source file with compliance tag
    fs::write(
        impl_src_dir.join("main.rs"),
        r"// [ENSURES: concept-test.group:TEST]
fn main() {}
",
    )?;

    Ok(())
}

#[tokio::test]
async fn compliance_report_success() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    // [ENSURES: concept-compliance-resources.resources.location]
    let (mime, text) = ws.read_text_resource("impl://testimpl/compliance").await?;
    assert_eq!(mime, "application/json");

    let report: serde_json::Value = serde_json::from_str(&text)?;

    // [ENSURES: concept-compliance-reporting.interface]
    assert_eq!(report["implementation"]["name"], "testimpl");
    assert_eq!(report["specification"]["name"], "testspec");

    // Check coverage
    // [ENSURES: concept-compliance-reporting.coverage]
    let coverage = &report["coverage"];
    assert!(
        coverage.get("concept-test.group").is_some(),
        "should cover concept-test.group"
    );

    // Check missing
    // [ENSURES: concept-compliance-reporting.semantics]
    let missing = report["missing"]
        .as_array()
        .expect("missing should be array");
    assert!(
        missing.iter().any(|v| v == "concept-test.missed"),
        "should list concept-test.missing"
    );

    Ok(())
}

#[tokio::test]
async fn compliance_report_wrong_scheme() -> Result<(), Box<dyn std::error::Error>> {
    let ws = TestWorkspace::create().await?;

    // [ENSURES: concept-compliance-resources.scope.schemes]
    let err = ws.read_text_resource("spec://testspec/compliance").await;
    assert!(err.is_err());

    Ok(())
}
