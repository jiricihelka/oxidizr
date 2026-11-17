//! The headless command pipeline (Phase 1T T0-d, `agents/plan/build/PHASE_1T.md`
//! P1T-D1): every P1 command body, ported verbatim from `oxidizr-cli` so the
//! binary and the e2e suite share one implementation.
//!
//! Contract ([`execute`]): values in / values out — never panics, never
//! renders, never touches the process env (hermetic [`oxidizr_dsl::engine::EnvLookup`]
//! map, `None` = production V-3 behavior) or stdout/stderr. All human output
//! rides the ordered [`Outcome::log`] (miette rendering and stream choice
//! stay CLI concerns) plus [`Outcome::bytes`] for stdout payloads. File
//! effects mirror the CLI exactly: inputs read from disk (`Input::File`,
//! canonicalized like the reference path semantics), `-o` rewrites and
//! `FmtMode::Write` in-place updates; `Input::Stdin` is resolved through the
//! injected [`StdinReader`] at the same point the old code read stdin (so a
//! `fmt --write -` usage exit never blocks on an open pipe).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_core::Workspace;
use oxidizr_diag::codes;
use oxidizr_dsl::engine::{EnvLookup, ParseOptions, ParseResult, parse_dsl};

use crate::Issue;
use crate::reconcile;
use crate::suggest;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Exit classes of the P1-D8 contract (`code()` maps to the process codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    /// Success.
    Ok,
    /// Diagnostics with an error (or a warning under `--strict`) — 1.
    Diagnostics,
    /// Usage error (the `fmt --write -` family; clap exits 2 before `execute`).
    Usage,
    /// I/O / input-policy failure — 3.
    Io,
}

impl ExitClass {
    /// The process exit code of the class.
    #[must_use]
    pub fn code(self) -> i32 {
        match self {
            ExitClass::Ok => 0,
            ExitClass::Diagnostics => 1,
            ExitClass::Usage => 2,
            ExitClass::Io => 3,
        }
    }
}

/// One human-output item in emission order. The CLI replay writes: issues
/// through the miette renderer (stderr), `OkLine` to stdout, `NoteLine` to
/// stderr — the stream split the command bodies used before extraction.
#[derive(Debug, Clone)]
pub enum LogItem {
    /// Diagnostic (severity decides `--strict` blocking, already applied to
    /// the exit class by the producing command).
    Issue(Issue),
    /// Non-error stdout chatter (the `renderer.ok` family).
    OkLine(String),
    /// Human report chatter on stderr (the `renderer.note` family).
    NoteLine(String),
}

/// Where a command reads its input from. `Bytes` carries stdin *semantics*
/// (no engine-visible file context — the reference `parse(String)` quirks
/// hold); `Stdin` is resolved lazily through [`ApiOptions::stdin`] exactly
/// where the pre-extraction code reached for stdin.
#[derive(Debug, Clone)]
pub enum Input {
    /// Read from disk (canonicalized for engine messages, like the CLI).
    File(PathBuf),
    /// In-memory bytes (no file context, no I/O).
    Bytes(Vec<u8>),
    /// Process stdin via the injected reader (CLI wiring).
    Stdin,
}

/// Reads the process stdin (CLI injects this; total — errors as values).
pub type StdinReader = fn() -> Result<Vec<u8>, Issue>;

/// Flags the command bodies consume (P1-D8 global flags; extension by
/// addition only).
#[derive(Debug, Clone, Default)]
pub struct ApiOptions {
    /// `--strict`: warnings block (exit 1).
    pub strict: bool,
    /// `--keep-unknown`: D3 superset round-trip (`json2json`).
    pub keep_unknown: bool,
    /// Hermetic `${VAR}` map (Phase 1T); `None` = the V-3 process-env
    /// default the production CLI keeps.
    pub env: Option<BTreeMap<String, String>>,
    /// `Input::Stdin` resolver; `None` rejects stdin inputs with a clean
    /// OXZ-5006 error (e2e never sends `Stdin`).
    pub stdin: Option<StdinReader>,
}

