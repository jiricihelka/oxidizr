//! M6-b `fmt` gates over the vendored corpus (P1-D7 invariants):
//! (1) `parse(fmt(x))` builds the identical workspace to `parse(x)` —
//! byte-compared through the JSON writer with volatile UUID values redacted
//! (`structurizr.dsl.identifier` auto-ids are `UUID.randomUUID()` per parse,
//! `engine/uuid.rs`); (2) `fmt(fmt(x)) == fmt(x)`.
//!
//! Plus proptest properties over ARBITRARY sources: the lexer is infallible, so
//! every string formats — formatting must be idempotent and preserve the
//! sequence of classified logical lines and their token values (the exact
//! input the reference parse loop consumes).

use std::path::{Path, PathBuf};

use oxidizr_core::Workspace;
use oxidizr_dsl::compat;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};
use oxidizr_dsl::fmt;
use oxidizr_dsl::lexer;
use oxidizr_dsl::preprocess;
use proptest::prelude::*;
use proptest::string::string_regex;

fn dsl_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

/// Every `.dsl` file in the corpus, recursively, sorted by path.
fn corpus() -> Vec<PathBuf> {
    let mut stack = vec![dsl_dir()];
    let mut out: Vec<PathBuf> = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "dsl") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn rel(path: &Path) -> String {
    path.strip_prefix(dsl_dir())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// The corpus member that is not valid UTF-8 (its own encoding test in
/// `m4_fixtures.rs`; `fmt` formats text, so it is skipped here).
fn latin1_only(relpath: &str) -> bool {
    relpath == "iso-8859.dsl"
}

fn json_loader(path: &str) -> Result<Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

fn file_opts(path: &str) -> ParseOptions<'_> {
    ParseOptions {
        file: Some(path),
        base_dir: None,
        json_loader: Some(json_loader),
        env_lookup: None,
    }
}

/// Volatile `UUID.randomUUID()` values (lowercase hyphenated v4) redacted so
/// two parses of the same content compare byte-equal.
fn redact_uuids(json: &[u8]) -> Vec<u8> {
    let hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    let seg = |w: &[u8], a: usize, b: usize| w[a..b].iter().all(|&c| hex(c));
    let mut out = Vec::with_capacity(json.len());
    let mut i = 0;
    while i < json.len() {
        let hit = json.len() >= i + 36 && {
            let w = &json[i..i + 36];
            w[8] == b'-'
                && w[13] == b'-'
                && w[18] == b'-'
                && w[23] == b'-'
                && seg(w, 0, 8)
                && seg(w, 9, 13)
                && seg(w, 14, 18)
                && seg(w, 19, 23)
                && seg(w, 24, 36)
        };
        if hit {
            out.extend_from_slice(b"<uuid>");
            i += 36;
        } else {
            out.push(json[i]);
            i += 1;
        }
    }
    out
}

fn workspace_bytes(ws: &Workspace) -> Vec<u8> {
    // The `structurizr.dsl` portability property IS the (base64) source, so it
    // legitimately differs between x and fmt(x) — P1-D7 invariant 3: it carries
    // the fresh source. Drop that line; redact the volatile UUIDs in the rest.
    const KEY: &[u8] = b"\"structurizr.dsl\"";
    let json = oxidizr_json::to_vec(ws).unwrap();
    let mut kept: Vec<u8> = Vec::with_capacity(json.len());
    let mut started = false;
    for l in json.split(|&b| b == b'\n') {
        if l.windows(KEY.len()).any(|w| w == KEY) {
            continue;
        }
        if started {
            kept.push(b'\n');
        }
        started = true;
        kept.extend_from_slice(l);
    }
    redact_uuids(&kept)
}

/// Every UTF-8 corpus member: `fmt` is idempotent.
#[test]
fn corpus_fmt_is_idempotent() {
    let mut checked = 0;
    for path in corpus() {
        let name = rel(&path);
        if latin1_only(&name) {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let once = fmt::format(&source);
        let twice = fmt::format(&once);
        assert_eq!(twice, once, "{name}: fmt is not idempotent");
        checked += 1;
    }
    assert!(checked >= 90, "corpus unexpectedly small: {checked}");
}

/// Invariant 1: `parse(fmt(x))` builds the identical workspace to `parse(x)`.
#[test]
fn corpus_fmt_preserves_workspace() {
    let mut checked = 0;
    for path in corpus() {
        let name = rel(&path);
        if latin1_only(&name) {
            continue;
        }
        let full = path.to_str().unwrap().to_string();
        let source = std::fs::read_to_string(&path).unwrap();
        let Ok(before) = parse_dsl(&source, file_opts(&full)) else {
            continue; // fatal parse: no workspace to compare (m4 register owns it)
        };
        let formatted = fmt::format(&source);
        let after = parse_dsl(&formatted, file_opts(&full)).unwrap_or_else(|e| {
            panic!(
                "{name}: parse of the FORMATTED source is fatal: {}",
                e.message
            )
        });
        if before.errors.is_empty() {
            assert!(
                after.errors.is_empty(),
                "{name}: formatting introduced errors: {:?}",
                after.errors[0].message
            );
        }
        let (want, got) = (
            workspace_bytes(&before.workspace),
            workspace_bytes(&after.workspace),
        );
        if got != want {
            let at = got
                .iter()
                .zip(&want)
                .position(|(a, b)| a != b)
                .unwrap_or(want.len());
            let win = |v: &[u8]| {
                String::from_utf8_lossy(&v[at.saturating_sub(120)..v.len().min(at + 120)])
                    .into_owned()
            };
            panic!(
                "{name}: workspace of fmt(x) differs from workspace of x at byte {at}\n--- fmt(x) ---\n{}\n--- x ---\n{}",
                win(&got),
                win(&want)
            );
        }
        checked += 1;
    }
    assert!(checked >= 90, "corpus unexpectedly small: {checked}");
}

/// The reference parse loop's input surface: classified logical lines
/// (blank lines carry no state and are excluded — `fmt` compacts them) with
/// their token values. Dropped EOF tails are excluded (never parsed).
fn logical_surface(source: &str) -> Vec<(char, Vec<String>)> {
    preprocess::preprocess(source)
        .iter()
        .filter(|l| !l.dropped)
        .filter_map(|l| {
            if l.text_block.is_none() && l.is_empty_line(source) {
                None
            } else if l.text_block.is_none() && l.is_comment_line(source) {
                Some(('#', vec![compat::java_trim(&l.joined(source)).to_owned()]))
            } else {
                let tokens = lexer::lex_line(source, l)
                    .tokens
                    .iter()
                    .map(|t| t.value(source))
                    .collect();
                Some(('c', tokens))
            }
        })
        .collect()
}

proptest! {
    /// `fmt` is total and idempotent on arbitrary input.
    #[test]
    fn arbitrary_fmt_is_idempotent(s in string_regex(DSLISH).unwrap()) {
        let once = fmt::format(&s);
        prop_assert_eq!(fmt::format(&once), once);
    }

    /// Formatting preserves the logical-line surface the parse loop consumes.
    #[test]
    fn arbitrary_fmt_preserves_logical_surface(s in string_regex(DSLISH).unwrap()) {
        let before = logical_surface(&s);
        let after = logical_surface(&fmt::format(&s));
        prop_assert_eq!(after, before);
    }
}

/// DSL-flavoured alphabet: token chars plus every byte the reference's
/// line machine reacts to (`"`, `{`, `}`, `\`, `/*`, `#`, `$`, newlines).
const DSLISH: &str = r#"[\"{}a-zA-Z$=/*\\ \t\n\r#]{0,400}"#;
