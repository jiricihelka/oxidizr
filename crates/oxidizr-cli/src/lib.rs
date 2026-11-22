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
//! failure like any other I/O problem). Phase 2 M0 added `export` (reference
//! `-w/--workspace`, required `-f/--format`, optional `-o/--output`): `theme`,
//! `json` and the Structurizr `plantuml` diagram leg (M2-d) write through the
//! `oxidizr_export` dispatch; `plantuml/c4plantuml` landed at M3-b,
//! `mermaid` at M4-b and `websequencediagrams` at P4 §P4-M2 N-5; the
//! remaining registry formats raise OXZ-5007; unknown format strings stay
//! the exit-0 note, mirroring `ExportCommand` (`agents/plan/build/PHASE_2.md`).
//!
//! Since Phase 1T T0-d the command bodies live in [`oxidizr_api`]
//! (`Request → execute() → Outcome`); this crate adds ONLY the argv surface,
//! the miette rendering ([`report`]) and the exit-code mapping. `run()` adds
//! no command logic — the invariant that keeps the binary and the e2e suite
//! (which drives the API in-process) in lockstep
//! (`agents/plan/build/PHASE_1T.md` P1T-D1).

pub mod oracle;
pub mod report;
pub use oxidizr_api::{reconcile, suggest};

use std::io::Read as _;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context as _; // §P3-M8: the `to_request` Option's internal-fault arm
use clap::{CommandFactory as _, Parser, Subcommand, ValueEnum};
use oxidizr_api::{
    ApiOptions, ExitClass, FmtMode, ImportFormat, InitTemplate, Input, LogItem, Outcome,
    QueryFormat, QueryKind, RelationshipsStrategy, Request, Severity,
};

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
    /// Fetch theme URLs during `export` (D5): without it a theme URL in
    /// `configuration.themes` fails OXZ-5001 and installed names /
    /// `--themes-dir` remain the offline path (P2-D3).
    #[arg(long, global = true)]
    pub allow_net: bool,
    /// Highest-priority installed-theme root (`<dir>/<name>/theme.json`);
    /// the chain continues `OXIDZR_THEMES_DIR` → `<workspace>/themes` →
    /// `STRUCTURIZR_THEMES` (P2-D3).
    #[arg(long, global = true, value_name = "DIR")]
    pub themes_dir: Option<std::path::PathBuf>,
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
    /// Export the workspace (Phase 2): `theme`/`json` and the diagram
    /// formats `plantuml`/`c4plantuml`/`mermaid`/`websequencediagrams`;
    /// the known-but-unlanded `static`/`png`/`svg` raise OXZ-5007 (exit 1 —
    /// the reference agrees for PNG/SVG); unknown format strings log the
    /// reference `Unknown export format:` note and still exit 0.
    Export {
        /// Workspace file (`.dsl` or `.json`) — reference `-w/--workspace`.
        #[arg(long, short = 'w')]
        workspace: PathBuf,
        /// Export format (lowercased): `theme` | `json` |
        /// `plantuml[/structurizr|/c4plantuml][-light|-dark]` | `mermaid` |
        /// `websequencediagrams` | `static` | `png` | `svg` — reference `-f/--format`.
        #[arg(long, short = 'f')]
        format: String,
        /// Output directory; default is the workspace file's directory
        /// (created if missing) — reference `-o/--output`.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Compare two workspaces (`diff`, F3, §P4-M0): canonical-identity
    /// diff, exit 1 when they differ (the P4-D1 exit-code extension).
    Diff {
        /// Left workspace file (`.dsl` or `.json`).
        a: PathBuf,
        /// Right workspace file.
        b: PathBuf,
        /// `json` prints the schema-1 document (`docs/diff-schema.md`);
        /// the default `human` prints the grouped ± report.
        #[arg(long, value_enum, default_value_t = DiffFormat::Human)]
        format: DiffFormat,
        /// Ignore view geometry (positions, routing, layout, animations).
        // Explicit spelling: the `Command` rename is `lowercase`, which
        // would otherwise flatten the field name to `--ignorelayout`.
        #[arg(long = "ignore-layout")]
        ignore_layout: bool,
    },
    /// Report the reference inspections over one workspace (`inspect`,
    /// F-inspect-parity, §P4-M1): advisory only (the JSON contract is
    /// untouched); exit 1 when any violation is REPORTED (the P4-D1
    /// exit-code family — the same code 1 as `diff`).
    Inspect {
        /// Workspace file (`.dsl` or `.json`).
        workspace: PathBuf,
        /// Only report violations at or above this severity (`ignore`
        /// is never reported regardless).
        // Explicit spelling (the `lowercase` rename lesson from `diff`).
        #[arg(long = "severity-threshold", value_enum, default_value_t = InspectThreshold::Info)]
        severity_threshold: InspectThreshold,
        /// Report ONLY these inspection types (exact type match;
        /// repeatable; applied before `--except`).
        #[arg(long)]
        only: Vec<String>,
        /// Never report these inspection types (exact type match;
        /// repeatable).
        #[arg(long)]
        except: Vec<String>,
        /// `json` prints the schema-1 violations document
        /// (`docs/inspect-schema.md`); the default `human` prints the
        /// `type | SEVERITY | message` lines.
        #[arg(long, value_enum, default_value_t = InspectFormat::Human)]
        format: InspectFormat,
    },
    /// Import one diagram into a fresh workspace (`import`, F12,
    /// §P4-M2 N-4): the C4-`PlantUML`/`Mermaid` macro legs produce the
    /// MODEL (the default views install at open), the `image` leg is the
    /// reference `ImageImporter` port (one `ImageView`).
    Import {
        /// Diagram file (`.puml`/`.iuml`/`.c4p`, `.mmd`/`.mermaid`, or a
        /// `.png`/`.jpg`/`.jpeg`/`.gif`/`.svg` image; `-` = stdin, which
        /// needs an explicit `--format`).
        file: Option<PathBuf>,
        /// Import format; the default sniffs the file extension
        /// (`.puml`/`.iuml`/`.c4p` → `c4plantuml`, `.mmd`/`.mermaid` →
        /// `mermaid`, image extensions → `image`).
        #[arg(long, value_enum)]
        format: Option<ImportCmdFormat>,
        /// `ImageView` key (image leg; omitted = generated `Image-001`,
        /// reference `generateViewKey`).
        #[arg(long = "view-key")]
        view_key: Option<String>,
        /// `ImageView` title (image leg; omitted = the file name).
        #[arg(long = "view-name")]
        view_name: Option<String>,
        /// Output file (`None` = stdout bytes).
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Query one workspace with the DSL filter grammar (`query`, F6,
    /// §P4-M3 O-1): model items through the pinned `ExpressionParser`
    /// arms (`element.tag==Web`, `user->system`, ` && `/` || ` …), views
    /// through the documented key/title/description subset —
    /// `docs/query-schema.md`. Informational: exit 0 even on zero
    /// matches; only a THROWING grammar arm exits 1.
    Query {
        /// Workspace file (`.dsl` or `.json`).
        workspace: PathBuf,
        /// Filter expression, e.g. `element.tag==Element` (REQUIRED —
        /// `query` without a filter would list the whole workspace).
        #[arg(long)]
        filter: String,
        /// Which classes to report; `all` (the default) runs the model
        /// grammar AND the view subset over every class.
        #[arg(long, value_enum, default_value_t = QueryCmdKind::All)]
        kind: QueryCmdKind,
        /// `table` (default) prints `id  kind  name  [tags]` rows,
        /// `ids` one id/key per line, `json` the schema-1 document.
        #[arg(long, value_enum, default_value_t = QueryCmdFormat::Table)]
        format: QueryCmdFormat,
        /// Output file (`None` = stdout bytes).
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
    /// Scaffold a starter project (`init`, F7, §P4-M3 O-2, P4-D4): the
    /// `include_str!`-embedded template files — zero network, zero new
    /// deps — written into DIR (created when missing; default `.`).
    /// Idempotent: existing files are SKIPPED with a note unless
    /// `--force`; the report lists the tree + the next commands.
    Init {
        /// Target directory (created when missing).
        dir: Option<PathBuf>,
        /// Starter content; `c4` (the default) is the canonical
        /// 3-software-system landscape, `minimal` a single system.
        // `C4` would kebab-case to `c-4` (the `c4plantuml` lesson).
        #[arg(long, value_enum, default_value_t = InitCmdTemplate::C4)]
        template: InitCmdTemplate,
        /// Overwrite existing files instead of skipping them.
        #[arg(long)]
        force: bool,
    },
    /// Landscape generators (`generate`, §P4-M3 O-4, P4-D9): the ONE
    /// subcommand is the reference `SystemLandscapeGenerator` port — the
    /// output NEVER carries views (the oracle witness settled the open
    /// P4-D9 question).
    Generate {
        /// Generator to run (only `system-landscape` exists).
        #[command(subcommand)]
        command: GenerateCommand,
    },
    /// Emit a shell-completion script for this CLI to stdout (`completions`,
    /// §P4-M3 O-3): rendered by `clap_complete` from the argv surface itself
    /// — dispatched in `run` BEFORE the pipeline (it never opens a
    /// workspace, so it is not a [`Request`], like `view`).
    Completions {
        /// Target shell (`bash`, `elvish`, `fish`, `powershell`, `zsh`).
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    /// Generate or verify reference-oracle goldens over USER fixture
    /// paths (`oracle`, F4 graduation, §P4-M5 E-1): fixture-author CI
    /// automation of the `structurizr export --format json` round —
    /// dispatched in `run` before the pipeline (it spawns the EXTERNAL
    /// reference binary; oxidizr never downloads anything, D5).
    /// Discovery: `--bin` → `$OXIDZR_STRUCTURIZR_BIN` → `structurizr`
    /// on PATH. Goldens: `<stem>.golden.json` next to the workspace (or
    /// under `--goldens DIR`). Exit: 0 all ok · 1 mismatch/missing ·
    /// 2 usage · 3 discovery/spawn/I-O.
    Oracle {
        /// `gen` writes the normalized reference output to the golden;
        /// `verify` re-runs the reference and byte-compares against it.
        #[arg(value_enum)]
        action: OracleAction,
        /// Workspace files (`.dsl` or `.json`) to run through the oracle.
        #[arg(required = true)]
        workspaces: Vec<PathBuf>,
        /// Reference binary path (default: `$OXIDZR_STRUCTURIZR_BIN` → PATH).
        #[arg(long, value_name = "PATH")]
        bin: Option<PathBuf>,
        /// Golden directory (default: next to each workspace file).
        #[arg(long, value_name = "DIR")]
        goldens: Option<PathBuf>,
        /// Machine-readable JSON report on stdout (`{"schema":1,…}`,
        /// for CI pipelines) instead of the human lines.
        #[arg(long)]
        json: bool,
    },
    /// Open the workspace in the native viewer window (F10, §P3-M8) —
    /// needs the cli `gui` feature at BUILD time; without it the
    /// subcommand stays in `--help` but exits with a rebuild hint.
    View {
        /// Workspace file to open (`.dsl` or `.json`; omitted = empty
        /// window, open it from the toolbar).
        file: Option<PathBuf>,
        /// Color scheme; `system` consults the OS preference, then the
        /// persisted runtime toggle (§P3-D7).
        #[arg(long, value_enum, default_value_t = ViewTheme::System)]
        theme: ViewTheme,
    },
}

/// `oxidizr generate` subcommands (§P4-M3 O-4).
#[derive(Subcommand, Debug)]
pub enum GenerateCommand {
    /// Merge 1..N workspaces into the system-landscape workspace
    /// (`SystemLandscapeGenerator` port): name-first person/system clone
    /// with description/url last-wins and group/tags/properties merge,
    /// relationship re-derivation, and the `{workspace:<id>}/diagrams`
    /// hyperlink on each source workspace's first container-bearing
    /// system. Always JSON, never views.
    SystemLandscape {
        /// Workspace files and/or directories (1..N). Directories are
        /// scanned recursively (sorted) and filtered by
        /// `--include`/`--exclude`; an explicit file ALWAYS loads
        /// (documented deviation — the reference applied its include to
        /// a single `-i` file too).
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Full-path include regex (full match, like the reference's
        /// `Pattern.matches`); default `.*workspace.(json|dsl)`.
        #[arg(long)]
        include: Option<String>,
        /// Full-path exclude regex (wins over `--include`).
        #[arg(long)]
        exclude: Option<String>,
        /// Which duplicate source→destination edges survive the merge.
        #[arg(long, value_enum, default_value_t = RelationshipsArg::First)]
        relationships: RelationshipsArg,
        /// Output file (`None` = stdout; ALWAYS JSON).
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// `oxidizr oracle` actions (§P4-M5 E-1, F4): the two golden legs of the
/// user-facing oracle command; maps onto [`oracle::Action`].
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug)]
pub enum OracleAction {
    /// Write the normalized reference output to `<stem>.golden.json`.
    Gen,
    /// Re-run the reference and byte-compare against the golden.
    Verify,
}

/// `oxidizr generate system-landscape --relationships` values: the
/// reference `RelationshipsStrategy`; maps onto
/// [`oxidizr_api::RelationshipsStrategy`].
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum RelationshipsArg {
    /// Keep the FIRST edge per source→destination pair (the default).
    #[default]
    First,
    /// Keep one edge per distinct description.
    All,
}

/// `oxidizr diff --format` values (§P4-M0 D-3): the human ± report (the
/// default) or the schema-1 JSON document (`docs/diff-schema.md`).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum DiffFormat {
    /// Grouped ± report on stdout.
    #[default]
    Human,
    /// Schema-1 document on stdout.
    Json,
}

/// `oxidizr inspect --format` values (§P4-M1 I-4): the reference
/// `type | SEVERITY | message` lines (the default) or the schema-1
/// violations document (`docs/inspect-schema.md`).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum InspectFormat {
    /// `type | SEVERITY | message` lines on stdout.
    #[default]
    Human,
    /// Schema-1 document on stdout.
    Json,
}

/// `oxidizr inspect --severity-threshold` values (§P4-M1 I-4): the three
/// REPORTABLE levels — `ignore` is not a threshold (nothing outranks
/// it; `ignore` violations are never reported regardless).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum InspectThreshold {
    /// Report every violation (the default).
    #[default]
    Info,
    /// Warnings and errors only.
    Warning,
    /// Errors only.
    Error,
}

/// `oxidizr import --format` values (§P4-M2 N-4, F12): which
/// `oxidizr-import` engine processes the source; omitted = sniffed from
/// the file extension. `C4PlantUml` spells its value `c4plantuml` — the
/// clap default would kebab-case it to `c4-plant-uml`.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug)]
pub enum ImportCmdFormat {
    /// C4-`PlantUML` macro diagram → the model.
    #[value(name = "c4plantuml")]
    C4PlantUml,
    /// `Mermaid` C4 diagram → the model.
    Mermaid,
    /// Image file → one `ImageView` (reference port).
    Image,
}

/// `oxidizr query --kind` values (§P4-M3 O-1, F6): which report classes
/// the query includes; maps onto [`oxidizr_api::QueryKind`].
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum QueryCmdKind {
    /// Matching elements only.
    Element,
    /// Matching relationships only.
    Relationship,
    /// Matching views only (the documented view subset).
    View,
    /// Elements, relationships AND views (the default).
    #[default]
    All,
}

/// `oxidizr query --format` values (§P4-M3 O-1, F6): output shape; maps
/// onto [`oxidizr_api::QueryFormat`] (`docs/query-schema.md`).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum QueryCmdFormat {
    /// `id  kind  name  [tags]` rows (the default).
    #[default]
    Table,
    /// One id (or view key) per line — pipeline-friendly.
    Ids,
    /// Schema-1 document on stdout.
    Json,
}

/// `oxidizr init --template` values (§P4-M3 O-2, F7, P4-D4): starter
/// content; maps onto [`oxidizr_api::InitTemplate`].
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug, Default)]
pub enum InitCmdTemplate {
    /// A single software system + landscape view.
    Minimal,
    /// The canonical 3-software-system C4 landscape (the default).
    #[default]
    C4,
}

