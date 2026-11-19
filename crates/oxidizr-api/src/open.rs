//! The viewer-facing open entry (P3-M1-1, `agents/plan/build/PHASE_3.md`
//! §P3-M1): load a workspace file for INSPECTION. This is deliberately NOT
//! a byte-contract path — the [`crate::pipeline::execute`] command bodies,
//! their goldens and the CLI are untouched; the entry only reuses the
//! pipeline's private load stages.
//!
//! Load tail mirror (reference `AbstractCommand.loadWorkspace`, same as
//! `load_export_workspace`): read → parse by extension (`.dsl` / `.json`)
//! → [`apply_default_views`]. Viewer twist (the M1-1 row): a DSL with
//! recoverable statement errors OPENS PARTIALLY — the decorated D1 issues
//! ride [`Opened::log`] and the repaired workspace renders what parsed
//! (the reference web tool has no DSL leg at all, so no parity constraint
//! exists here; `--strict` has no meaning for a read-only viewer). JSON is
//! strict (a half-read workspace would silently misrepresent the file).
//! The core-validation tail runs INFORMATIONALLY: violations join the log,
//! they never block the open.

use oxidizr_core::Workspace;
use oxidizr_diag::codes;

use crate::Issue;
use crate::pipeline::{
    ApiOptions, ExitClass, Input, LogItem, Outcome, apply_default_views, parse_dsl_input,
    read_input, statement_errors,
};

/// A workspace opened for viewing.
#[derive(Debug)]
pub struct Opened {
    /// The loaded workspace (default views applied).
    pub workspace: Workspace,
    /// Input display name (canonicalized path, `-` for stdin/bytes).
    pub display: String,
    /// Diagnostics in emission order: DSL statement errors (partial open)
    /// first, then core-validation violations. Never blocking (the open
    /// succeeded); the viewer presents them in its diagnostics panel.
    pub log: Vec<LogItem>,
}

/// Open a workspace for the viewer.
///
/// Format dispatch follows the `validate`/`export` loaders: the file
/// extension decides (`.dsl` → DSL parse, `.json` → `oxidizr_json`);
/// extension-less inputs ([`Input::Bytes`] / [`Input::Stdin`]) are DSL,
/// mirroring the `dsl2json` stdin semantics.
///
/// # Errors
///
/// Outcome-shaped failure: exit class `Io` for read/UTF-8 failures,
/// `Diagnostics` for fatal parse failures and unknown formats — the same
/// classes the command bodies use for the same conditions.
pub fn open(input: &Input, opts: &ApiOptions) -> Result<Opened, Outcome> {
    let source = read_input(input, opts).map_err(|e| failing(e, ExitClass::Io))?;
    let mut log: Vec<LogItem> = Vec::new();
    let mut workspace = match extension_of(input).as_deref() {
        Some("json") => oxidizr_json::from_slice(source.text.as_bytes()).map_err(|e| {
            failing(
                Issue::from_json(&e, &source.text, &source.display),
                ExitClass::Diagnostics,
            )
        })?,
        Some("dsl") | None => {
            let result =
                parse_dsl_input(&source, opts).map_err(|e| failing(e, ExitClass::Diagnostics))?;
            // D1 recovery, viewer twist: the repaired workspace opens even
            // with statement errors (see module docs).
            log.extend(statement_errors(&result).into_iter().map(LogItem::Issue));
            result.workspace
        }
        Some(_) => {
            return Err(failing(
                Issue::error(
                    codes::CLI_IO,
                    format!(
                        "cannot determine the workspace format of `{}` (expected `.dsl` or `.json`)",
                        source.display
                    ),
                ),
                ExitClass::Diagnostics,
            ));
        }
    };
    apply_default_views(&mut workspace).map_err(|e| failing(e, ExitClass::Diagnostics))?;
    log.extend(
        oxidizr_core::validate(&workspace.model, &workspace.views)
            .iter()
            .map(|v| LogItem::Issue(Issue::validation(&v.message))),
    );
    Ok(Opened {
        workspace,
        display: source.display,
        log,
    })
}

/// Extension dispatch key (lowercased); non-file inputs have none (→ DSL).
fn extension_of(input: &Input) -> Option<String> {
    match input {
        Input::File(path) => path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase),
        Input::Bytes(_) | Input::Stdin => None,
    }
}

/// Outcome-shaped failure (the `Fail` ladder flattened at this boundary).
fn failing(issue: Issue, exit: ExitClass) -> Outcome {
    Outcome {
        exit,
        log: vec![LogItem::Issue(issue)],
        bytes: None,
    }
}
