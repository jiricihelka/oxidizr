//! `oxidizr-cli` — the user-facing binary (P1-D8). Command surface is defined here
//! from day one (stable flags); implementations land with their milestones.
//!
//! Exit codes: `0` ok · `1` diagnostics errors (or warnings with `--strict`) ·
//! `2` usage (clap default) · `3` I/O. `-` means stdin/stdout where a command takes a
//! file argument. No config files: flags + the documented env family only (AGENTS.md).
// P1-M0 skeleton: the full command surface (P1-D8) parses, but the pipeline deps are
// declared before use (json M3+, dsl M4/M5, miette rendering M6). Remove as they land.
#![allow(unused_crate_dependencies)]

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Structurizr-compatible C4 workspace tooling.
#[derive(Parser, Debug)]
#[command(name = "oxidizr", version, about, propagate_version = true)]
// Global flags are the stable P1-D8 CLI surface (AGENTS.md: flags + env only, no
// config files) — a bool per flag is the clap idiom, not state to model as enums.
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Treat warnings as errors (exit 1 on any warning diagnostic).
    #[arg(long, global = true)]
    pub strict: bool,
    /// Keep non-schema JSON keys on read (round-trip them on write; output is then
    /// intentionally not reference-parity — D3).
    #[arg(long, global = true)]
    pub keep_unknown: bool,
    /// Enable `${VAR}` environment-variable fallback (default off; parity default
    /// pinned by V-3).
    #[arg(long, global = true)]
    pub env: bool,
    /// Disable color output (also honored via `NO_COLOR`).
    #[arg(long, global = true)]
    pub no_color: bool,
    /// Suppress non-error output.
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "lowercase")] // stable surface: dsl2json / json2json (clap's
// default kebab-case would emit `dsl2-json`)
pub enum Command {
    /// Parse a DSL workspace and write standard JSON (`-` = stdin/stdout).
    Dsl2Json {
        /// Input `.dsl` file (`-` or omitted = stdin).
        dsl: Option<PathBuf>,
        /// Output `.json` file (omitted = stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Read standard JSON and write standard JSON (normalization check).
    Json2Json {
        /// Input `.json` workspace (`-` or omitted = stdin).
        json: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Validate workspace file(s); format auto-detected by extension.
    Validate {
        /// Workspace files to validate.
        files: Vec<PathBuf>,
    },
    /// Reformat DSL source (canonical style, P1-D7 invariants).
    Fmt {
        /// DSL files to format.
        files: Vec<PathBuf>,
        /// Rewrite files in place.
        #[arg(short, long)]
        write: bool,
        /// Exit 1 if any file is not formatted (no output rewrite).
        #[arg(long)]
        check: bool,
    },
    /// Remap layout-JSON ids onto a re-parsed DSL (F1; geometry-preserving edit loop).
    Reconcile {
        /// Re-parsed (edited) DSL workspace.
        #[arg(long)]
        dsl: PathBuf,
        /// Layout JSON produced from the pre-edit workspace.
        #[arg(long)]
        layout: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Run the CLI. Returns the process exit code (usage errors exit via clap with `2`).
///
/// M0 scaffold: the full surface parses; commands report `not implemented yet`
/// (exit `1`) until their milestones land (json: M3+; dsl: M4/M5; fmt/reconcile: M6).
///
/// # Errors
///
/// [`anyhow::Error`] on command failure (diagnostics with errors, I/O problems, or —
/// until the milestones land — invoking an unimplemented subcommand). Usage errors
/// never reach `Err`: clap exits `2` directly.
pub fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    anyhow::bail!(
        "`oxidizr` scaffold (P1-M0): subcommand `{}` lands in a later milestone",
        command_name(&cli.command)
    );
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Dsl2Json { .. } => "dsl2json",
        Command::Json2Json { .. } => "json2json",
        Command::Validate { .. } => "validate",
        Command::Fmt { .. } => "fmt",
        Command::Reconcile { .. } => "reconcile",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_parses() {
        Cli::try_parse_from(["oxidizr", "dsl2json", "ws.dsl", "-o", "ws.json"]).unwrap();
        Cli::try_parse_from(["oxidizr", "json2json", "-"]).unwrap();
        Cli::try_parse_from(["oxidizr", "validate", "a.dsl", "b.json"]).unwrap();
        Cli::try_parse_from(["oxidizr", "fmt", "--check", "x.dsl"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "--strict",
            "reconcile",
            "--dsl",
            "e.dsl",
            "--layout",
            "l.json",
        ])
        .unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "merge"]).is_err());
    }

    #[test]
    fn command_names_stable() {
        let cli = Cli::try_parse_from(["oxidizr", "dsl2json"]).unwrap();
        assert_eq!(command_name(&cli.command), "dsl2json");
    }
}