/// `fmt` modes (P1-D8): default prints to stdout (→ [`Outcome::bytes`]),
/// `Write` rewrites files in place (the only side-effecting mode), `Check`
/// lists unformatted inputs and exits 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FmtMode {
    /// Print formatted text (stdout bytes).
    Print,
    /// Rewrite input files in place, only when they change.
    Write,
    /// Report unformatted inputs; exit `Diagnostics`.
    Check,
}

/// One command invocation — the argv surface of P1-D8 in value form.
#[derive(Debug, Clone)]
pub enum Request {
    /// Parse DSL, emit standard JSON.
    Dsl2Json {
        /// DSL source.
        input: Input,
        /// Output file (`None` = stdout bytes on the outcome).
        output: Option<PathBuf>,
    },
    /// Re-serialize standard JSON (normalization / D3 superset).
    Json2Json {
        /// JSON source.
        input: Input,
        /// Output file (`None` = stdout bytes).
        output: Option<PathBuf>,
    },
    /// Validate workspace file(s); format by extension.
    Validate {
        /// Workspace files (at least one — like clap).
        files: Vec<PathBuf>,
    },
    /// Reformat DSL sources (P1-D7 printer).
    Fmt {
        /// Inputs (empty never happens: the CLI maps "no files" to `[Stdin]`).
        inputs: Vec<Input>,
        /// Mode.
        mode: FmtMode,
    },
    /// Re-attach layout JSON to a re-parsed DSL (F1).
    Reconcile {
        /// Edited DSL (model authority).
        dsl: Input,
        /// Layout JSON (geometry authority).
        layout: Input,
        /// Output file (`None` = stdout bytes).
        output: Option<PathBuf>,
    },
    /// Export the workspace (`export`, Phase 2): format strings per the
    /// reference CLI (`theme`, `json`, `plantuml…`, `mermaid`, …).
    Export {
        /// Workspace file (`.dsl` or `.json`) — the reference requires a
        /// file path (no stdin).
        workspace: PathBuf,
        /// Raw `--format` value (lowercased and parsed by `ExportFormat`).
        format: String,
        /// Output directory (`None` = the workspace file's directory,
        /// created as needed — reference `ExportCommand`).
        output: Option<PathBuf>,
    },
}

/// The full result of one [`execute`]: exit class, ordered human log, and
/// the stdout payload (if any). The CLI shim writes `bytes` first, then
/// replays `log`; a stdout write failure replaces the outcome with exit 3
/// and suppresses the log (exactly what the extracted code did at its
/// `write_output` step).
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Exit class.
    pub exit: ExitClass,
    /// Human output in emission order.
    pub log: Vec<LogItem>,
    /// Standard-output payload (JSON / formatted text); `None` = nothing.
    pub bytes: Option<Vec<u8>>,
}

impl Default for Outcome {
    fn default() -> Self {
        Outcome {
            exit: ExitClass::Ok,
            log: Vec::new(),
            bytes: None,
        }
    }
}

impl Outcome {
    fn fail(&mut self, issue: Issue, exit: ExitClass) -> &Self {
        self.log.push(LogItem::Issue(issue));
        self.exit = exit;
        self
    }
}

/// Run one command. Total and side-effect-free except the file effects the
/// CLI contract already had (`-o` writes, `FmtMode::Write`).
#[must_use]
pub fn execute(request: Request, opts: &ApiOptions) -> Outcome {
    match request {
        Request::Dsl2Json { input, output } => dsl2json(&input, output.as_deref(), opts),
        Request::Json2Json { input, output } => json2json(&input, output.as_deref(), opts),
        Request::Validate { files } => validate(&files, opts),
        Request::Fmt { inputs, mode } => fmt_cmd(inputs, mode, opts),
        Request::Reconcile {
            dsl,
            layout,
            output,
        } => reconcile_cmd(&dsl, &layout, output.as_deref(), opts),
        Request::Export {
            workspace,
            format,
            output,
        } => export_cmd(&workspace, &format, output.as_deref(), opts),
    }
}

