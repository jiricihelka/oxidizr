//! Phase 2 M2-d golden byte-parity for the Structurizr-PlantUML export
//! (Category A): every `testdata/golden/plantuml/<fixture>/` set is the raw
//! oracle output (`cargo xtask oracle gen --plantuml`, structurizr 2026.10.26
//! @ ad34036 — per-view `structurizr-<id>-<key>.puml` + `-key` legend files,
//! no normalization); this test runs our `plantuml` export over the matching
//! `testdata/dsl/<fixture>.dsl` and byte-compares file sets and contents.
//!
//! The corpus is the full-corpus A/B sweep SAME set MINUS vacuous (viewless
//! or error-abort) fixtures — see `agents/notes/RESUME.md`. Deliberate
//! deviations stay OUT of the corpus (they are DIFFs by design, registered in
//! `agents/plan/DEVIATIONS.md`): `bulk-operations` (D13 identity-hash fan-out
//! order), `script-inline` / `script-in-dynamic-view` (D2), `text-block`
//! (D11), theme-URL fixtures (offline oracle crash — D3 interim rule).
//!
//! The last test is a RENDER smoke over one golden per category (static /
//! dynamic / custom + legend), skipped silently when no `plantuml` binary is
//! on PATH (PR CI has none; the pinned dev shell does).
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

fn golden_base() -> PathBuf {
    root().join("testdata/golden/plantuml")
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
    }
}

fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m2d-gold-{topic}-{}", std::process::id()));
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

fn golden_fixtures() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(golden_base())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn assert_fixture(fixture: &str) {
    let ws = root().join("testdata/dsl").join(format!("{fixture}.dsl"));
    assert!(ws.is_file(), "fixture DSL missing: {}", ws.display());
    let out = temp_dir(fixture);
    let outcome = execute(
        Request::Export {
            workspace: ws,
            format: "plantuml".into(),
            output: Some(out.clone()),
        },
        &opts(),
    );
    assert_eq!(outcome.exit, ExitClass::Ok, "{fixture}: {:?}", outcome.log);
    let produced = puml_map(&out);
    let expected = puml_map(&golden_base().join(fixture));
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
fn plantuml_goldens_byte_parity() {
    let fixtures = golden_fixtures();
    assert!(
        fixtures.len() >= 30,
        "golden corpus unexpectedly small: {fixtures:?}"
    );
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

/// Render smoke: plantuml 1.2026.x must parse-and-render representative
/// goldens (context + sequence + custom + legend). Skips when plantuml is
/// absent — PR CI never requires it (agents/plan/TESTING.md render-check rule).
/// Each golden is COPIED into a scratch dir first: plantuml writes its PNG
/// next to the source, and `testdata/` is never touched.
#[test]
fn plantuml_goldens_render_smoke() {
    if which("plantuml").is_none() {
        eprintln!("render smoke: `plantuml` not on PATH — skipping");
        return;
    }
    let base = golden_base();
    for (fixture, file) in [
        (
            "getting-started-short",
            "structurizr-SystemContext-001.puml",
        ),
        ("dynamic", "structurizr-Dynamic-001.puml"),
        ("custom-view-animation", "structurizr-Custom-001.puml"),
        ("shapes", "structurizr-shapes-key.puml"), // legend
    ] {
        let golden = base.join(fixture).join(file);
        assert!(golden.is_file(), "golden missing: {}", golden.display());
        let src_dir = temp_dir(&format!("render-{fixture}"));
        let src = src_dir.join(file);
        std::fs::copy(&golden, &src).unwrap();
        let status = std::process::Command::new("plantuml")
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
