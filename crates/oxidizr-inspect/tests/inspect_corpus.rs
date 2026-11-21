//! §P4-M1 I-3 robustness sweep: `inspect` must NEVER panic on a
//! reference-corpus workspace (Category A, hermetic — the engine is pure
//! model logic; network includes/extends are rejected by the DSL parser,
//! so nothing here touches the network). Unparsable fixtures are skipped
//! (the parser's own corpus tests own those expectations).
#![allow(clippy::unwrap_used)]

use oxidizr_dsl::engine::{ParseOptions, parse_dsl};
use oxidizr_inspect::inspect;

fn dsl_files() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "dsl"))
        .collect();
    files.sort();
    files
}

#[test]
fn corpus_sweep_never_panics() {
    let mut parsed = 0;
    let mut total_violations = 0;
    for path in dsl_files() {
        // `iso-8859.dsl` is a deliberate Latin-1 fixture — lossy-read it
        // (parse may then fail, which the skip below already covers).
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let source = String::from_utf8_lossy(&bytes);
        let Ok(result) = parse_dsl(&source, ParseOptions::file(&path.to_string_lossy())) else {
            continue; // network includes, `!docs`, missing extends bases…
        };
        parsed += 1;
        // The whole contract: the engine terminates on every reference
        // workspace — arbitrary ids, unresolvable view systems, dangling
        // relationship ends included.
        total_violations += inspect(&result.workspace).len();
    }
    // Most of the 98-file corpus must parse for the sweep to be a
    // meaningful gate.
    assert!(parsed >= 60, "only {parsed} corpus files parsed");
    assert!(
        total_violations > 100,
        "sweep found no work: {total_violations}"
    );
}