/// `oxidizr view --theme` values (F10) — mirrors the viewer's own
/// `ThemeArg` so both entries read alike; mapped when the `gui`
/// feature links the shell.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum, Debug)]
pub enum ViewTheme {
    /// Force the light scheme.
    Light,
    /// Force the dark scheme.
    Dark,
    /// Persisted toggle, else the OS preference, else light.
    System,
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
/// command logic here — EXCEPT F10 (§P3-M8): `view` is the GUI shell,
/// dispatched before the pipeline and never a [`Request`].
///
/// # Errors
///
/// [`anyhow::Error`] only for unimplemented milestones / internal faults
/// and — in builds WITHOUT the `gui` feature — the `view` rebuild hint.
pub fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    let renderer = Renderer::new(cli.no_color, cli.quiet);
    // §P4-M3 O-3: `completions` renders the argv surface via
    // `clap_complete` — CLI-only machinery, dispatched before the
    // pipeline like `view` (the script is stdout payload, never an
    // `Outcome` byte contract).
    if let Command::Completions { shell } = &cli.command {
        let mut command = Cli::command();
        clap_complete::generate(*shell, &mut command, "oxidizr", &mut std::io::stdout());
        return Ok(0);
    }
    // §P3-M8 / F10: `view` is the GUI shell, not a pipeline command —
    // it dispatches before anything touches `oxidizr-api`.
    if let Command::View { file, theme } = &cli.command {
        return run_view(file.as_deref(), *theme);
    }
    // §P4-M5 E-1 / F4: `oracle` spawns the EXTERNAL reference binary —
    // no workspace is ever opened here, so it is not a `Request`
    // (dispatched like `view`/`completions`; the report is the stdout
    // payload, the exit family 0/1/3 per §P4-M5 E-1).
    if let Command::Oracle {
        action,
        workspaces,
        bin,
        goldens,
        json,
    } = &cli.command
    {
        let action = match action {
            OracleAction::Gen => oracle::Action::Gen,
            OracleAction::Verify => oracle::Action::Verify,
        };
        let outcome = oracle::run(
            action,
            workspaces,
            bin.as_deref(),
            goldens.as_deref(),
            *json,
            cli.quiet,
        );
        return Ok(replay(&renderer, outcome));
    }
    let options = ApiOptions {
        strict: cli.strict,
        keep_unknown: cli.keep_unknown,
        // Production keeps the V-3 process-env behavior (ApiOptions::env=None).
        env: None,
        stdin: Some(read_stdin),
        allow_net: cli.allow_net,
        themes_dir: cli.themes_dir,
        // The `report::Renderer` rule (§P4-M0 D-3): `--no-color` or a set
        // `NO_COLOR` both disable the `diff` human paint.
        color: !cli.no_color && std::env::var_os("NO_COLOR").is_none(),
    };
    let request = to_request(cli.command, cli.quiet)
        .context("internal: `view` must dispatch before the pipeline")?;
    let outcome = oxidizr_api::execute(request, &options);
    Ok(replay(&renderer, outcome))
}

