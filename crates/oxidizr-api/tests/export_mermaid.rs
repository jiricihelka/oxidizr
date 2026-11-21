//! `export mermaid` dispatch integration tests (Phase 2 M4-b): the
//! `MermaidDiagramExporter` branch end-to-end — `.mmd` file names, the
//! standalone `sequenceDiagram` form gated on `mermaid.sequenceDiagram`,
//! custom views EXPORTED (unlike the C4 dialect's skip) and the
//! `DiagramExporter` branch's viewless `log.error` leg (exit 0, no files).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, ExitClass, LogItem, Request, execute};

/// Hermetic options: empty env map, no stdin, no network.
fn opts() -> ApiOptions {
    ApiOptions {
        strict: false,
        keep_unknown: false,
        env: Some(BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m4b-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Workspace (id 0 → the bare `structurizr` prefix): person `1` → system
/// `2` in a landscape view keyed `sys-landscape`, no `autoLayout` (so the
/// header direction stays the `graph TB` default).
fn landscape_dsl() -> &'static str {
    "workspace \"Bank\" {\n  \
     model {\n    p = person \"Alice\"\n    s = softwareSystem \"Big Bank\"\n    p -> s \"Uses\"\n  \
     }\n  views {\n    systemLandscape \"sys-landscape\" {\n      include *\n    \
     }\n  }\n}\n"
}

fn write_ws(dir: &Path, content: &str) -> PathBuf {
    let ws = dir.join("workspace.dsl");
    std::fs::write(&ws, content).unwrap();
    ws
}

fn run(dir: &Path, ws: &Path) -> oxidizr_api::pipeline::Outcome {
    let _ = dir;
    execute(
        Request::Export {
            workspace: ws.to_path_buf(),
            format: "mermaid".into(),
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
fn mermaid_writes_mmd_files() {
    let dir = temp_dir("dispatch");
    let ws = write_ws(&dir, landscape_dsl());
    let out = run(&dir, &ws);
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert_eq!(
        files_with_ext(&dir, "mmd"),
        ["structurizr-sys-landscape.mmd"]
    );
    let notes = note_lines(&out);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("Writing ") && notes[0].ends_with("structurizr-sys-landscape.mmd")
    );
    let body = std::fs::read_to_string(dir.join("structurizr-sys-landscape.mmd")).unwrap();
    assert!(body.starts_with("graph TB\n"), "{body}");
    assert!(body.contains("linkStyle default fill:#ffffff\n"), "{body}");
    assert!(
        body.contains(
            "1[\"<div style='font-weight: bold'>Alice</div><div style='font-size: 70%; \
             margin-top: 0px'>[Person]</div>\"]"
        ),
        "{body}"
    );
    assert!(
        body.contains("1-. \"<div>Uses</div><div style='font-size: 70%'></div>\" .->2"),
        "{body}"
    );
}

#[test]
fn sequence_form_gated_on_the_view_property() {
    // Without the property the dynamic view is an ordinary `graph`
    // document; `mermaid.sequenceDiagram` = "true" switches it to the
    // standalone `sequenceDiagram` form (catalog §9).
    let dynamic = |extra: &str| {
        format!(
            "workspace {{\n  model {{\n    a = softwareSystem \"A\"\n    b = softwareSystem \
             \"B\"\n    a -> b \"Sends data to\"\n  }}\n  views {{\n    dynamic * {{\n      \
             a -> b\n{extra}    }}\n  }}\n}}\n"
        )
    };
    let dir = temp_dir("seq-gate");
    let ws = write_ws(&dir, &dynamic("      "));
    let out = run(&dir, &ws);
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    let files = files_with_ext(&dir, "mmd");
    assert_eq!(files.len(), 1, "{files:?}");
    let body = std::fs::read_to_string(dir.join(&files[0])).unwrap();
    assert!(body.starts_with("graph TB\n"), "{body}");

    let ws = write_ws(
        &dir,
        &dynamic("      properties {\n        mermaid.sequenceDiagram \"true\"\n      }\n"),
    );
    let out = run(&dir, &ws);
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    let body = std::fs::read_to_string(dir.join(&files[0])).unwrap();
    assert!(body.starts_with("sequenceDiagram\n"), "{body}");
    assert!(
        body.contains("participant 1 as A<br />[Software System]"),
        "{body}"
    );
    assert!(body.ends_with("1->>2: Sends data to"), "{body}");
}

#[test]
fn custom_views_are_exported() {
    // `MermaidDiagramExporter` has no `export(CustomView)` skip (unlike
    // C4): the witness sweep's `custom-view-animation` produced one file
    // per custom VIEW (frames are never emitted).
    let dir = temp_dir("custom");
    let dsl = "workspace {\n  model {\n    p = person \"Alice\"\n  \
               }\n  views {\n    systemLandscape \"sys-landscape\" {\n      include *\n    \
               }\n    custom \"cv\" \"Title\" \"Description\" {\n    }\n  }\n}\n";
    let ws = write_ws(&dir, dsl);
    let out = run(&dir, &ws);
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert_eq!(
        files_with_ext(&dir, "mmd"),
        ["structurizr-cv.mmd", "structurizr-sys-landscape.mmd"]
    );
}

#[test]
fn viewless_diagram_export_notes_and_exits_zero() {
    // Empty model AND empty views: the reference `ExportCommand` logs its
    // error line and still returns true.
    let dir = temp_dir("noviews");
    let ws = write_ws(&dir, "workspace {\n  model {\n  }\n  views {\n  }\n}\n");
    let out = run(&dir, &ws);
    assert_eq!(out.exit, ExitClass::Ok, "{:?}", out.log);
    assert!(
        note_lines(&out)
            .iter()
            .any(|n| n == "The workspace contains no views"),
        "{:?}",
        out.log
    );
    assert_eq!(files_with_ext(&dir, "mmd"), Vec::<String>::new());
}
