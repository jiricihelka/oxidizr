//! The `ExportCommand` format registry and dispatch. The reference parses
//! `--format` lowercased and looks it up in a static map
//! (`json`, `theme`, `plantuml`/`plantuml[-light|-dark]`,
//! `plantuml/structurizr[±light|-dark]`, `plantuml/c4plantuml[±light|-dark]`,
//! `mermaid`, `websequencediagrams`), with `static` handled before the map
//! and `png`/`svg` before that (rejected: the Playwright build is absent);
//! anything else the JVM cannot class-load logs `Unknown export format: …`
//! and the command STILL EXITS 0 — mirrored as [`ExportFormat::Unknown`].

use oxidizr_core::Workspace;

use crate::diagram::WorkspaceExport;
use crate::error::ExportError;
use crate::naming::{ExportedFile, workspace_export_files};

/// `-light` / `-dark` format suffix (the reference constructs the exporter
/// with that `ColorScheme`; unmarked formats keep the workspace theme).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// No suffix.
    Default,
    /// `-light`.
    Light,
    /// `-dark`.
    Dark,
}

/// A parsed `--format` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportFormat {
    /// `json` — `JsonWorkspaceExporter` (the standard workspace JSON).
    Json,
    /// `theme` — `JsonWorkspaceThemeExporter` (`ThemeUtils.toJson`).
    Theme,
    /// `plantuml`, `plantuml-light|dark`, `plantuml/structurizr[±light|dark]`.
    StructurizrPlantUml(ColorMode),
    /// `plantuml/c4plantuml[±light|dark]`.
    C4PlantUml(ColorMode),
    /// `mermaid`.
    Mermaid,
    /// `websequencediagrams` (kept out of the Phase 2 scope, see
    /// `PHASE_2.md`).
    WebSequenceDiagrams,
    /// `static` — the reference site generator; not an oxidizr goal.
    Static,
    /// `png` — the reference build itself rejects this (no Playwright).
    Png,
    /// `svg` — as [`ExportFormat::Png`].
    Svg,
    /// Not in the registry and no JVM class name resolves — the reference
    /// logs and exits 0; the pipeline notes and succeeds.
    Unknown(String),
}

impl ExportFormat {
    /// Parse like `cmd.getOptionValue("format").toLowerCase()` + the
    /// `EXPORTERS` map keys. Only the `PlantUML` family carries a `-light` /
    /// `-dark` suffix there — `theme-light` etc. fall through to the
    /// class-load attempt (unknown here).
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let f = raw.to_ascii_lowercase();
        match f.as_str() {
            "json" => Self::Json,
            "theme" => Self::Theme,
            "mermaid" => Self::Mermaid,
            "websequencediagrams" => Self::WebSequenceDiagrams,
            "static" => Self::Static,
            "png" => Self::Png,
            "svg" => Self::Svg,
            other => match split_color_suffix(other) {
                ("plantuml" | "plantuml/structurizr", mode) => Self::StructurizrPlantUml(mode),
                ("plantuml/c4plantuml", mode) => Self::C4PlantUml(mode),
                _ => Self::Unknown(f),
            },
        }
    }

    /// Whether the format is known to the reference CLI (unknown strings are
    /// the exit-0 "Unknown export format" note, not an error).
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// The format string for diagnostics (canonical spelling of the request).
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Json => "json".into(),
            Self::Theme => "theme".into(),
            Self::StructurizrPlantUml(mode) => format!("plantuml{}", mode.suffix()),
            Self::C4PlantUml(mode) => format!("plantuml/c4plantuml{}", mode.suffix()),
            Self::Mermaid => "mermaid".into(),
            Self::WebSequenceDiagrams => "websequencediagrams".into(),
            Self::Static => "static".into(),
            Self::Png => "png".into(),
            Self::Svg => "svg".into(),
            Self::Unknown(f) => f.clone(),
        }
    }

    /// `ExportCommand` dispatch for the `WorkspaceExporter` branch — the M0
    /// implemented set (`json`, `theme`). Diagram formats report
    /// [`ExportError::NotImplemented`] until M1–M4; [`ExportFormat::Unknown`]
    /// is the CALLER's exit-0 note path (the reference never gets here).
    ///
    /// # Errors
    ///
    /// [`ExportError::NotImplemented`] for known-but-unlanded formats,
    /// [`ExportError::NoExtension`] when the workspace file name has no `.`,
    /// [`ExportError::Json`] when serialization fails.
    pub fn export(
        &self,
        workspace: &Workspace,
        workspace_file_name: &str,
    ) -> Result<Vec<ExportedFile>, ExportError> {
        let (export, theme) = match self {
            Self::Json => (
                WorkspaceExport::new(oxidizr_json::to_vec(workspace)?, "json".into()),
                false,
            ),
            Self::Theme => (
                WorkspaceExport::new(oxidizr_json::theme_to_vec(workspace), "json".into()),
                true,
            ),
            other => {
                return Err(ExportError::NotImplemented {
                    format: other.label(),
                });
            }
        };
        workspace_export_files(&export, theme, workspace_file_name)
    }
}

