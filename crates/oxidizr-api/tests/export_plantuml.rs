//! `export` `PlantUML` dispatch integration tests (Phase 2 M2-d/M3-b): the
//! reference `ExportCommand` write loop end-to-end — id-keyed per-view
//! files, `-key` legend files, `Writing <path>` notes — plus the
//! `DiagramExporter` branch's viewless `log.error` leg (exit 0, no files)
//! and the C4 dialect (M3-b: diagram files, no legends, custom views
//! skipped).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, ExitClass, LogItem, Request, execute};

/// Hermetic options: empty env map, no stdin, no network (the theme legs
/// stay resolvable offline — P2-D3 keeps unknown theme names working).
fn opts() -> ApiOptions {
    ApiOptions {
        strict: false,
        keep_unknown: false,
        env: Some(BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
    }
}

fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m2d-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A deterministic landscape workspace (id 0 → the bare `structurizr`
/// prefix); key `sys-landscape`.
fn landscape_dsl() -> &'static str {
    "workspace \"Bank\" {\n  \
     model {\n    p = person \"Alice\"\n    s = softwareSystem \"Big Bank\"\n    p -> s \"Uses\"\n  \
     }\n  views {\n    systemLandscape \"sys-landscape\" {\n      include *\n      autoLayout\n    \
     }\n  }\n}\n"
}

fn write_ws(dir: &Path, content: &str) -> PathBuf {
    let ws = dir.join("workspace.dsl");
    std::fs::write(&ws, content).unwrap();
    ws
}

fn run(dir: &Path, ws: &Path, format: &str) -> oxidizr_api::pipeline::Outcome {
    let _ = dir;
    execute(
        Request::Export {
            workspace: ws.to_path_buf(),
            format: format.into(),
            output: None,
        },
        &opts(),
    )
}

fn note_lines(out: &oxidizr_api::pipeline::Outcome) -> Vec<String> {
    out.log
        .iter()
        .filter_map(|l| match l {
            LogItem::NoteLine(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

fn files_with_ext(dir: &Path, ext: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(&format!(".{ext}")))
        .collect();
    names.sort();
    names
}

#[test]
fn plantuml_writes_view_and_legend_files() {
    let dir = temp_dir("dispatch");
    let ws = write_ws(&dir, landscape_dsl());
    let out = run(&dir, &ws, "plantuml");
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert_eq!(
        files_with_ext(&dir, "puml"),
        [
            "structurizr-sys-landscape-key.puml",
            "structurizr-sys-landscape.puml",
        ]
    );
    // The reference write loop logs `Writing <canonical path>` per file,
    // diagram first, legend second.
    let notes = note_lines(&out);
    assert_eq!(notes.len(), 2, "{notes:?}");
    assert!(
        notes[0].starts_with("Writing ") && notes[0].ends_with("structurizr-sys-landscape.puml")
    );
    assert!(
        notes[1].starts_with("Writing ")
            && notes[1].ends_with("structurizr-sys-landscape-key.puml")
    );
    let body = std::fs::read_to_string(dir.join("structurizr-sys-landscape.puml")).unwrap();
    assert!(body.starts_with("@startuml\n"), "{body}");
    assert!(body.ends_with("@enduml"), "{body}");
    assert!(
        body.contains(
            "rectangle \"==Alice\\n<size:16>[Person]</size>\" <<Element-RWxlbWVudA==>> as Alice"
        ),
        "{body}"
    );
    assert!(
        body.contains("Alice --> BigBank <<Relationship-UmVsYXRpb25zaGlw>> : \"Uses\""),
        "{body}"
    );
    let legend = std::fs::read_to_string(dir.join("structurizr-sys-landscape-key.puml")).unwrap();
    assert!(
        legend.contains("set separator none\nhide stereotype\n"),
        "{legend}"
    );
}

#[test]
fn dark_suffix_reaches_the_engine() {
    let dir = temp_dir("dark");
    let ws = write_ws(&dir, landscape_dsl());
    let out = run(&dir, &ws, "plantuml-dark");
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    let body = std::fs::read_to_string(dir.join("structurizr-sys-landscape.puml")).unwrap();
    assert!(body.contains("BackgroundColor: #111111;"), "{body}");
}

#[test]
fn viewless_diagram_export_notes_and_exits_zero() {
    // Empty model AND empty views: V-10 creates no defaults, the reference
    // `ExportCommand` logs its error line and still returns true.
    let dir = temp_dir("noviews");
    let ws = write_ws(&dir, "workspace {\n  model {\n  }\n  views {\n  }\n}\n");
    let out = run(&dir, &ws, "plantuml");
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert!(
        note_lines(&out)
            .iter()
            .any(|n| n == "The workspace contains no views"),
        "{:?}",
        out.log
    );
    assert_eq!(files_with_ext(&dir, "puml"), Vec::<String>::new());
}

#[test]
fn c4plantuml_writes_view_files_without_legends() {
    // M3-b: `plantuml/c4plantuml` exports through the C4 dialect — and
    // the Java `createLegend` returns `null`, so there are NO `-key`
    // legend files (unlike the Structurizr dialect's M2-d pair).
    let dir = temp_dir("c4-export");
    let ws = write_ws(&dir, landscape_dsl());
    let out = run(&dir, &ws, "plantuml/c4plantuml");
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert_eq!(
        files_with_ext(&dir, "puml"),
        ["structurizr-sys-landscape.puml"]
    );
    let notes = note_lines(&out);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].ends_with("structurizr-sys-landscape.puml"));
    let body = std::fs::read_to_string(dir.join("structurizr-sys-landscape.puml")).unwrap();
    assert!(body.starts_with("@startuml\n"), "{body}");
    assert!(body.contains("set separator none\n"), "{body}");
    assert!(body.contains("!include <C4/C4>\n"), "{body}");
    assert!(
        body.contains("Person(Alice, \"Alice\", $descr=\"\", $tags=\"\", $link=\"\")\n"),
        "{body}"
    );
    assert!(
        body.ends_with("SHOW_LEGEND(true)\nhide stereotypes\n@enduml"),
        "{body}"
    );
}

#[test]
fn c4plantuml_skips_custom_views() {
    // The Java `C4PlantUMLExporter.export(CustomView)` overload returns
    // `null` — the view writes NO file (witness: the `custom-view-animation`
    // C4 sweep directory is empty). The landscape view still exports, so
    // the reference's "no views" leg stays silent.
    let dir = temp_dir("c4-custom");
    let dsl = "workspace {\n  model {\n    p = person \"Alice\"\n  \
               }\n  views {\n    systemLandscape \"sys-landscape\" {\n      include *\n    \
               }\n    custom \"cv\" \"Title\" \"Description\" {\n    }\n  }\n}\n";
    let ws = write_ws(&dir, dsl);
    let out = run(&dir, &ws, "plantuml/c4plantuml");
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert_eq!(
        files_with_ext(&dir, "puml"),
        ["structurizr-sys-landscape.puml"]
    );
}