/// argv → Request mapping: `-`/omitted paths become the lazy [`Input::Stdin`]
/// variant (nothing is read before `execute` decides — a `fmt --write -`
/// usage exit must not block on an open pipe). 2R R-mis-10: the `Command`
/// MOVES into the [`Request`] — the old `&Cli` leg cloned every path the
/// caller was about to drop anyway. `None` ONLY for the §P3-M8 `view`
/// shell: [`run`] dispatches it BEFORE the pipeline — the GUI entry (F10)
/// is not a byte-contract command and never becomes a [`Request`]. The
/// global `--quiet` reaches `diff` (exit code only, §P4-M0 D-3) — the other
/// commands consume it in the [`Renderer`].
#[allow(clippy::too_many_lines)] // one arm per `Command` variant: the argv → Request table
fn to_request(command: Command, quiet: bool) -> Option<Request> {
    Some(match command {
        Command::Dsl2Json { dsl, output } => Request::Dsl2Json {
            input: input_of(dsl.as_deref()),
            output,
        },
        Command::Json2Json { json, output } => Request::Json2Json {
            input: input_of(json.as_deref()),
            output,
        },
        Command::Validate { files } => Request::Validate { files },
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
            mode: if check {
                FmtMode::Check
            } else if write {
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
            dsl: input_of(Some(&dsl)),
            layout: input_of(Some(&layout)),
            output,
        },
        Command::Export {
            workspace,
            format,
            output,
        } => Request::Export {
            workspace,
            format,
            output,
        },
        Command::Diff {
            a,
            b,
            format,
            ignore_layout,
        } => Request::Diff {
            a,
            b,
            json: format == DiffFormat::Json,
            ignore_layout,
            quiet,
        },
        Command::Inspect {
            workspace,
            severity_threshold,
            only,
            except,
            format,
        } => Request::Inspect {
            workspace,
            threshold: match severity_threshold {
                InspectThreshold::Info => Severity::Info,
                InspectThreshold::Warning => Severity::Warning,
                InspectThreshold::Error => Severity::Error,
            },
            only,
            except,
            json: format == InspectFormat::Json,
            quiet,
        },
        Command::Import {
            file,
            format,
            view_key,
            view_name,
            output,
        } => Request::Import {
            input: input_of(file.as_deref()),
            format: format.map(|f| match f {
                ImportCmdFormat::C4PlantUml => ImportFormat::C4PlantUml,
                ImportCmdFormat::Mermaid => ImportFormat::Mermaid,
                ImportCmdFormat::Image => ImportFormat::Image,
            }),
            view_key,
            view_name,
            output,
        },
        Command::Query {
            workspace,
            filter,
            kind,
            format,
            output,
        } => Request::Query {
            workspace,
            filter,
            kind: match kind {
                QueryCmdKind::Element => QueryKind::Element,
                QueryCmdKind::Relationship => QueryKind::Relationship,
                QueryCmdKind::View => QueryKind::View,
                QueryCmdKind::All => QueryKind::All,
            },
            format: match format {
                QueryCmdFormat::Table => QueryFormat::Table,
                QueryCmdFormat::Ids => QueryFormat::Ids,
                QueryCmdFormat::Json => QueryFormat::Json,
            },
            output,
        },
        Command::Init {
            dir,
            template,
            force,
        } => Request::Init {
            dir: dir.unwrap_or_else(|| PathBuf::from(".")),
            template: match template {
                InitCmdTemplate::Minimal => InitTemplate::Minimal,
                InitCmdTemplate::C4 => InitTemplate::C4,
            },
            force,
        },
        Command::Generate {
            command:
                GenerateCommand::SystemLandscape {
                    inputs,
                    include,
                    exclude,
                    relationships,
                    output,
                },
        } => Request::Generate {
            inputs,
            include,
            exclude,
            strategy: match relationships {
                RelationshipsArg::First => RelationshipsStrategy::First,
                RelationshipsArg::All => RelationshipsStrategy::All,
            },
            output,
        },
        // §P3-M8 + §P4-M3 O-3 + §P4-M5 E-1: dispatched in `run` before the
        // pipeline is reached (`view` = the GUI shell, `completions` = the
        // argv table, `oracle` = the external reference binary — none of
        // them is a `Request`).
        Command::View { .. } | Command::Completions { .. } | Command::Oracle { .. } => return None,
    })
}

