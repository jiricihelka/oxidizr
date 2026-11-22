//! `oxidizr-import` — diagram-model importers (P4 §P4-M2, F12 + the
//! reference image port).
//!
//! Each importer turns ONE diagram file into a fresh
//! [`oxidizr_core::Workspace`]: the `image` leg is the port of
//! `structurizr-import` `diagrams/image/ImageImporter` + `ImageUtils`
//! (file → `ImageView` data-URI / content-type / title); the
//! C4-PlantUML and Mermaid model importers are the agreed additional
//! feature **F12** (`agents/plan/ADDITIONAL_FEATURES.md` — the pinned
//! reference has NO model-parser counterpart, P4-V3), scope per
//! `agents/plan/build/PHASE_4.md` §P4-M2.
//!
//! Contract invariants:
//! - the output is a standard Structurizr workspace — features may never
//!   bend the JSON byte contract (rule 1/2 of the registry; the bless
//!   307×unchanged tripwire stands behind every milestone);
//! - warning-and-skip: unrecognized source constructs ride
//!   [`Imported::warnings`], they never abort the import (P4-D8, R-1);
//! - bodies-in-API stays upstream: loading, view-key policy at the CLI
//!   edge, serialization and reporting live in `oxidizr-api` (T0-d); this
//!   crate is pure construction via the reference `WorkspaceBuilder`
//!   rules (generated view keys, view order, id allocation).

pub mod c4plantuml;
pub mod decisions;
pub mod docs;
pub mod image;
pub mod macrocall;
pub mod mermaid;
pub mod regexlite;

#[cfg(test)]
pub(crate) mod testutil;

use std::path::PathBuf;

use oxidizr_core::{CoreError, Workspace};
use thiserror::Error;

/// The result of one successful import: the fresh workspace plus the
/// non-fatal notes collected along the way (warning-and-skip — the
/// `image` leg never warns; the macro importers report every skipped
/// construct here).
#[derive(Debug, Clone)]
pub struct Imported {
    /// The imported workspace: the `image` leg carries one `ImageView`,
    /// the macro legs carry the model only (no view kind may host a
    /// mixed C4 element set — see `c4plantuml` module docs; the default
    /// views install at open via `api::open`).
    pub workspace: Workspace,
    /// Human-readable skip notes, source order; rendered by the CLI.
    pub warnings: Vec<String>,
}

/// Everything that can stop an import. Messages quote the reference
/// `IllegalArgumentException` texts verbatim (minus its
/// `getCanonicalPath()`: diagnostics print the path as given).
#[derive(Debug, Error)]
pub enum ImportError {
    /// The source file is absent (Java: `"<path> does not exist."`).
    #[error("{0} does not exist.")]
    Missing(PathBuf),
    /// The source path exists but is not a regular file (Java:
    /// `"<path> is not a file."`).
    #[error("{0} is not a file.")]
    NotFile(PathBuf),
    /// Not an image type the reference recognizes (Java:
    /// `"<path> is not a supported image file."`).
    #[error("{0} is not a supported image file.")]
    Unsupported(PathBuf),
    /// An I/O failure reading the source.
    #[error("could not read {}", path.display())]
    Io {
        /// The source path.
        path: PathBuf,
        /// The OS error.
        source: std::io::Error,
    },
    /// A workspace-construction failure (view-key uniqueness…).
    #[error(transparent)]
    Core(#[from] CoreError),
    /// A `!docs`/`!adrs` import failure carrying the reference
    /// exception MESSAGE verbatim (the `DocumentationImportException` /
    /// `IllegalArgumentException` texts, `java.io.File` paths included —
    /// the DSL layer prepends `Error importing documentation: ` /
    /// `Error importing decisions: ` exactly like the reference `end()`
    /// handlers).
    #[error("{0}")]
    Failed(String),
}

/// `java.io.File.getAbsolutePath()` — lexical only: prepend the CWD and
/// normalize `.`/`..` segments WITHOUT touching the filesystem (Java never
/// fails here; symlinks stay unresolved, that is `getCanonicalPath`).
pub(crate) fn java_absolute(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};
    let raw = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let mut out = PathBuf::from(if raw.is_absolute() { "/" } else { "" });
    for c in raw.components() {
        match c {
            Component::RootDir => out.push("/"),
            Component::Normal(seg) => out.push(seg),
            Component::ParentDir => {
                // Java pops the last name segment unless the path is just
                // `/`; an empty result keeps `/`.
                if !matches!(out.components().next_back(), Some(Component::RootDir)) {
                    out.pop();
                }
            }
            Component::CurDir | Component::Prefix(_) => {}
        }
    }
    if out.as_os_str().is_empty() {
        out.push("/");
    }
    out
}

