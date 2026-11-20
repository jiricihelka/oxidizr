//! The `oxidizr-viewer` binary: a thin argv shim over
//! [`oxidizr_viewer::shell`] (§P3-M8) — the ONLY other GUI surface is
//! the `gui`-gated shell module itself, which the standalone
//! `oxidizr view` subcommand of `oxidizr-cli` (cli feature `gui`) also
//! drives, so the two entries can never drift.

use std::path::PathBuf;

use clap::Parser;
use oxidizr_viewer::shell::{self, ThemeArg};

/// CLI arguments (flags + positional only, `agents/plan/WORKFLOW.md`).
#[derive(Parser)]
#[command(
    name = "oxidizr-viewer",
    version,
    about = "Native Structurizr workspace viewer + layout editor (P3)"
)]
struct Args {
    /// Workspace file to open at startup (`.dsl` or `.json`).
    file: Option<PathBuf>,
    /// Color scheme; `system` consults the OS preference, then the
    /// persisted runtime toggle (§P3-D7).
    #[arg(long, value_enum, default_value_t = ThemeArg::System)]
    theme: ThemeArg,
}

fn main() -> eframe::Result {
    let args = Args::parse();
    shell::run(args.file.as_deref(), args.theme)
}