// ---------------------------------------------------------------------------
// Input plumbing (moved from the CLI verbatim in behavior)
// ---------------------------------------------------------------------------

/// `oxidizr-json` as the engine's `.json` base loader for
/// `workspace extends <file.json>` (topology P1-D1: the dsl crate must not
/// depend on json — the pipeline injects it here).
fn json_loader(path: &str) -> Result<Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// One decoded input: engine-visible file path (canonical, `None` for stdin
/// so the reference `parse(String)` quirks hold) + UTF-8 text + display name.
struct Source {
    file: Option<String>,
    text: String,
    display: String,
}

/// Resolve an [`Input`]: stdin via the injected reader, bytes decoded, or
/// file read; errors are OXZ-5006 issues (exit 3 family).
fn read_input(input: &Input, opts: &ApiOptions) -> Result<Source, Issue> {
    match input {
        Input::Stdin => {
            let buf = match opts.stdin {
                Some(read) => read()?,
                None => {
                    return Err(Issue::error(
                        codes::CLI_IO,
                        "no stdin reader configured for this execution",
                    ));
                }
            };
            let text = decode(&buf, "-")?;
            Ok(Source {
                file: None,
                text,
                display: "-".to_string(),
            })
        }
        Input::Bytes(bytes) => {
            let text = decode(bytes, "-")?;
            Ok(Source {
                file: None,
                text,
                display: "-".to_string(),
            })
        }
        Input::File(path) => {
            let bytes = std::fs::read(path)
                .map_err(|e| Issue::io(&format!("failed to read `{}`", path.display()), &e))?;
            let text = decode(&bytes, &path.display().to_string())?;
            let file = std::fs::canonicalize(path)
                .ok()
                .map(|p| p.display().to_string())
                .or_else(|| Some(path.display().to_string()));
            Ok(Source {
                file,
                text,
                display: path.display().to_string(),
            })
        }
    }
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

/// Write exact bytes to an `-o` file (stdout payloads ride [`Outcome::bytes`]
/// — the CLI owns stdout writing). No trailing newline added: the file
/// carries the same bytes stdout would.
fn write_output_file(bytes: &[u8], path: &Path) -> Result<(), Issue> {
    std::fs::write(path, bytes)
        .map_err(|e| Issue::io(&format!("failed to write `{}`", path.display()), &e))
}

/// Parse a DSL input with pipeline options (JSON-base loader injected,
/// hermetic env map when [`ApiOptions::env`] is set). The `Err` arm is the
/// fatal statement-level failure (empty source) — exit 1.
fn parse_dsl_input(input: &Source, opts: &ApiOptions) -> Result<ParseResult, Issue> {
    let env_lookup = opts.env.as_ref().map(|map| {
        let map = map.clone();
        EnvLookup::new(move |name| map.get(name).cloned())
    });
    let opts = match &input.file {
        Some(f) => ParseOptions {
            json_loader: Some(json_loader),
            env_lookup,
            ..ParseOptions::file(f)
        },
        None => ParseOptions {
            json_loader: Some(json_loader),
            env_lookup,
            ..ParseOptions::fragment()
        },
    };
    parse_dsl(&input.text, opts).map_err(|e| suggest::decorate(Issue::from_dsl(&e), None))
}

/// Issues blocking exit 1 (severity vs `--strict`) — the old `emit` decision
/// without the rendering.
fn blocks(issues: &[Issue], strict: bool) -> bool {
    issues.iter().any(|i| i.blocks(strict))
}

fn push_issues(out: &mut Outcome, issues: Vec<Issue>) {
    out.log.extend(issues.into_iter().map(LogItem::Issue));
}

/// Emit produced JSON/text: `-o` file write (side effect, like the CLI) or
/// stdout payload on the outcome.
fn emit_bytes(out: &mut Outcome, bytes: Vec<u8>, output: Option<&Path>) -> Result<(), Issue> {
    match output {
        None => {
            out.bytes = Some(bytes);
            Ok(())
        }
        Some(p) => write_output_file(&bytes, p),
    }
}

// ---------------------------------------------------------------------------
// Command bodies (ported from oxidizr-cli; flow, order and messages identical)
// ---------------------------------------------------------------------------

fn dsl2json(input: &Input, output: Option<&Path>, opts: &ApiOptions) -> Outcome {
    let mut out = Outcome::default();
    let source = match read_input(input, opts) {
        Ok(s) => s,
        Err(e) => {
            out.fail(e, ExitClass::Io);
            return out;
        }
    };
    let result = match parse_dsl_input(&source, opts) {
        Ok(r) => r,
        Err(e) => {
            out.fail(e, ExitClass::Diagnostics);
            return out;
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
        push_issues(&mut out, issues);
        out.exit = ExitClass::Diagnostics;
        return out;
    }
    let bytes = match oxidizr_json::to_vec(&result.workspace) {
        Ok(b) => b,
        Err(e) => {
            out.fail(
                Issue::from_json(&e, &source.text, &source.display),
                ExitClass::Diagnostics,
            );
            return out;
        }
    };
    if let Err(e) = emit_bytes(&mut out, bytes, output) {
        out.fail(e, ExitClass::Io);
    }
    out
}

fn json2json(input: &Input, output: Option<&Path>, opts: &ApiOptions) -> Outcome {
    let mut out = Outcome::default();
    let source = match read_input(input, opts) {
        Ok(s) => s,
        Err(e) => {
            out.fail(e, ExitClass::Io);
            return out;
        }
    };
    let bytes = source.text.as_bytes();
    let produced = if opts.keep_unknown {
        // D3 superset mode: unknown / legacy / writer-omitted members ride
        // through verbatim (output intentionally non-parity).
        oxidizr_json::roundtrip_keep_unknown(bytes)
    } else {
        oxidizr_json::from_slice(bytes).and_then(|ws| oxidizr_json::to_vec(&ws))
    };
    match produced {
        Ok(b) => {
            if let Err(e) = emit_bytes(&mut out, b, output) {
                out.fail(e, ExitClass::Io);
            }
            out
        }
        Err(e) => {
            out.fail(
                Issue::from_json(&e, &source.text, &source.display),
                ExitClass::Diagnostics,
            );
            out
        }
    }
}

/// `reconcile` (F1, P1-D8): re-parse the edited DSL (the model authority),
/// load the pre-edit layout JSON (the geometry authority), remap ids
/// (identifier exact → canonical fallback, ambiguity = OXZ-3002, never
/// guessed) and graft the layout. The standard JSON is emitted FIRST (the
/// contract emits even when ambiguous), then the human report on stderr
/// (`--quiet` silences it); ambiguous entries exit 1, I/O 3, else 0.
fn reconcile_cmd(dsl: &Input, layout: &Input, output: Option<&Path>, opts: &ApiOptions) -> Outcome {
    let mut out = Outcome::default();
    let source = match read_input(dsl, opts) {
        Ok(s) => s,
        Err(e) => {
            out.fail(e, ExitClass::Io);
            return out;
        }
    };
    let result = match parse_dsl_input(&source, opts) {
        Ok(r) => r,
        Err(e) => {
            out.fail(e, ExitClass::Diagnostics);
            return out;
        }
    };
    if !result.errors.is_empty() {
        let issues: Vec<Issue> = result
            .errors
            .iter()
            .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&result.workspace)))
            .collect();
        push_issues(&mut out, issues);
        out.exit = ExitClass::Diagnostics;
        return out;
    }
    let layout_source = match read_input(layout, opts) {
        Ok(s) => s,
        Err(e) => {
            out.fail(e, ExitClass::Io);
            return out;
        }
    };
    let layout_ws = match oxidizr_json::from_slice(layout_source.text.as_bytes()) {
        Ok(ws) => ws,
        Err(e) => {
            out.fail(
                Issue::from_json(&e, &layout_source.text, &layout_source.display),
                ExitClass::Diagnostics,
            );
            return out;
        }
    };
    let (reconciled, report) = reconcile::reconcile(&layout_ws, &result.workspace);
    let bytes = match oxidizr_json::to_vec(&reconciled) {
        Ok(b) => b,
        Err(e) => {
            out.fail(
                Issue::from_json(&e, &source.text, &source.display),
                ExitClass::Diagnostics,
            );
            return out;
        }
    };
    if let Err(e) = emit_bytes(&mut out, bytes, output) {
        out.fail(e, ExitClass::Io);
        return out; // the contract: no report after a failed write
    }
    out.log.push(LogItem::NoteLine(report.summary()));
    for line in &report.dropped {
        out.log.push(LogItem::NoteLine(format!("dropped: {line}")));
    }
    let blocked = blocks(&report.ambiguous, false);
    push_issues(&mut out, report.ambiguous);
    if blocked {
        out.exit = ExitClass::Diagnostics;
    }
    out
}

