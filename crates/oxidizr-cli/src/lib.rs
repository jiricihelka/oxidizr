//! `oxidizr-cli` — the user-facing binary (P1-D8). Command surface is defined here
//! from day one (stable flags); implementations land with their milestones.
//!
//! Exit codes: `0` ok · `1` diagnostics errors (or warnings with `--strict`) ·
//! `2` usage (clap default) · `3` I/O. `-` means stdin/stdout where a command takes a
//! file argument. No config files: flags + the documented env family only (agents/plan/WORKFLOW.md §CLI/config policy).
//!
//! M6-a landed `dsl2json` / `json2json` / `validate` (miette-rendered
//! diagnostics, JSON-base loader injected for `workspace extends <*.json>`,
//! D3 `--keep-unknown` capture through `oxidizr_json::roundtrip_keep_unknown`);
//! M6-b landed `fmt` (the P1-D7 printer lives in `oxidizr_dsl::fmt` — this
//! crate only adds the file/stdin plumbing and the check/write modes);
//! M6-c landed `reconcile` (the F1 geometry re-attach, [`reconcile`], plus
//! its OXZ-3002 ambiguity code). Input decoding is UTF-8-only at v0.1
//! (V-8 resolved: no `--encoding` flag; non-UTF-8 input is an OXZ-5006 exit-3
//! failure like any other I/O problem).

pub mod reconcile;
pub mod report;
pub use oxidizr_api::suggest;

use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use oxidizr_core::Workspace;
use oxidizr_diag::codes;
use oxidizr_dsl::engine::{ParseOptions, ParseResult, parse_dsl};

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
/// # Errors
///
/// [`anyhow::Error`] only for unimplemented milestones / internal faults.
pub fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    let renderer = Renderer::new(cli.no_color, cli.quiet);
    let code = match &cli.command {
        Command::Dsl2Json { dsl, output } => dsl2json(&renderer, dsl.as_deref(), output.as_deref()),
        Command::Json2Json { json, output } => {
            json2json(&cli, &renderer, json.as_deref(), output.as_deref())
        }
        Command::Validate { files } => validate(&cli, &renderer, files),
        Command::Fmt {
            files,
            write,
            check,
        } => fmt_cmd(&renderer, files, *write, *check),
        Command::Reconcile {
            dsl,
            layout,
            output,
        } => reconcile_cmd(&renderer, dsl, layout, output.as_deref()),
    };
    Ok(code)
}

/// `oxidizr-json` as the engine's `.json` base loader for
/// `workspace extends <file.json>` (topology P1-D1: the dsl crate must not
/// depend on json — the CLI injects it here).
fn json_loader(path: &str) -> Result<Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// One decoded input: engine-visible file path (canonical, `None` for stdin
/// so the reference `parse(String)` quirks hold) + UTF-8 text + display name.
struct Input {
    file: Option<String>,
    text: String,
    display: String,
}

/// Read `-`/stdin or a file; strict UTF-8 (V-8). Errors are OXZ-5006 issues
/// (exit 3 family).
fn read_input(path: Option<&Path>) -> Result<Input, Issue> {
    if path.is_none_or(|p| p == Path::new("-")) {
        let mut buf = Vec::new();
        std::io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| Issue::io("failed to read stdin", &e))?;
        let text = decode(&buf, "-")?;
        return Ok(Input {
            file: None,
            text,
            display: "-".to_string(),
        });
    }
    let path = path.unwrap_or_else(|| Path::new("-"));
    let bytes = std::fs::read(path)
        .map_err(|e| Issue::io(&format!("failed to read `{}`", path.display()), &e))?;
    let text = decode(&bytes, &path.display().to_string())?;
    let file = std::fs::canonicalize(path)
        .ok()
        .map(|p| p.display().to_string())
        .or_else(|| Some(path.display().to_string()));
    Ok(Input {
        file,
        text,
        display: path.display().to_string(),
    })
}

/// V-8 (resolved): UTF-8 only at v0.1 — no `--encoding` flag; the reference
/// `parse(File)` reads the platform charset, oxidizr pins UTF-8 and fails
/// loudly instead of garbling.
fn decode(bytes: &[u8], name: &str) -> Result<String, Issue> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        Issue::error(
            codes::CLI_IO,
            format!("`{name}` is not valid UTF-8 (oxidizr reads UTF-8 only)"),
        )
    })
}

/// Write exact bytes to `-o`/file or stdout (no trailing newline added —
/// stdout carries the same bytes as the file).
fn write_output(bytes: &[u8], output: Option<&Path>) -> Result<(), Issue> {
    match output {
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(bytes)
                .and_then(|()| stdout.flush())
                .map_err(|e| Issue::io("failed to write stdout", &e))
        }
        Some(p) => std::fs::write(p, bytes)
            .map_err(|e| Issue::io(&format!("failed to write `{}`", p.display()), &e)),
    }
}