/// Peel a `-light`/`-dark` suffix off a `PlantUML` family format string
/// (`EXPORTERS` puts the suffix on the `plantuml*` keys only).
fn split_color_suffix(f: &str) -> (&str, ColorMode) {
    match f.rsplit_once('-') {
        Some((base, "light")) => (base, ColorMode::Light),
        Some((base, "dark")) => (base, ColorMode::Dark),
        _ => (f, ColorMode::Default),
    }
}

impl ColorMode {
    fn suffix(self) -> &'static str {
        match self {
            Self::Default => "",
            Self::Light => "-light",
            Self::Dark => "-dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_keys_parse_like_the_reference() {
        use ExportFormat::{
            C4PlantUml, Json, Mermaid, Png, Static, StructurizrPlantUml, Svg, Theme,
            WebSequenceDiagrams,
        };
        let cases = [
            ("json", Json),
            ("THEME", Theme),
            ("plantuml", StructurizrPlantUml(ColorMode::Default)),
            ("plantuml-light", StructurizrPlantUml(ColorMode::Light)),
            ("plantuml-dark", StructurizrPlantUml(ColorMode::Dark)),
            (
                "plantuml/structurizr",
                StructurizrPlantUml(ColorMode::Default),
            ),
            (
                "PlantUML/Structurizr-LIGHT",
                StructurizrPlantUml(ColorMode::Light),
            ),
            ("plantuml/c4plantuml", C4PlantUml(ColorMode::Default)),
            ("plantuml/c4plantuml-dark", C4PlantUml(ColorMode::Dark)),
            ("mermaid", Mermaid),
            ("websequencediagrams", WebSequenceDiagrams),
            ("static", Static),
            ("png", Png),
            ("svg", Svg),
        ];
        for (raw, want) in cases {
            assert_eq!(ExportFormat::parse(raw), want, "{raw}");
            assert!(ExportFormat::parse(raw).is_known(), "{raw}");
        }
    }

    #[test]
    fn anything_else_is_the_exit_zero_unknown_path() {
        // The reference tries to CLASS-LOAD these; without a JVM match it
        // logs "Unknown export format" and returns true (exit 0).
        for raw in [
            "foo",
            "com.example.MyExporter",
            "plantuml/other",
            "plantuml-cyan",
        ] {
            let f = ExportFormat::parse(raw);
            assert!(
                matches!(&f, ExportFormat::Unknown(s) if s == &raw.to_ascii_lowercase()),
                "{raw}"
            );
            assert!(!f.is_known(), "{raw}");
        }
    }

    #[test]
    fn labels_round_trip_known_formats() {
        for raw in [
            "json",
            "theme",
            "plantuml",
            "plantuml-light",
            "plantuml/structurizr-dark",
            "plantuml/c4plantuml",
            "mermaid",
            "websequencediagrams",
            "static",
        ] {
            let f = ExportFormat::parse(raw);
            assert_eq!(ExportFormat::parse(&f.label()), f, "{raw}");
        }
    }
}