/// `validate`: format by extension (P1-D8), DSL parse or JSON read + hydrate,
/// then the M2 validation catalog on the loaded workspace. Exit: `Diagnostics`
/// when any issue blocks (severity vs `--strict`), else `Io` on I/O trouble,
/// else `Ok`.
fn validate(files: &[PathBuf], opts: &ApiOptions) -> Outcome {
    let mut out = Outcome::default();
    let strict = opts.strict;
    let mut blocked = false;
    let mut io_error = false;
    for path in files {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let source = if let Some("dsl" | "json") = ext.as_deref() {
            match read_input(&Input::File(path.clone()), opts) {
                Ok(s) => s,
                Err(e) => {
                    out.log.push(LogItem::Issue(e));
                    io_error = true;
                    continue;
                }
            }
        } else {
            let issue = Issue::error(
                codes::CLI_IO,
                format!(
                    "cannot determine the workspace format of `{}` (expected `.dsl` or `.json`)",
                    path.display()
                ),
            );
            blocked |= issue.blocks(strict);
            out.log.push(LogItem::Issue(issue));
            continue;
        };
        let workspace = if ext.as_deref() == Some("dsl") {
            match parse_dsl_input(&source, opts) {
                Ok(r) if r.errors.is_empty() => r.workspace,
                Ok(r) => {
                    let issues: Vec<Issue> = r
                        .errors
                        .iter()
                        .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&r.workspace)))
                        .collect();
                    blocked |= blocks(&issues, strict);
                    push_issues(&mut out, issues);
                    continue;
                }
                Err(e) => {
                    blocked |= e.blocks(strict);
                    out.log.push(LogItem::Issue(e));
                    continue;
                }
            }
        } else {
            match oxidizr_json::from_slice(source.text.as_bytes()) {
                Ok(ws) => ws,
                Err(e) => {
                    let issue = Issue::from_json(&e, &source.text, &source.display);
                    blocked |= issue.blocks(strict);
                    out.log.push(LogItem::Issue(issue));
                    continue;
                }
            }
        };
        let violations = oxidizr_core::validate(&workspace.model, &workspace.views);
        if violations.is_empty() {
            out.log
                .push(LogItem::OkLine(format!("{}: ok", source.display)));
        } else {
            let issues: Vec<Issue> = violations
                .iter()
                .map(|v| Issue::validation(&v.message))
                .collect();
            blocked |= blocks(&issues, strict);
            push_issues(&mut out, issues);
        }
    }
    out.exit = if blocked {
        ExitClass::Diagnostics
    } else if io_error {
        ExitClass::Io
    } else {
        ExitClass::Ok
    };
    out
}