/// `java.io.File.getCanonicalPath()` — resolve symlinks + `..` against the
/// filesystem; Java throws `IOException` when resolution fails, the
/// importers never catch that path separately, so on failure the port
/// falls back to the absolute form (an I/O-failure analogue).
pub(crate) fn java_canonical(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| java_absolute(path))
}

// ---------------------------------------------------------------------------
// The `!docs`/`!adrs` importer registry (P4-M2f)
// ---------------------------------------------------------------------------

/// FQN of `documentation/DefaultDocumentationImporter` — the `!docs`
/// default (`DocsParser.DEFAULT_DOCUMENT_IMPORTER`).
pub const DEFAULT_DOCUMENTATION_IMPORTER: &str =
    "com.structurizr.importer.documentation.DefaultDocumentationImporter";
/// FQN of `documentation/RecursiveDefaultDocumentationImporter`.
pub const RECURSIVE_DOCUMENTATION_IMPORTER: &str =
    "com.structurizr.importer.documentation.RecursiveDefaultDocumentationImporter";
/// FQN of `documentation/DefaultImageImporter` — DSL-reachable through
/// `!docs`; the eager variant (no-arg constructor).
pub const DEFAULT_IMAGE_IMPORTER: &str =
    "com.structurizr.importer.documentation.DefaultImageImporter";
/// FQN of `documentation/AdrToolsDecisionImporter` — the `!adrs` default
/// (`DecisionsParser`: `DECISION_IMPORTERS.get("adrtools")`).
pub const ADRTOLS_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.AdrToolsDecisionImporter";
/// FQN of `documentation/MadrDecisionImporter`.
pub const MADR_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.MadrDecisionImporter";
/// FQN of `documentation/Log4brainsDecisionImporter`.
pub const LOG4BRAINS_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.Log4brainsDecisionImporter";

/// One of the six built-in importer classes the reference `loadClass`s
/// (`DocumentationDslContext`/`DecisionsDslContext.end()`). Everything
/// else is the `"… was not found"` arm (the JVM `URLClassLoader` over
/// plugin jars is out of reach by design — D7/D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTarget {
    /// `DefaultDocumentationImporter` — `!docs` default.
    Docs,
    /// `RecursiveDefaultDocumentationImporter`.
    RecursiveDocs,
    /// `DefaultImageImporter` (eager — no-arg constructor).
    Images,
    /// `AdrToolsDecisionImporter` — `!adrs` default.
    AdrTools,
    /// `MadrDecisionImporter`.
    Madr,
    /// `Log4brainsDecisionImporter`.
    Log4brains,
}

/// `Class.forName(fqn)` against the six built-ins.
#[must_use]
pub fn resolve_importer(fqn: &str) -> Option<ImportTarget> {
    match fqn {
        DEFAULT_DOCUMENTATION_IMPORTER => Some(ImportTarget::Docs),
        RECURSIVE_DOCUMENTATION_IMPORTER => Some(ImportTarget::RecursiveDocs),
        DEFAULT_IMAGE_IMPORTER => Some(ImportTarget::Images),
        ADRTOLS_DECISION_IMPORTER => Some(ImportTarget::AdrTools),
        MADR_DECISION_IMPORTER => Some(ImportTarget::Madr),
        LOG4BRAINS_DECISION_IMPORTER => Some(ImportTarget::Log4brains),
        _ => None,
    }
}

/// `DecisionsParser.DECISION_IMPORTERS.getOrDefault(token, token)` — the
/// three aliases are case-SENSITIVE (Java `HashMap<String,String>`); any
/// other token passes through as the FQN to resolve.
#[must_use]
pub fn decision_importer_fqn(token: &str) -> String {
    match token {
        "adrtools" => ADRTOLS_DECISION_IMPORTER.to_owned(),
        "madr" => MADR_DECISION_IMPORTER.to_owned(),
        "log4brains" => LOG4BRAINS_DECISION_IMPORTER.to_owned(),
        other => other.to_owned(),
    }
}

/// `DocumentationContent.setContent` — `replaceAll("\\r\\n|\\r|\\r|\\n",
/// "\n")`: every newline run collapses to a single `\n` in ONE pass.
pub(crate) fn normalize_newlines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                out.push('\n');
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            '\n' => out.push('\n'),
            other => out.push(other),
        }
    }
    out
}
