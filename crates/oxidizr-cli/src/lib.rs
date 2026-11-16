//! `oxidizr-cli` — the user-facing binary (P1-D8). Command surface is defined
//! here from day one (stable flags).
//!
//! Exit codes: `0` ok · `1` diagnostics errors (or warnings with `--strict`) ·
//! `2` usage (clap default) · `3` I/O. `-` means stdin/stdout where a command takes a
//! file argument. No config files: flags + the documented env family only (agents/plan/WORKFLOW.md §CLI/config policy).
//!
//! M6-a landed `dsl2json` / `json2json` / `validate` (miette-rendered
//! diagnostics, JSON-base loader injected for `workspace extends <*.json>`,
//! D3 `--keep-unknown` capture through `oxidizr_json::roundtrip_keep_unknown`);
//! M6-b landed `fmt` (the P1-D7 printer lives in `oxidizr_dsl::fmt`);
//! M6-c landed `reconcile` (the F1 geometry re-attach, [`reconcile`], plus
//! its OXZ-3002 ambiguity code). Input decoding is UTF-8-only at v0.1
//! (V-8 resolved: no `--encoding` flag; non-UTF-8 input is an OXZ-5006 exit-3
//! failure like any other I/O problem).
//!
//! Since Phase 1T T0-d the command bodies live in [`oxidizr_api`]
//! (`Request → execute() → Outcome`); this crate adds ONLY the argv surface,
//! the miette rendering ([`report`]) and the exit-code mapping. `run()` adds
//! no command logic — the invariant that keeps the binary and the e2e suite
//! (which drives the API in-process) in lockstep
//! (`agents/plan/build/PHASE_1T.md` P1T-D1).

pub mod report;
pub use oxidizr_api::{reconcile, suggest};

use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use oxidizr_api::{ApiOptions, ExitClass, FmtMode, Input, LogItem, Outcome, Request};

use report::{Issue, Renderer};

/// Structurizr-compatible C4 workspace tooling.
#[derive(Parser, Debug)]
#[command(name = "oxidizr", version, about, propagate_version = true)]
// Global flags are the stable P1-D8 CLI surface (agents/plan/WORKFLOW.md: flags + env only, no
// config files) — a bool per flag is the clap idiom, not state to model as enums.
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Treat warnings as errors (exit 1 on any warning diagnostic).
    #[arg(long, global = true)]
    pub strict: bool,
    /// Keep non-schema / writer-omitted JSON keys on read and re-write them
    /// (`json2json` only; superset round-trip, intentionally non-parity — D3).
    #[arg(long, global = true)]
    pub keep_unknown: bool,
    /// Request `${VAR}` environment-variable fallback. The reference parser
    /// enables env substitution by default (V-3), so it is ON here too; the
    /// flag is accepted for explicitness and future feature gates.
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
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// Reformat DSL source (canonical style, P1-D7 invariants).
    Fmt {
        /// DSL files to format (`-` or none = stdin; output goes to stdout
        /// unless `--write`).
        files: Vec<PathBuf>,
        /// Rewrite files in place (only files whose formatting changes).
        #[arg(short, long)]
        write: bool,
        /// Exit 1 if any file is not formatted (no output rewrite).
        #[arg(long, conflicts_with = "write")]
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
/// `Ok(0)` success · `Ok(1)` diagnostics with an error (or a warning under
/// `--strict`) · `Ok(3)` I/O failure; diagnostics are rendered by miette on
/// stderr before returning ([`report`]). Every P1 command has landed; `Err`
/// is reserved for internal faults and main maps it to exit `1`. Usage
/// errors never reach `Err`: clap exits `2` directly.
///
/// Pipeline (T0-d): argv → [`Request`] → [`oxidizr_api::execute`] → replay
/// the [`Outcome`] (stdout bytes, then the ordered log) → exit code. No
/// command logic here.
///
/// # Errors
///
/// [`anyhow::Error`] only for unimplemented milestones / internal faults.
pub fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    let renderer = Renderer::new(cli.no_color, cli.quiet);
    let options = ApiOptions {
        strict: cli.strict,
        keep_unknown: cli.keep_unknown,
        // Production keeps the V-3 process-env behavior (ApiOptions::env=None).
        env: None,
        stdin: Some(read_stdin),
    };
    let outcome = oxidizr_api::execute(to_request(&cli), &options);
    Ok(replay(&renderer, &outcome))
}

