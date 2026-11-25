//! Java `String.hashCode` + `HashMap` iteration-order emulation.
//!
//! In the official format every free-form `properties` map is written in
//! Java `HashMap` ITERATION order — not sorted, not insertion order. Free-form
//! maps are therefore the one place where key order is not alphabetical; the
//! writer replays each map's INSERTION sequence (kept verbatim in
//! [`oxidizr_core::properties::Properties`]) through [`entry_order`] to
//! reproduce the byte order exactly.
//!
//! Semantics replicated (Java 8+ `HashMap`, witnessed by
//! `testdata/golden/m3-order.json`):
//!
//! * `String.hashCode`: `h = 31*h + utf16Unit` with `int` wrap-around.
//! * Spread: `h ^ (h >>> 16)` (LOGICAL shift on the 32-bit pattern).
//! * Capacity starts at 16 and doubles whenever `size > 3*cap/4` after a
//!   put of a NEW key (`HashMap.resize()` threshold; no removals happen in
//!   this codebase, so the final capacity depends only on the number of
//!   distinct keys).
//! * Iteration = buckets `0..cap-1`; an entry lives in bucket
//!   `spread(hash) & (cap-1)` of the FINAL capacity (Java's power-of-two
//!   resize preserves exactly this), and entries of one bucket keep their
//!   insertion order.
//!
//! Accepted deviations: treeified buckets
//! (≥ 8 same-bucket keys at cap ≥ 64) iterate in tree order in Java,
//! insertion order here; capacity inputs containing stripped
//! `structurizr.inspection.*` keys follow the golden-stripping rule.

/// `java.lang.String.hashCode()` over UTF-16 code units (surrogate pairs
/// count as TWO units — Rust `char`s must be expanded, not iterated raw).
#[must_use]
pub(crate) fn string_hash_code(s: &str) -> i32 {
    let mut h: i32 = 0;
    // UTF-16 code units: Java counts surrogate halves separately, so expand
    // Rust scalars (surrogate halves never appear in valid Rust `char`s).
    for unit in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    h
}

/// `HashMap.hash(o)`: spread the high bits down (`h ^ (h >>> 16)`).
// The casts reinterpret bits, never values — exactly what Java `>>>` does
// on the int's unsigned view (`cast_unsigned`/`cast_signed` would be the
// lint-free spelling but are stable only since 1.87, above our 1.85 MSRV).
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
#[must_use]
pub fn spread(h: i32) -> i32 {
    let u = h as u32;
    (u ^ (u >> 16)) as i32
}

/// Insertion-ordered `keys` → the indices in Java `HashMap` ITERATION order.
///
/// `keys` is the sequence of PUTS (duplicate keys keep the FIRST position,
/// matching `Properties::insert` replacing in place). The final capacity is
/// derived by replaying the puts (a put of a new key doubles capacity while
/// `size > 3*cap/4`).
// The `i32 as usize` below keeps the low bits Java buckets use (`& mask`
// discards the sign extension), so the sign "loss" is deliberate.
#[allow(clippy::cast_sign_loss)]
#[must_use]
pub fn entry_order(keys: &[&str]) -> Vec<usize> {
    // Pass 1: distinct keys + final capacity.
    let mut distinct: Vec<&str> = Vec::with_capacity(keys.len());
    let mut cap: usize = 16;
    for &k in keys {
        if distinct.contains(&k) {
            continue;
        }
        distinct.push(k);
        if distinct.len() > cap * 3 / 4 {
            cap *= 2;
        }
    }
    // Pass 2: bucket = spread & (cap-1); iterate buckets ascending, within a
    // bucket keep insertion order (stable grouping).
    let mut order = Vec::with_capacity(distinct.len());
    let mask = cap - 1;
    for bucket in 0..cap {
        for (i, &k) in distinct.iter().enumerate() {
            if spread(string_hash_code(k)) as usize & mask == bucket {
                order.push(i);
            }
        }
    }
    order
}

