//! The stdio binary: the ONLY OS-facing surface of `oxidizr-lsp`
//! (§P4-LSP L-1/L-2). Everything real lives in
//! [`oxidizr_lsp::server`]; here it is `Connection::stdio()` + the
//! shared `run` loop, nothing else.

use std::process::ExitCode;

/// `--version`/`-V` answer and exit (the P4-D10 binary smoke hook);
/// any other argv opens the stdio server loop.
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.any(|arg| arg == "--version" || arg == "-V") {
        println!("oxidizr-lsp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (connection, io_threads) = lsp_server::Connection::stdio();
    // `run` returns on `exit` (clean) or when the client vanishes;
    // the io threads must be joined before process exit so the last
    // responses actually reach the pipe. (io_threads.join() speaks
    // std::io::Error — lifted into the anyhow value.)
    let joined = oxidizr_lsp::server::run(&connection)
        .and_then(|()| io_threads.join().map_err(anyhow::Error::new));
    match joined {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("oxidizr-lsp: {error}");
            ExitCode::FAILURE
        }
    }
}
