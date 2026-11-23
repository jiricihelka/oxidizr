//! Doc-reference checker (`cargo xtask refs`): every internal path reference and
//! `file.md §Section` pointer in the repo's tracked text files must resolve.
//!
//! Rationale: the corpus has been re-pathed twice (`plan/` → `agents/plan/`, the
//! AGENTS.md charter split), and each move needed a hand-audit for stale pointers.
//! This walks the tree and checks three rules mechanically:
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
//! 3. **Human-text hygiene** (`check_hygiene`) — shipped crates' `src/` carries
//!    no process ids and no implementation-provenance narration in comments or
//!    string literals. Dev harnesses, `tests/` and the `agents/` corpus are out
//!    of scope by design: they keep the provenance.
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

// --- Human-text hygiene leg -------------------------------------------------
//
// Shipped sources are user-visible prose (rustdoc, `--help`, UI strings): they
// state facts, never process ids or implementation-history narration — the
// provenance lives in the `agents/` corpus and the git history.
//
// Scope: `crates/oxidizr-*/src/**/*.rs` except the dev-only `oxidizr-e2e`
// harness. Out of scope by construction: `tests/` (quotes pinned corpus text
// on purpose), `agents/` (the provenance register itself) and `xtask` (keeps
// its checker-grammar examples).

/// True for shipped-crate sources (see the scope note above).
fn is_hygiene_target(rel: &str) -> bool {
    let ext_ok = Path::new(rel)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"));
    rel.starts_with("crates/oxidizr-")
        && !rel.starts_with("crates/oxidizr-e2e/")
        && rel.contains("/src/")
        && ext_ok
}

/// Split a line at its first out-of-string `//`: (code part, comment part).
fn split_comment(line: &str) -> (String, String) {
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '/' && line[i..].starts_with("//") {
            return (line[..i].to_string(), line[i..].to_string());
        }
    }
    (line.to_string(), String::new())
}

/// Bodies of the double-quoted literals in the code part (line-local
/// heuristic: escapes are skipped, raw-string edges are treated loosely).
fn string_literals(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            let mut lit = String::new();
            i += 1;
            while i < chars.len() {
                match chars[i] {
                    '\\' => {
                        i += 1;
                        if i < chars.len() {
                            lit.push(chars[i]);
                        }
                    }
                    '"' => break,
                    c => lit.push(c),
                }
                i += 1;
            }
            out.push(lit);
        }
        i += 1;
    }
    out
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Process-id families in one text span: `§P` pointers, `P4-D2`/`P3-M8`
/// phase-task ids, `M<n>` milestones, `O-/N-/V-<n>` verification ids,
/// `T<n>-<x>` task ids, `D10`/`D11` deferred items, `Phase <n>` mentions and
/// the `2R R-`/`R-<area>-<n>` review ids. `f_tags` additionally enables the
/// bare `F1`..`F19` feature tags — comments only, because a string-literal
/// `F1` is fixture data (e.g. a view key), not prose.
fn process_id(text: &str, f_tags: bool) -> Option<&'static str> {
    if text.contains("§P") {
        return Some("a section-pointer id");
    }
    if text.contains("2R R-") {
        return Some("a refactor id");
    }
    if ["R-mis-", "R-tool-", "R-design-", "R-ui-", "R-fuzz-"]
        .iter()
        .any(|w| text.contains(*w))
    {
        return Some("a review id");
    }
    let cs: Vec<char> = text.chars().collect();
    let n = cs.len();
    let at = |i: usize| cs.get(i).copied();
    let digit = |c: char| c.is_ascii_digit();
    let mut i = 0;
    while i < n {
        if !is_word(cs[i]) || (i > 0 && is_word(cs[i - 1])) {
            i += 1;
            continue;
        }
        let c = cs[i];
        // dashed families first (a word run would stop at the `-`):
        if c == 'P' && at(i + 1).is_some_and(|d| ('1'..='4').contains(&d)) && at(i + 2) == Some('-')
        {
            let third = at(i + 3);
            let after = if matches!(third, Some('D' | 'M' | 'T' | 'R')) {
                at(i + 4)
            } else {
                third
            };
            if after.is_some_and(digit) {
                return Some("a phase-task id");
            }
        }
        if matches!(c, 'O' | 'N' | 'V') && at(i + 1) == Some('-') && at(i + 2).is_some_and(digit) {
            let mut j = i + 3;
            if at(j).is_some_and(digit) {
                j += 1;
            }
            if !at(j).is_some_and(is_word) {
                return Some("a verification id");
            }
        }
        if c == 'T'
            && at(i + 1).is_some_and(digit)
            && at(i + 2) == Some('-')
            && at(i + 3).is_some_and(|c| c.is_ascii_lowercase())
            && !at(i + 4).is_some_and(is_word)
        {
            return Some("a task id");
        }
        if c == 'P'
            && cs[i..].starts_with(&['P', 'h', 'a', 's', 'e', ' '])
            && at(i + 6).is_some_and(digit)
        {
            return Some("a phase mention");
        }
        // word-run families:
        let mut j = i;
        while j < n && is_word(cs[j]) {
            j += 1;
        }
        let run: String = cs[i..j].iter().collect();
        if let Some(d) = run.strip_prefix('M')
            && !d.is_empty()
            && d.len() <= 2
            && d.chars().all(digit)
        {
            return Some("a milestone id");
        }
        if run == "D10" || run == "D11" {
            return Some("a deferred-item id");
        }
        if f_tags
            && let Some(d) = run.strip_prefix('F')
            && d.chars().all(digit)
            && ((d.len() == 1 && d != "0") || (d.len() == 2 && d.starts_with('1')))
        {
            return Some("a feature tag");
        }
        i = j;
    }
    None
}

