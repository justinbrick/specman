use std::env;
use std::path::PathBuf;

use specman_mcp::run_stdio_server_with_root;

#[tokio::main]
async fn main() {
    let mut args = env::args().skip(1);
    let mut workspace_root: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        if let Some(path) = arg.strip_prefix("--workspace=") {
            workspace_root = Some(PathBuf::from(path));
        } else if arg == "--workspace" {
            match args.next() {
                Some(path) => workspace_root = Some(PathBuf::from(path)),
                None => {
                    eprintln!("--workspace requires a path argument");
                    std::process::exit(1);
                }
            }
        } else {
            eprintln!("unknown argument: {arg}");
            std::process::exit(1);
        }
    }

    if let Err(err) = run_stdio_server_with_root(workspace_root).await {
        eprintln!("specman-mcp server failed: {err}");
        std::process::exit(1);
    }
}
