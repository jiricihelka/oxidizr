//! P1-M5 golden byte-parity for the `workspace extends` path (Category A).
//!
//! Parses the deterministic extend fixtures end-to-end (DSL → workspace →
//! writer bytes) and compares against the committed oracle goldens
//! (`cargo xtask oracle gen`, oracle 2026.10.26 @ ad34036). This golden
//! witnesses: the three-file extends chain, id-counter continuation over
//! bases, identifier registration across extends boundaries, and
//! `clearDsl`/no-retention across the whole chain (the workspace-level
//! `properties` object is ABSENT — normalized empty-shell-free, M5).
//!
//! Two extend fixtures are deliberately NOT goldens:
//! * `extend-workspace-from-dsl-file.dsl` — its base has an
//!   identifier-less element, so the reference stamps a random UUID
//!   `structurizr.dsl.identifier` (non-deterministic, ungoldenable);
//! * `source-child.dsl` / `extend-workspace-from-json-file.dsl` — a loaded
//!   workspace with NO views makes the structurizr CLI app inject default
//!   generated views into its export (app harness, not the pinned library;
//!   `agents/notes/testing/oracle-app-harness.md`). The golden bytes would
//!   not be library semantics.
//!
//! All three are covered behaviorally in `m5_extends.rs`.
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use oxidizr_core::Workspace;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

fn dsl_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

fn golden_loader(path: &str) -> Result<Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn assert_golden(fixture: &str, golden: &str) {
    let path = dsl_dir().join(fixture);
    let full = path.to_str().unwrap().to_string();
    let opts = ParseOptions {
        file: Some(&full),
        base_dir: None,
        json_loader: Some(golden_loader),
        env_lookup: None,
        installed_themes: None,
    };
    let source = std::fs::read_to_string(&path).unwrap();
    let result = parse_dsl(&source, opts).unwrap();
    assert!(result.errors.is_empty(), "{fixture}: {:?}", result.errors);
    let produced = oxidizr_json::to_vec(&result.workspace).unwrap();
    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/golden")
        .join(format!("{golden}.json"));
    let expected = std::fs::read(&golden_path).unwrap();
    assert!(
        produced == expected,
        "{fixture}: writer output differs from golden {golden}\n--- produced ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&produced),
        String::from_utf8_lossy(&expected)
    );
}

#[test]
fn extend_chain_golden() {
    assert_golden("extend/4.dsl", "4");
}
