//! `oxidizr-import` — diagram-model importers (the additional feature
//! plus the official image importer).
//!
//! Each importer turns ONE diagram file into a fresh
//! [`oxidizr_core::Workspace`]: the `image` leg ports the official
//! image importer and image utilities
//! (file → `ImageView` data-URI / content-type / title); the
//! C4-PlantUML and Mermaid model importers are the agreed additional
//! features (registered: the official tooling has NO model-parser
//! counterpart), scope per the phase plan.
//!
//! Contract invariants:
//! - the output is a standard Structurizr workspace — features may never
//!   bend the JSON byte contract (rule 1/2 of the registry; the bless
//!   307×unchanged tripwire stands behind every milestone);
//! - warning-and-skip: unrecognized source constructs ride
//!   [`Imported::warnings`], they never abort the import;
//! - bodies-in-API stays upstream: loading, view-key policy at the CLI
//!   edge, serialization and reporting live in `oxidizr-api`; this
//!   crate is pure construction under the official workspace-builder
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

/// Everything that can stop an import. Messages quote the official tooling
/// `invalid-argument crash` texts verbatim (minus its
/// `getCanonicalPath()`: diagnostics print the path as given).
#[derive(Debug, Error)]
pub enum ImportError {
    /// The source file is absent (the official tooling: `"<path> does not exist."`).
    #[error("{0} does not exist.")]
    Missing(PathBuf),
    /// The source path exists but is not a regular file (the official tooling:
    /// `"<path> is not a file."`).
    #[error("{0} is not a file.")]
    NotFile(PathBuf),
    /// Not an image type the official tooling recognizes (the official tooling:
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
    /// A `!docs`/`!adrs` import failure carrying the official
    /// exception MESSAGE verbatim (the wrapped-cause texts and file
    /// paths included —
    /// the DSL layer prepends `Error importing documentation: ` /
    /// `Error importing decisions: ` exactly like the official
    /// end-of-context handlers).
    #[error("{0}")]
    Failed(String),
}

/// `oxidizr import` legs: which `oxidizr-import`
/// engine processes the source. Travels on the `oxidizr-api` pipeline
/// `Request::Import` (re-exported there unchanged); the CLI
/// `--format` clap enum maps onto it 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    /// C4-`PlantUML` macro diagram → the model.
    C4PlantUml,
    /// `Mermaid` C4 diagram → the model.
    Mermaid,
    /// Image file → a workspace with one `ImageView` (the official tooling oxidizr).
    Image,
}

impl ImportFormat {
    /// Extension sniff: `.puml`/`.iuml`/`.c4p` →
    /// C4-`PlantUML`, `.mmd`/`.mermaid` → `Mermaid`, the official tooling image
    /// extensions (the `ImageUtils` table behind
    /// [`image::content_type_of`]) → image.
    /// `None` = not guessable from the name.
    #[must_use]
    pub fn sniff(name: &str) -> Option<Self> {
        let dot = name.rfind('.')?;
        match name[dot..].to_ascii_lowercase().as_str() {
            ".puml" | ".iuml" | ".c4p" => Some(Self::C4PlantUml),
            ".mmd" | ".mermaid" => Some(Self::Mermaid),
            _ => image::content_type_of(name).map(|_| Self::Image),
        }
    }
}

/// The absolute-path form — lexical only: prepend the CWD and
/// normalize `.`/`..` segments WITHOUT touching the filesystem (the official tooling never
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
                // the official tooling pops the last name segment unless the path is just
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

/// The canonical-path form — resolve symlinks + `..` against the
/// filesystem; the official tooling throws when resolution fails, and the
/// importers never catch that path separately, so on failure oxidizr
/// falls back to the absolute form (an I/O-failure analogue).
pub(crate) fn java_canonical(path: &std::path::Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| java_absolute(path))
}

// ---------------------------------------------------------------------------
// The `!docs`/`!adrs` importer registry
// ---------------------------------------------------------------------------

/// FQN of the default `!docs` documentation importer — the
/// official default.
pub const DEFAULT_DOCUMENTATION_IMPORTER: &str =
    "com.structurizr.importer.documentation.DefaultDocumentationImporter";
/// FQN of the recursive `!docs` documentation importer.
pub const RECURSIVE_DOCUMENTATION_IMPORTER: &str =
    "com.structurizr.importer.documentation.RecursiveDefaultDocumentationImporter";
/// FQN of the default image importer — DSL-reachable through
/// `!docs`; the eager variant.
pub const DEFAULT_IMAGE_IMPORTER: &str =
    "com.structurizr.importer.documentation.DefaultImageImporter";
/// FQN of the `AdrTools` decision importer — the `!adrs` default
/// (`adrtools` token).
pub const ADRTOLS_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.AdrToolsDecisionImporter";
/// FQN of the Madr decision importer.
pub const MADR_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.MadrDecisionImporter";
/// FQN of the Log4brains decision importer.
pub const LOG4BRAINS_DECISION_IMPORTER: &str =
    "com.structurizr.importer.documentation.Log4brainsDecisionImporter";

/// One of the six built-in importer classes the official tooling loads
/// by name when the DSL contexts close. Everything
/// else is the `"… was not found"` arm (a plugin class loader is
/// out of reach by design).
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

/// The decision-importer token lookup (default: the token itself) — the
/// three aliases are case-SENSITIVE (a string-keyed table); any
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

#[cfg(test)]
mod tests {
    use super::ImportFormat;

    /// The sniff table (moved from `oxidizr-api`'s private `sniff_format`
    /// earlier — behavior byte-identical): the three macro families,
    /// case-insensitive extension match, image fallback via the official
    /// image-utils table, and the two `None` arms the CLI turns into the
    /// "pass --format" usage error.
    #[test]
    fn sniff_format_table() {
        assert_eq!(
            ImportFormat::sniff("diagram.puml"),
            Some(ImportFormat::C4PlantUml)
        );
        assert_eq!(
            ImportFormat::sniff("diagram.iuml"),
            Some(ImportFormat::C4PlantUml)
        );
        assert_eq!(
            ImportFormat::sniff("diagram.c4p"),
            Some(ImportFormat::C4PlantUml)
        );
        assert_eq!(
            ImportFormat::sniff("DIAGRAM.PUML"),
            Some(ImportFormat::C4PlantUml)
        );
        assert_eq!(ImportFormat::sniff("seq.mmd"), Some(ImportFormat::Mermaid));
        assert_eq!(
            ImportFormat::sniff("seq.Mermaid"),
            Some(ImportFormat::Mermaid)
        );
        assert_eq!(ImportFormat::sniff("pic.PNG"), Some(ImportFormat::Image));
        assert_eq!(ImportFormat::sniff("pic.jpeg"), Some(ImportFormat::Image));
        assert_eq!(ImportFormat::sniff("pic.gif"), Some(ImportFormat::Image));
        assert_eq!(ImportFormat::sniff("pic.svg"), Some(ImportFormat::Image));
        assert_eq!(
            ImportFormat::sniff("noext"),
            None,
            "no dot is not guessable"
        );
        assert_eq!(
            ImportFormat::sniff("weird.xyz"),
            None,
            "unknown ext is not guessable"
        );
    }
}
