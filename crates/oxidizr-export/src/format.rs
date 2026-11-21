//! The `ExportCommand` format registry and dispatch. The reference parses
//! `--format` lowercased and looks it up in a static map
//! (`json`, `theme`, `plantuml`/`plantuml[-light|-dark]`,
//! `plantuml/structurizr[±light|-dark]`, `plantuml/c4plantuml[±light|-dark]`,
//! `mermaid`, `websequencediagrams`), with `static` handled before the map
//! and `png`/`svg` before that (rejected: the Playwright build is absent);
//! anything else the JVM cannot class-load logs `Unknown export format: …`
//! and the command STILL EXITS 0 — mirrored as [`ExportFormat::Unknown`].

use oxidizr_core::Workspace;
use oxidizr_core::views::ColorScheme;

use crate::diagram::WorkspaceExport;
use crate::error::ExportError;
use crate::naming::{ExportedFile, diagram_files, diagram_prefix, workspace_export_files};
use crate::plantuml::{C4PlantUmlDialect, Engine, StructurizrDialect};

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
    /// `websequencediagrams` — the `WebSequenceDiagramsExporter` port
    /// (P4 §P4-M2 N-5): custom and dynamic views only, `.wsd` files.
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

    /// Whether the reference dispatches this format through a
    /// `DiagramExporter` — the branch that runs `ThemeUtils.loadThemes`
    /// BEFORE the export (`ExportCommand` 222-227). `static` is handled
    /// before the registry map, `png`/`svg` are rejected at build time, so
    /// none of them load themes.
    #[must_use]
    pub const fn is_diagram(&self) -> bool {
        matches!(
            self,
            Self::StructurizrPlantUml(_)
                | Self::C4PlantUml(_)
                | Self::Mermaid
                | Self::WebSequenceDiagrams
        )
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

    /// `ExportCommand` dispatch. `plantuml` / `plantuml/structurizr` (M2-d)
    /// run the `DiagramExporter` branch — the engine + `StructurizrDialect`,
    /// one file per diagram plus `-key` legend and numbered frame files, the
    /// id-keyed names of the reference write loop. `json` and `theme` are the
    /// implemented `WorkspaceExporter` pair (M0); `plantuml`/`structurizr`,
    /// `plantuml/c4plantuml`, `mermaid` (M2–M4) and `websequencediagrams`
    /// (P4 §P4-M2 N-5) are the implemented `DiagramExporter` quartet. The
    /// remaining registry formats report [`ExportError::NotImplemented`];
    /// [`ExportFormat::Unknown`] is the CALLER's exit-0 note path (the
    /// reference never gets here).
    ///
    /// # Errors
    ///
    /// [`ExportError::NotImplemented`] for known-but-unlanded formats,
    /// [`ExportError::NoExtension`] when the workspace file name has no `.`
    /// (the `WorkspaceExporter` branch only), [`ExportError::Json`] when
    /// serialization fails, [`ExportError::ReferenceAbort`] when the
    /// diagram engine hits a Java abort path (§14).
    pub fn export(
        &self,
        workspace: &Workspace,
        workspace_file_name: &str,
    ) -> Result<Vec<ExportedFile>, ExportError> {
        // 2R R-exp-18: ONE dispatch — the old shape matched `self` twice
        // for the plantuml pair and used a separate `matches!` for mermaid.
        let diagrams = match self {
            // The `DiagramExporter` branch of `ExportCommand.run`: the
            // workspace file name plays no role (names key on the workspace
            // ID — `prefix(workspaceId)`).
            Self::StructurizrPlantUml(mode) => {
                let mut engine = Engine::new(workspace, mode.color_scheme());
                engine.export_all(&mut StructurizrDialect::new())?
            }
            Self::C4PlantUml(mode) => {
                let mut engine = Engine::new(workspace, mode.color_scheme());
                engine.export_all(&mut C4PlantUmlDialect::new())?
            }
            // `MermaidDiagramExporter` has no color-scheme constructor: the
            // no-arg export is `this(ColorScheme.Light)`. The `.mmd`
            // extension rides each `Diagram` (`diagram_files` threads
            // `file_extension`).
            Self::Mermaid => {
                let mut engine = Engine::new(workspace, ColorScheme::Light);
                engine.export_all(&mut crate::mermaid::MermaidDialect::new())?
            }
            // `WebSequenceDiagramsExporter` carries no color scheme and its
            // static-structure/deployment overloads return `null`, so the
            // view loop (Custom + Dynamic only) lives in `wsd::export_all`
            // rather than on the engine.
            Self::WebSequenceDiagrams => crate::wsd::export_all(workspace)?,
            Self::Json => {
                let export = WorkspaceExport::new(oxidizr_json::to_vec(workspace)?, "json".into());
                return workspace_export_files(&export, false, workspace_file_name);
            }
            Self::Theme => {
                let export =
                    WorkspaceExport::new(oxidizr_json::theme_to_vec(workspace), "json".into());
                return workspace_export_files(&export, true, workspace_file_name);
            }
            other => {
                return Err(ExportError::NotImplemented {
                    format: other.label(),
                });
            }
        };
        Ok(diagram_files(&diagram_prefix(workspace.id), &diagrams))
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

    /// The exporter's `ColorScheme`: the suffixless key builds the no-arg
    /// exporter, which is `this(ColorScheme.Light)` (`StructurizrPlantUMLExporter`).
    pub(crate) const fn color_scheme(self) -> ColorScheme {
        match self {
            Self::Dark => ColorScheme::Dark,
            Self::Default | Self::Light => ColorScheme::Light,
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

    /// A one-person landscape workspace, id 7 (the `prefix(workspaceId)`
    /// key of the reference write loop).
    fn workspace() -> Workspace {
        use oxidizr_core::Id;
        use oxidizr_core::model::{ModelItem, Person};
        use oxidizr_core::views::{ElementView, SystemLandscapeView, ViewBase, ViewContent};
        let mut ws = Workspace {
            id: 7,
            ..Default::default()
        };
        ws.model.people = vec![Person {
            item: ModelItem::new(Id::new("1"), "Alice".to_string()),
            group: None,
            relationships: Vec::new(),
        }];
        let mut view = SystemLandscapeView {
            base: ViewBase::new("SystemLandscape-001", 1),
            content: ViewContent::default(),
            enterprise_boundary_visible: false,
            animations: Vec::new(),
        };
        view.content.elements = vec![ElementView {
            id: Id::new("1"),
            x: 0,
            y: 0,
        }];
        ws.views.system_landscape_views.push(view);
        ws
    }

    #[test]
    fn plantuml_diagram_leg_writes_id_keyed_files() {
        let files = ExportFormat::parse("plantuml")
            .export(&workspace(), "no-dot-needed")
            .expect("diagram leg needs no file stem");
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "structurizr-7-SystemLandscape-001.puml",
                "structurizr-7-SystemLandscape-001-key.puml",
            ]
        );
        let definition = String::from_utf8(files[0].bytes.clone()).expect("utf-8");
        assert!(definition.starts_with("@startuml\n"), "{definition}");
        assert!(definition.ends_with("@enduml"), "{definition}");
    }

    #[test]
    fn plantuml_color_suffixes_reach_the_engine() {
        let dark = ExportFormat::parse("plantuml-dark")
            .export(&workspace(), "ws.dsl")
            .expect("dark");
        let definition = String::from_utf8(dark[0].bytes.clone()).expect("utf-8");
        assert!(
            definition.contains("BackgroundColor: #111111"),
            "{definition}"
        );
        let light = ExportFormat::parse("plantuml-light")
            .export(&workspace(), "ws.dsl")
            .expect("light");
        assert!(
            String::from_utf8(light[0].bytes.clone())
                .expect("utf-8")
                .contains("BackgroundColor: #ffffff;")
        );
    }

    #[test]
    fn other_registry_formats_stay_not_implemented() {
        // `mermaid` landed at M4-b and `websequencediagrams` at P4 §P4-M2
        // N-5 (see the arm tests below + the `export_*` API suites);
        // `static` is the registry's last unlanded entry.
        let err = ExportFormat::parse("static")
            .export(&workspace(), "ws.dsl")
            .expect_err("unlanded format");
        assert!(
            matches!(&err, ExportError::NotImplemented { format }
                if format == &ExportFormat::Static.label()),
            "{err}"
        );
    }

    /// P4 §P4-M2 N-5: `websequencediagrams` dispatches into
    /// `wsd::export_all` — a landscape-only workspace writes NO files (the
    /// reference's static-structure/deployment overloads return `null`) but
    /// the export SUCCEEDS.
    #[test]
    fn websequencediagrams_lands_the_wsd_dialect() {
        let files = ExportFormat::parse("websequencediagrams")
            .export(&workspace(), "ws.dsl")
            .expect("wsd export");
        assert!(files.is_empty(), "{files:?}");
    }

    /// M4-b: `mermaid` exports through `MermaidDialect` — `.mmd` files
    /// with the `graph TB` header (no `automaticLayout` on this view), the
    /// `subgraph diagram` wrapper and the div-label element legs.
    #[test]
    fn mermaid_lands_the_mermaid_dialect() {
        let files = ExportFormat::parse("mermaid")
            .export(&workspace(), "ws.dsl")
            .expect("mermaid export");
        assert_eq!(files.len(), 1, "{files:?}"); // never a legend file
        assert_eq!(files[0].name, "structurizr-7-SystemLandscape-001.mmd");
        let definition = String::from_utf8(files[0].bytes.clone()).expect("utf-8");
        assert!(definition.starts_with("graph TB\n"), "{definition}");
        assert!(
            definition.contains("subgraph diagram [\"System Landscape View\"]"),
            "{definition}"
        );
    }

    /// M3-b: `plantuml/c4plantuml` exports through `C4PlantUmlDialect` —
    /// the C4 header (`set separator none`, the ALWAYS `<style>` root
    /// block, the `<C4/…>` includes) and NO legend file (the Java
    /// `createLegend` returns `null`).
    #[test]
    fn c4plantuml_lands_the_c4_dialect() {
        let files = ExportFormat::parse("plantuml/c4plantuml")
            .export(&workspace(), "ws.dsl")
            .expect("c4 export");
        assert_eq!(files.len(), 1, "{files:?}"); // no `-key` legend file
        assert_eq!(files[0].name, "structurizr-7-SystemLandscape-001.puml");
        let definition = String::from_utf8(files[0].bytes.clone()).expect("utf-8");
        assert!(definition.starts_with("@startuml\n"), "{definition}");
        assert!(definition.contains("set separator none\n"), "{definition}");
        assert!(
            definition.contains("top to bottom direction\n"),
            "{definition}"
        );
        assert!(definition.contains("<style>\n  root {\n"), "{definition}");
        assert!(
            definition.contains("BackgroundColor: #ffffff\n"),
            "{definition}"
        );
        assert!(definition.contains("!include <C4/C4>\n"), "{definition}");
        assert!(
            definition.contains("!include <C4/C4_Context>\n"),
            "{definition}"
        );
        assert!(
            definition.contains("SHOW_LEGEND(true)\nhide stereotypes\n@enduml"),
            "{definition}"
        );
    }

    /// M3-b: the `-dark` suffix renders the DARK exporter defaults.
    #[test]
    fn c4plantuml_dark_suffix_renders_dark_defaults() {
        let files = ExportFormat::parse("plantuml/c4plantuml-dark")
            .export(&workspace(), "ws.dsl")
            .expect("c4 dark");
        let definition = String::from_utf8(files[0].bytes.clone()).expect("utf-8");
        assert!(
            definition.contains("BackgroundColor: #111111\n"),
            "{definition}"
        );
        assert!(definition.contains("FontColor: #cccccc\n"), "{definition}");
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
