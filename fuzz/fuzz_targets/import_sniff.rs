//! Fuzz the two TEXT macro importers (§P4T T-5, gap 3): any byte soup fed
//! to the C4-PlantUML and Mermaid `import` entries must never panic (the
//! F12 semantic bar only constrains accepted documents — the failure path
//! is the fuzz surface). Seeds: the authored fixture corpus under
//! `crates/oxidizr-import/tests/fixtures/` (staged by `cargo xtask fuzz`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_import::{c4plantuml, mermaid};

fuzz_target!(|data: &[u8]| {
    // The importers take `&str`; fuzz raw bytes through the same lossy
    // gate the engine's text reads would produce (`dsl_parse` precedent).
    let text = String::from_utf8_lossy(data);
    // Results are irrelevant — ImportError is a value (no panics, no
    // unbounded recursion is the contract).
    let _ = c4plantuml::import(&text);
    let _ = mermaid::import(&text);
});
