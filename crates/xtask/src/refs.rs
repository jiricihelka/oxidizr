//! Doc-reference checker (`cargo xtask refs`): every internal path reference and
//! `file.md §Section` pointer in the repo's tracked text files must resolve.
//!
//! Rationale: the corpus has been re-pathed twice (`plan/` → `agents/plan/`, the
//! AGENTS.md charter split), and each move needed a hand-audit for stale pointers.
//! This walks the tree and checks two rules mechanically:
//!
//! 1. **Paths** — tokens like `agents/plan/TESTING.md` or `crates/oxidizr-core/`
//!    (anchored at the known roots) must exist, resolved from the repo root or the
//!    referencing file's own directory. Template names (`PHASE_N.md`, globs, `<…>`)
//!    and corpus-relative paths (`testdata/**` internals, `structurizr.github.io/...`
//!    docs paths) are out of scope by construction.
//! 2. **§pointers** — `NAME(.md)? §Heading-words` must name a file that resolves
//!    (agents/plan/, agents/plan/build/, agents/notes/, root) and a heading in it
//!    (case-insensitive substring, so `§Source of` matches `## Source of truth`).
//!    `§6`-style numbered pointers are skipped (criterion/WRITE-CATALOG numbering).
//!
//! Vendored `testdata/**` and `reports/**` are never scanned or touched.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;

/// Roots whose relative references must resolve (repo-local paths only).
const ROOTS: &[&str] = &["agents/", "crates/", "testdata/", "bench-baseline/"];

/// Directories never walked (vendored bytes, scratch, build output).
const SKIP_DIRS: &[&str] = &[".git", "target", "testdata", "reports", "node_modules"];

/// Extensions scanned as text.
const TEXT_EXT: &[&str] = &[
    "md",
    "rs",
    "toml",
    "nix",
    "yml",
    "yaml",
    "sh",
    "gitattributes",
    "gitignore",
];

/// Per-line escape hatch (`refs:ignore`, in each file's comment syntax): for
/// forward references to files a later milestone will create (e.g. `PHASE_2`'s
/// planned EXPORT-PROPERTIES.md) — not for hiding genuine drift.
const IGNORE_DIRECTIVE: &str = "refs:ignore";

#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub what: String,
}

/// Path-like token starting at a known root, ending in a known text/data extension
/// (or a trailing `/` for directories). ASCII-only body keeps prose/bidi out.
fn extract_paths(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && is_path_byte(bytes[i]) {
            i += 1;
        }
        if i > start && (start == 0 || !is_path_byte(bytes[start - 1])) {
            let tok = &line[start..i];
            if ROOTS.iter().any(|r| tok.starts_with(r)) {
                let tok = tok.trim_end_matches(['.', ',', ';', ':', ')', ']']);
                // template/glob mentions (`bench-baseline/<LABEL>/`, PHASE_N.md, globs)
                let template = tok.contains(['<', '>', '*', '…'])
                    || tok.contains("_N.")
                    || tok.contains("PHASE_N");
                let dir_like = tok.ends_with('/');
                let has_ext = Path::new(tok)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.contains('.'));
                if !template && (dir_like || has_ext) {
                    out.push(tok.to_string());
                }
            }
        }
        i += 1; // step past the non-path byte (also guarantees forward progress)
    }
    out
}

fn is_path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'.' | b'*' | b'<' | b'>')
}

/// `NAME(.md)? §Section words…` → (name, section-fragment). §N (numeric) skipped.
fn extract_section_refs(line: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = line;
    let mut offset = 0usize;
    while let Some(pos) = rest.find('§') {
        let abs = offset + pos;
        // name = preceding identifier ending just before optional spaces + §
        // (NOTE: `pos` is relative to `rest`; the prefix must come from `line`)
        let prefix = &line[..abs];
        let name_end = prefix.trim_end().len();
        let name_start = prefix[..name_end]
            .char_indices()
            .rfind(|(_, c)| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.')))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let name = &line[name_start..name_end];
        // section = up to 4 words / 40 bytes after §, trimmed of sentence punctuation
        let after = &line[abs + '§'.len_utf8()..];
        let after = after.strip_prefix(' ').unwrap_or(after);
        let mut sec = String::new();
        for w in after.split_whitespace().take(4) {
            let ends_clause = w.ends_with(',');
            let w = w.trim_end_matches([',', '.', ';', ':', ')', ']', '’', '"']);
            if w.is_empty() {
                break;
            }
            if !sec.is_empty() {
                sec.push(' ');
            }
            sec.push_str(w);
            if ends_clause || sec.len() > 40 {
                break;
            }
        }
        if !name.is_empty()
            && !sec.is_empty()
            && sec
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() && !c.is_ascii_digit())
        {
            out.push((name.to_string(), sec));
        }
        offset = abs + '§'.len_utf8();
        rest = &line[abs + '§'.len_utf8()..];
    }
    out
}