/// Reference-implementation narration in one text span (provenance belongs to
/// the plan corpus, not to shipped prose).
fn narration(text: &str) -> Option<&'static str> {
    let low = text.to_lowercase();
    for (pat, label) in [
        (
            "reference implementation",
            "reference-implementation narration",
        ),
        ("structurizr-monolith", "reference-repo narration"),
        ("ported from", "port provenance"),
    ] {
        if low.contains(pat) {
            return Some(label);
        }
    }
    // `the reference` with a trailing word boundary: bare prose like
    // "the referenced-image set" is not narration.
    let mut from = 0;
    while let Some(p) = low[from..].find("the reference") {
        let end = from + p + "the reference".len();
        if !low[end..].chars().next().is_some_and(is_word) {
            return Some("reference narration");
        }
        from = end;
    }
    if let Some(p) = low.find(".java") {
        let end = p + ".java".len();
        if !low.as_bytes()[end..]
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        {
            return Some("a Java-source citation");
        }
    }
    None
}

/// What this source line's human text violates (comment + string literals),
/// honoring the per-line `refs:ignore` escape hatch.
fn hygiene_line(line: &str) -> Option<&'static str> {
    if line.contains(IGNORE_DIRECTIVE) {
        return None;
    }
    let (code, comment) = split_comment(line);
    if !comment.is_empty() {
        if let Some(w) = process_id(&comment, true).or_else(|| narration(&comment)) {
            return Some(w);
        }
    }
    for lit in string_literals(&code) {
        if let Some(w) = process_id(&lit, false).or_else(|| narration(&lit)) {
            return Some(w);
        }
    }
    None
}

/// Walk `crates/` and flag process ids / provenance narration in the human
/// text of shipped crates (scope: [`is_hygiene_target`]).
pub fn check_hygiene(root: &Path) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    let crates_dir = root.join("crates");
    if !crates_dir.is_dir() {
        return Ok(findings);
    }
    let mut stack = vec![crates_dir];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
                continue;
            }
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            if !is_hygiene_target(&rel) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue; // binary/unreadable: skip
            };
            for (idx, line) in text.lines().enumerate() {
                if let Some(what) = hygiene_line(line) {
                    findings.push(Finding {
                        file: rel.clone(),
                        line: idx + 1,
                        what: format!("human text carries {what}"),
                    });
                }
            }
        }
    }
    Ok(findings)
}

pub fn cmd_refs() -> Result<()> {
    let root = crate::repo_root()?;
    let mut findings = check_tree(&root, &[])?;
    findings.extend(check_hygiene(&root)?);
    if findings.is_empty() {
        println!("refs: all doc references resolve");
        println!("refs: shipped human text is process-id and provenance clean");
        return Ok(());
    }
    for f in &findings {
        println!("{}:{}: {}", f.file, f.line, f.what);
    }
    anyhow::bail!("refs: {} finding(s)", findings.len());
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
    fn hygiene_flags_process_ids_in_comments() {
        for l in [
            "//! `diff` — workspace-aware model diff (F3).",
            "/// Pan: the plain-drag behavior of M1, unchanged.",
            "// The `!docs`/`!adrs` importer registry (P4-M2f)",
            "//! `oxidizr-api` in T0-c because the command bodies",
            "/// V-8 (resolved): UTF-8 only at v0.1",
            "//! the F12 fidelity bar is the semantic model",
            "// per the P2-M1 decision register",
            "// see §P3-M4 for the design row",
            "// 2R R-tool-04 depth scan",
            "// pinned as R-mis-7",
            "// deferred to Phase 4",
            "// deferred item D11 stays deferred",
        ] {
            assert!(hygiene_line(l).is_some(), "must flag: {l}");
        }
    }

    #[test]
    fn hygiene_flags_narration_in_comments_and_strings() {
        for l in [
            "/// Element style in `ElementStyle.java` declaration order:",
            "let m = \"image import needs a file (the reference reads the bytes)\";",
            "ui.weak(\"Hide tagged items (absent mode in the reference)\");",
            "s.push_str(\"ported from the reference implementation\");",
            "//! mirrored from Structurizr-Monolith",
        ] {
            assert!(hygiene_line(l).is_some(), "must flag: {l}");
        }
    }

    #[test]
    fn hygiene_passes_clean_prose_and_fixture_data() {
        for l in [
            "/// Compare two workspaces (`diff`): both sides load via `open`.",
            "// the official tooling reads the bytes off disk",
            "base: ViewBase::new(\"F1\", 2),",
            "assert_eq!(scene.view_key, \"F2\");",
            "let css = \"#00FF11\";",
            "let url = \"https://structurizr.com/dsl\";",
            "let v = Map::<String, String>::new(); // HashMap order is contract",
            "fn f32_bits() {}",
            "// planned forward reference refs:ignore",
        ] {
            assert_eq!(hygiene_line(l), None, "must pass: {l}");
        }
    }

    #[test]
    fn shipped_tree_is_hygiene_clean() {
        // self-gating: the strip waves landed, so HEAD must pass its own leg.
        let root = crate::repo_root().unwrap();
        let findings = check_hygiene(&root).unwrap();
        assert_eq!(findings, Vec::new(), "hygiene violations: {findings:?}");
    }

    #[test]
    fn real_tree_is_clean() {
        // self-gating: the repo itself must pass (replaces the hand audits).
        let root = crate::repo_root().unwrap();
        let findings = check_tree(&root, &[]).unwrap();
        assert_eq!(findings, Vec::new(), "unresolved: {findings:?}");
    }
}
