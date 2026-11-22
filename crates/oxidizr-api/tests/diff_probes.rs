//! §P4-M0 D-4 probe tests (Category A, `agents/plan/build/PHASE_4.md`):
//! the hand-curated `testdata/probes/diff` pairs pinned as snapshots, the
//! cross-format identity invariant (a `.dsl` and its JSON twin MUST diff
//! EMPTY — loaders differ in REPRESENTATION (derived vs re-stored tag
//! defaults, `""` vs absent text), never in the model), and the `diff(a, a)`
//! reflexivity sweep over the whole vendored corpus: two independent loads
//! of the SAME file must be equal (the tripwire for every volatile-value
//! trap — provenance stamps, `HashMap` orders, anything nondeterministic).
//!
//! These tests drive `oxidizr_api::execute` (the pipeline wiring) with the
//! hermetic options; the fixture files are real paths, so nothing touches
//! stdin or the process env.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use expect_test::expect;
use oxidizr_api::pipeline::{ApiOptions, DiffFormat, ExitClass, Input, Request, execute};
use oxidizr_api::{diff, open};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .canonicalize()
        .unwrap()
}

fn probe(name: &str) -> PathBuf {
    testdata().join("probes/diff").join(name)
}

/// Hermetic options (same posture as the `open` tests): empty env map, no
/// stdin, no network, PLAIN report.
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

/// Every file with the given extension under `dir`, recursively, sorted.
fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == ext) {
                out.push(path);
            }
        }
    }
}

/// One `diff` run through the pipeline; `(exit class, stdout text)`.
fn diff_run(a: &Path, b: &Path, format: DiffFormat, ignore_layout: bool) -> (ExitClass, String) {
    let out = execute(
        Request::Diff {
            a: a.to_path_buf(),
            b: b.to_path_buf(),
            format,
            ignore_layout,
            quiet: false,
        },
        &opts(),
    );
    let text = String::from_utf8(out.bytes.unwrap_or_default()).unwrap();
    (out.exit, text)
}

fn is_empty_diff(a: &Path, b: &Path) -> bool {
    let a_ws = open(&Input::File(a.to_path_buf()), &opts())
        .unwrap()
        .workspace;
    let b_ws = open(&Input::File(b.to_path_buf()), &opts())
        .unwrap()
        .workspace;
    diff::diff(&a_ws, &b_ws, &diff::DiffOptions::default()).is_empty()
}

#[test]
fn self_diff_is_empty_over_the_dsl_corpus() {
    let mut files = Vec::new();
    walk(&testdata().join("dsl"), "dsl", &mut files);
    files.sort();
    assert!(files.len() > 100, "corpus moved? got {}", files.len());
    let mut opened = 0;
    for f in &files {
        // Two INDEPENDENT loads of the same file (the determinism tripwire);
        // fixtures the open entry cannot load (component-context children,
        // network/JVM directives — the `m4_fixtures` KNOWN_ERRORS family)
        // are skipped: the registry that decides them is tested there.
        let (left, right) = (
            open(&Input::File(f.clone()), &opts()),
            open(&Input::File(f.clone()), &opts()),
        );
        let (Ok(a), Ok(b)) = (left, right) else {
            continue;
        };
        opened += 1;
        let d = diff::diff(&a.workspace, &b.workspace, &diff::DiffOptions::default());
        assert!(
            d.is_empty(),
            "self-diff of {} is not empty: {d:?}",
            f.display()
        );
    }
    // A loader regression must not silently empty the sweep.
    assert!(
        opened >= 90,
        "only {opened}/{} fixtures opened — coverage collapse",
        files.len()
    );
}

#[test]
fn self_diff_is_empty_over_the_json_corpus() {
    // LEGAL workspaces only: the `testdata/validation/*` fixtures are
    // deliberately INVALID (duplicate names, dangling view refs — above
    // the engine's documented `Unresolved://` fallback they legitimately
    // self-pair non-empty); the validator's register for them is tested
    // where it lives, not here.
    let mut files = Vec::new();
    walk(&testdata().join("read-corpus"), "json", &mut files);
    walk(&testdata().join("probes/diff"), "json", &mut files);
    files.sort();
    files.dedup();
    assert!(files.len() > 5, "corpus moved? got {}", files.len());
    for f in &files {
        // Two INDEPENDENT loads of the same file (the determinism
        // tripwire) — every file here MUST open.
        let (left, right) = (
            open(&Input::File(f.clone()), &opts()),
            open(&Input::File(f.clone()), &opts()),
        );
        let (a, b) = (
            left.expect("legal workspace"),
            right.expect("legal workspace"),
        );
        let d = diff::diff(&a.workspace, &b.workspace, &diff::DiffOptions::default());
        assert!(
            d.is_empty(),
            "self-diff of {} is not empty: {d:?}",
            f.display()
        );
    }
}

