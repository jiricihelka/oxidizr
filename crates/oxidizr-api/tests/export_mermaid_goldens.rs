//! Phase 2 M4-d golden byte-parity for the `Mermaid` and `theme` exports
//! (Category A): `testdata/golden/mermaid/<fixture>/*.mmd` sets and
//! `testdata/golden/theme/<fixture>.json` are the raw oracle output
//! (`cargo xtask oracle gen --mermaid|--theme`, structurizr 2026.10.26 @
//! ad34036, no normalization); these tests run our `mermaid` / `theme`
//! export over the matching `testdata/dsl/<fixture>.dsl` and byte-compare
//! file sets and contents.
//!
//! The mermaid corpus is the 30-fixture SAME set of the M4-c byte sweep —
//! the same exclusions as the C4 corpus apply (`agents/plan/build/
//! EXPORT-MERMAID.md` §11): `bulk-operations` (D13 relationship-id fan-out
//! order), `script-inline` / `script-in-dynamic-view` (D2 loader rejects),
//! the oracle-load failures, and `text-block` (loads but Mermaid writes 0
//! files — vacuous). Unlike C4, `custom-view-animation` IS in the corpus:
//! `MermaidDiagramExporter` exports `CustomView`s (base class) — two VIEWS,
//! never animation frames.
//!
//! The theme corpus is six style-bearing fixtures (URL-theme fixtures stay
//! out — D3 interim rule); the theme export itself landed at M0, these are
//! its goldens.
//!
//! Render checks follow the P2-D9 rule: run where the tool exists, skip
//! LOUDLY (an `eprintln!` line in the test output) when `plantuml` / `mmdc`
//! are absent — PR CI has neither (agents/plan/TESTING.md).
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

fn golden_base(subdir: &str) -> PathBuf {
    root().join("testdata/golden").join(subdir)
}

/// Hermetic options as in `export_mermaid.rs`: empty env, no stdin, no
/// network (theme legs stay resolvable offline — P2-D3).
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
    let dir = std::env::temp_dir().join(format!("oxidizr-m4d-gold-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `{ name => bytes }` of the `*.{ext}` files in `dir` (missing dir = empty).
fn file_map(dir: &Path, ext: &str) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == ext) {
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

fn assert_fixture(subdir: &str, format: &str, ext: &str, fixture: &str) {
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
    let produced = file_map(&out, ext);
    let expected = file_map(&golden_base(subdir).join(fixture), ext);
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

fn assert_corpus(subdir: &str, format: &str, ext: &str, min_corpus: usize) {
    let fixtures = golden_fixtures(subdir);
    assert!(
        fixtures.len() >= min_corpus,
        "golden corpus unexpectedly small: {fixtures:?}"
    );
    let mut mismatches = Vec::new();
    for fixture in &fixtures {
        // Collect-and-report so one drift shows all failing fixtures at once.
        if std::panic::catch_unwind(|| assert_fixture(subdir, format, ext, fixture)).is_err() {
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
fn mermaid_goldens_byte_parity() {
    assert_corpus("mermaid", "mermaid", "mmd", 30);
}

/// The theme goldens are single files: `golden/theme/<fixture>.json` vs our
/// `<fixture>-theme.json` (the workspace-stem naming, M0).
#[test]
fn theme_goldens_byte_parity() {
    let base = golden_base("theme");
    let mut fixtures: Vec<String> = std::fs::read_dir(&base)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| e.path().file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    fixtures.sort();
    assert!(fixtures.len() >= 6, "theme corpus small: {fixtures:?}");
    let mut mismatches = Vec::new();
    for fixture in &fixtures {
        if std::panic::catch_unwind(|| {
            let ws = fixture_dsl(fixture);
            assert!(ws.is_file(), "fixture DSL missing: {}", ws.display());
            let out = temp_dir(&format!("theme-{fixture}"));
            let outcome = execute(
                Request::Export {
                    workspace: ws,
                    format: "theme".into(),
                    output: Some(out.clone()),
                },
                &opts(),
            );
            assert_eq!(outcome.exit, ExitClass::Ok, "{fixture}: {:?}", outcome.log);
            let produced = std::fs::read(out.join(format!("{fixture}-theme.json")))
                .expect("theme file written");
            let expected = std::fs::read(base.join(format!("{fixture}.json"))).unwrap();
            assert!(
                produced == expected,
                "{fixture}: theme bytes differ\n--- produced ---\n{}\n--- expected ---\n{}",
                String::from_utf8_lossy(&produced),
                String::from_utf8_lossy(&expected)
            );
            let _ = std::fs::remove_dir_all(&out);
        })
        .is_err()
        {
            mismatches.push(fixture.clone());
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} of {} theme golden(s) FAILED: {mismatches:?}",
        mismatches.len(),
        fixtures.len()
    );
}

/// Render smoke (P2-D9): `mmdc` (Mermaid CLI) must parse-and-render
/// representative goldens — a `graph` with boundaries, the standalone
/// `sequenceDiagram` form and a custom view. Skips LOUDLY when `mmdc` is
/// absent (it is NOT on this machine's PATH; the nix dev shell may add it).
#[test]
fn mermaid_goldens_render_smoke() {
    let Some(mmdc) = which("mmdc") else {
        eprintln!("mermaid render smoke: `mmdc` not on PATH — SKIPPING (P2-D9 loud skip)");
        return;
    };
    for (fixture, file) in [
        ("getting-started-short", "structurizr-SystemContext-001.mmd"),
        ("dynamic", "structurizr-Dynamic-001.mmd"),
        ("groups-nested", "structurizr-SystemLandscape-001.mmd"),
        ("custom-view-animation", "structurizr-Custom-001.mmd"),
    ] {
        let golden = golden_base("mermaid").join(fixture).join(file);
        assert!(golden.is_file(), "golden missing: {}", golden.display());
        let src_dir = temp_dir(&format!("render-{fixture}"));
        let src = src_dir.join(file);
        let svg = src_dir.join("out.svg");
        std::fs::copy(&golden, &src).unwrap();
        let status = std::process::Command::new(&mmdc)
            .arg("-i")
            .arg(&src)
            .arg("-o")
            .arg(&svg)
            .status()
            .expect("mmdc run failed");
        assert!(status.success(), "mmdc render FAILED: {}", golden.display());
        let rendered = svg.is_file() && svg.metadata().is_ok_and(|m| m.len() > 0);
        let _ = std::fs::remove_dir_all(&src_dir);
        assert!(rendered, "mmdc produced no SVG for {}", golden.display());
    }
}

fn which(binary: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(binary))
        .find(|p| p.is_file())
}
