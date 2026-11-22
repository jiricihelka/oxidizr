//! Fuzz the identity axis of the diff engine (§P4T T-5, gap 3): a
//! workspace loaded from ANY bytes, then diffed against ITSELF, must
//! report ZERO findings — the P4-M0 invariant pushed through the whole
//! load pipeline. A panic, or a self-diff that is not empty, is a bug in
//! the loader (id instability) or in `diff`. Seeds: the vendored `.dsl`
//! fixture corpus (staged by `cargo xtask fuzz`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_api::{ApiOptions, Input, diff::diff, open};

fuzz_target!(|data: &[u8]| {
    // Hermetic load: empty env map (no `${VAR}` from the fuzz process),
    // default everything else (no net, no themes).
    let opts = ApiOptions {
        env: Some(std::collections::BTreeMap::new()),
        ..Default::default()
    };
    let Ok(opened) = open(&Input::Bytes(data.to_vec()), &opts) else {
        return; // load failures are values — the exit-class tests own them
    };
    let workspace = &opened.workspace;
    let report = diff(workspace, workspace, &Default::default());
    assert!(
        report.is_empty(),
        "self-diff of a fuzz-loaded workspace must be empty, got {} added / \
         {} removed / {} changed / {} idChanged",
        report.added.len(),
        report.removed.len(),
        report.changed.len(),
        report.id_changed.len(),
    );
});
