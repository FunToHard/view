fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3
        || args[0] != "--enable-session"
        || args[1].is_empty()
        || !["--stdio", "--mcp"].contains(&args[2].as_str())
    {
        return Err("usage: view-tooling --enable-session <unique-session-id> --stdio|--mcp; no listener starts by default".into());
    }
    if args[2] == "--mcp" {
        #[cfg(feature = "mcp")]
        return view_tooling::mcp::run(args[1].clone());
        #[cfg(not(feature = "mcp"))]
        return Err("MCP feature is not enabled".into());
    }
    let mut session = view_tooling::fixture(args[1].clone())?;
    view_tooling::serve(
        &mut session,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    )?;
    Ok(())
}
