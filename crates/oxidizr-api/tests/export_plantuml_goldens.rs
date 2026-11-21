//! Phase 2 M2-d/M3-d golden byte-parity for the `PlantUML` exports
//! (Category A): every `testdata/golden/plantuml[-c4]/<fixture>/` set is the
//! raw oracle output (`cargo xtask oracle gen --plantuml [--c4]`, structurizr
//! 2026.10.26 @ ad34036 — per-view `structurizr-<id>-<key>.puml` (+ `-key`
//! legend files, C4 none), no normalization); these tests run our
//! `plantuml` / `plantuml/c4plantuml` export over the matching
//! `testdata/dsl/<fixture>.dsl` and byte-compare file sets and contents.
//!
//! The corpora are the full-corpus A/B sweep SAME sets MINUS vacuous
//! (viewless or error-abort) fixtures — `custom-view-animation` is C4-vacuous
//! (`CustomView`s skipped → zero files) and stays out of the C4 corpus.
//! Deliberate deviations stay OUT of the corpora (they are DIFFs by design,
//! registered in `agents/plan/DEVIATIONS.md`): `bulk-operations` (D13
//! identity-hash fan-out order), `script-inline` / `script-in-dynamic-view`
//! (D2), `text-block` (D11), theme-URL fixtures (offline oracle crash — D3
//! interim rule).
//!
//! The last tests are RENDER smokes over representative goldens, skipped
//! silently when no `plantuml` binary is on PATH (PR CI has none; the pinned
//! dev shell does, and it bundles the C4 stdlib offline).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, ExitClass, Request, execute};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn golden_base(subdir: &str) -> PathBuf {
    root().join("testdata/golden").join(subdir)
}

/// Hermetic options as in `export_plantuml.rs`: empty env, no stdin, no
/// network (theme legs stay resolvable offline — P2-D3).
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
    let dir = std::env::temp_dir().join(format!("oxidizr-pgl-gold-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `{ name => bytes }` of the `*.puml` files in `dir` (missing dir = empty).
fn puml_map(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "puml") {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                out.insert(name, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

fn golden_fixtures(subdir: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(golden_base(subdir))
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
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

fn assert_fixture(subdir: &str, format: &str, fixture: &str) {
    let ws = fixture_dsl(fixture);
    assert!(ws.is_file(), "fixture DSL missing: {}", ws.display());
    let out = temp_dir(&format!("{subdir}-{fixture}"));
    let outcome = execute(
        Request::Export {
            workspace: ws,
            format: format.into(),
            output: Some(out.clone()),
        },
        &opts(),
    );
    assert_eq!(outcome.exit, ExitClass::Ok, "{fixture}: {:?}", outcome.log);
    let produced = puml_map(&out);
    let expected = puml_map(&golden_base(subdir).join(fixture));
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

fn assert_corpus(subdir: &str, format: &str, min_corpus: usize) {
    let fixtures = golden_fixtures(subdir);
    assert!(
        fixtures.len() >= min_corpus,
        "golden corpus unexpectedly small: {fixtures:?}"
    );
    let mut mismatches = Vec::new();
    for fixture in &fixtures {
        // Collect-and-report so one drift shows all failing fixtures at once.
        if std::panic::catch_unwind(|| assert_fixture(subdir, format, fixture)).is_err() {
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

#[test]
fn plantuml_goldens_byte_parity() {
    assert_corpus("plantuml", "plantuml", 30);
}

#[test]
fn c4plantuml_goldens_byte_parity() {
    assert_corpus("plantuml-c4", "plantuml/c4plantuml", 30);
}

/// Render smoke: plantuml 1.2026.x must parse-and-render representative
/// goldens (context + sequence + custom + legend). Skips when plantuml is
/// absent — PR CI never requires it (agents/plan/TESTING.md render-check rule).
/// Each golden is COPIED into a scratch dir first: plantuml writes its PNG
/// next to the source, and `testdata/` is never touched.
#[test]
fn plantuml_goldens_render_smoke() {
    let Some(plantuml) = which("plantuml") else {
        eprintln!("render smoke: `plantuml` not on PATH — skipping");
        return;
    };
    render_cases(
        "plantuml",
        &plantuml,
        &[
            (
                "getting-started-short",
                "structurizr-SystemContext-001.puml",
            ),
            ("dynamic", "structurizr-Dynamic-001.puml"),
            ("custom-view-animation", "structurizr-Custom-001.puml"),
            ("shapes", "structurizr-shapes-key.puml"), // legend
        ],
    );
    // C4 macros resolve from the C4-PlantUML stdlib bundled with plantuml —
    // offline, matching the golden's `stdlib=true` default.
    render_cases(
        "plantuml-c4",
        &plantuml,
        &[
            (
                "getting-started-short",
                "structurizr-SystemContext-001.puml",
            ),
            ("dynamic", "structurizr-Dynamic-001.puml"),
            ("groups-nested", "structurizr-SystemLandscape-001.puml"), // boundaries
        ],
    );
}

fn render_cases(subdir: &str, plantuml: &Path, cases: &[(&str, &str)]) {
    for (fixture, file) in cases {
        let golden = golden_base(subdir).join(fixture).join(file);
        assert!(golden.is_file(), "golden missing: {}", golden.display());
        let src_dir = temp_dir(&format!("render-{subdir}-{fixture}"));
        let src = src_dir.join(file);
        std::fs::copy(&golden, &src).unwrap();
        let status = std::process::Command::new(plantuml)
            .arg("-tpng")
            .arg(&src)
            .status()
            .expect("plantuml run failed");
        assert!(
            status.success(),
            "plantuml render FAILED: {}",
            golden.display()
        );
        let rendered = std::fs::read_dir(&src_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .any(|e| e.metadata().is_ok_and(|m| m.len() > 0));
        let _ = std::fs::remove_dir_all(&src_dir);
        assert!(
            rendered,
            "plantuml produced no PNG for {}",
            golden.display()
        );
    }
}

fn which(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|p| p.is_file())
}