/// The `export` loader — the `validate` file loader for ONE path: read by
/// extension (`.dsl` → parse, `.json` → `oxidizr_json::from_slice`),
/// returning the source (for error spans) plus the workspace. Failures are
/// recorded into `out` (the caller returns its [`Outcome`]); unblocked DSL
/// statement errors are pushed there too but do NOT stop the export (D1
/// recovery carries the repaired workspace, `--strict` blocks).
fn load_export_workspace(
    workspace_file: &Path,
    opts: &ApiOptions,
    out: &mut Outcome,
) -> Option<(Source, Workspace)> {
    let ext = workspace_file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let source = if matches!(ext.as_deref(), Some("dsl" | "json")) {
        match read_input(&Input::File(workspace_file.to_path_buf()), opts) {
            Ok(s) => s,
            Err(e) => {
                out.fail(e, ExitClass::Io);
                return None;
            }
        }
    } else {
        out.fail(
            Issue::error(
                codes::CLI_IO,
                format!(
                    "cannot determine the workspace format of `{}` (expected `.dsl` or `.json`)",
                    workspace_file.display()
                ),
            ),
            ExitClass::Diagnostics,
        );
        return None;
    };
    let workspace = if ext.as_deref() == Some("dsl") {
        match parse_dsl_input(&source, opts) {
            Ok(r) if r.errors.is_empty() => r.workspace,
            Ok(r) => {
                let issues: Vec<Issue> = r
                    .errors
                    .iter()
                    .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&r.workspace)))
                    .collect();
                let blocked = blocks(&issues, opts.strict);
                push_issues(out, issues);
                if blocked {
                    out.exit = ExitClass::Diagnostics;
                    return None;
                }
                r.workspace
            }
            Err(e) => {
                out.fail(e, ExitClass::Diagnostics);
                return None;
            }
        }
    } else {
        match oxidizr_json::from_slice(source.text.as_bytes()) {
            Ok(ws) => ws,
            Err(e) => {
                out.fail(
                    Issue::from_json(&e, &source.text, &source.display),
                    ExitClass::Diagnostics,
                );
                return None;
            }
        }
    };
    Some((source, workspace))
}

