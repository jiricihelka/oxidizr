//! `oxidizr` binary entry point. Exit codes per P1-D8: 0 ok, 1 diagnostics/errors,
//! 2 usage (clap), 3 I/O.
// Diagnostics are rendered (miette) and mapped to exit codes inside the library;
// this shim only converts the final code, so the linked workspace deps are all
// used by `oxidizr-cli` itself — hence the crate-level allow below.
#![allow(unused_crate_dependencies)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match oxidizr_cli::run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(err) => {
            // Unimplemented milestones (`fmt` M6-b, `reconcile` M6-c) and
            // internal faults; diagnostics are already on stderr by then.
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
