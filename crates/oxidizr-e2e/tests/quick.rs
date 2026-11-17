//! Quick-tier manifest check (P1T-D5): every corpus entry, all applicable
//! commands, in-process and parallel (`std::thread::scope` — std-only; the
//! "tens of thousands of processes" trap is structurally avoided). Rides
//! `cargo test --workspace` on every PR.
#![allow(clippy::unwrap_used)]

use oxidizr_e2e::corpus;

#[test]
fn corpus_matches_manifests() {
    let entries = corpus::discover().expect("corpus discovers");
    assert!(
        !entries.is_empty(),
        "e2e/corpus is empty — seed entries are missing"
    );

    // Fan out; collect per-entry failure lists (one panic at the end keeps
    // the report readable across parallel threads).
    let mut results: Vec<(String, Result<Vec<String>, String>)> =
        vec![const { (String::new(), Ok(Vec::new())) }; entries.len()];
    std::thread::scope(|scope| {
        let slots: Vec<&mut (String, Result<Vec<String>, String>)> = results.iter_mut().collect();
        for (slot, entry) in slots.into_iter().zip(&entries) {
            let entry = entry.clone();
            scope.spawn(move || {
                slot.0 = entry.full_name();
                slot.1 = oxidizr_e2e::runner::check(&entry);
            });
        }
    });

    let mut problems: Vec<String> = Vec::new();
    for (name, result) in results {
        match result {
            Ok(failures) if failures.is_empty() => {}
            Ok(failures) => problems.push(format!(
                "{name}:\n    {}",
                failures.join(&format!("\n  {name}:    "))
            )),
            Err(e) => problems.push(format!("{name}: {e}")),
        }
    }
    assert!(
        problems.is_empty(),
        "{} e2e entr{} diverged from the blessed manifests (re-bless with \
         `cargo xtask e2e bless` ONLY for intended behavior changes):\n{}",
        problems.len(),
        if problems.len() == 1 { "y" } else { "ies" },
        problems.join("\n")
    );
}