/// `export` (P2-D10, Phase 2 M0): load the workspace by extension (the
/// `validate` loader), resolve the output directory exactly like
/// `ExportCommand` (default = the canonicalized workspace parent; `mkdirs`
/// happens BEFORE format dispatch, so it is created even for formats that
/// then fail), and write every exported file verbatim, logging a
/// `Writing <path>` line per file. Dispatch follows the reference registry:
/// `theme` and `json` are implemented (M0); other registry formats are
/// OXZ-5007 until M1–M4; unknown strings mirror the reference's
/// class-load miss — `Unknown export format: …`, exit 0. Theme pre-loading
/// (`ThemeUtils.loadThemes`, only for `DiagramExporter` formats) joins this
/// dispatch when the diagram exporters land (M1).
fn export_cmd(
    workspace_file: &Path,
    format: &str,
    output: Option<&Path>,
    opts: &ApiOptions,
) -> Outcome {
    let mut out = Outcome::default();
    let Some((source, workspace)) = load_export_workspace(workspace_file, opts, &mut out) else {
        return out;
    };

    // `ExportCommand`: canonical parent of the workspace file; `mkdirs`
    // before the format dispatch (reference does it before `findExporter`).
    // Canonicalization can only fail for a file we just read — fall back to
    // the CWD instead of the reference's NPE crash.
    let out_dir = match output {
        Some(dir) => dir.to_path_buf(),
        None => std::fs::canonicalize(workspace_file)
            .ok()
            .and_then(|abs| abs.parent().map(std::path::Path::to_path_buf))
            .unwrap_or_default(),
    };
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        out.fail(
            Issue::io(
                &format!("failed to create output directory `{}`", out_dir.display()),
                &e,
            ),
            ExitClass::Io,
        );
        return out;
    }

    let export_format = oxidizr_export::ExportFormat::parse(format);
    if !export_format.is_known() {
        // The reference tries to class-load the format string, logs
        // `Unknown export format: …` and STILL returns true — exit 0.
        out.log.push(LogItem::NoteLine(format!(
            "Unknown export format: {}",
            export_format.label()
        )));
        return out;
    }
    let file_name = workspace_file
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let files = match export_format.export(&workspace, file_name) {
        Ok(files) => files,
        Err(e @ oxidizr_export::ExportError::NotImplemented { .. }) => {
            out.fail(
                Issue::error(codes::EXPORT_NOT_IMPLEMENTED, e.to_string()),
                ExitClass::Diagnostics,
            );
            return out;
        }
        Err(e @ oxidizr_export::ExportError::NoExtension { .. }) => {
            out.fail(Issue::error(codes::CLI_IO, e.to_string()), ExitClass::Io);
            return out;
        }
        Err(oxidizr_export::ExportError::Json(je)) => {
            out.fail(
                Issue::from_json(&je, &source.text, &source.display),
                ExitClass::Diagnostics,
            );
            return out;
        }
    };
    for file in files {
        let path = out_dir.join(&file.name);
        if let Err(e) = std::fs::write(&path, &file.bytes) {
            out.fail(
                Issue::io(&format!("failed to write `{}`", path.display()), &e),
                ExitClass::Io,
            );
            return out;
        }
        // `writeToFile` logs the canonical path; fall back to the joined
        // path when canonicalization fails (never right after a write).
        let shown = std::fs::canonicalize(&path).unwrap_or(path);
        out.log
            .push(LogItem::NoteLine(format!("Writing {}", shown.display())));
    }
    out
}