/// Parse a DSL input with production options (JSON-base loader injected).
/// The `Err` arm is the fatal statement-level failure (empty source) — exit 1.
fn parse_dsl_input(input: &Input) -> Result<ParseResult, Issue> {
    let opts = match &input.file {
        Some(f) => ParseOptions {
            json_loader: Some(json_loader),
            env_lookup: None,
            ..ParseOptions::file(f)
        },
        None => ParseOptions {
            file: None,
            base_dir: None,
            json_loader: Some(json_loader),
            env_lookup: None,
        },
    };
    parse_dsl(&input.text, opts).map_err(|e| suggest::decorate(Issue::from_dsl(&e), None))
}

/// Render a batch and report whether any issue blocks exit 1 (severity vs
/// `--strict`).
fn emit(renderer: &Renderer, issues: &[Issue], strict: bool) -> bool {
    renderer.issues(issues);
    issues.iter().any(|i| i.blocks(strict))
}

fn dsl2json(renderer: &Renderer, input: Option<&Path>, output: Option<&Path>) -> i32 {
    let source = match read_input(input) {
        Ok(s) => s,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 3;
        }
    };
    let result = match parse_dsl_input(&source) {
        Ok(r) => r,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 1;
        }
    };
    if !result.errors.is_empty() {
        // D1 recovery: all recorded statement errors render; the reference
        // export would have aborted on the first one — no partial output.
        let issues: Vec<Issue> = result
            .errors
            .iter()
            .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&result.workspace)))
            .collect();
        renderer.issues(&issues);
        return 1;
    }
    let bytes = match oxidizr_json::to_vec(&result.workspace) {
        Ok(b) => b,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&Issue::from_json(
                &e,
                &source.text,
                &source.display,
            )));
            return 1;
        }
    };
    match write_output(&bytes, output) {
        Ok(()) => 0,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            3
        }
    }
}

fn json2json(cli: &Cli, renderer: &Renderer, input: Option<&Path>, output: Option<&Path>) -> i32 {
    let source = match read_input(input) {
        Ok(s) => s,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 3;
        }
    };
    let bytes = source.text.as_bytes();
    let produced = if cli.keep_unknown {
        // D3 superset mode: unknown / legacy / writer-omitted members ride
        // through verbatim (output intentionally non-parity).
        oxidizr_json::roundtrip_keep_unknown(bytes)
    } else {
        oxidizr_json::from_slice(bytes).and_then(|ws| oxidizr_json::to_vec(&ws))
    };
    match produced {
        Ok(b) => match write_output(&b, output) {
            Ok(()) => 0,
            Err(e) => {
                renderer.issues(std::slice::from_ref(&e));
                3
            }
        },
        Err(e) => {
            renderer.issues(std::slice::from_ref(&Issue::from_json(
                &e,
                &source.text,
                &source.display,
            )));
            1
        }
    }
}

/// `reconcile` (F1, P1-D8): re-parse the edited DSL (the model authority),
/// load the pre-edit layout JSON (the geometry authority), remap ids
/// (identifier exact → canonical fallback, ambiguity = OXZ-3002, never
/// guessed) and graft the layout. The standard JSON is emitted FIRST (the
/// contract emits even when ambiguous), then the human report on stderr
/// (`--quiet` silences it); ambiguous entries exit 1, I/O 3, else 0.
fn reconcile_cmd(renderer: &Renderer, dsl: &Path, layout: &Path, output: Option<&Path>) -> i32 {
    let source = match read_input(Some(dsl)) {
        Ok(s) => s,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 3;
        }
    };
    let result = match parse_dsl_input(&source) {
        Ok(r) => r,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 1;
        }
    };
    if !result.errors.is_empty() {
        let issues: Vec<Issue> = result
            .errors
            .iter()
            .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&result.workspace)))
            .collect();
        renderer.issues(&issues);
        return 1;
    }
    let layout_source = match read_input(Some(layout)) {
        Ok(s) => s,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&e));
            return 3;
        }
    };
    let layout_ws = match oxidizr_json::from_slice(layout_source.text.as_bytes()) {
        Ok(ws) => ws,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&Issue::from_json(
                &e,
                &layout_source.text,
                &layout_source.display,
            )));
            return 1;
        }
    };
    let (reconciled, report) = reconcile::reconcile(&layout_ws, &result.workspace);
    let bytes = match oxidizr_json::to_vec(&reconciled) {
        Ok(b) => b,
        Err(e) => {
            renderer.issues(std::slice::from_ref(&Issue::from_json(
                &e,
                &source.text,
                &source.display,
            )));
            return 1;
        }
    };
    if let Err(e) = write_output(&bytes, output) {
        renderer.issues(std::slice::from_ref(&e));
        return 3;
    }
    renderer.note(&report.summary());
    for line in &report.dropped {
        renderer.note(&format!("dropped: {line}"));
    }
    renderer.issues(&report.ambiguous);
    i32::from(report.ambiguous.iter().any(|i| i.blocks(false)))
}

