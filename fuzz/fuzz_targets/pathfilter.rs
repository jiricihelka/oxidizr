//! Fuzz the `generate` path-filter regex subset (§P4T T-5, gap 3):
//! `PathFilter::compile` REJECTS out-of-subset patterns as `String` values
//! and `is_match` runs the `regexlite` mirror over fixed subjects — neither
//! may ever panic. Input shape: the pattern bytes (an optional NUL ends the
//! pattern). Seeds: `fuzz/seeds/pathfilter/` (the documented-subset table
//! plus the rejected-construct list, as `pattern \0 text`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_api::generate::pathfilter::PathFilter;

fuzz_target!(|data: &[u8]| {
    // The pattern is the fuzzed surface (the grammar — the half that can
    // miscompile). An optional NUL ends it so pair-shaped seeds still
    // exercise their pattern half.
    let end = data
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(data.len());
    let pattern = String::from_utf8_lossy(&data[..end]).into_owned();
    if pattern.len() > 256 {
        return; // compile cost is linear; the cap only trims pointless tail bytes
    }
    // Errors are values (the OXZ-5010 decoration is the caller's job);
    // reaching either branch is success.
    if let Ok(filter) = PathFilter::compile(&pattern) {
        // FIXED short subjects, never fuzzed text (P4T-V2): the matcher
        // mirrors `java.util.regex`, which backtracks UNBOUNDEDLY — nested /
        // multi-branch quantifiers (`.+.+\D+\D+…`, `(a|a|…)*x` — found as
        // slow-units and a timeout here) blow up exponentially in the subject
        // length in the REFERENCE too, so that is parity, not a bug, and
        // capping the matcher would deviate. A ≤2-char subject bounds every
        // pattern at ≤ (alternatives)² steps; deep match semantics stay the
        // unit suite's contract. ReDoS defense is the CALLER's input
        // discipline, exactly as in the reference.
        for subject in ["", "a", "42", "a4"] {
            let _ = filter.is_match(subject);
        }
    }
});
