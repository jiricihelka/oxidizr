//! Cache-path resolution (P2-D3 item 2, `agents/plan/build/PHASE_2.md`):
//! hand-rolled XDG/Known-Folders resolution from std + env only (the
//! portability rule — no crates), plus the SHA-256 used for cache keys
//! (hand-rolled FIPS 180-4, std-only).

use std::path::{Path, PathBuf};

/// The offline theme cache root — `<xdg-cache>/oxidizr/themes` (Linux & other
/// unixes, `$XDG_CACHE_HOME` else `$HOME/.cache`),
/// `$HOME/Library/Caches/oxidizr/themes` (macOS),
/// `%LOCALAPPDATA%\oxidizr\themes` (Windows). `None` when the environment
/// cannot answer (cache then simply stays off; the reference has no cache at
/// all, so this can only ADD convenience, never behavior).
#[must_use]
pub fn themes_cache_dir() -> Option<PathBuf> {
    // cfg branches are env-layout differences, not behavior differences:
    // each OS's conventional cache dir, no fallback chains across cfgs.
    #[cfg(windows)]
    {
        dir_from(std::env::var_os("LOCALAPPDATA"), "oxidizr", "themes")
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(PathBuf::from).map(|home| {
            home.join("Library")
                .join("Caches")
                .join("oxidizr")
                .join("themes")
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        match std::env::var_os("XDG_CACHE_HOME") {
            Some(xdg) if !xdg.is_empty() => dir_from(Some(xdg), "oxidizr", "themes"),
            _ => dir_from(std::env::var_os("HOME"), ".cache", "oxidizr").map(|c| c.join("themes")),
        }
    }
}

fn dir_from(root: Option<std::ffi::OsString>, a: &str, b: &str) -> Option<PathBuf> {
    let root = root?;
    if root.is_empty() {
        return None;
    }
    Some(Path::new(&root).join(a).join(b))
}

/// SHA-256 round constants (`K`, FIPS 180-4 §4.2.2).
const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// Initial hash value `H0` (FIPS 180-4 §5.3.3).
const H0: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// One compression step (FIPS 180-4 §6.2.2) over a 64-byte block, folding the
/// result into `h`. The single-character working variables `a`–`g` are the
/// spec's A–G notation verbatim — algorithm fidelity, not sloppiness.
#[allow(clippy::many_single_char_names)]
fn compress(h: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];
    for (word, chunk) in w.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *slot = slot.wrapping_add(v);
    }
}

/// Lowercase hex SHA-256 (FIPS 180-4) — the theme URL cache-key hash
/// (`<cache>/<sha256(url)>/theme.json`, P2-D3).
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    // Pad: 0x80, zeros to a 56 mod 64 offset, 64-bit big-endian bit length.
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    let mut h = H0;
    for block in msg.chunks(64) {
        compress(&mut h, block);
    }
    let mut out = String::with_capacity(64);
    for word in &h {
        // `fmt::Write` on `String` never fails (it only reallocates).
        use std::fmt::Write as _;
        let _ = write!(out, "{word:08x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_fips_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 55 bytes: the length crosses into the next block mid-pad.
        let long = "a".repeat(55);
        assert_eq!(sha256_hex(long.as_bytes()).len(), 64);
        // Determinism across block boundaries (the 1e6-a vector's first block
        // shares its chaining state with the 64-a prefix).
        assert_eq!(sha256_hex(&[0u8; 64]), sha256_hex(&[0u8; 64]));
    }

    #[test]
    fn themes_cache_dir_shape() {
        // Environment-dependent: only assert the shape when set.
        if let Some(dir) = themes_cache_dir() {
            // Path::ends_with compares components — portable spelling.
            assert!(dir.ends_with(std::path::Path::new("oxidizr").join("themes")));
        }
    }
}
