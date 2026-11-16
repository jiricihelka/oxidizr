//! P1-M5 `workspace extends` behaviour tests (Category A).
//!
//! Mirrors the reference `DslTests` assertions for the extend fixtures
//! (`test_extendWorkspaceFromDsl`, `test_extendWorkspaceFromJsonFile`,
//! `test_extendWorkspaceFromDslFiles`, `test_findElement` source-retention
//! half) plus the M5-b engine guarantees: id-counter continuation through
//! the base, `configureFrom` scope hand-over (hierarchical identifiers keep
//! working in the extending file), the loader-absent guard, and the
//! `could not be found` / `should be a single file` file guards.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use oxidizr_core::Workspace;
use oxidizr_core::model::SoftwareSystem;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

fn dsl_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

/// Production JSON-base wiring (the CLI injects `oxidizr-json`; the dsl
/// crate must not depend on it — P1-D1).
fn json_loader(path: &str) -> Result<Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn file_opts(path: &str) -> ParseOptions<'_> {
    ParseOptions {
        file: Some(path),
        base_dir: None,
        json_loader: Some(json_loader),
    }
}

fn parse_fixture(rel: &str) -> Workspace {
    let path = dsl_dir().join(rel);
    let full = path.to_str().unwrap().to_string();
    let source = std::fs::read_to_string(&path).unwrap();
    let result = parse_dsl(&source, file_opts(&full)).unwrap();
    assert!(result.errors.is_empty(), "{rel}: {:?}", result.errors);
    result.workspace
}

fn system<'a>(ws: &'a Workspace, name: &str) -> &'a SoftwareSystem {
    ws.model
        .software_systems
        .iter()
        .find(|s| s.item.name == name)
        .unwrap_or_else(|| panic!("no software system {name}"))
}

#[test]
fn extend_dsl_file_layers_over_base() {
    // `DslTests.test_extendWorkspaceFromDsl` (file arm).
    let ws = parse_fixture("extend/extend-workspace-from-dsl-file.dsl");
    assert_eq!(ws.model.people.len(), 1);
    assert_eq!(ws.model.software_systems.len(), 3);
    let user = &ws.model.people[0];
    assert_eq!(user.item.name, "User");
    let s1 = system(&ws, "Software System 1");
    assert_eq!(s1.containers.len(), 1);
    assert_eq!(s1.containers[0].item.name, "Web Application");
    // `user -> softwareSystem1 "Uses"` resolved base identifiers.
    assert!(
        ws.model.relationships.iter().any(|r| {
            r.source == user.item.id
                && r.destination == s1.item.id
                && r.description.as_deref() == Some("Uses")
        }),
        "user -> Software System 1 (Uses) missing"
    );
    // `softwareSystem3.webapp -> softwareSystem3.db` — dot-path into the base.
    let s3 = system(&ws, "Software System 3");
    let webapp = s3
        .containers
        .iter()
        .find(|c| c.item.name == "Web Application")
        .unwrap();
    let db = s3
        .containers
        .iter()
        .find(|c| c.item.name == "Database")
        .unwrap();
    assert!(
        ws.model
            .relationships
            .iter()
            .any(|r| r.source == webapp.item.id && r.destination == db.item.id),
        "webapp -> db missing"
    );
    // `configureFrom` moved the base's `!identifiers hierarchical` scope into
    // the extending parse: the NEW container registers with its parent prefix.
    assert_eq!(
        s1.containers[0]
            .item
            .properties
            .get("structurizr.dsl.identifier"),
        Some("softwareSystem1.webapp")
    );
}