/// `fmt`: reformat DSL sources to the canonical P1-D7 style. The printer is
/// `oxidizr_dsl::fmt::format` (it never validates — an invalid statement is
/// re-printed token-identically and fails to parse exactly as before).
/// Modes (P1-D8): [`FmtMode::Print`] concatenates formatted text into
/// [`Outcome::bytes`]; [`FmtMode::Write`] rewrites files in place, only when
/// they change (the only file effect); [`FmtMode::Check`] lists the
/// unformatted inputs and exits `Diagnostics`. Stdin has no in-place target:
/// `Write` with a stdin-semantics input is a usage error (exit `Usage`) —
/// detected BEFORE reading the input (a live pipe must not block the exit).
/// I/O and UTF-8 failures are OXZ-5006 → exit `Io`.
fn fmt_cmd(inputs: Vec<Input>, mode: FmtMode, opts: &ApiOptions) -> Outcome {
    let mut out = Outcome::default();
    let mut unformatted = false;
    let mut io_error = false;
    for input in inputs {
        let stdin_like = matches!(input, Input::Stdin | Input::Bytes(_));
        if mode == FmtMode::Write && stdin_like {
            out.fail(
                Issue::error(
                    codes::CLI_IO,
                    "`fmt --write` needs a file to rewrite (stdin has no in-place target)",
                ),
                ExitClass::Usage,
            );
            return out; // usage error (P1-D8 code 2), not an I/O failure
        }
        let source = match read_input(&input, opts) {
            Ok(i) => i,
            Err(e) => {
                out.log.push(LogItem::Issue(e));
                io_error = true;
                continue;
            }
        };
        let formatted = oxidizr_dsl::fmt::format(&source.text);
        match mode {
            FmtMode::Check => {
                if formatted != source.text {
                    unformatted = true;
                    out.log.push(LogItem::OkLine(source.display));
                }
            }
            FmtMode::Write => {
                if formatted != source.text {
                    // Only changed files are rewritten (mtime stability).
                    if let Input::File(path) = &input {
                        if let Err(e) = write_output_file(formatted.as_bytes(), path) {
                            out.log.push(LogItem::Issue(e));
                            io_error = true;
                        } else {
                            out.log.push(LogItem::OkLine(source.display));
                        }
                    }
                }
            }
            FmtMode::Print => match &mut out.bytes {
                Some(buf) => buf.extend_from_slice(formatted.as_bytes()),
                None => out.bytes = Some(formatted.into_bytes()),
            },
        }
    }
    out.exit = if unformatted {
        ExitClass::Diagnostics
    } else if io_error {
        ExitClass::Io
    } else {
        ExitClass::Ok
    };
    out
}
