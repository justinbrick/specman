fn main() -> std::process::ExitCode {
    if let Err(err) = specman_mcp::bootstrap_stdio() {
        eprintln!("specman-mcp: {err}");
        return std::process::ExitCode::from(1);
    }

    std::process::ExitCode::SUCCESS
}
