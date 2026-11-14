//! `oxidizr` binary entry point. Exit codes per P1-D8: 0 ok, 1 diagnostics/errors,
//! 2 usage (clap), 3 I/O (M6 refinement).
// See oxidizr-cli lib.rs: deps declared for the milestone surface before use.
#![allow(unused_crate_dependencies)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match oxidizr_cli::run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(err) => {
            // M6 replaces this with miette rendering + snippets (P1-D2).
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
