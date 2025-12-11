use specman_mcp::run_stdio_server;

#[tokio::main]
async fn main() {
    if let Err(err) = run_stdio_server().await {
        eprintln!("specman-mcp server failed: {err}");
        std::process::exit(1);
    }
}
