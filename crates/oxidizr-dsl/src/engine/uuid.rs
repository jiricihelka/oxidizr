//! Dependency-free UUID v4 strings (the two reference uses of
//! `UUID.randomUUID()` in the DSL pipeline: anonymous identifier registration
//! and text-block constant names).
//!
//! The VALUES are volatile — elements registered without a DSL identifier get
//! `structurizr.dsl.identifier: <uuid>` properties whose exact bytes can never
//! be golden-compared against the reference (that is why `--strip-volatile`
//! exists, `plan/ADDITIONAL_FEATURES.md` F-). Only the FORMAT is pinned:
//! lowercase hex, `xxxxxxxx-xxxx-4xxx-y[89ab]xxx-xxxxxxxxxxxx`.
//!
//! Non-cryptographic (xorshift64* over a nanos+counter seed): std-only per the
//! portability rule, and nothing here needs entropy an adversary can't guess.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_random() -> u64 {
    let seed_base = {
        // Truncation is intended: this is an entropy seed, not a value
        // that must survive intact (uuid4 only needs the bits mixed).
        #[allow(clippy::cast_possible_truncation)]
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        // Mix in the address of a stack local for cross-process spread
        // (the reference's process-unique randomness is equally unmodelled).
        let local = 0_u8;
        let addr = std::ptr::from_ref(&local) as u64;
        nanos ^ counter.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ addr.rotate_left(17)
    };
    let mut x = if seed_base == 0 {
        0x2545_F491_4F6C_DD1D
    } else {
        seed_base
    };
    // xorshift64*
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1F)
}

/// A fresh UUID v4 string (lowercase, hyphenated) — the shape Java's
/// `UUID.randomUUID().toString()` produces.
#[must_use]
pub fn uuid_v4() -> String {
    let hi = next_random();
    let mut lo = next_random();
    // version 4 nibble = MSN bits 60-63 = byte 7's HIGH nibble (hex digit 14,
    // the first digit after the 2nd dash); variant `10xx` = LSN bits 62-63
    // (hex digit 19, the first digit after the 3rd dash).
    let hi = (hi & 0xFFFF_FFFF_FFFF_0FFF) | 0x0000_0000_0000_4000;
    lo = (lo & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    let bytes = [hi.to_be_bytes(), lo.to_be_bytes()].concat();
    let hex = |b: u8| -> [char; 2] {
        let digits = [
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
        ];
        [digits[usize::from(b >> 4)], digits[usize::from(b & 0x0F)]]
    };
    let mut out = String::with_capacity(36);
    for (i, &b) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        for c in hex(b) {
            out.push(c);
        }
    }
    out
}

/// Reference `IdentifiersRegister` UUID probe:
/// `[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}`.
#[must_use]
pub fn looks_like_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let mut parts = s.split('-');
    for want in groups {
        match parts.next() {
            Some(p)
                if p.len() == want
                    && p.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()) => {}
            _ => return false,
        }
    }
    parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape_and_distinctness() {
        let a = uuid_v4();
        let b = uuid_v4();
        assert_eq!(a.len(), 36);
        assert!(looks_like_uuid(&a), "{a}");
        assert!(looks_like_uuid(&b));
        assert_ne!(a, b);
        // format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(&a[14..15], "4", "{a}");
    }

    #[test]
    fn uuid_version_and_variant_digits() {
        for _ in 0..50 {
            let u = uuid_v4();
            assert_eq!(&u[14..15], "4", "{u}");
            assert!(matches!(&u[19..20], "8" | "9" | "a" | "b"), "{u}");
        }
    }

    #[test]
    fn uuid_probe_rejects_other_shapes() {
        assert!(!looks_like_uuid("ci"));
        assert!(!looks_like_uuid("ZZ233376-e37a-4847-8801-4255fc5348f8"));
        assert!(!looks_like_uuid("d8233376-e37a-4847-8801-4255fc5348f"));
        assert!(looks_like_uuid("d8233376-e37a-4847-8801-4255fc5348f8"));
    }
}