#[test]
fn extend_json_file_layers_over_base() {
    // `DslTests.test_extendWorkspaceFromJsonFile` (file arm).
    let ws = parse_fixture("extend/extend-workspace-from-json-file.dsl");
    assert_eq!(ws.model.people.len(), 1);
    assert_eq!(ws.model.software_systems.len(), 3);
    let user = &ws.model.people[0];
    let s1 = system(&ws, "Software System 1");
    assert!(
        ws.model.relationships.iter().any(|r| {
            r.source == user.item.id
                && r.destination == s1.item.id
                && r.description.as_deref() == Some("Uses")
        }),
        "user -> Software System 1 (Uses) missing"
    );
    let mut names: Vec<&str> = s1.containers.iter().map(|c| c.item.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Web Application 1", "Web Application 2"]);
}

#[test]
fn extend_chain_accumulates_three_levels() {
    // `DslTests.test_extendWorkspaceFromDslFiles`: 4 ← 3 ← 2 ← 1.
    let ws = parse_fixture("extend/4.dsl");
    assert_eq!(ws.model.people.len(), 3);
    assert_eq!(ws.views.system_landscape_views.len(), 1);
}

#[test]
fn extend_child_keeps_no_source_and_continues_ids() {
    // `test_findElement` retention half: `DslUtils.getDsl` == "" — the
    // base's retention is cleared and the extending file is not retained
    // (`setDslPortable(false)`); ids continue after the base's highest id.
    let ws = parse_fixture("source-child.dsl");
    assert_eq!(ws.properties.get("structurizr.dsl"), None);
    assert_eq!(ws.model.software_systems.len(), 2);
    assert_eq!(system(&ws, "A").item.id.as_str(), "1");
    assert_eq!(system(&ws, "B").item.id.as_str(), "2");
}

#[test]
fn extends_without_loader_rejects_json_base() {
    let path = dsl_dir().join("extend/extend-workspace-from-json-file.dsl");
    let full = path.to_str().unwrap().to_string();
    let source = std::fs::read_to_string(&path).unwrap();
    let result = parse_dsl(&source, ParseOptions::file(&full)).unwrap();
    // D1 recovery continues past the failed extends, so the body errors too
    // — the FIRST error is the reference-shaped loader-absent guard.
    assert!(
        result.errors[0]
            .message
            .contains("requires a JSON workspace loader"),
        "{:?}",
        result.errors[0]
    );
}

#[test]
fn extends_file_guards_missing_and_directory() {
    let dir = std::env::temp_dir().join(format!("oxidizr-m5-guards-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let cases = [
        ("nope.dsl", "could not be found"),
        ("sub", "should be a single file"),
    ];
    for (source, expect) in cases {
        let child = dir.join(format!(
            "child-{}.dsl",
            if source == "sub" { "dir" } else { "missing" }
        ));
        std::fs::write(&child, format!("workspace extends {source} {{\n}}\n")).unwrap();
        let full = child.to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&child).unwrap();
        let result = parse_dsl(&text, file_opts(&full)).unwrap();
        // D1 recovery continues, so the closing `}` may add its own error;
        // the FIRST error is the reference guard (reference: abort).
        assert!(!result.errors.is_empty(), "{source}: expected an error");
        assert!(
            result.errors[0].message.ends_with(expect),
            "{source}: {:?}",
            result.errors[0].message
        );
    }
}

#[test]
fn extends_reuses_base_element_without_identifier() {
    // The `isExtendingWorkspace()` get-or-add branch: a hand-made JSON base
    // (no `structurizr.dsl.identifier` properties — nothing registered) plus
    // a re-declaring statement: the base element is reused, NOT duplicated
    // (the plain add would fail "A person with the name … already exists")
    // and the statement's fields layer over it.
    let dir = std::env::temp_dir().join(format!("oxidizr-m5-reuse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("base.json"),
        r#"{"name":"Base","model":{"people":[{"id":"7","name":"User","description":"orig"}]}}"#,
    )
    .unwrap();
    let child = dir.join("child.dsl");
    std::fs::write(
        &child,
        "workspace extends base.json {\n\n    model {\n        person \"User\" \"Layered\"\n        person \"New Guy\"\n    }\n\n}\n",
    )
    .unwrap();
    let full = child.to_str().unwrap().to_string();
    let text = std::fs::read_to_string(&child).unwrap();
    let result = parse_dsl(&text, file_opts(&full)).unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let people = &result.workspace.model.people;
    assert_eq!(people.len(), 2);
    assert_eq!(people[0].item.id.as_str(), "7"); // reused, same id
    assert_eq!(people[0].item.description.as_deref(), Some("Layered"));
    // Id counter continued after the base's highest id (found() replay).
    assert_eq!(people[1].item.id.as_str(), "8");
}
