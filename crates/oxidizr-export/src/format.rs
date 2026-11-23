//! The `export` format registry and dispatch. `--format` is parsed
//! lowercased against a fixed registry (`json`, `theme`,
//! `plantuml`/`plantuml[-light|-dark]`,
//! `plantuml/structurizr[±light|-dark]`, `plantuml/c4plantuml[±light|-dark]`,
//! `mermaid`, `websequencediagrams`), with `static` handled before the map
//! and `png`/`svg` before that (the official build rejects them: no browser
//! toolchain); anything else logs `Unknown export format: …` and the
//! command STILL EXITS 0 — modeled as [`ExportFormat::Unknown`].

use oxidizr_core::Workspace;
use oxidizr_core::views::ColorScheme;

use crate::diagram::WorkspaceExport;
use crate::error::ExportError;
use crate::naming::{ExportedFile, diagram_files, diagram_prefix, workspace_export_files};
use crate::plantuml::{C4PlantUmlDialect, Engine, StructurizrDialect};

/// `-light` / `-dark` format suffix (selects a `ColorScheme`; unmarked
/// formats keep the workspace theme).
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
    /// `json` — the standard workspace JSON.
    Json,
    /// `theme` — the workspace theme as JSON.
    Theme,
    /// `plantuml`, `plantuml-light|dark`, `plantuml/structurizr[±light|dark]`.
    StructurizrPlantUml(ColorMode),
    /// `plantuml/c4plantuml[±light|dark]`.
    C4PlantUml(ColorMode),
    /// `mermaid`.
    Mermaid,
    /// `websequencediagrams` — custom and dynamic views only, `.wsd` files.
    WebSequenceDiagrams,
    /// `static` — the official site generator; not an oxidizr goal.
    Static,
    /// `png` — the official build rejects this too (no browser toolchain).
    Png,
    /// `svg` — as [`ExportFormat::Png`].
    Svg,
    /// Not in the registry. The official tooling also tries the string as
    /// a custom exporter class name; when nothing resolves it logs and
    /// exits 0, so the pipeline notes and succeeds.
    Unknown(String),
}

impl ExportFormat {
    /// Parse `--format` lowercased against the registry keys. Only the
    /// `plantuml` family carries a `-light` / `-dark` suffix — `theme-light`
    /// etc. fall through to the unknown path.
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

    /// Whether the official CLI knows this format (unknown strings are the
    /// exit-0 "Unknown export format" note, not an error).
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }

    /// Whether this format exports through [`crate::diagram::DiagramExporter`]
    /// — the branch that loads themes BEFORE the export. `static` is handled
    /// before the registry map and `png`/`svg` are rejected outright, so
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

    /// The dispatch. `json` and `theme` are the [`crate::diagram::WorkspaceExporter`]
    /// pair; `plantuml`/`plantuml/structurizr`, `plantuml/c4plantuml`,
    /// `mermaid` and `websequencediagrams` are the
    /// [`crate::diagram::DiagramExporter`] formats — the
    /// engine plus a dialect, one file per diagram, a `-key` legend file and
    /// numbered animation-frame files, all named from the workspace id. The
    /// remaining registry formats report [`ExportError::NotImplemented`];
    /// [`ExportFormat::Unknown`] is the CALLER's exit-0 note path (the
    /// official tooling never errors here).
    ///
    /// # Errors
    ///
    /// [`ExportError::NotImplemented`] for known-but-unimplemented formats,
    /// [`ExportError::NoExtension`] when the workspace file name has no `.`
    /// (the [`crate::diagram::WorkspaceExporter`] branch only),
    /// [`ExportError::Json`] when
    /// serialization fails, [`ExportError::ReferenceAbort`] when the
    /// diagram engine hits a case where the official tooling aborts.
    pub fn export(
        &self,
        workspace: &Workspace,
        workspace_file_name: &str,
    ) -> Result<Vec<ExportedFile>, ExportError> {
        let diagrams = match self {
            // The diagram branch: the workspace file name plays no role
            // (names key on the workspace ID via `diagram_prefix`).
            Self::StructurizrPlantUml(mode) => {
                let mut engine = Engine::new(workspace, mode.color_scheme());
                engine.export_all(&mut StructurizrDialect::new())?
            }
            Self::C4PlantUml(mode) => {
                let mut engine = Engine::new(workspace, mode.color_scheme());
                engine.export_all(&mut C4PlantUmlDialect::new())?
            }
            // Mermaid has no color-scheme option: the export always uses
            // the light scheme. The `.mmd` extension rides each `Diagram`
            // (`diagram_files` threads `file_extension`).
            Self::Mermaid => {
                let mut engine = Engine::new(workspace, ColorScheme::Light);
                engine.export_all(&mut crate::mermaid::MermaidDialect::new())?
            }
            // WebSequenceDiagrams carries no color scheme and supports
            // neither static-structure nor deployment views, so the view
            // loop (Custom + Dynamic only) lives in `wsd::export_all`
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

/// Peel a `-light`/`-dark` suffix off a `plantuml`-family format string
/// (only the `plantuml*` registry keys carry one).
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

    /// The `ColorScheme` this suffix selects: suffixless means the default,
    /// which is light.
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
        // The official tooling also tries these as custom exporter class
        // names; with no match it logs "Unknown export format" and exits 0.
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

    /// A one-person landscape workspace, id 7 (diagram file names key on
    /// this id).
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
        // `static` is the only registry format still reported as not
        // implemented (see the arm tests below + the `export_*` API suites).
        let err = ExportFormat::parse("static")
            .export(&workspace(), "ws.dsl")
            .expect_err("unlanded format");
        assert!(
            matches!(&err, ExportError::NotImplemented { format }
                if format == &ExportFormat::Static.label()),
            "{err}"
        );
    }

    /// `websequencediagrams` dispatches into `wsd::export_all` — a
    /// landscape-only workspace writes NO files (static-structure views are
    /// not supported there) but the export SUCCEEDS.
    #[test]
    fn websequencediagrams_lands_the_wsd_dialect() {
        let files = ExportFormat::parse("websequencediagrams")
            .export(&workspace(), "ws.dsl")
            .expect("wsd export");
        assert!(files.is_empty(), "{files:?}");
    }

    /// `mermaid` exports through `MermaidDialect` — `.mmd` files
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

    /// `plantuml/c4plantuml` exports through `C4PlantUmlDialect` —
    /// the C4 header (`set separator none`, the ALWAYS `<style>` root
    /// block, the `<C4/…>` includes) and NO legend file (C4 `PlantUML`
    /// exports carry no legend).
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

    /// The `-dark` suffix renders the DARK exporter defaults.
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
