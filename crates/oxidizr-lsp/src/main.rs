//! The stdio binary: the ONLY OS-facing surface of `oxidizr-lsp`.
//! Everything real lives in [`oxidizr_lsp::server`]; here it is
//! `Connection::stdio()` + the shared `run` loop, nothing else.

use std::process::ExitCode;

/// `--version`/`-V` answer and exit (what the binary smoke test runs);
/// any other argv opens the stdio server loop.
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    if args.any(|arg| arg == "--version" || arg == "-V") {
        println!("oxidizr-lsp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (connection, io_threads) = lsp_server::Connection::stdio();
    // `run` returns on `exit` (clean) or when the client vanishes. It
    // OWNS the connection: returning drops our sender half, which is
    // what makes lsp-server's writer thread finish — joining the io
    // threads BEFORE that drop would hang forever.
    let result = oxidizr_lsp::server::run(connection);
    match result.and_then(|()| io_threads.join().map_err(anyhow::Error::new)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("oxidizr-lsp: {error}");
            ExitCode::FAILURE
        }
    }
}
