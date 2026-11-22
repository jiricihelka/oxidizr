//! Fuzz the full DSL pipeline: preprocess → parse must never panic and never
//! blow up unboundedly (M6-d; PHASE_1 §P1-D3 pipeline, §P1-D11 fuzz targets).
//! The seed corpus is the vendored `.dsl` fixture set (staged by
//! `cargo xtask fuzz`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

fuzz_target!(|data: &[u8]| {
    // The engine takes `&str`; fuzz raw bytes through the same lossy gate the
    // CLI's UTF-8 check would reject (V-8) — beyond that boundary the
    // pipeline must still be panic-free on any text.
    let source = String::from_utf8_lossy(data);
    let _ = parse_dsl(
        &source,
        ParseOptions {
            file: None,
            base_dir: None,
            json_loader: None,
            env_lookup: None,
            // M1-d field: `None` = empty registry, the fuzz-safe default.
            installed_themes: None,
        },
    );
});