/// §P3-M8 / F10: `oxidizr view` opens the native viewer window
/// IN-PROCESS when the cli is built with `gui` — the SAME
/// `oxidizr_viewer::shell::run` the standalone `oxidizr-viewer` binary
/// drives, so the entries cannot drift. eframe faults (no display)
/// travel through `anyhow` to the binary's exit-1 error arm.
#[cfg(feature = "gui")]
fn run_view(file: Option<&Path>, theme: ViewTheme) -> anyhow::Result<i32> {
    let theme = match theme {
        ViewTheme::Light => oxidizr_viewer::shell::ThemeArg::Light,
        ViewTheme::Dark => oxidizr_viewer::shell::ThemeArg::Dark,
        ViewTheme::System => oxidizr_viewer::shell::ThemeArg::System,
    };
    oxidizr_viewer::shell::run(file, theme)?;
    Ok(0)
}

/// The `gui`-less build (§P3-M8): the argv is valid clap surface, but
/// this binary links no shell — exit 1 via the binary's error arm with
/// the rebuild hint (the request is unserveable, P1-D8 codes).
#[cfg(not(feature = "gui"))]
fn run_view(_file: Option<&Path>, _theme: ViewTheme) -> anyhow::Result<i32> {
    anyhow::bail!(
        "`view` needs the `gui` feature: rebuild with `cargo build --features gui` \
         (or run the standalone `oxidizr-viewer` binary)"
    )
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
/// as one batch (single stderr write, as before). 2R R-mis-10: the outcome
/// MOVES in — issues step into the render batch instead of being cloned
/// item by item.
fn replay(renderer: &Renderer, outcome: Outcome) -> i32 {
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
    for item in outcome.log {
        match item {
            LogItem::Issue(issue) => batch.push(issue),
            LogItem::OkLine(line) => {
                flush(renderer, &mut batch);
                renderer.ok(&line);
            }
            LogItem::NoteLine(line) => {
                flush(renderer, &mut batch);
                renderer.note(&line);
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
            Command::Export { .. } => "export",
            Command::Diff { .. } => "diff",
            Command::Inspect { .. } => "inspect",
            Command::Import { .. } => "import",
            Command::Query { .. } => "query",
            Command::Init { .. } => "init",
            Command::Generate { .. } => "generate",
            Command::Completions { .. } => "completions",
            Command::Oracle { .. } => "oracle",
            Command::View { .. } => "view",
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)] // the argv matrix IS the content
    fn surface_parses() {
        Cli::try_parse_from(["oxidizr", "dsl2json", "ws.dsl", "-o", "ws.json"]).unwrap();
        Cli::try_parse_from(["oxidizr", "json2json", "-"]).unwrap();
        Cli::try_parse_from(["oxidizr", "validate", "a.dsl", "b.json"]).unwrap();
        Cli::try_parse_from(["oxidizr", "fmt", "--check", "x.dsl"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "inspect",
            "ws.dsl",
            "--severity-threshold",
            "warning",
            "--only",
            "model.empty",
            "--except",
            "views.empty",
            "--format",
            "json",
        ])
        .unwrap();
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
        Cli::try_parse_from(["oxidizr", "export", "-w", "ws.dsl", "-f", "theme"]).unwrap();
        // §P4-M2 N-4: `import` parses bare, with the sniffed format, and
        // with every explicit option (`c4plantuml` is the spelled value).
        Cli::try_parse_from(["oxidizr", "import", "diagram.puml"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "import",
            "-",
            "--format",
            "c4plantuml",
            "--view-key",
            "img",
            "--view-name",
            "Diagram",
            "-o",
            "ws.json",
        ])
        .unwrap();
        assert!(
            Cli::try_parse_from(["oxidizr", "import", "d.dsl", "--format", "plantuml"]).is_err()
        );
        Cli::try_parse_from([
            "oxidizr",
            "export",
            "-w",
            "ws.dsl",
            "-f",
            "plantuml",
            "--allow-net",
            "--themes-dir",
            "themes",
        ])
        .unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "export",
            "--workspace",
            "ws.json",
            "--format",
            "plantuml/c4plantuml-dark",
            "--output",
            "out/",
        ])
        .unwrap();
        // `--format` is required (reference parity: the option is REQUIRED);
        // missing it is a clap usage error, exit 2.
        assert!(Cli::try_parse_from(["oxidizr", "export", "-w", "ws.dsl"]).is_err());
        // §P3-M8 / F10: `view` parses with and without its optional file.
        Cli::try_parse_from(["oxidizr", "view"]).unwrap();
        Cli::try_parse_from(["oxidizr", "view", "ws.dsl", "--theme", "dark"]).unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "view", "--theme", "sepia"]).is_err());
        // §P4-M0 / F3: `diff` needs both sides; `--format` defaults to human.
        Cli::try_parse_from(["oxidizr", "diff", "a.dsl", "b.json"]).unwrap();
        Cli::try_parse_from(["oxidizr", "diff", "a.json", "b.json", "--format", "json"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "--quiet",
            "diff",
            "a.dsl",
            "b.dsl",
            "--ignore-layout",
        ])
        .unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "diff", "only-a.dsl"]).is_err());
        assert!(
            Cli::try_parse_from(["oxidizr", "diff", "a.dsl", "b.dsl", "--format", "yaml"]).is_err()
        );
        assert!(Cli::try_parse_from(["oxidizr", "merge"]).is_err());
        // `validate` needs at least one file (clap usage error, exit 2).
        assert!(Cli::try_parse_from(["oxidizr", "validate"]).is_err());
        // §P4-M3 O-3: `completions` takes exactly one known shell token.
        Cli::try_parse_from(["oxidizr", "completions", "bash"]).unwrap();
        Cli::try_parse_from(["oxidizr", "completions", "powershell"]).unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "completions", "csh"]).is_err());
        assert!(Cli::try_parse_from(["oxidizr", "completions"]).is_err());
        // §P4-M3 O-4: `generate system-landscape` takes 1..N inputs and
        // the full filter/strategy surface; zero inputs and unknown
        // strategies are clap usage errors (exit 2).
        Cli::try_parse_from(["oxidizr", "generate", "system-landscape", "ws.dsl"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "generate",
            "system-landscape",
            "dir",
            "extra.json",
            "--include",
            r".*alpha.*",
            "--exclude",
            r".*beta.*",
            "--relationships",
            "all",
            "-o",
            "landscape.json",
        ])
        .unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "generate", "system-landscape"]).is_err());
        assert!(
            Cli::try_parse_from([
                "oxidizr",
                "generate",
                "system-landscape",
                "a",
                "--relationships",
                "none"
            ])
            .is_err()
        );
        // §P4-M5 E-1: `oracle` — value-enum action positional, 1..N
        // workspace positionals; zero workspaces and unknown actions are
        // clap usage errors (exit 2).
        Cli::try_parse_from(["oxidizr", "oracle", "gen", "a.dsl"]).unwrap();
        Cli::try_parse_from([
            "oxidizr",
            "oracle",
            "verify",
            "a.dsl",
            "b.json",
            "--bin",
            "/opt/structurizr",
            "--goldens",
            "goldens",
            "--json",
        ])
        .unwrap();
        assert!(Cli::try_parse_from(["oxidizr", "oracle", "gen"]).is_err());
        assert!(Cli::try_parse_from(["oxidizr", "oracle", "check", "a.dsl"]).is_err());
    }

    #[test]
    fn command_names_stable() {
        let cli = Cli::try_parse_from(["oxidizr", "dsl2json"]).unwrap();
        assert_eq!(command_name(&cli.command), "dsl2json");
        // §P4-M3 O-4: the `generate` family reports ONE name (the
        // generator is a subcommand, like `export --format`).
        let cli =
            Cli::try_parse_from(["oxidizr", "generate", "system-landscape", "ws.dsl"]).unwrap();
        assert_eq!(command_name(&cli.command), "generate");
    }
}