/// argv → Request mapping: `-`/omitted paths become the lazy [`Input::Stdin`]
/// variant (nothing is read before `execute` decides — a `fmt --write -`
/// usage exit must not block on an open pipe).
fn to_request(cli: &Cli) -> Request {
    match &cli.command {
        Command::Dsl2Json { dsl, output } => Request::Dsl2Json {
            input: input_of(dsl.as_deref()),
            output: output.clone(),
        },
        Command::Json2Json { json, output } => Request::Json2Json {
            input: input_of(json.as_deref()),
            output: output.clone(),
        },
        Command::Validate { files } => Request::Validate {
            files: files.clone(),
        },
        Command::Fmt {
            files,
            write,
            check,
        } => Request::Fmt {
            // No file arguments = stdin (P1-D8: `-` stands for stdin/stdout).
            inputs: if files.is_empty() {
                vec![Input::Stdin]
            } else {
                files.iter().map(|p| input_of(Some(p))).collect()
            },
            mode: if *check {
                FmtMode::Check
            } else if *write {
                FmtMode::Write
            } else {
                FmtMode::Print
            },
        },
        Command::Reconcile {
            dsl,
            layout,
            output,
        } => Request::Reconcile {
            dsl: input_of(Some(dsl)),
            layout: input_of(Some(layout)),
            output: output.clone(),
        },
    }
}

fn input_of(path: Option<&Path>) -> Input {
    match path {
        None => Input::Stdin,
        Some(p) if p == Path::new("-") => Input::Stdin,
        Some(p) => Input::File(p.to_path_buf()),
    }
}

/// The process-stdin reader the pipeline resolves `Input::Stdin` through
/// (OXZ-5006 exit-3 family on failure, like the pre-extraction code).
fn read_stdin() -> Result<Vec<u8>, Issue> {
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .map_err(|e| Issue::io("failed to read stdin", &e))?;
    Ok(buf)
}

/// Replay an [`Outcome`] onto the process streams: stdout payload first (a
/// write failure is the old `write_output` exit 3 and suppresses the log —
/// the extracted contract), then the ordered log. Consecutive issues render
/// as one batch (single stderr write, as before).
fn replay(renderer: &Renderer, outcome: &Outcome) -> i32 {
    if let Some(bytes) = &outcome.bytes {
        let mut stdout = std::io::stdout().lock();
        if let Err(e) = stdout.write_all(bytes).and_then(|()| stdout.flush()) {
            renderer.issues(std::slice::from_ref(&Issue::io(
                "failed to write stdout",
                &e,
            )));
            return ExitClass::Io.code();
        }
    }
    let mut batch: Vec<Issue> = Vec::new();
    for item in &outcome.log {
        match item {
            LogItem::Issue(issue) => batch.push(issue.clone()),
            LogItem::OkLine(line) => {
                flush(renderer, &mut batch);
                renderer.ok(line);
            }
            LogItem::NoteLine(line) => {
                flush(renderer, &mut batch);
                renderer.note(line);
            }
        }
    }
    flush(renderer, &mut batch);
    outcome.exit.code()
}

fn flush(renderer: &Renderer, batch: &mut Vec<Issue>) {
    if !batch.is_empty() {
        renderer.issues(batch);
        batch.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_name(command: &Command) -> &'static str {
        match command {
            Command::Dsl2Json { .. } => "dsl2json",
            Command::Json2Json { .. } => "json2json",
            Command::Validate { .. } => "validate",
            Command::Fmt { .. } => "fmt",
            Command::Reconcile { .. } => "reconcile",
        }
    }

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
        // `validate` needs at least one file (clap usage error, exit 2).
        assert!(Cli::try_parse_from(["oxidizr", "validate"]).is_err());
    }

    #[test]
    fn command_names_stable() {
        let cli = Cli::try_parse_from(["oxidizr", "dsl2json"]).unwrap();
        assert_eq!(command_name(&cli.command), "dsl2json");
    }
}
