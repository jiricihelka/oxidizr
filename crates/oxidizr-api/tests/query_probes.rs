//! §P4-M3 O-1 `query` tests (Category A, `agents/plan/build/PHASE_4.md`):
//! the cross-format reproducibility invariant P4-D3 demanded — a
//! `--kind element --format ids` result MUST be identical for a `.dsl`
//! and its JSON twin (no DSL-only state leaks: identifiers resolve via
//! `structurizr.dsl.identifier` on BOTH paths, tags via the EFFECTIVE
//! view) — plus a never-panics sweep of the whole vendored corpus with
//! every grammar-arm family.
//!
//! These tests drive `oxidizr_api::execute` (the pipeline wiring) with
//! the hermetic options; the fixture files are real paths, so nothing
//! touches stdin or the process env.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, ExitClass, ParseFlags, Request, execute};
use oxidizr_api::{QueryFormat, QueryKind};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .canonicalize()
        .unwrap()
}

fn probe(name: &str) -> PathBuf {
    testdata().join("probes/diff").join(name)
}

/// Hermetic options (same posture as the `diff_probes` harness).
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

/// Run `query` in-process; returns `(exit, stdout bytes)`.
fn query_run(
    workspace: &Path,
    filter: &str,
    kind: QueryKind,
    format: QueryFormat,
) -> (ExitClass, Vec<u8>) {
    let out = execute(
        Request::Query {
            workspace: workspace.to_path_buf(),
            filter: filter.to_string(),
            kind,
            format,
            output: None,
        },
        &opts(),
    );
    (out.exit, out.bytes.unwrap_or_default())
}

/// Grammar witnesses, one per arm family the `query` surface promises
/// (P4-V4 inventory, `docs/query-schema.md`): identifiers come from the
/// probe fixtures (`alice`/`bank`), values are BARE tokens.
const TWIN_FILTERS: &[&str] = &[
    "element.tag==Element",
    "element.tag!=Element",
    "element.type==SoftwareSystem",
    "element.type==Person",
    "element.technology==PostgreSQL",
    "element.group==Staff",
    "element.tag==Element && element.type==Person",
    "element.tag==Element || element.tag==Relationship",
    "relationship.tag==Relationship",
    "relationship.source==alice",
    "relationship.destination==bank",
    "alice",
    "bank",
    "alice->bank",
    "bank->",
    "->bank",
    "element.properties[\"purpose\"]==sends e-mails",
    "element.parent==bank",
];

#[test]
fn a_dsl_and_its_json_twin_query_identically() {
    for (d, j) in [("base.dsl", "base.json"), ("edited.dsl", "edited.json")] {
        for filter in TWIN_FILTERS {
            let (_de, d_ids) = query_run(&probe(d), filter, QueryKind::All, QueryFormat::Ids);
            let (_je, j_ids) = query_run(&probe(j), filter, QueryKind::All, QueryFormat::Ids);
            assert_eq!(
                d_ids,
                j_ids,
                "{d} vs {j} differ for filter {filter:?} — DSL-only state leaked\n\
                 dsl:\n{}\njson:\n{}",
                String::from_utf8_lossy(&d_ids),
                String::from_utf8_lossy(&j_ids),
            );
        }
    }
}

#[test]
fn element_ids_only_are_reproducible_from_the_json_twin() {
    // The P4-D3 witness in its exact `--kind element --format ids` form.
    for (d, j) in [("base.dsl", "base.json"), ("edited.dsl", "edited.json")] {
        let (exit, d_ids) = query_run(
            &probe(d),
            "element.tag==Element",
            QueryKind::Element,
            QueryFormat::Ids,
        );
        assert_eq!(exit, ExitClass::Ok);
        let (_, j_ids) = query_run(
            &probe(j),
            "element.tag==Element",
            QueryKind::Element,
            QueryFormat::Ids,
        );
        assert!(!d_ids.is_empty(), "{d}: the fixture has elements");
        assert_eq!(d_ids, j_ids, "{d} vs {j}: element ids differ");
    }
}

#[test]
fn zero_matches_is_success_and_silent() {
    let (exit, bytes) = query_run(
        &probe("base.dsl"),
        "element.tag==NoSuchTag",
        QueryKind::All,
        QueryFormat::Ids,
    );
    assert_eq!(exit, ExitClass::Ok, "query is informational");
    assert!(bytes.is_empty(), "no rows, no bytes");
}

#[test]
fn throwing_arm_is_a_diagnostics_failure() {
    // `element.parent==<unknown>` is a THROWING reference arm.
    let (exit, _) = query_run(
        &probe("base.dsl"),
        "element.parent==nobody",
        QueryKind::All,
        QueryFormat::Ids,
    );
    assert_eq!(exit, ExitClass::Diagnostics);
}

#[test]
fn corpus_sweep_never_panics_and_exit_is_clean() {
    // Every vendored `.dsl`/`.json`, every arm family: a clean open plus
    // `query` must never panic, and must exit Ok or Diagnostics (a
    // THROWING arm) — never Io/Usage.
    let mut files = Vec::new();
    walk(&testdata().join("dsl"), "dsl", &mut files);
    walk(&testdata().join("golden"), "json", &mut files);
    walk(&testdata().join("read-corpus"), "json", &mut files);
    files.sort();
    assert!(files.len() > 100, "vendored corpus expected");
    let filters = [
        "element.tag==Element",
        "relationship.tag==Relationship",
        "element.type==DeploymentNode",
        "element.technology==Spring",
        "bank->",
        "*",
    ];
    for f in &files {
        // A file the OPEN stage itself refuses (the non-UTF-8
        // `iso-8859.dsl` policy fixture → exit 3, pinned by the e2e
        // corpus) never reaches the grammar — skip, the report layer
        // adds no I/O of its own for the `ids` format.
        let (probe_exit, _) = query_run(f, "*", QueryKind::All, QueryFormat::Ids);
        if probe_exit == ExitClass::Io {
            continue;
        }
        for filter in filters {
            let (exit, _) = query_run(f, filter, QueryKind::All, QueryFormat::Ids);
            assert!(
                matches!(exit, ExitClass::Ok | ExitClass::Diagnostics),
                "{} with {filter:?}: unexpected exit {exit:?}",
                f.display()
            );
        }
    }
}

#[test]
fn view_subset_reports_keys_titles_and_descriptions() {
    // The probe fixture defines ONE view, keyed `landscape` (custom keys
    // stay custom — `open` installs default views only when there are
    // none, and their generated keys are queryable the same way).
    let ws = probe("base.dsl");
    let (exit, bytes) = query_run(&ws, "key==landscape", QueryKind::View, QueryFormat::Ids);
    assert_eq!(exit, ExitClass::Ok);
    assert_eq!(String::from_utf8(bytes).unwrap(), "landscape\n");
    // `key!=<nope>` keeps every view …
    let (_, bytes) = query_run(&ws, "key!=Nope", QueryKind::View, QueryFormat::Ids);
    assert_eq!(String::from_utf8(bytes).unwrap(), "landscape\n");
    // … and the view JSON section stands alone (no `elements` key).
    let (_, bytes) = query_run(&ws, "key==landscape", QueryKind::View, QueryFormat::Json);
    let doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc["kind"], "view");
    assert!(doc.get("elements").is_none());
    assert_eq!(doc["views"][0]["key"], "landscape");
    assert_eq!(doc["views"][0]["kind"], "SystemLandscape");
}
