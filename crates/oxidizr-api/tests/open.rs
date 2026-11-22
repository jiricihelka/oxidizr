//! `open` entry integration tests (P3-M1-1, `agents/plan/build/PHASE_3.md`
//! §P3-M1): the viewer-facing load entry mirrors the command load tail
//! (extension dispatch, default views) with the viewer twists (partial DSL,
//! informational validate tail). The `execute` bodies are untouched — these
//! tests never drive a byte contract.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use oxidizr_api::open;
use oxidizr_api::pipeline::{ApiOptions, ExitClass, Input, LogItem, ParseFlags};

/// Hermetic options: empty env map (no process-env leak), no stdin, no
/// network (same posture as the `export` tests).
fn opts() -> ApiOptions {
    ApiOptions {
        parse: ParseFlags {
            strict: false,
            keep_unknown: false,
        },
        env: Some(BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

fn temp_file(topic: &str, name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m11-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    path
}

/// Model-only DSL: the default-views tail must fire on open.
const DSL: &str = r#"workspace "Bank Demo" "M1-1 fixture" {
    model {
        alice = person "Alice" "A user" "Live"
        bank = softwaresystem "Bank" "Core banking" "Live"
        alice -> bank "Uses" "HTTPS"
    }
}"#;

#[test]
fn dsl_opens_with_default_views() {
    let path = temp_file("dsl", "demo.dsl", DSL);
    let opened = open(&Input::File(path), &opts()).unwrap();
    assert_eq!(opened.workspace.name.as_deref(), Some("Bank Demo"));
    // Clean workspace + generated views: nothing to report.
    assert_eq!(opened.log.len(), 0);
    // `apply_default_views` tail: a non-empty model with no views generates.
    assert!(!opened.workspace.views.all_views().is_empty());
    // Display name is the (canonicalized) path, like the command loaders.
    assert!(opened.display.contains("demo.dsl"));
}

#[test]
fn json_round_trip_opens() {
    let dsl_path = temp_file("jsonrt", "demo.dsl", DSL);
    let first = open(&Input::File(dsl_path), &opts()).unwrap();
    let json = oxidizr_json::to_vec(&first.workspace).unwrap();
    let json_path = temp_file("jsonrt", "demo.json", &String::from_utf8(json).unwrap());
    let again = open(&Input::File(json_path), &opts()).unwrap();
    assert_eq!(again.workspace.name.as_deref(), Some("Bank Demo"));
    assert_eq!(
        again.workspace.views.all_views().len(),
        first.workspace.views.all_views().len()
    );
    assert_eq!(again.log.len(), 0);
}

#[test]
fn dsl_statement_errors_open_partially() {
    // Drop the closing brace: a recoverable statement error (D1), the same
    // family the e2e mutation corpus drives through `dsl2json`.
    let broken = DSL[..DSL.rfind('}').unwrap()].to_string();
    let path = temp_file("partial", "broken.dsl", &broken);
    let opened = open(&Input::File(path), &opts()).unwrap();
    // The repaired workspace still carries the parsed model …
    assert_eq!(opened.workspace.model.people.len(), 1);
    assert_eq!(opened.workspace.model.software_systems.len(), 1);
    // … and every statement error rides the log.
    assert!(!opened.log.is_empty());
    assert!(
        opened
            .log
            .iter()
            .all(|item| matches!(item, LogItem::Issue(_)))
    );
}

#[test]
fn json_parse_failure_is_diagnostics() {
    let path = temp_file("badjson", "demo.json", "{ not json ]");
    let err = open(&Input::File(path), &opts()).unwrap_err();
    assert_eq!(err.exit, ExitClass::Diagnostics);
    assert_eq!(err.log.len(), 1);
}

#[test]
fn unknown_extension_is_rejected_like_validate() {
    let path = temp_file("ext", "demo.txt", DSL);
    let err = open(&Input::File(path), &opts()).unwrap_err();
    assert_eq!(err.exit, ExitClass::Diagnostics);
    let LogItem::Issue(issue) = &err.log[0] else {
        panic!("expected an issue");
    };
    assert!(
        issue
            .message
            .contains("cannot determine the workspace format")
    );
}

#[test]
fn missing_file_is_io() {
    let path = std::env::temp_dir().join(format!("oxidizr-m11-missing-{}.dsl", std::process::id()));
    let err = open(&Input::File(path), &opts()).unwrap_err();
    assert_eq!(err.exit, ExitClass::Io);
}

#[test]
fn extensionless_input_is_dsl() {
    // Mirrors the `dsl2json` stdin semantics (module docs).
    let opened = open(&Input::Bytes(DSL.as_bytes().to_vec()), &opts()).unwrap();
    assert_eq!(opened.display, "-");
    assert_eq!(opened.workspace.model.software_systems.len(), 1);
}

#[test]
fn dsl_open_exposes_the_include_closure() {
    // §P3-M4: the viewer's live-reload watcher watches display + this
    // set; URL includes are policy-dead, so it is file-system-only.
    let dir = std::env::temp_dir().join(format!("oxidizr-m4-open-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.dsl"),
        "workspace \"I\" {\n    model {\n        !include people.dsl\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join("people.dsl"), "alice = person \"Alice\"\n").unwrap();
    let opened = open(&Input::File(dir.join("main.dsl")), &opts()).unwrap();
    assert_eq!(opened.includes.len(), 1);
    assert!(opened.includes[0].ends_with("people.dsl"));

    // JSON and stdin have no include tree.
    let json = temp_file(
        "inc-json",
        "ws.json",
        &String::from_utf8(oxidizr_json::to_vec(&opened.workspace).unwrap()).unwrap(),
    );
    let none: Vec<String> = Vec::new();
    assert_eq!(open(&Input::File(json), &opts()).unwrap().includes, none);
    let stdin = open(&Input::Bytes(DSL.as_bytes().to_vec()), &opts()).unwrap();
    assert_eq!(stdin.includes, none);
}
