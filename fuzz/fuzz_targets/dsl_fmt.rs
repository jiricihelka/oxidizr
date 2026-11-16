//! Fuzz the `fmt` printer: any text formats without panicking, and formatting
//! is idempotent (P1-D7 invariant 1 — the fuzz counterpart of the `m6_fmt`
//! proptest gate, over arbitrary bytes instead of generated DSL flavour).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_dsl::fmt;

fuzz_target!(|data: &[u8]| {
    let source = String::from_utf8_lossy(data);
    let once = fmt::format(&source);
    let twice = fmt::format(&once);
    assert_eq!(once, twice, "fmt is not idempotent");
});
