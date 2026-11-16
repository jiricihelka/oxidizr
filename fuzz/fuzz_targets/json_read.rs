//! Fuzz the JSON reader: `from_slice` is total (no panics on hostile input —
//! the reference reads leniently, oxidizr must not be stricter in a way that
//! crashes), and anything it accepts must round-trip the canonical writer:
//! `to_vec` never fails, and re-reading those bytes reproduces them exactly
//! (the canonical-form fixpoint the M3b parity gates assert on the corpus).

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxidizr_json::{from_slice, to_vec};

fuzz_target!(|data: &[u8]| {
    let Ok(ws) = from_slice(data) else {
        return; // rejected input is fine — the reader validates as values
    };
    let Ok(bytes) = to_vec(&ws) else {
        panic!("writer rejected a workspace the reader produced");
    };
    let again = from_slice(&bytes).expect("canonical bytes must re-read");
    let bytes2 = to_vec(&again).expect("re-write of canonical bytes");
    assert_eq!(bytes, bytes2, "canonical form is not a fixpoint");
});