/// `validate`: format by extension (P1-D8), DSL parse or JSON read + hydrate,
/// then the M2 validation catalog on the loaded workspace. Exit: 1 when any
/// issue blocks (severity vs `--strict`), else 3 on I/O trouble, else 0.
fn validate(cli: &Cli, renderer: &Renderer, files: &[PathBuf]) -> i32 {
    let strict = cli.strict;
    let mut blocked = false;
    let mut io_error = false;
    for path in files {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let source = if let Some("dsl" | "json") = ext.as_deref() {
            match read_input(Some(path)) {
                Ok(s) => s,
                Err(e) => {
                    renderer.issues(std::slice::from_ref(&e));
                    io_error = true;
                    continue;
                }
            }
        } else {
            blocked |= emit(
                renderer,
                std::slice::from_ref(&Issue::error(
                    codes::CLI_IO,
                    format!(
                        "cannot determine the workspace format of `{}` (expected `.dsl` or `.json`)",
                        path.display()
                    ),
                )),
                strict,
            );
            continue;
        };
        let workspace = if ext.as_deref() == Some("dsl") {
            match parse_dsl_input(&source) {
                Ok(r) if r.errors.is_empty() => r.workspace,
                Ok(r) => {
                    let issues: Vec<Issue> = r
                        .errors
                        .iter()
                        .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&r.workspace)))
                        .collect();
                    blocked |= emit(renderer, &issues, strict);
                    continue;
                }
                Err(e) => {
                    blocked |= emit(renderer, std::slice::from_ref(&e), strict);
                    continue;
                }
            }
        } else {
            match oxidizr_json::from_slice(source.text.as_bytes()) {
                Ok(ws) => ws,
                Err(e) => {
                    blocked |= emit(
                        renderer,
                        std::slice::from_ref(&Issue::from_json(&e, &source.text, &source.display)),
                        strict,
                    );
                    continue;
                }
            }
        };
        let violations = oxidizr_core::validate(&workspace.model, &workspace.views);
        if violations.is_empty() {
            renderer.ok(&format!("{}: ok", source.display));
        } else {
            let issues: Vec<Issue> = violations
                .iter()
                .map(|v| Issue::validation(&v.message))
                .collect();
            blocked |= emit(renderer, &issues, strict);
        }
    }
    if blocked {
        1
    } else if io_error {
        3
    } else {
        0
    }
}

/// `fmt`: reformat DSL sources to the canonical P1-D7 style. The printer is
/// `oxidizr_dsl::fmt::format` (it never validates — an invalid statement is
/// re-printed token-identically and fails to parse exactly as before).
/// Modes (P1-D8): default prints the formatted text of every input to stdout;
/// `--write` rewrites files in place, only when they change; `--check` lists
/// the unformatted files and exits `1`. Stdin has no in-place target:
/// `--write` with `-` is a usage error (exit `2`). I/O and UTF-8 failures are
/// OXZ-5006 → exit `3`.
fn fmt_cmd(renderer: &Renderer, files: &[PathBuf], write: bool, check: bool) -> i32 {
    // No file arguments = stdin (P1-D8: `-` stands for stdin/stdout).
    let inputs: Vec<Option<&Path>> = if files.is_empty() {
        vec![None]
    } else {
        files.iter().map(|p| Some(p.as_path())).collect()
    };
    let mut unformatted = false;
    let mut io_error = false;
    for path in inputs {
        let stdin = path.is_none_or(|p| p == Path::new("-"));
        if write && stdin {
            renderer.issues(std::slice::from_ref(&Issue::error(
                codes::CLI_IO,
                "`fmt --write` needs a file to rewrite (stdin has no in-place target)",
            )));
            return 2; // usage error (P1-D8 code 2), not an I/O failure
        }
        let input = match read_input(path) {
            Ok(i) => i,
            Err(e) => {
                renderer.issues(std::slice::from_ref(&e));
                io_error = true;
                continue;
            }
        };
        let formatted = oxidizr_dsl::fmt::format(&input.text);
        if check {
            if formatted != input.text {
                unformatted = true;
                renderer.ok(&input.display);
            }
        } else if write {
            if formatted != input.text {
                // Only changed files are rewritten (mtime stability).
                if let Err(e) = write_output(formatted.as_bytes(), path) {
                    renderer.issues(std::slice::from_ref(&e));
                    io_error = true;
                } else {
                    renderer.ok(&input.display);
                }
            }
        } else if let Err(e) = write_output(formatted.as_bytes(), None) {
            renderer.issues(std::slice::from_ref(&e));
            io_error = true;
        }
    }
    if unformatted {
        1
    } else if io_error {
        3
    } else {
        0
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