/// The properties map of `Properties` (insertion-ordered) in Java `HashMap`
/// output order: `(key, value)` pairs.
#[must_use]
pub(crate) fn ordered_properties(
    props: &oxidizr_core::properties::Properties,
) -> Vec<(&str, &str)> {
    let pairs: Vec<(&str, &str)> = props.iter().collect();
    let keys: Vec<&str> = pairs.iter().map(|(k, _)| *k).collect();
    entry_order(&keys).into_iter().map(|i| pairs[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::{entry_order, spread, string_hash_code};

    /// The person map of `testdata/probes/m3-order.dsl` (14 keys; the
    /// golden key order is the test vector).
    #[test]
    fn golden_person_map_order() {
        let inserted = [
            "structurizr.dsl.identifier",
            "aaa",
            "bbb",
            "ccc",
            "ddd",
            "eee",
            "fff",
            "ggg",
            "hhh",
            "iii",
            "jjj",
            "kkk",
            "lll",
            "mmm",
        ];
        let order = entry_order(&inserted);
        let emitted: Vec<&str> = order.iter().map(|&i| inserted[i]).collect();
        assert_eq!(
            emitted,
            [
                "structurizr.dsl.identifier",
                "aaa",
                "ccc",
                "bbb",
                "eee",
                "ddd",
                "ggg",
                "fff",
                "iii",
                "hhh",
                "kkk",
                "jjj",
                "mmm",
                "lll",
            ]
        );
    }

    /// Relationship map (3 keys) golden vector: r1,r2,r3 → r2,r3,r1.
    #[test]
    fn golden_relationship_map_order() {
        let inserted = ["r1", "r2", "r3"];
        let order = entry_order(&inserted);
        let emitted: Vec<&str> = order.iter().map(|&i| inserted[i]).collect();
        assert_eq!(emitted, ["r2", "r3", "r1"]);
    }

    /// `testdata/golden/workspace-properties.json` vector (cap 16).
    #[test]
    fn golden_workspace_properties_map_order() {
        let inserted = ["structurizr.dsl", "structurizr.dslEditor"];
        let order = entry_order(&inserted);
        let emitted: Vec<&str> = order.iter().map(|&i| inserted[i]).collect();
        assert_eq!(emitted, ["structurizr.dsl", "structurizr.dslEditor"]);
    }

    #[test]
    fn java_string_hash_code_vectors() {
        // java.lang.String.hashCode values (jshell-verified constants).
        assert_eq!(string_hash_code(""), 0);
        assert_eq!(string_hash_code("a"), 97);
        assert_eq!(string_hash_code("abc"), 96_354);
        assert_eq!(string_hash_code("police"), -982_670_050);
        assert_eq!(string_hash_code("testing"), -1_422_446_064);
        // 32-bit wrap-around back to positive (jshell-verified):
        assert_eq!(string_hash_code("abcdefghijklmnopqrstuvwxyz"), 958_031_277);
    }

    #[test]
    fn surrogate_pairs_use_two_utf16_units() {
        // Java "😀".hashCode() == 1_772_899 (verified JDK 21 output):
        // surrogate pair D83D DE00, h = ((0*31 + 0xD83D) * 31) + 0xDE00.
        assert_eq!(string_hash_code("\u{1F600}"), 1_772_899);
    }

    #[test]
    fn spread_matches_java() {
        // `h ^ (h >>> 16)` — jshell-verified (JDK 21, 2026-11-14).
        assert_eq!(spread(0), 0);
        assert_eq!(spread(96_354), 96_355);
        assert_eq!(spread(-982_670_050), -982_687_629);
        assert_eq!(spread(-1_422_446_064), -1_422_419_673);
        assert_eq!(spread(-1), -65_536); // 0xFFFFFFFF ^ 0x0000FFFF
    }

    #[test]
    fn duplicate_put_keeps_first_position() {
        // "a" is bucket 1, "b" bucket 2 (cap 16); the re-put of "a" updates
        // its value in place, so iteration order is unaffected: a, b.
        let inserted = ["a", "b", "a"];
        let order = entry_order(&inserted);
        let emitted: Vec<&str> = order.iter().map(|&i| inserted[i]).collect();
        assert_eq!(emitted, ["a", "b"]);
    }

    #[test]
    fn capacity_doubles_at_load_factor() {
        // 12 distinct single-char keys: bucket distinctness aside, the order
        // must be a permutation of ALL keys and deterministic (property tests
        // pin the emulator harder).
        let keys: Vec<String> = (b'a'..=b'l').map(|c| (c as char).to_string()).collect();
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let mut order = entry_order(&refs);
        assert_eq!(order.len(), 12);
        order.sort_unstable();
        assert_eq!(order, (0..12).collect::<Vec<_>>());
    }
}
