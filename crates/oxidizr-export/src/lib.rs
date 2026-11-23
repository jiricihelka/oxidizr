//! `oxidizr-export` — the exporter engine. It depends only on `core` +
//! `json`, never on the DSL/AST: exporters consume the workspace model,
//! not sources.
//!
//! The parts: [`indent::IndentingWriter`], the [`diagram`] model
//! (`Diagram`, `Legend`, the [`WorkspaceExporter`]/[`DiagramExporter`]
//! shapes), the shared property cascade ([`property::export_property`])
//! and the `export` command's dispatch + file-naming rules
//! ([`mod@format`], [`naming`]):
//!
//! * format strings are lowercased, the registry is `json`, `theme`,
//!   `plantuml[/structurizr|/c4plantuml][-light|-dark]`, `mermaid`,
//!   `websequencediagrams`, `static`, `png`, `svg`; anything else is the
//!   "unknown format" path — the official tooling logs it and exits 0, so
//!   we note and exit 0;
//! * workspace exports (`json`, `theme`) write `<workspace-file-stem>[-theme].json`;
//! * diagrams write `<prefix>-<key>.<ext>` (`<prefix>-<key>-key.<ext>` for
//!   the legend, `<prefix>-<key>-<n>.<ext>` for animation frames) with
//!   `prefix = structurizr-<id>` (`structurizr` when the id is not positive).
//!
//! `json` and `theme` are byte-for-byte identical to what the official
//! Structurizr tooling writes; the `plantuml`, `plantuml/structurizr`,
//! `plantuml/c4plantuml`, `mermaid` and `websequencediagrams` diagram
//! formats are implemented too. `static` (the official site generator) is
//! not an oxidizr goal, and `png`/`svg` are rejected by the official build
//! itself (no browser toolchain) — all three report
//! `ExportError::NotImplemented`.

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