#[test]
fn a_dsl_and_its_json_twin_diff_empty() {
    // The D-4 cross-format witnesses (the pair that caught the
    // effective-tags / empty-text gaps: the DSL path keeps kind defaults
    // DERIVED and `""` descriptions, the JSON path re-stores the CSV and
    // omits empty text — the EFFECTIVE views make the pair equal).
    for (d, j) in [("base.dsl", "base.json"), ("edited.dsl", "edited.json")] {
        assert!(
            is_empty_diff(&probe(d), &probe(j)),
            "{d} vs {j} must diff EMPTY"
        );
        let (exit, text) = diff_run(&probe(d), &probe(j), DiffFormat::Json, false);
        assert_eq!(exit, ExitClass::Ok, "{d} vs {j}: {text}");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // the inline schema-1 snapshot IS the content
fn edited_pair_json_snapshot() {
    // The frozen schema-1 document over the hand-curated edit story: add,
    // remove, the rename pair via the relationship DESCRIPTION, field and
    // style changes, the membership shift and the F1 id drift.
    let (exit, text) = diff_run(
        &probe("base.dsl"),
        &probe("edited.dsl"),
        DiffFormat::Json,
        false,
    );
    assert_eq!(exit, ExitClass::Differences);
    expect![[r##"
        {
          "schema": 1,
          "added": [
            {
              "path": "model.people[Mallory]",
              "kind": "person"
            },
            {
              "path": "model.relationships[Person://Alice -> SoftwareSystem://Bank (Uses over HTTPS)]",
              "kind": "relationship"
            },
            {
              "path": "views.styles.elements[Software System]",
              "kind": "elementStyle"
            }
          ],
          "removed": [
            {
              "path": "model.people[Bob]",
              "kind": "person"
            },
            {
              "path": "model.relationships[Person://Alice -> SoftwareSystem://Bank (Uses)]",
              "kind": "relationship"
            }
          ],
          "changed": [
            {
              "path": "model.people[Alice]",
              "field": "description",
              "before": "Primary user",
              "after": "Primary user, promoted"
            },
            {
              "path": "model.softwareSystems[Mail]",
              "field": "description",
              "before": null,
              "after": "SMTP relay"
            },
            {
              "path": "model.softwareSystems[Mail]",
              "field": "tags",
              "before": [
                "Element",
                "Software System"
              ],
              "after": [
                "Element",
                "Software System",
                "Mailgun"
              ]
            },
            {
              "path": "views.styles.elements[Person]",
              "field": "background",
              "before": "#112233",
              "after": "#445566"
            },
            {
              "path": "views[landscape]",
              "field": "automaticLayout",
              "before": {
                "rankDirection": "LeftRight",
                "rankSeparation": 100,
                "nodeSeparation": 50,
                "edgeSeparation": 50,
                "vertices": true
              },
              "after": null
            },
            {
              "path": "views[landscape]",
              "field": "elements",
              "before": [
                "Person://Alice",
                "Person://Bob",
                "SoftwareSystem://Bank",
                "SoftwareSystem://Mail"
              ],
              "after": [
                "Person://Alice",
                "Person://Mallory",
                "SoftwareSystem://Bank",
                "SoftwareSystem://Mail"
              ]
            },
            {
              "path": "views[landscape]",
              "field": "position[Bob]",
              "before": {
                "x": 0,
                "y": 0
              },
              "after": null
            },
            {
              "path": "views[landscape]",
              "field": "position[Mallory]",
              "before": null,
              "after": {
                "x": 0,
                "y": 0
              }
            },
            {
              "path": "views[landscape]",
              "field": "relationships",
              "before": [
                "Relationship://Person://Alice -> SoftwareSystem://Bank (Uses)",
                "Relationship://SoftwareSystem://Bank -> SoftwareSystem://Mail (Sends via)"
              ],
              "after": [
                "Relationship://Person://Alice -> SoftwareSystem://Bank (Uses over HTTPS)",
                "Relationship://SoftwareSystem://Bank -> SoftwareSystem://Mail (Sends via)"
              ]
            },
            {
              "path": "workspace",
              "field": "description",
              "before": "P4-M0 fixture pair — base side",
              "after": "P4-M0 fixture pair — edited side"
            }
          ],
          "idChanged": [
            {
              "path": "model.people[Alice]",
              "before": "1",
              "after": "2"
            },
            {
              "path": "model.softwareSystems[Bank]",
              "before": "3",
              "after": "4"
            },
            {
              "path": "model.softwareSystems[Bank].containers[DB]",
              "before": "5",
              "after": "6"
            },
            {
              "path": "model.softwareSystems[Bank].containers[Web]",
              "before": "4",
              "after": "5"
            },
            {
              "path": "model.softwareSystems[Mail]",
              "before": "6",
              "after": "3"
            }
          ]
        }
    "##]].assert_eq(&text);
}

#[test]
fn edited_pair_ignore_layout_snapshot() {
    // `--ignore-layout` drops ONLY the geometry fields (the position legs
    // and `automaticLayout`); membership stays — it is not layout.
    let (exit, text) = diff_run(
        &probe("base.dsl"),
        &probe("edited.dsl"),
        DiffFormat::Json,
        true,
    );
    assert_eq!(exit, ExitClass::Differences);
    assert!(!text.contains("position["), "{text}");
    assert!(!text.contains("automaticLayout"), "{text}");
    assert!(text.contains("\"elements\""), "{text}");
}

#[test]
fn edited_pair_human_snapshot() {
    // PLAIN (hermetic `ApiOptions::color = false`), one line per entry,
    // grouped by category in schema order.
    let (exit, text) = diff_run(
        &probe("base.dsl"),
        &probe("edited.dsl"),
        DiffFormat::Human,
        false,
    );
    assert_eq!(exit, ExitClass::Differences);
    expect![[r##"
        added (3):
        + model.people[Mallory] (person)
        + model.relationships[Person://Alice -> SoftwareSystem://Bank (Uses over HTTPS)] (relationship)
        + views.styles.elements[Software System] (elementStyle)
        removed (2):
        - model.people[Bob] (person)
        - model.relationships[Person://Alice -> SoftwareSystem://Bank (Uses)] (relationship)
        changed (10):
        ~ model.people[Alice] description: "Primary user" -> "Primary user, promoted"
        ~ model.softwareSystems[Mail] description: null -> "SMTP relay"
        ~ model.softwareSystems[Mail] tags: ["Element","Software System"] -> ["Element","Software System","Mailgun"]
        ~ views.styles.elements[Person] background: "#112233" -> "#445566"
        ~ views[landscape] automaticLayout: {"rankDirection":"LeftRight","rankSeparation":100,"nodeSeparation":50,"edgeSeparation":50,"vertices":true} -> null
        ~ views[landscape] elements: ["Person://Alice","Person://Bob","SoftwareSystem://Bank","SoftwareSystem://Mail"] -> ["Person://Alice","Person://Mallory","SoftwareSystem://Bank","SoftwareSystem://Mail"]
        ~ views[landscape] position[Bob]: {"x":0,"y":0} -> null
        ~ views[landscape] position[Mallory]: null -> {"x":0,"y":0}
        ~ views[landscape] relationships: ["Relationship://Person://Alice -> SoftwareSystem://Bank (Uses)","Relationship://SoftwareSystem://Bank -> SoftwareSystem://Mail (Sends via)"] -> ["Relationship://Person://Alice -> SoftwareSystem://Bank (Uses over HTTPS)","Relationship://SoftwareSystem://Bank -> SoftwareSystem://Mail (Sends via)"]
        ~ workspace description: "P4-M0 fixture pair — base side" -> "P4-M0 fixture pair — edited side"
        idChanged (5):
        # model.people[Alice] id: 1 -> 2
        # model.softwareSystems[Bank] id: 3 -> 4
        # model.softwareSystems[Bank].containers[DB] id: 5 -> 6
        # model.softwareSystems[Bank].containers[Web] id: 4 -> 5
        # model.softwareSystems[Mail] id: 6 -> 3
    "##]].assert_eq(&text);
}
