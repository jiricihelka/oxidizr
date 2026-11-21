//! `oxidizr-import` — diagram-model importers (P4 §P4-M2, F12 + the
//! reference image port).
//!
//! Each importer turns ONE diagram file into a fresh
//! [`Workspace`](oxidizr_core::Workspace): the `image` leg is the port of
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
pub mod image;
pub mod macrocall;
pub mod mermaid;

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
}
