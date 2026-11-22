//! Phase 4 §P4-M2 N-5 golden byte-parity for the `websequencediagrams`
//! export (Category A): `testdata/golden/websequencediagrams/<fixture>/*.wsd`
//! are the raw oracle output (`cargo xtask oracle gen --wsd`, structurizr
//! 2026.10.26 @ ad34036, no normalization); these tests run our
//! `websequencediagrams` export over the matching
//! `testdata/dsl/<fixture>.dsl` and byte-compare file sets and contents.
//!
//! The corpus is the nine fixtures whose workspaces carry a `CustomView` or
//! a `DynamicView` — the only two view kinds `WebSequenceDiagramsExporter`
//! does not null out. `script-in-dynamic-view` stays out for the standing
//! D2 reason (the loader rejects the fixture).
//!
//! No render smoke leg: `WebSequenceDiagrams` renders only through the
//! websequencediagrams.com service — nothing to run locally (the P2-D9
//! loud-skip pattern has no candidate binary here).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, ExitClass, ParseFlags, Request, execute};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn golden_base() -> PathBuf {
    root().join("testdata/golden/websequencediagrams")
}

/// Hermetic options as in `export_mermaid_goldens.rs`: empty env, no stdin,
/// no network.
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

fn temp_dir(topic: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("oxidizr-p4m2e-gold-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `{ name => bytes }` of the `*.wsd` files in `dir` (missing dir = empty).
fn file_map(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "wsd") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                out.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn fixture_dsl(fixture: &str) -> PathBuf {
    let dsl = root().join("testdata/dsl").join(format!("{fixture}.dsl"));
    if dsl.is_file() {
        return dsl;
    }
    root()
        .join("testdata/probes")
        .join(format!("{fixture}.dsl"))
}

fn assert_fixture(fixture: &str) {
    let ws = fixture_dsl(fixture);
    assert!(ws.is_file(), "fixture DSL missing: {}", ws.display());
    let out = temp_dir(fixture);
    let outcome = execute(
        Request::Export {
            workspace: ws,
            format: "websequencediagrams".into(),
            output: Some(out.clone()),
        },
        &opts(),
    );
    assert_eq!(outcome.exit, ExitClass::Ok, "{fixture}: {:?}", outcome.log);
    let produced = file_map(&out);
    let expected = file_map(&golden_base().join(fixture));
    assert!(!expected.is_empty(), "{fixture}: empty golden dir");
    let missing: Vec<&String> = expected
        .keys()
        .filter(|k| !produced.contains_key(*k))
        .collect();
    let extra: Vec<&String> = produced
        .keys()
        .filter(|k| !expected.contains_key(*k))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{fixture}: file-set mismatch\n  missing (oracle-only): {missing:?}\n  extra (ours-only): {extra:?}"
    );
    for (name, want) in &expected {
        let got = &produced[name];
        assert!(
            got == want,
            "{fixture}/{name}: bytes differ\n--- produced ---\n{}\n--- expected ---\n{}",
            String::from_utf8_lossy(got),
            String::from_utf8_lossy(want)
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn wsd_goldens_byte_parity() {
    let mut fixtures: Vec<String> = std::fs::read_dir(golden_base())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    fixtures.sort();
    assert!(fixtures.len() >= 9, "wsd corpus small: {fixtures:?}");
    let mut mismatches = Vec::new();
    for fixture in &fixtures {
        // Collect-and-report so one drift shows all failing fixtures at once.
        if std::panic::catch_unwind(|| assert_fixture(fixture)).is_err() {
            mismatches.push(fixture.clone());
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} golden fixture(s) FAILED: {mismatches:?}",
        mismatches.len(),
        fixtures.len()
    );
}
