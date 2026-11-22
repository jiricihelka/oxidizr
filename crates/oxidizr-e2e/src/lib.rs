//! `oxidizr-e2e` — the Phase 1T regression harness (`agents/plan/build/PHASE_1T.md`).
//!
//! Drives `oxidizr-api::execute` **in-process** over the shared corpus under
//! `e2e/corpus/`, asserting USE-CASES only — exit class, sorted OXZ code
//! set and structural counts from blessable `workspace.e2e.json` sidecars.
//! Never bytes, never message text (those may change; the codes and shapes
//! are the contract). The `testdata/` byte-parity goldens stay a separate,
//! untouched gate.
//!
//! Hermetic by construction ([`hermetic_options`]): empty env map (nothing
//! in the process env leaks into a parse), no stdin reader (the corpus is
//! file/bytes-only), no rendering — the harness reads `Outcome::log` items
//! as data.
//!
//! Consumers: `tests/quick.rs` (manifest check, rides `cargo test
//! --workspace`), `tests/coverage.rs` (OXZ-code coverage gate),
//! `tests/subprocess_smoke.rs` (binary anchor), and `cargo xtask e2e bless`
//! (sidecar regeneration — the git diff of a bless is the review artifact).

pub mod bless;
pub mod corpus;
pub mod manifest;
pub mod mutate;
pub mod runner;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::{ApiOptions, ParseFlags};

/// Repo root (this crate lives at `crates/oxidizr-e2e`).
#[must_use]
pub fn repo_root() -> PathBuf {
    match Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
    {
        Some(root) => root.to_path_buf(),
        None => PathBuf::from("."),
    }
}

/// The corpus root (`e2e/corpus` outside `testdata/` by P1T-D2: authored,
/// blessable fixtures must not touch the vendored byte-contract).
#[must_use]
pub fn corpus_root() -> PathBuf {
    repo_root().join("e2e").join("corpus")
}

/// Hermetic pipeline options for corpus runs: an EMPTY env map (unresolved
/// `${VAR}` stays raw — the process env never leaks), no stdin reader (the
/// harness sends `File`/`Bytes` inputs only).
#[must_use]
pub fn hermetic_options() -> ApiOptions {
    ApiOptions {
        parse: ParseFlags {
            strict: false,
            keep_unknown: false,
        },
        env: Some(BTreeMap::new()),
        stdin: None,
        // No network in-process; theme dir sources come only from the
        // hermetic env map above and explicit fixtures.
        allow_net: false,
        themes_dir: None,
        // PLAIN: the harness never paints (the diff human report rides
        // `ApiOptions::color`, §P4-M0 D-3).
        color: false,
    }
}

/// Join under a base path as string for error messages.
pub(crate) fn display(path: &Path) -> String {
    path.display().to_string()
}
