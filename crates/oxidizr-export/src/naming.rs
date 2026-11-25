//! The `export` file-naming rules:
//!
//! * workspace exports (`json`, `theme`): the workspace FILE name up to its
//!   LAST `.`, `-theme` for the theme format, then `.<export extension>` —
//!   written into the output directory;
//! * diagrams: `structurizr-<id>-<key>.<ext>` (`structurizr-<key>.<ext>`
//!   when the id is not positive), legend as
//!   `…-<key>-key.<ext>`, animation frames as `…-<key>-<n>.<ext>` starting
//!   at 1, the frames carrying the PARENT diagram's key and extension.

use crate::diagram::{Diagram, WorkspaceExport};
use crate::error::ExportError;

/// One named payload the pipeline writes into the output directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    /// File name (no directory) — the caller joins the output directory.
    pub name: String,
    /// Exact bytes (no trailing newline added).
    pub bytes: Vec<u8>,
}

/// The diagram file prefix: `structurizr-<id>` for a positive id,
/// bare `structurizr` otherwise.
#[must_use]
pub(crate) fn diagram_prefix(workspace_id: i64) -> String {
    if workspace_id > 0 {
        format!("structurizr-{workspace_id}")
    } else {
        "structurizr".to_string()
    }
}

/// The workspace-export stem: name up to (not including) the LAST `.`.
fn stem(file_name: &str) -> Result<&str, ExportError> {
    file_name
        .rfind('.')
        .map(|i| &file_name[..i])
        .ok_or_else(|| ExportError::NoExtension {
            name: file_name.to_string(),
        })
}

/// The single file of a whole-workspace export:
/// `<stem>[-theme].<extension>`.
///
/// # Errors
///
/// [`ExportError::NoExtension`] when `workspace_file_name` has no `.` —
/// the official tooling fails outright there; oxidizr reports an input
/// error instead.
pub(crate) fn workspace_export_files(
    export: &WorkspaceExport,
    theme: bool,
    workspace_file_name: &str,
) -> Result<Vec<ExportedFile>, ExportError> {
    let suffix = if theme { "-theme" } else { "" };
    let stem = stem(workspace_file_name)?;
    Ok(vec![ExportedFile {
        name: format!("{stem}{suffix}.{}", export.file_extension()),
        bytes: export.definition().to_vec(),
    }])
}

/// The [`crate::diagram::DiagramExporter`] files: one per diagram, plus
/// `-key` legends and
/// numbered frame files (the parent's key and extension, frames indexed
/// from 1).
#[must_use]
pub(crate) fn diagram_files(prefix: &str, diagrams: &[Diagram]) -> Vec<ExportedFile> {
    let mut out = Vec::new();
    for diagram in diagrams {
        out.push(ExportedFile {
            name: format!("{prefix}-{}.{}", diagram.key(), diagram.file_extension()),
            bytes: diagram.definition().as_bytes().to_vec(),
        });
        if let Some(legend) = diagram.legend() {
            out.push(ExportedFile {
                name: format!(
                    "{prefix}-{}-key.{}",
                    diagram.key(),
                    diagram.file_extension()
                ),
                bytes: legend.definition().as_bytes().to_vec(),
            });
        }
        for (index, frame) in diagram.frames().iter().enumerate() {
            out.push(ExportedFile {
                name: format!(
                    "{prefix}-{}-{}.{}",
                    diagram.key(),
                    index + 1,
                    diagram.file_extension()
                ),
                bytes: frame.definition().as_bytes().to_vec(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::Legend;

    #[test]
    fn prefix_mirrors_the_reference() {
        assert_eq!(diagram_prefix(1), "structurizr-1");
        assert_eq!(diagram_prefix(0), "structurizr");
        assert_eq!(diagram_prefix(-1), "structurizr");
    }

    #[test]
    fn workspace_names_use_the_stem_and_the_theme_suffix() {
        let export = WorkspaceExport::new(b"{}".to_vec(), "json".into());
        let files = workspace_export_files(&export, false, "big-bank-plc.json").unwrap();
        assert_eq!(files[0].name, "big-bank-plc.json");
        let files = workspace_export_files(&export, true, "big-bank-plc.dsl").unwrap();
        assert_eq!(files[0].name, "big-bank-plc-theme.json");
        // LAST dot splits:
        let files = workspace_export_files(&export, true, "a.b.dsl").unwrap();
        assert_eq!(files[0].name, "a.b-theme.json");
        // no dot: the official tooling fails outright — oxidizr reports
        // the input:
        assert!(matches!(
            workspace_export_files(&export, true, "workspace"),
            Err(ExportError::NoExtension { .. })
        ));
    }

    #[test]
    fn diagram_frames_and_legends_follow_the_parent() {
        let mut d = Diagram::new("SystemLandscape".into(), "main".into(), "puml".into());
        d.set_legend(Legend::new("legend".into()));
        d.add_frame(Diagram::new(
            "frame-key".into(),
            "f1".into(),
            "ignored.ext".into(),
        ));
        d.add_frame(Diagram::new("x".into(), "f2".into(), "x".into()));
        let files = diagram_files("structurizr-1", &[d]);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "structurizr-1-SystemLandscape.puml",
                "structurizr-1-SystemLandscape-key.puml",
                "structurizr-1-SystemLandscape-1.puml",
                "structurizr-1-SystemLandscape-2.puml",
            ]
        );
        assert_eq!(files[2].bytes, b"f1");
    }
}