/// Resolve a doc name (`TESTING`, `TESTING.md`, `WRITE-CATALOG`, `AGENTS`, …)
/// against the doc directories; `None` = unknown name (silently skipped by caller).
fn resolve_named_doc(name: &str, from_dir: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let stem = name.trim_end_matches(".md");
    let fname = format!("{stem}.md");
    let mut dirs: Vec<PathBuf> = roots.to_vec();
    dirs.push(from_dir.to_path_buf());
    dirs.push(Path::new("agents/plan").to_path_buf());
    dirs.push(Path::new("agents/plan/build").to_path_buf());
    for d in dirs {
        let cand = d.join(&fname);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Case-insensitive, whitespace-insensitive check: does any heading of `file`
/// contain the fragment or one of its leading word-prefixes (so `§Source of`,
/// `§Code quality, native runners` and `§Where things are is the pointer` all
/// match their headings without exact-boundary luck)?
fn heading_matches(file: &Path, frag: &str) -> Result<bool> {
    let text = fs::read_to_string(file)?;
    let norm = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    let heads: Vec<String> = text
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(|l| norm(l.trim_start_matches('#').trim()))
        .collect();
    let words: Vec<&str> = frag.split_whitespace().collect();
    for n in (1..=words.len()).rev() {
        let f = norm(&words[..n].join(" "));
        if heads.iter().any(|h| h.contains(&f)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check one file's lines; returns findings. `base` = the root that rooted
/// relative tokens resolve against (the repo root, never the cwd — so the
/// `real_tree_is_clean` test passes whatever cwd `cargo test` uses);
/// `from_dir` = dirs for relative tokens and bare-name § lookups.
pub fn check_file(path: &Path, base: &Path, from_dir: &Path) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    // binary/unreadable: skip
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(findings);
    };
    let rel = path.to_string_lossy().replace('\\', "/");

    // fence tracking: skip fenced code blocks? kept simple — fences ARE scanned
    // (clone URLs etc. are anchored away by ROOTS).
    for (idx, line) in text.lines().enumerate() {
        if line.contains(IGNORE_DIRECTIVE) {
            continue;
        }
        for tok in extract_paths(line) {
            let dir_like = tok.ends_with('/');
            let ok = [base.to_path_buf(), from_dir.to_path_buf()]
                .iter()
                .any(|b| {
                    let p = b.join(&tok);
                    if dir_like { p.is_dir() } else { p.exists() }
                });
            if !ok {
                findings.push(Finding {
                    file: rel.clone(),
                    line: idx + 1,
                    what: format!("broken path reference `{tok}`"),
                });
            }
        }
        for (name, sec) in extract_section_refs(line) {
            let Some(target) = resolve_named_doc(&name, from_dir, &[base.to_path_buf()]) else {
                continue; // unknown NAME: not our business (avoids prose false positives)
            };
            if !heading_matches(&target, &sec)? {
                findings.push(Finding {
                    file: rel.clone(),
                    line: idx + 1,
                    what: format!("stale `§{sec}` pointer into {name}"),
                });
            }
        }
    }
    Ok(findings)
}

/// Walk the repo from `root`, check every text file. `skip_extra` = absolute prefixes.
pub fn check_tree(root: &Path, skip_extra: &[PathBuf]) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut seen: HashSet<PathBuf> = HashSet::new();
    while let Some(dir) = stack.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string());
        if let Some(n) = &name {
            if SKIP_DIRS.contains(&n.as_str()) {
                continue;
            }
        }
        if skip_extra.iter().any(|p| dir.starts_with(p)) {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| TEXT_EXT.contains(&e))
                && path.file_name().is_some_and(|f| f != "RESUME.md")
            {
                let from_dir = path.parent().unwrap_or(root).to_path_buf();
                findings.extend(check_file(&path, root, &from_dir)?);
            }
        }
    }
    Ok(findings)
}

pub fn cmd_refs() -> Result<()> {
    let root = crate::repo_root()?;
    let findings = check_tree(&root, &[])?;
    if findings.is_empty() {
        println!("refs: all doc references resolve");
        return Ok(());
    }
    for f in &findings {
        println!("{}:{}: {}", f.file, f.line, f.what);
    }
    anyhow::bail!("refs: {} broken reference(s)", findings.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_rooted_tokens_only() {
        let l = "see agents/plan/TESTING.md and crates/oxidizr-core/ plus dsl/71-language.md \
                 (docs) structurizr.github.io/dsl/x.md — `crates/oxidizr-dsl/src/{a,b}.rs` skipped";
        let got = extract_paths(l);
        assert!(got.contains(&"agents/plan/TESTING.md".to_string()));
        assert!(got.contains(&"crates/oxidizr-core/".to_string()));
        assert!(!got.iter().any(|t| t.contains("71-language")));
        assert!(!got.iter().any(|t| t.contains('{')));
    }

    #[test]
    fn templates_and_globs_are_ignored() {
        assert_eq!(
            extract_paths("build/PHASE_N.md entry"),
            Vec::<String>::new()
        );
        assert!(
            extract_paths("testdata/dsl/**/*.dsl")
                .iter()
                .all(|t| !t.contains('*'))
        );
    }

    #[test]
    fn section_refs_parsed_and_numeric_skipped() {
        let got = extract_section_refs("per agents/plan/WORKFLOW.md §Code quality, native runners");
        assert_eq!(got[0].1, "Code quality", "comma ends the heading");
        let got2 = extract_section_refs("WRITE-CATALOG §3.1 formulas; DEVIATIONS §Locked list");
        assert!(
            got2.iter().all(|(_, s)| !s.starts_with('3')),
            "got {got2:?}"
        );
        assert!(
            got2.iter()
                .any(|(n, s)| n == "DEVIATIONS" && s == "Locked list")
        );
        // `§…` (prose placeholder) is not a pointer.
        assert_eq!(
            extract_section_refs("all `AGENTS.md §…` references"),
            Vec::<(String, String)>::new()
        );
    }

    #[test]
    fn real_tree_is_clean() {
        // self-gating: the repo itself must pass (replaces the hand audits).
        let root = crate::repo_root().unwrap();
        let findings = check_tree(&root, &[]).unwrap();
        assert_eq!(findings, Vec::new(), "unresolved: {findings:?}");
    }
}
