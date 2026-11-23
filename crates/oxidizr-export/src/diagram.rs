//! The export bean model: one [`Diagram`] per view (key, definition, file
//! extension, animation frames, optional legend), the [`Legend`] side-car,
//! and the [`WorkspaceExport`] whole-workspace payload. The two exporter
//! shapes become traits: [`WorkspaceExporter`] (whole workspace, one
//! document) and [`DiagramExporter`] (one diagram per view).

use oxidizr_core::Workspace;

use crate::error::ExportError;

/// A single definition string written next to its diagram as
/// `<prefix>-<key>-key.<ext>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Legend {
    definition: String,
}

impl Legend {
    #[must_use]
    pub fn new(definition: String) -> Self {
        Self { definition }
    }

    #[must_use]
    pub fn definition(&self) -> &str {
        &self.definition
    }
}

/// The export of one view. The view key is stored at construction because
/// the exporters own key naming: frame diagrams reuse the PARENT key for
/// file names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    key: String,
    definition: String,
    file_extension: String,
    frames: Vec<Diagram>,
    legend: Option<Legend>,
}

impl Diagram {
    #[must_use]
    pub fn new(key: String, definition: String, file_extension: String) -> Self {
        Self {
            key,
            definition,
            file_extension,
            frames: Vec::new(),
            legend: None,
        }
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn definition(&self) -> &str {
        &self.definition
    }

    #[must_use]
    pub fn file_extension(&self) -> &str {
        &self.file_extension
    }

    #[must_use]
    pub fn frames(&self) -> &[Diagram] {
        &self.frames
    }

    pub fn add_frame(&mut self, frame: Diagram) {
        self.frames.push(frame);
    }

    #[must_use]
    pub fn legend(&self) -> Option<&Legend> {
        self.legend.as_ref()
    }

    pub fn set_legend(&mut self, legend: Legend) {
        self.legend = Some(legend);
    }
}

/// One whole-workspace document plus its file extension (`json` for the
/// workspace JSON and the theme export).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceExport {
    definition: Vec<u8>,
    file_extension: String,
}

impl WorkspaceExport {
    #[must_use]
    pub fn new(definition: Vec<u8>, file_extension: String) -> Self {
        Self {
            definition,
            file_extension,
        }
    }

    #[must_use]
    pub fn definition(&self) -> &[u8] {
        &self.definition
    }

    #[must_use]
    pub fn file_extension(&self) -> &str {
        &self.file_extension
    }
}

/// The whole workspace as one document.
pub trait WorkspaceExporter {
    /// Exports the workspace as a single document.
    ///
    /// # Errors
    ///
    /// Format-specific failures (see [`ExportError`]).
    fn export(&self, workspace: &Workspace) -> Result<WorkspaceExport, ExportError>;
}

/// One diagram per view.
pub trait DiagramExporter {
    /// Exports every view of the workspace as a diagram.
    ///
    /// # Errors
    ///
    /// Format-specific failures (see [`ExportError`]).
    fn export_diagrams(&self, workspace: &Workspace) -> Result<Vec<Diagram>, ExportError>;
}
