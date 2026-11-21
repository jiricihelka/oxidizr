//! `oxidizr-export` — the Phase 2 exporter engine (P2-D1: a crate of its
//! own, depending only on `core` + `json`; no DSL/AST — exporters consume
//! the workspace model, not sources).
//!
//! Ports of `com.structurizr.export` (`structurizr-export` at the pinned
//! monorepo revision): [`indent::IndentingWriter`], the [`diagram`] bean
//! model (`Diagram`, `Legend`, the `WorkspaceExporter`/`DiagramExporter`
//! shapes), the shared property cascade ([`property::export_property`],
//! P2-D8) and the `export` command's dispatch + file-naming rules
//! ([`format`], [`naming`]) per `ExportCommand` (oracle-verified 2026-11-17,
//! P2-V1):
//!
//! * format strings are lowercased, the registry is `json`, `theme`,
//!   `plantuml[/structurizr|/c4plantuml][-light|-dark]`, `mermaid`,
//!   `websequencediagrams`, `static`, `png`, `svg`; anything else the JVM
//!   cannot class-load is the "unknown format" path — the reference LOGS
//!   and exits 0, so we note and exit 0;
//! * workspace exports (`json`, `theme`) write `<workspace-file-stem>[-theme].json`;
//! * diagrams write `<prefix>-<key>.<ext>` (`<prefix>-<key>-key.<ext>` for
//!   the legend, `<prefix>-<key>-<n>.<ext>` for animation frames) with
//!   `prefix = structurizr-<id>` (`structurizr` when the id is not positive).
//!
//! The two implemented formats are `theme` (`oxidizr_json::theme_to_vec`,
//! byte-parity per its tests) and `json` (`oxidizr_json::to_vec`); the
//! diagram formats surface `ExportError::NotImplemented` until M1–M4 land.

pub mod diagram;
pub mod error;
pub mod format;
pub mod indent;
pub mod mermaid;
pub mod naming;
pub mod paths;
pub mod plantuml;
pub mod property;
pub mod themes;
pub mod wsd;

pub use diagram::{Diagram, DiagramExporter, Legend, WorkspaceExport, WorkspaceExporter};
pub use error::ExportError;
pub use format::{ColorMode, ExportFormat};
pub use indent::{IndentType, IndentingWriter};
pub use naming::{ExportedFile, diagram_files, diagram_prefix, workspace_export_files};
pub use property::export_property;
pub use themes::{ThemeError, ThemeStore};
