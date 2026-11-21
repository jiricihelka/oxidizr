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

use oxidizr_core::{Workspace, WorkspaceBuilder};
use oxidizr_diag::codes;
use oxidizr_dsl::engine::{EnvLookup, ParseOptions, ParseResult, parse_dsl};

use crate::Issue;
use crate::diff::DiffOptions;
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
    /// Differences found (the `diff` command; the documented P4-D1
    /// extension of the P1-D8 contract, GNU-diff convention) — 1.
    Differences,
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
            // `Differences` shares code 1 with `Diagnostics` (the P4-D1
            // extension keeps the P1-D8 contract's codes).
            ExitClass::Diagnostics | ExitClass::Differences => 1,
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
// One bool per CLI flag is the surface itself (same justification as the
// `Cli` struct in `oxidizr-cli`), not state to model as enums.
#[allow(clippy::struct_excessive_bools)]
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
    /// `--allow-net` (D5, ships P2-M1): permits the theme-URL fetch in the
    /// `export` diagram branch (`ThemeUtils.loadThemes` mirror). Without it
    /// a `themes` URL fails OXZ-5001; parse-time URL inputs (`!include`,
    /// `extends`) keep the hardened P1 behavior regardless (`PHASE_2.md`
    /// P2-D3 item 3 — the flag applies to theme URLs).
    pub allow_net: bool,
    /// `--themes-dir <dir>`: highest-priority installed-theme source
    /// (`<dir>/<name>/theme.json`, P2-D3); `None` starts the chain at
    /// `OXIDZR_THEMES_DIR`.
    pub themes_dir: Option<std::path::PathBuf>,
    /// Paint the `diff` human report (± colors, §P4-M0 D-3). The CLI fills
    /// it from `!no_color && NO_COLOR unset` (the `report::Renderer` rule);
    /// the default stays PLAIN so API / e2e bytes are stable.
    pub color: bool,
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
    /// Compare two workspaces (`diff`, F3, §P4-M0): both sides load via
    /// [`crate::open::open`] (extension dispatch, partial DSL opens ride
    /// the log), then the pure engine in [`crate::diff`] decides.
    Diff {
        /// Left side (`A`).
        a: PathBuf,
        /// Right side (`B`).
        b: PathBuf,
        /// `--format json` (schema-1 document on [`Outcome::bytes`]);
        /// `false` = the human ± report on `bytes`.
        json: bool,
        /// `--ignore-layout`: skip view geometry fields.
        ignore_layout: bool,
        /// `--quiet`: exit code only (no bytes, no log chatter).
        quiet: bool,
    },
    /// Report the reference inspections over one workspace (`inspect`,
    /// F-inspect-parity, §P4-M1): loaded via [`crate::open::open`] (like
    /// [`Request::Diff`]), then the pure engine in [`oxidizr_inspect`]
    /// decides. Exit [`ExitClass::Differences`] iff any violation is
    /// REPORTED after the filters.
    Inspect {
        /// Workspace file (`.dsl` or `.json`) — a file path, like `diff`.
        workspace: PathBuf,
        /// Report violations at or above this severity (rank
        /// `error > warning > info`); `ignore` is never reported.
        threshold: oxidizr_inspect::Severity,
        /// `--only`: report ONLY these inspection types (exact type
        /// match, run order preserved). Applied BEFORE `except`.
        only: Vec<String>,
        /// `--except`: never report these inspection types.
        except: Vec<String>,
        /// `--format json` (schema-1 document on [`Outcome::bytes`]);
        /// `false` = the human `type | SEVERITY | message` lines.
        json: bool,
        /// `--quiet`: exit code only (no bytes, no log chatter).
        quiet: bool,
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

/// A command body's abort (2R R-mis-01): the [`Outcome`] as accumulated
/// when the command stopped — the partial log included — with the failing
/// issue appended and the exit class set. Exactly what the old
/// `out.fail(issue, class); return out` ladders handed back as the final
/// `Outcome`; [`execute`] flattens it at the frozen boundary.
struct Fail(Outcome);

impl Fail {
    /// Abort from an empty outcome (the legs of helpers the command had
    /// not emitted into yet).
    fn new(issue: Issue, exit: ExitClass) -> Fail {
        Fail(Outcome {
            exit,
            log: vec![LogItem::Issue(issue)],
            bytes: None,
        })
    }
}

impl Outcome {
    /// Record the failing issue on the accumulated state and abort with
    /// it (2R R-mis-01 — the `?` arm of every former fail ladder; takes a
    /// copy so the accumulator itself is never half-mutated).
    fn fail(&self, issue: Issue, exit: ExitClass) -> Fail {
        let mut out = self.clone();
        out.log.push(LogItem::Issue(issue));
        out.exit = exit;
        Fail(out)
    }
}

/// Run one command. Total and side-effect-free except the file effects the
/// CLI contract already had (`-o` writes, `FmtMode::Write`).
#[must_use]
pub fn execute(request: Request, opts: &ApiOptions) -> Outcome {
    // 2R R-mis-01: the bodies run in Result shape ([`Fail`] carries the
    // partial outcome the old `out.fail(…); return out` ladders returned);
    // the frozen byte contract flattens it back at this boundary.
    command(request, opts).unwrap_or_else(|fail| fail.0)
}

/// Dispatch (2R R-mis-01): every former fail-laddered body returns the
/// finished [`Outcome`] or the [`Fail`] it stopped at; `validate` has no
/// fail legs (its per-file loop keeps the errors as data), so it wraps.
fn command(request: Request, opts: &ApiOptions) -> Result<Outcome, Fail> {
    match request {
        Request::Dsl2Json { input, output } => dsl2json(&input, output.as_deref(), opts),
        Request::Json2Json { input, output } => json2json(&input, output.as_deref(), opts),
        Request::Validate { files } => Ok(validate(&files, opts)),
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
        Request::Diff {
            a,
            b,
            json,
            ignore_layout,
            quiet,
        } => diff_cmd(&a, &b, json, ignore_layout, quiet, opts),
        Request::Inspect {
            workspace,
            threshold,
            only,
            except,
            json,
            quiet,
        } => inspect_cmd(&workspace, threshold, &only, &except, json, quiet, opts),
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
pub(crate) struct Source {
    pub(crate) file: Option<String>,
    pub(crate) text: String,
    pub(crate) display: String,
}

/// Resolve an [`Input`]: stdin via the injected reader, bytes decoded, or
/// file read; errors are OXZ-5006 issues (exit 3 family).
pub(crate) fn read_input(input: &Input, opts: &ApiOptions) -> Result<Source, Issue> {
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
pub(crate) fn parse_dsl_input(input: &Source, opts: &ApiOptions) -> Result<ParseResult, Issue> {
    let env_lookup = opts.env.as_ref().map(|map| {
        let map = map.clone();
        EnvLookup::new(move |name| map.get(name).cloned())
    });
    // M1-d (P2-D3): the installed-theme registry is built BEFORE parsing —
    // reference `AbstractCommand` order — and injected as the parser's
    // installed-name check (`ThemeParser.addTheme`).
    let store = theme_store(input.file.as_deref(), opts);
    let installed_themes = Some(oxidizr_dsl::engine::InstalledThemes::new(move |name| {
        store.is_installed(name)
    }));
    let opts = match &input.file {
        Some(f) => ParseOptions {
            json_loader: Some(json_loader),
            env_lookup,
            installed_themes,
            ..ParseOptions::file(f)
        },
        None => ParseOptions {
            json_loader: Some(json_loader),
            env_lookup,
            installed_themes,
            ..ParseOptions::fragment()
        },
    };
    parse_dsl(&input.text, opts).map_err(|e| suggest::decorate(Issue::from_dsl(&e), None))
}

/// `AbstractCommand.loadWorkspace`'s tail (M1-e-B): after the shared load —
/// DSL and JSON legs alike — the reference creates default views when the
/// model is non-empty but the view set is empty (`!model.isEmpty() &&
/// views.isEmpty()`; `Model.isEmpty` covers people + software systems +
/// deployment nodes only, `ViewSet.isEmpty` all nine collections), before
/// the workspace reaches the command. Applied where the load result becomes
/// observable: `dsl2json`/`json2json` serialization and the `export` loader.
/// `validate` deliberately does NOT hook: its violations mirror the
/// reference steps that run BEFORE this tail (`WorkspaceScopeValidator`,
/// `DefaultInspector`), and the reference `ValidateCommand` never serializes
/// the views — creating them could only add noise to our added-feature
/// diagnostics. `create_default_views` mirrors `ViewSet.createDefaultViews`;
/// its `Err` is the reference's unguarded throw (aborts the command there),
/// so the workspace is restored and the failure surfaces as exit 1.
pub(crate) fn apply_default_views(workspace: &mut Workspace) -> Result<(), Issue> {
    let model_empty = workspace.model.people.is_empty()
        && workspace.model.software_systems.is_empty()
        && workspace.model.deployment_nodes.is_empty();
    if model_empty || !workspace.views.all_views().is_empty() {
        return Ok(());
    }
    let mut builder = WorkspaceBuilder::new();
    builder.adopt(std::mem::take(workspace));
    let created = builder.create_default_views();
    *workspace = builder.build();
    created.map_err(|e| Issue::validation(format!("could not create default views: {e}")))
}

/// The install-chain of `AbstractCommand.installThemes` + `ThemeUtils`
/// (P2-D3, catalog `EXPORT-PROPERTIES.md` §6.2): source order
/// `--themes-dir` → `OXIDZR_THEMES_DIR` → `<workspace-parent>/themes` →
/// `STRUCTURIZR_THEMES`, first directory wins. The env closure is the
/// hermetic [`ApiOptions::env`] map when set (in-process runs never leak
/// the process environment), the process environment otherwise (V-3).
/// Non-file inputs (`Bytes`/stdin) skip the directory probe — the reference
/// has no such input flow, and a CWD probe would break hermetic purity.
fn theme_store(input_file: Option<&str>, opts: &ApiOptions) -> oxidizr_export::ThemeStore {
    let parent = input_file
        .map(std::path::Path::new)
        .and_then(std::path::Path::parent);
    let env = opts.env.clone();
    let lookup = move |name: &str| match &env {
        Some(map) => map.get(name).cloned(),
        None => std::env::var(name).ok(),
    };
    oxidizr_export::ThemeStore::install(opts.themes_dir.as_deref(), parent, &lookup, opts.allow_net)
}

/// Issues blocking exit 1 (severity vs `--strict`) — the old `emit` decision
/// without the rendering.
fn blocks(issues: &[Issue], strict: bool) -> bool {
    issues.iter().any(|i| i.blocks(strict))
}

fn push_issues(out: &mut Outcome, issues: Vec<Issue>) {
    out.log.extend(issues.into_iter().map(LogItem::Issue));
}

/// The D1 statement-error decoration ladder (2R R-mis-05: one helper for
/// the old ×4 duplicate): every recorded statement error becomes a
/// `did you mean`-decorated issue against the repaired workspace.
pub(crate) fn statement_errors(result: &ParseResult) -> Vec<Issue> {
    result
        .errors
        .iter()
        .map(|e| suggest::decorate(Issue::from_dsl(e), Some(&result.workspace)))
        .collect()
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

fn dsl2json(input: &Input, output: Option<&Path>, opts: &ApiOptions) -> Result<Outcome, Fail> {
    let mut out = Outcome::default();
    let source = read_input(input, opts).map_err(|e| out.fail(e, ExitClass::Io))?;
    let result = parse_dsl_input(&source, opts).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    if !result.errors.is_empty() {
        // D1 recovery: all recorded statement errors render; the reference
        // export would have aborted on the first one — no partial output.
        push_issues(&mut out, statement_errors(&result));
        out.exit = ExitClass::Diagnostics;
        return Ok(out);
    }
    let mut workspace = result.workspace;
    apply_default_views(&mut workspace).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    let bytes = oxidizr_json::to_vec(&workspace).map_err(|e| {
        out.fail(
            Issue::from_json(&e, &source.text, &source.display),
            ExitClass::Diagnostics,
        )
    })?;
    emit_bytes(&mut out, bytes, output).map_err(|e| out.fail(e, ExitClass::Io))?;
    Ok(out)
}

fn json2json(input: &Input, output: Option<&Path>, opts: &ApiOptions) -> Result<Outcome, Fail> {
    let mut out = Outcome::default();
    let source = read_input(input, opts).map_err(|e| out.fail(e, ExitClass::Io))?;
    let bytes = source.text.as_bytes();
    if opts.keep_unknown {
        // D3 superset mode: unknown / legacy / writer-omitted members ride
        // through verbatim (output intentionally non-parity — the reference
        // load tail, default views included, is part of the parity contract
        // only).
        let kept = oxidizr_json::roundtrip_keep_unknown(bytes).map_err(|e| {
            out.fail(
                Issue::from_json(&e, &source.text, &source.display),
                ExitClass::Diagnostics,
            )
        })?;
        emit_bytes(&mut out, kept, output).map_err(|e| out.fail(e, ExitClass::Io))?;
        return Ok(out);
    }
    let mut workspace = oxidizr_json::from_slice(bytes).map_err(|e| {
        out.fail(
            Issue::from_json(&e, &source.text, &source.display),
            ExitClass::Diagnostics,
        )
    })?;
    apply_default_views(&mut workspace).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    let json = oxidizr_json::to_vec(&workspace).map_err(|e| {
        out.fail(
            Issue::from_json(&e, &source.text, &source.display),
            ExitClass::Diagnostics,
        )
    })?;
    emit_bytes(&mut out, json, output).map_err(|e| out.fail(e, ExitClass::Io))?;
    Ok(out)
}

/// `reconcile` (F1, P1-D8): re-parse the edited DSL (the model authority),
/// load the pre-edit layout JSON (the geometry authority), remap ids
/// (identifier exact → canonical fallback, ambiguity = OXZ-3002, never
/// guessed) and graft the layout. The standard JSON is emitted FIRST (the
/// contract emits even when ambiguous), then the human report on stderr
/// (`--quiet` silences it); ambiguous entries exit 1, I/O 3, else 0.
fn reconcile_cmd(
    dsl: &Input,
    layout: &Input,
    output: Option<&Path>,
    opts: &ApiOptions,
) -> Result<Outcome, Fail> {
    let mut out = Outcome::default();
    let source = read_input(dsl, opts).map_err(|e| out.fail(e, ExitClass::Io))?;
    let result = parse_dsl_input(&source, opts).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    if !result.errors.is_empty() {
        push_issues(&mut out, statement_errors(&result));
        out.exit = ExitClass::Diagnostics;
        return Ok(out);
    }
    let layout_source = read_input(layout, opts).map_err(|e| out.fail(e, ExitClass::Io))?;
    let mut layout_ws = oxidizr_json::from_slice(layout_source.text.as_bytes()).map_err(|e| {
        out.fail(
            Issue::from_json(&e, &layout_source.text, &layout_source.display),
            ExitClass::Diagnostics,
        )
    })?;
    // The `AbstractCommand.loadWorkspace` tail (M1-e-B) on BOTH reconcile
    // sides: the reference creates default views after every load, so a
    // layout export carrying generated views must not drop them against a
    // DSL whose views would be generated (P2T full-tier catch, 2026-11-18).
    let mut updated = result.workspace;
    apply_default_views(&mut updated).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    apply_default_views(&mut layout_ws).map_err(|e| out.fail(e, ExitClass::Diagnostics))?;
    let (reconciled, report) = reconcile::reconcile(&layout_ws, &updated);
    let bytes = oxidizr_json::to_vec(&reconciled).map_err(|e| {
        out.fail(
            Issue::from_json(&e, &source.text, &source.display),
            ExitClass::Diagnostics,
        )
    })?;
    // The contract: no report after a failed write — `?` aborts with the
    // Io-classed failure before the report legs below (the old leg's
    // `return out; // the contract: no report after a failed write`).
    emit_bytes(&mut out, bytes, output).map_err(|e| out.fail(e, ExitClass::Io))?;
    out.log.push(LogItem::NoteLine(report.summary()));
    for line in &report.dropped {
        out.log.push(LogItem::NoteLine(format!("dropped: {line}")));
    }
    let blocked = blocks(&report.ambiguous, false);
    push_issues(&mut out, report.ambiguous);
    if blocked {
        out.exit = ExitClass::Diagnostics;
    }
    Ok(out)
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
                    let issues = statement_errors(&r);
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
/// returning the source (for error spans) plus the workspace. Failures
/// RETURN as [`Fail`] (2R R-mis-01: the old shape mutated the caller's
/// accumulator and answered `Option`); unblocked DSL statement errors ride
/// out as the third tuple leg — they join the final log but do NOT stop
/// the export (D1 recovery carries the repaired workspace, `--strict`
/// blocks).
fn load_export_workspace(
    workspace_file: &Path,
    opts: &ApiOptions,
) -> Result<(Source, Workspace, Vec<Issue>), Fail> {
    let ext = workspace_file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let source = if matches!(ext.as_deref(), Some("dsl" | "json")) {
        read_input(&Input::File(workspace_file.to_path_buf()), opts)
            .map_err(|e| Fail::new(e, ExitClass::Io))?
    } else {
        return Err(Fail::new(
            Issue::error(
                codes::CLI_IO,
                format!(
                    "cannot determine the workspace format of `{}` (expected `.dsl` or `.json`)",
                    workspace_file.display()
                ),
            ),
            ExitClass::Diagnostics,
        ));
    };
    let (mut workspace, recovered) = if ext.as_deref() == Some("dsl") {
        match parse_dsl_input(&source, opts) {
            Ok(r) if r.errors.is_empty() => (r.workspace, Vec::new()),
            Ok(r) => {
                let issues = statement_errors(&r);
                if blocks(&issues, opts.strict) {
                    let mut out = Outcome::default();
                    push_issues(&mut out, issues);
                    out.exit = ExitClass::Diagnostics;
                    return Err(Fail(out));
                }
                (r.workspace, issues)
            }
            Err(e) => return Err(Fail::new(e, ExitClass::Diagnostics)),
        }
    } else {
        match oxidizr_json::from_slice(source.text.as_bytes()) {
            Ok(ws) => (ws, Vec::new()),
            Err(e) => {
                return Err(Fail::new(
                    Issue::from_json(&e, &source.text, &source.display),
                    ExitClass::Diagnostics,
                ));
            }
        }
    };
    apply_default_views(&mut workspace).map_err(|e| Fail::new(e, ExitClass::Diagnostics))?;
    Ok((source, workspace, recovered))
}

/// `export` (P2-D10, Phase 2 M0): load the workspace by extension (the
/// `validate` loader), resolve the output directory exactly like
/// `ExportCommand` (default = the canonicalized workspace parent; `mkdirs`
/// happens BEFORE format dispatch, so it is created even for formats that
/// then fail), and write every exported file verbatim, logging a
/// `Writing <path>` line per file. Dispatch follows the reference registry:
/// `theme` and `json` are implemented (M0) with the Structurizr `plantuml`
/// diagram leg (M2-d — id-keyed per-view, `-key` legend and numbered frame
/// files, and the reference's viewless `log.error` branch), the C4
/// dialect (M3-b — no legend files, custom views skipped) and the Mermaid
/// exporter (M4-b — `.mmd` files, custom views exported, the standalone
/// `sequenceDiagram` form); the one unlanded registry format
/// (`websequencediagrams`) raises OXZ-5007; unknown strings mirror the
/// reference's class-load miss — `Unknown export format: …`, exit 0. Theme
/// pre-loading (`ThemeUtils.loadThemes`, P2-D3, M1-d) runs for
/// `DiagramExporter` formats BEFORE the export attempt — exactly the
/// reference order, so a broken theme (or a URL without `--allow-net`)
/// reports OXZ-5008/OXZ-5001 ahead of the format's OXZ-5007.
fn export_cmd(
    workspace_file: &Path,
    format: &str,
    output: Option<&Path>,
    opts: &ApiOptions,
) -> Result<Outcome, Fail> {
    // The loader's failures abort via `?` (they carry their own partial
    // outcome); the recovered statement errors join the log up front.
    let (source, mut workspace, recovered) = load_export_workspace(workspace_file, opts)?;
    let mut out = Outcome::default();
    push_issues(&mut out, recovered);

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
        return Err(out.fail(
            Issue::io(
                &format!("failed to create output directory `{}`", out_dir.display()),
                &e,
            ),
            ExitClass::Io,
        ));
    }

    let export_format = oxidizr_export::ExportFormat::parse(format);
    if !export_format.is_known() {
        // The reference tries to class-load the format string, logs
        // `Unknown export format: …` and STILL returns true — exit 0.
        out.log.push(LogItem::NoteLine(format!(
            "Unknown export format: {}",
            export_format.label()
        )));
        return Ok(out);
    }
    if export_format.is_diagram() {
        // `ExportCommand` 222-227: `ThemeUtils.loadThemes` BEFORE the export
        // — reference order, so theme failures precede the OXZ-5007 an
        // unlanded diagram format raises. Themes append to the non-serialized
        // `Styles.themes` (resolution-time only, P2-D2).
        let store = theme_store(workspace_file.to_str(), opts);
        if let Err(e) = store.load_themes(&mut workspace) {
            let code = match &e {
                oxidizr_export::ThemeError::NetDisabled { .. } => codes::NET_DISABLED,
                _ => codes::EXPORT_THEME,
            };
            return Err(out.fail(Issue::error(code, e.to_string()), ExitClass::Diagnostics));
        }
        if workspace.views.all_views().is_empty() {
            // `ExportCommand` 233: the `DiagramExporter` branch logs the
            // error and STILL finishes (exit 0) — no files written.
            out.log.push(LogItem::NoteLine(
                "The workspace contains no views".to_string(),
            ));
            return Ok(out);
        }
    }
    let file_name = workspace_file
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let files = match export_format.export(&workspace, file_name) {
        Ok(files) => files,
        Err(e @ oxidizr_export::ExportError::NotImplemented { .. }) => {
            return Err(out.fail(
                Issue::error(codes::EXPORT_NOT_IMPLEMENTED, e.to_string()),
                ExitClass::Diagnostics,
            ));
        }
        Err(e @ oxidizr_export::ExportError::NoExtension { .. }) => {
            return Err(out.fail(Issue::error(codes::CLI_IO, e.to_string()), ExitClass::Io));
        }
        Err(oxidizr_export::ExportError::Json(je)) => {
            return Err(out.fail(
                Issue::from_json(&je, &source.text, &source.display),
                ExitClass::Diagnostics,
            ));
        }
        Err(e @ oxidizr_export::ExportError::ReferenceAbort(_)) => {
            return Err(out.fail(
                Issue::error(codes::EXPORT_ABORT, e.to_string()),
                ExitClass::Diagnostics,
            ));
        }
    };
    for file in files {
        let path = out_dir.join(&file.name);
        if let Err(e) = std::fs::write(&path, &file.bytes) {
            return Err(out.fail(
                Issue::io(&format!("failed to write `{}`", path.display()), &e),
                ExitClass::Io,
            ));
        }
        // `writeToFile` logs the canonical path; fall back to the joined
        // path when canonicalization fails (never right after a write).
        let shown = std::fs::canonicalize(&path).unwrap_or(path);
        out.log
            .push(LogItem::NoteLine(format!("Writing {}", shown.display())));
    }
    Ok(out)
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
fn fmt_cmd(inputs: Vec<Input>, mode: FmtMode, opts: &ApiOptions) -> Result<Outcome, Fail> {
    let mut out = Outcome::default();
    let mut unformatted = false;
    let mut io_error = false;
    for input in inputs {
        let stdin_like = matches!(input, Input::Stdin | Input::Bytes(_));
        if mode == FmtMode::Write && stdin_like {
            // usage error (P1-D8 code 2), not an I/O failure — 2R R-mis-01
            // aborts with the partial outcome the earlier files emitted.
            return Err(out.fail(
                Issue::error(
                    codes::CLI_IO,
                    "`fmt --write` needs a file to rewrite (stdin has no in-place target)",
                ),
                ExitClass::Usage,
            ));
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
    Ok(out)
}

/// `diff` (F3, §P4-M0 D-3): load BOTH sides through [`crate::open::open`]
/// (extension dispatch, `.dsl`/`.json`; a recoverable DSL opens partially —
/// its diagnostics ride the log INFORMATIONALLY, the reference has no `diff`
/// so no parity rule binds them), run the pure engine, and carry the result:
/// `Differences` (exit 1, the documented P4-D1 extension) when any category
/// is non-empty, `Ok` otherwise. `json` rides [`Outcome::bytes`] as the
/// schema-1 document (`docs/diff-schema.md`); the human format is the ±
/// report, painted per [`ApiOptions::color`]. `--quiet` = exit code only:
/// no bytes, no chatter (load failures still abort with their own class —
/// an unreadable side is not "no differences").
fn diff_cmd(
    a: &Path,
    b: &Path,
    json: bool,
    ignore_layout: bool,
    quiet: bool,
    opts: &ApiOptions,
) -> Result<Outcome, Fail> {
    let left = crate::open::open(&Input::File(a.to_path_buf()), opts).map_err(Fail)?;
    let right = crate::open::open(&Input::File(b.to_path_buf()), opts).map_err(Fail)?;
    let d = crate::diff::diff(
        &left.workspace,
        &right.workspace,
        &DiffOptions { ignore_layout },
    );
    let mut out = Outcome {
        exit: if d.is_empty() {
            ExitClass::Ok
        } else {
            ExitClass::Differences
        },
        ..Outcome::default()
    };
    if quiet {
        // The exit code IS the result (the `fmt --check` pattern, minus its
        // list): nothing rides the streams.
        return Ok(out);
    }
    // Informational open diagnostics (partial DSL opens, validation): they
    // join AFTER the payload decision so `--quiet` keeps them out too.
    if json {
        let doc = serde_json::to_string_pretty(&d.to_json())
            .map_err(|e| Fail::new(Issue::error(codes::CLI_IO, e.to_string()), ExitClass::Io))?;
        out.bytes = Some(format!("{doc}\n").into_bytes());
    } else if !d.is_empty() {
        // Silent when equal, like GNU diff; one line per entry.
        let mut text = d.human_lines(opts.color).join("\n");
        text.push('\n');
        out.bytes = Some(text.into_bytes());
    }
    out.log.extend(left.log);
    out.log.extend(right.log);
    Ok(out)
}

/// `oxidizr inspect` (§P4-M1 I-4): load via [`crate::open::open`] (same
/// partial-DSL-open semantics as [`Request::Diff`]), run the pure engine
/// in [`oxidizr_inspect`], then filter and report. The engine output IS
/// the contract (run order, verbatim messages); this body decides only
/// what is REPORTED:
/// - `ignore` severity NEVER (the reference counts it, never prints it),
/// - the threshold keeps violations at or above its rank,
/// - `--only` (when non-empty) BEFORE `--except`, exact type match.
///
/// Exit is [`ExitClass::Differences`] iff anything survives — the P4-D1
/// family, same process code 1 as `diff`.
fn inspect_cmd(
    workspace: &Path,
    threshold: oxidizr_inspect::Severity,
    only: &[String],
    except: &[String],
    json: bool,
    quiet: bool,
    opts: &ApiOptions,
) -> Result<Outcome, Fail> {
    use oxidizr_inspect::Severity;

    let opened = crate::open::open(&Input::File(workspace.to_path_buf()), opts).map_err(Fail)?;
    let all = oxidizr_inspect::inspect(&opened.workspace);
    let reported: Vec<&oxidizr_inspect::Violation> = all
        .iter()
        .filter(|v| v.severity != Severity::Ignore && v.severity.rank() >= threshold.rank())
        .filter(|v| only.is_empty() || only.iter().any(|t| t == v.inspection))
        .filter(|v| !except.iter().any(|t| t == v.inspection))
        .collect();
    let mut out = Outcome {
        exit: if reported.is_empty() {
            ExitClass::Ok
        } else {
            ExitClass::Differences
        },
        ..Outcome::default()
    };
    if quiet {
        // The exit code IS the result (the `diff --quiet` pattern).
        return Ok(out);
    }
    if json {
        // Schema 1 (`docs/inspect-schema.md`): frozen key order, run
        // order preserved, lowercase severities. Provenance stamps never
        // appear (the `diff` schema rule).
        let doc = serde_json::json!({
            "schema": 1,
            "violations": reported
                .iter()
                .map(|v| {
                    serde_json::json!({
                        "type": v.inspection,
                        "severity": v.severity.as_str(),
                        "message": v.message,
                    })
                })
                .collect::<Vec<serde_json::Value>>(),
        });
        let doc = serde_json::to_string_pretty(&doc)
            .map_err(|e| Fail::new(Issue::error(codes::CLI_IO, e.to_string()), ExitClass::Io))?;
        out.bytes = Some(format!("{doc}\n").into_bytes());
    } else if !reported.is_empty() {
        // Silent when clean, like `diff` and like the reference CLI; one
        // `type | SEVERITY | message` line per violation (`toString`).
        let mut text = reported
            .iter()
            .map(|v| inspect_human_line(v, opts.color))
            .collect::<Vec<String>>()
            .join("\n");
        text.push('\n');
        out.bytes = Some(text.into_bytes());
    }
    out.log.extend(opened.log);
    Ok(out)
}

/// One human report line: the reference `Violation.toString()` verbatim;
/// `color` paints ONLY the SEVERITY token (error red, warning yellow,
/// info cyan). The CLI goldens run `NO_COLOR`, so the PLAIN bytes are
/// the golden contract.
fn inspect_human_line(v: &oxidizr_inspect::Violation, color: bool) -> String {
    if !color {
        return v.to_string();
    }
    let code = match v.severity {
        oxidizr_inspect::Severity::Error => "31",
        oxidizr_inspect::Severity::Warning => "33",
        oxidizr_inspect::Severity::Info => "36",
        // Never reported, but keep the renderer total.
        oxidizr_inspect::Severity::Ignore => return v.to_string(),
    };
    format!(
        "{} | \x1b[{code}m{}\x1b[0m | {}",
        v.inspection,
        v.severity.java_name(),
        v.message
    )
}
