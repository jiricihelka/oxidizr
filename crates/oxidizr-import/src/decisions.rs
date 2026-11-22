//! The `!adrs` decision importers — ports of `structurizr-import`
//! `documentation/AdrToolsDecisionImporter`, `MadrDecisionImporter` and
//! `Log4brainsDecisionImporter` (the `AbstractDecisionImporter` excludes
//! matcher is shared with [`crate::docs`]). Full pinned contract:
//! `agents/plan/build/IMPORT-DOCS-ADRS.md`.
//!
//! Pinned reference behaviors (verified @ `ad34036`):
//! - all three are DIRECTORY-only: `"<abs> does not exist."` /
//!   `"<abs> is not a directory."` (`getAbsolutePath` — lexical), thrown
//!   OUTSIDE the importer's try-blocks (clean messages);
//! - content is UTF-8 (lossy), then `replace("\r", "")` before ANY
//!   parsing, and `Decision.setContent` newline-normalizes;
//! - Java `String.split("\\n")` drops TRAILING empty segments —
//!   `java_split_newlines` replicates it (leading empties stay);
//! - dates parse via lenient `SimpleDateFormat` in `TimeZone.getDefault()`
//!   — the port parses UTC (deviation D15), rolling out-of-range values
//!   over like a lenient `Calendar`, and ignores trailing text after the
//!   pattern;
//! - post-import, EVERY decision's content has each imported FILENAME
//!   replaced by `#` + urlEncode(id) (`URLEncoder`, spaces as `%20`; ids
//!   are digits ⇒ plain), and links resolve against the imported
//!   FILENAME set only (unknown targets silently drop, self-links no-op);
//! - failure wrapping decides the surfaced message: AdrTools/Log4brains
//!   wrap everything after the guards in `DocumentationImportException
//!   (cause)` (⇒ `cause.toString()`, e.g.
//!   `"java.lang.IllegalArgumentException: …"`); Madr wraps EACH FILE in
//!   `"Error importing decision from + <abs>"` (literal `+`) and its
//!   link/rewrite loop swallows failures (warn-log);
//! - the port SORTS every directory walk by name (`AdrTools`' `listFiles`
//!   is FS-order — non-deterministic in the reference; determinism note
//!   in the port contract).
//!
//! Empty `exclude` tokens (`"A regex must be provided."`) are a
//! DSL-surface concern checked at the statement edge, not here.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use oxidizr_core::docs::{Decision, Documentation, Format, Link};

use crate::docs::{excluded, read_utf8};
use crate::regexlite::Regex;
use crate::{ImportError, java_absolute, normalize_newlines};

/// Which decision importer the DSL selected (the `!adrs` DEFAULT is
/// [`DecisionKind::AdrTools`]; the others are alias- or FQN-reachable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    /// `AdrToolsDecisionImporter` — adr-tools (`npryce/adr-tools`).
    AdrTools,
    /// `MadrDecisionImporter` — MADR (`adr/madr`).
    Madr,
    /// `Log4brainsDecisionImporter` — Log4brains (`thomvaill/log4brains`).
    Log4brains,
}

/// `importDocumentation(documentable, path)` for the decision family.
/// `path` is the JOINED path (DSL dir + token); `excludes` are the
/// context's `exclude` tokens.
///
/// # Errors
///
/// [`ImportError::Failed`] carries the reference inner message EXACTLY as
/// `DocumentationDslContext.end()` would surface it after the
/// `"Error importing decisions: "` prefix — guards clean, wrapped causes
/// `toString()`-shaped (see the module docs).
pub fn import_decisions(
    kind: DecisionKind,
    target: &mut Documentation,
    path: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    if !path.exists() {
        return Err(ImportError::Failed(format!(
            "{} does not exist.",
            java_absolute(path).display()
        )));
    }
    if !path.is_dir() {
        return Err(ImportError::Failed(format!(
            "{} is not a directory.",
            java_absolute(path).display()
        )));
    }
    match kind {
        DecisionKind::AdrTools => adrtools(target, path, excludes),
        DecisionKind::Madr => madr(target, path, excludes),
        DecisionKind::Log4brains => log4brains(target, path, excludes),
    }
}

/// `listFiles(filter)` + the port's sorted-by-name walk (determinism).
/// Exactly one of `suffix` (`AdrTools` `endsWith(".md")`) / `keep` (the
/// regex filters) is used by the callers.
fn md_files(
    dir: &Path,
    keep: Option<&Regex>,
    suffix: Option<&str>,
) -> Result<Vec<PathBuf>, ImportError> {
    let mut out: Vec<PathBuf> = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|source| ImportError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in rd {
        let entry = entry.map_err(|source| ImportError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // `File.listFiles` yields directories too — a directory passing
        // the name filter would fail the reference's `Files.readString`
        // (wrapped `FileSystemException`); the port skips it (the corpus
        // never carries one and the skip is the deterministic reading).
        let matched = suffix.is_none_or(|s| name.ends_with(s))
            && keep.is_none_or(|re| re.is_match_full(&name));
        if matched && entry.path().is_file() {
            out.push(entry.path());
        }
    }
    out.sort();
    Ok(out)
}

/// `Documentation.addDecision` — the validation order that can actually
/// fire from these importers (id/title/content/format are always set;
/// status is never null). `Err` carries the `IllegalArgumentException`
// message; each importer decides the wrapping.
fn check_addable(target: &Documentation, d: &Decision) -> Result<(), String> {
    if d.title.is_empty() {
        return Err("A title must be specified.".into());
    }
    if target.decisions.iter().any(|x| x.id == d.id) {
        return Err(format!(
            "A decision with an ID of {} already exists in this scope.",
            d.id
        ));
    }
    Ok(())
}

/// `Integer.parseInt(name.substring(0, 4))` → id; the 4-char prefix is
/// what adr-tools guarantees; a bad prefix aborts the WHOLE import.
fn parse_id4(name: &str) -> Result<String, ImportError> {
    let prefix: String = name.chars().take(4).collect();
    match prefix.parse::<i64>() {
        Ok(v) => Ok(v.to_string()),
        Err(_) => Err(java_error(
            "NumberFormatException",
            &format!("For input string: \"{prefix}\""),
        )),
    }
}

/// `java.lang.<Class>: <message>` — the `DocumentationImportException
/// (cause)` wrapping seen through `e.getMessage()` at the DSL edge.
pub(crate) fn java_error(class: &str, message: &str) -> ImportError {
    ImportError::Failed(format!("java.lang.{class}: {message}"))
}

/// `String.split("\\n")` — trailing empty segments dropped; the empty
/// string splits to ONE empty segment, `"\n\n"` to NONE (pinned Java
/// behavior — limit 0 discards all-trailing empties, nothing to discard
/// when the input is empty).
pub(crate) fn java_split_newlines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut parts: Vec<&str> = s.split('\n').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// `#` + `URLEncoder.encode(id, UTF_8)` with `+` → `%20`. Ids are digits
/// in all three importers ⇒ the encode is a pass-through; spelled out
/// for fidelity.
fn calculate_url(id: &str) -> String {
    let mut out = String::from("#");
    for c in id.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '-' | '_' | '*' => out.push(c),
            ' ' => out.push_str("%20"),
            _ => {
                for b in c.encode_utf8(&mut [0u8; 4]).as_bytes() {
                    let _ = write!(out, "%{b:02X}");
                }
            }
        }
    }
    out
}

/// The post-import filename → `#id` content rewrite, INSERTION order
/// (the reference iterates a `HashMap` keySet — FS/hash order, non-
/// deterministic; sorted insertion is the port's registered choice).
fn rewrite_filenames(content: &mut String, by_filename: &[(String, String)]) {
    for (filename, id) in by_filename {
        *content = content.replace(filename, &calculate_url(id));
    }
}

/// `File.lastModified()` seconds (`new Date(ms)` truncated by the UTC
/// JSON formatter); 0 on failure, like Java.
fn mtime_secs(file: &Path) -> i64 {
    std::fs::metadata(file)
        .and_then(|m| m.modified())
        .map_or(0, |t| {
            t.duration_since(std::time::UNIX_EPOCH).map_or_else(
                |e| -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX / 2),
                |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX / 2),
            )
        })
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Lenient UTC dates (deviation D15: the reference parses in JVM-TZ)
// ---------------------------------------------------------------------------

/// Days since epoch for a (possibly normalized) civil date — Howard
/// Hinnant's `days_from_civil` (proleptic Gregorian, no year 0 issues at
/// the corpus range).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `SimpleDateFormat` lenient semantics: the numbers normalize (month 13
/// rolls into next January, day 40 rolls over); trailing junk after the
/// pattern is ignored by the parsers, missing structure fails to parse.
fn parse_lenient(y: i64, m: i64, d: i64) -> i64 {
    let m0 = m - 1;
    let y = y + m0.div_euclid(12);
    let m = m0.rem_euclid(12) + 1;
    (days_from_civil(y, m, 1) + d - 1) * 86_400
}

/// `yyyy-MM-dd` lenient: three digit runs split by `-`, trailing text
/// after the day ignored.
fn parse_yyyy_mm_dd(src: &str) -> Result<i64, ImportError> {
    let fail = || java_error("ParseException", &format!("Unparseable date: \"{src}\""));
    let buf: Vec<char> = src.chars().collect();
    let mut pos = 0_usize;
    let digits = |i: &mut usize| -> Option<String> {
        let start = *i;
        while *i < buf.len() && buf[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == start {
            None
        } else {
            Some(buf[start..*i].iter().collect())
        }
    };
    let y = digits(&mut pos).ok_or_else(fail)?;
    if buf.get(pos) != Some(&'-') {
        return Err(fail());
    }
    pos += 1;
    let m = digits(&mut pos).ok_or_else(fail)?;
    if buf.get(pos) != Some(&'-') {
        return Err(fail());
    }
    pos += 1;
    let d = digits(&mut pos).ok_or_else(fail)?;
    let (y, m, d) = (
        y.parse::<i64>().map_err(|_| fail())?,
        m.parse::<i64>().map_err(|_| fail())?,
        d.parse::<i64>().map_err(|_| fail())?,
    );
    Ok(parse_lenient(y, m, d))
}

/// `yyyyMMdd` lenient (Log4brains filenames — the filter guarantees the
/// digits, so this cannot fail on filtered input).
fn parse_yyyy_mm_dd_compact(s: &str) -> Result<i64, ImportError> {
    let c: Vec<char> = s.chars().collect();
    if c.len() < 8 || !c[..8].iter().all(char::is_ascii_digit) {
        return Err(java_error(
            "ParseException",
            &format!("Unparseable date: \"{s}\""),
        ));
    }
    let num = |r: std::ops::Range<usize>| {
        c[r].iter()
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or(i64::MIN)
    };
    Ok(parse_lenient(num(0..4), num(4..6), num(6..8)))
}

// ---------------------------------------------------------------------------
// AdrTools
// ---------------------------------------------------------------------------

const ADR_STATUS_HEADING: &str = "## Status";
const ADR_CONTEXT_HEADING: &str = "## Context";

fn adrtools(
    target: &mut Documentation,
    dir: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    let files = md_files(dir, None, Some(".md"))?;
    let mut order: Vec<usize> = Vec::new();
    let mut by_filename: Vec<(String, String)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for file in files {
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if excluded(&name, excludes)? {
            continue;
        }
        let d = adrtools_import_decision(&file, &name)?;
        if let Err(msg) = check_addable(target, &d) {
            return Err(java_error("IllegalArgumentException", &msg));
        }
        let idx = target.decisions.len();
        target.decisions.push(d);
        order.push(idx);
        by_filename.push((name.clone(), target.decisions[idx].id.clone()));
        index.insert(name, idx);
    }
    let link_re = Regex::new(r"(.*) \[.*]\((.*)\)").expect("static");
    for &idx in &order {
        // extractLinks — the ## Status section up to ## Context, ONE
        // link per line (`if (matcher.find())`, not `while`)
        let content = target.decisions[idx].content.clone();
        let self_id = target.decisions[idx].id.clone();
        let mut links = Vec::new();
        let mut in_status = false;
        for line in java_split_newlines(&content) {
            if !in_status {
                if line.starts_with(ADR_STATUS_HEADING) {
                    in_status = true;
                }
            } else if line.starts_with(ADR_CONTEXT_HEADING) {
                break;
            } else if !line.is_empty() {
                if let Some(m) = link_re.find(line, 0) {
                    let desc = m.group(line, 1).unwrap_or_default().to_owned();
                    let tgt = m.group(line, 2).unwrap_or_default();
                    if let Some(&ti) = index.get(tgt) {
                        let tid = target.decisions[ti].id.clone();
                        if tid != self_id {
                            links.push((tid, desc));
                        }
                    }
                }
            }
        }
        for (tid, desc) in links {
            target.decisions[idx].links.push(Link {
                id: tid,
                description: desc,
            });
        }
        rewrite_filenames(&mut target.decisions[idx].content, &by_filename);
    }
    Ok(())
}

fn adrtools_import_decision(file: &Path, name: &str) -> Result<Decision, ImportError> {
    let id = parse_id4(name)?;
    let raw = read_utf8(file)?;
    let content = raw.replace('\r', "");
    let lines = java_split_newlines(&content);
    let title = adrtools_title(lines.first().copied().unwrap_or(""))?;
    let date = adrtools_date(&lines)?;
    let status = adrtools_status(&lines);
    Ok(Decision {
        id,
        title,
        status,
        date,
        links: Vec::new(),
        content: normalize_newlines(&content),
        format: Some(Format::Markdown),
        element_id: None,
    })
}

/// `titleLine.substring(titleLine.indexOf(".") + 2)` — no dot ⇒ drops
/// the first character; too short ⇒ `StringIndexOutOfBoundsException`.
fn adrtools_title(line: &str) -> Result<String, ImportError> {
    let begin = line.find('.').map_or(1, |dot| dot + 2);
    if begin > line.len() {
        return Err(java_error(
            "StringIndexOutOfBoundsException",
            &format!("String index out of range: {begin}"),
        ));
    }
    Ok(line[begin..].to_owned())
}

fn adrtools_date(lines: &[&str]) -> Result<i64, ImportError> {
    for line in lines {
        if let Some(rest) = line.strip_prefix("Date: ") {
            return parse_yyyy_mm_dd(rest);
        }
    }
    // `new Date()` — NOW; fixtures always carry dates (determinism note)
    Ok(now_secs())
}

fn adrtools_status(lines: &[&str]) -> String {
    let mut in_status = false;
    for line in lines {
        if !in_status {
            if line.starts_with(ADR_STATUS_HEADING) {
                in_status = true;
            }
        } else if !line.is_empty() {
            let mut status = line.split(' ').next().unwrap_or_default();
            if status == "Superceded" {
                status = "Superseded";
            }
            return status.to_owned();
        }
    }
    "Proposed".into()
}

// ---------------------------------------------------------------------------
// Madr
// ---------------------------------------------------------------------------

const MADR_FRONT_MATTER_SEPARATOR: &str = "---";
const MADR_DEFAULT_STATUS: &str = "accepted";

fn madr(target: &mut Documentation, dir: &Path, excludes: &[String]) -> Result<(), ImportError> {
    let re = Regex::new(r"\d{4}-.+?.md").expect("static");
    let files = md_files(dir, Some(&re), None)?;
    let mut order: Vec<usize> = Vec::new();
    let mut by_filename: Vec<(String, String)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for file in files {
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if excluded(&name, excludes)? {
            continue;
        }
        // the per-file wrap: `DocumentationImportException(message,
        // cause)` — `getMessage()` is the message ONLY (the literal `+`,
        // the cause text never surfaces at the DSL edge)
        let abs = java_absolute(&file).display().to_string();
        let wrap = || ImportError::Failed(format!("Error importing decision from + {abs}"));
        let d = madr_import_decision(&file, &name).map_err(|_| wrap())?;
        if check_addable(target, &d).is_err() {
            return Err(wrap());
        }
        let idx = target.decisions.len();
        target.decisions.push(d);
        order.push(idx);
        by_filename.push((name.clone(), target.decisions[idx].id.clone()));
        index.insert(name, idx);
    }
    // pass 2 — Madr SWALLOWS failures here (warn-log); the port's
    // extraction cannot fail, so the loop runs unguarded, honestly.
    let link_re = Regex::new(r"\[.*]\((.*)\)").expect("static");
    for &idx in &order {
        let content = target.decisions[idx].content.clone();
        let self_id = target.decisions[idx].id.clone();
        let mut links = Vec::new();
        for line in java_split_newlines(&content) {
            for m in link_re.find_iter(line) {
                let tgt = m.group(line, 1).unwrap_or_default();
                if let Some(&ti) = index.get(tgt) {
                    let tid = target.decisions[ti].id.clone();
                    if tid != self_id {
                        links.push(tid);
                    }
                }
            }
        }
        for tid in links {
            target.decisions[idx].links.push(Link {
                id: tid,
                description: "Links to".into(),
            });
        }
        rewrite_filenames(&mut target.decisions[idx].content, &by_filename);
    }
    Ok(())
}

fn madr_import_decision(file: &Path, name: &str) -> Result<Decision, ImportError> {
    let id = parse_id4(name)?;
    let raw = read_utf8(file)?;
    let content = raw.replace('\r', "");
    let stripped = strip_front_matter_once(&content);
    let lines_with = java_split_newlines(&content);
    let lines_without = java_split_newlines(&stripped);

    let title = lines_without
        .iter()
        .find(|l| l.starts_with("# "))
        .map_or_else(|| "Title".to_owned(), |l| l[2..].to_owned());

    // mtime first, front-matter `date:` wins when present
    let mut date = mtime_secs(file);
    if let Some(d) = madr_front_matter_value(&lines_with, "date: ") {
        date = parse_yyyy_mm_dd(d)?;
    }
    let status = madr_front_matter_value(&lines_with, "status: ")
        .unwrap_or(MADR_DEFAULT_STATUS)
        .to_owned();

    Ok(Decision {
        id,
        title,
        status,
        date,
        links: Vec::new(),
        content: normalize_newlines(&stripped),
        format: Some(Format::Markdown),
        element_id: None,
    })
}

/// The front-matter walk: only when the FIRST line starts `---`; stops
/// at the next such line (`None` = default).
fn madr_front_matter_value<'a>(lines: &'a [&'a str], prefix: &str) -> Option<&'a str> {
    if !lines
        .first()
        .is_some_and(|l| l.starts_with(MADR_FRONT_MATTER_SEPARATOR))
    {
        return None;
    }
    for line in &lines[1..] {
        if line.starts_with(MADR_FRONT_MATTER_SEPARATOR) {
            return None;
        }
        if let Some(rest) = line.strip_prefix(prefix) {
            return Some(rest);
        }
    }
    None
}

/// Port of `content.replaceFirst("(?m)^---\n(^.*\n)*?---\n", "")`:
/// the EARLIEST line-start `---\n` that has a LATER line-start `---\n`
/// (zero or more lines between) anchors the removal of both delimiter
/// lines plus everything between. Not engine-fed (`(?m)` is out of
/// `regexlite` scope) — scanned procedurally, documented so.
fn strip_front_matter_once(content: &str) -> String {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let mut offsets: Vec<usize> = Vec::with_capacity(lines.len() + 1);
    let mut off = 0_usize;
    for l in &lines {
        offsets.push(off);
        off += l.len();
    }
    for o in 0..lines.len() {
        if lines[o] != "---\n" {
            continue;
        }
        for c in o + 1..lines.len() {
            if lines[c] == "---\n" {
                let start = offsets[o];
                let end = offsets[c] + lines[c].len();
                let mut out = String::with_capacity(content.len());
                out.push_str(&content[..start]);
                out.push_str(&content[end..]);
                return out;
            }
        }
    }
    content.to_owned()
}

// ---------------------------------------------------------------------------
// Log4brains
// ---------------------------------------------------------------------------

const L4B_DATE_PREFIX: &str = "- Date: ";
const L4B_STATUS_PREFIX: &str = "- Status: ";
const L4B_LINKS_HEADING: &str = "## Links";

fn log4brains(
    target: &mut Documentation,
    dir: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    let re = Regex::new(r"\d{4}\d{2}\d{2}-.+?.md").expect("static");
    let files = md_files(dir, Some(&re), None)?;
    let mut order: Vec<usize> = Vec::new();
    let mut by_filename: Vec<(String, String)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut decision_id = 1_i64;
    for file in files {
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if excluded(&name, excludes)? {
            continue;
        }
        let d = log4brains_import_decision(decision_id, &file, &name)?;
        if let Err(msg) = check_addable(target, &d) {
            return Err(java_error("IllegalArgumentException", &msg));
        }
        let idx = target.decisions.len();
        target.decisions.push(d);
        order.push(idx);
        by_filename.push((name.clone(), target.decisions[idx].id.clone()));
        index.insert(name, idx);
        // the counter advances ONLY inside the import branch (pinned)
        decision_id += 1;
    }
    let status_re = Regex::new(r"- Status: (.*) \[.*]\((.*)\)").expect("static");
    let link_re = Regex::new(r"- (.*) \[.*]\((.*)\)").expect("static");
    for &idx in &order {
        let content = target.decisions[idx].content.clone();
        let self_id = target.decisions[idx].id.clone();
        let mut links: Vec<(String, String)> = Vec::new();
        let mut in_links = false;
        for line in java_split_newlines(&content) {
            if line.starts_with(L4B_STATUS_PREFIX) {
                if let Some(m) = status_re.find(line, 0) {
                    let desc = m.group(line, 1).unwrap_or_default().to_owned();
                    let tgt = m.group(line, 2).unwrap_or_default();
                    if let Some(&ti) = index.get(tgt) {
                        let tid = target.decisions[ti].id.clone();
                        if tid != self_id {
                            links.push((tid, desc));
                        }
                    }
                }
            }
            if line.starts_with(L4B_LINKS_HEADING) {
                in_links = true;
            }
            if in_links {
                if let Some(m) = link_re.find(line, 0) {
                    let desc = m.group(line, 1).unwrap_or_default().to_owned();
                    let tgt = m.group(line, 2).unwrap_or_default();
                    if let Some(&ti) = index.get(tgt) {
                        let tid = target.decisions[ti].id.clone();
                        if tid != self_id {
                            links.push((tid, desc));
                        }
                    }
                }
            }
        }
        for (tid, desc) in links {
            target.decisions[idx].links.push(Link {
                id: tid,
                description: desc,
            });
        }
        rewrite_filenames(&mut target.decisions[idx].content, &by_filename);
    }
    Ok(())
}

fn log4brains_import_decision(id: i64, file: &Path, name: &str) -> Result<Decision, ImportError> {
    let raw = read_utf8(file)?;
    let content = raw.replace('\r', "");
    let lines = java_split_newlines(&content);

    // the filename date (`yyyyMMdd` on the first 8 chars) is the base;
    // a `- Date: ` line overrides it
    let head: String = name.chars().take(8).collect();
    let mut date = parse_yyyy_mm_dd_compact(&head)?;
    for line in &lines {
        if let Some(rest) = line.strip_prefix(L4B_DATE_PREFIX) {
            date = parse_yyyy_mm_dd(rest)?;
            break;
        }
    }

    // title: `lines[0].substring(2)` — NO `# ` check (pinned)
    let first = lines.first().copied().unwrap_or("");
    if first.len() < 2 {
        return Err(java_error(
            "StringIndexOutOfBoundsException",
            "String index out of range: 2",
        ));
    }
    let title = first[2..].to_owned();

    let mut status = String::new();
    for line in &lines {
        if let Some(rest) = line.strip_prefix(L4B_STATUS_PREFIX) {
            status = if rest.starts_with("superseded") {
                "superseded".into()
            } else {
                rest.to_owned()
            };
            break;
        }
    }

    Ok(Decision {
        id: id.to_string(),
        title,
        status,
        date,
        links: Vec::new(),
        content: normalize_newlines(&content),
        format: Some(Format::Markdown),
        element_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::tests::scratch;

    const ADRT_1: &str = concat!(
        "# 1. Record architecture decisions\n",
        "\n",
        "Date: 2016-02-12\n",
        "\n",
        "## Status\n",
        "\n",
        "Accepted\n",
        "\n",
        "## Context\n",
        "\n",
        "We need a lightweight format.\n"
    );
    const ADRT_2: &str = concat!(
        "# 2. Use shell scripts\n",
        "\n",
        "Date: 2016-02-13\n",
        "\n",
        "## Status\n",
        "\n",
        "Superceded by [0001-record.md](0001-record.md)\n",
        "\n",
        "## Context\n",
        "\n",
        "See also [the first ADR](0001-record.md).\n"
    );

    fn utc(y: i64, m: i64, d: i64) -> i64 {
        days_from_civil(y, m, d) * 86_400
    }

    #[test]
    fn adrtools_parses_id_title_date_status() {
        let dir = scratch(
            "adr-tools",
            &[
                ("0001-record.md", ADRT_1),
                ("0002-use-shell-scripts.md", ADRT_2),
            ],
        );
        let mut doc = Documentation::default();
        import_decisions(DecisionKind::AdrTools, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.decisions.len(), 2);
        let d = &doc.decisions[0];
        assert_eq!(d.id, "1");
        assert_eq!(d.title, "Record architecture decisions");
        assert_eq!(d.date, utc(2016, 2, 12));
        assert_eq!(d.status, "Accepted");
        // `Superceded` normalizes; the link description carries the line
        // prefix and the status-section link resolves against FILENAMES
        let d = &doc.decisions[1];
        assert_eq!(d.status, "Superseded");
        assert_eq!(d.links.len(), 1);
        assert_eq!(d.links[0].id, "1");
        assert_eq!(d.links[0].description, "Superceded by");
        // content rewrite: filename -> #id (both mentions, incl. the
        // context-section link target)
        assert!(!d.content.contains("0001-record.md"));
        assert!(d.content.contains("[the first ADR](#1)"));
        // status-section links stop at ## Context — the context link is
        // NOT a decision link (only 1 above) but its TEXT was rewritten
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn adrtools_status_defaults_and_missing_date() {
        // no ## Status section content and no `Date:` line: status
        // `Proposed` (capital P — pinned constant), date = NOW
        let dir = scratch("adr-defaults", &[("0003-x.md", "# 3. X\n\nplain\n")]);
        let mut doc = Documentation::default();
        import_decisions(DecisionKind::AdrTools, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.decisions[0].status, "Proposed");
        assert!(doc.decisions[0].date > utc(2024, 1, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn adrtools_duplicate_id_aborts_with_wrapped_message() {
        let dir = scratch(
            "adr-dup",
            &[
                ("0004-a.md", ADRT_1),
                ("0004-b.md", ADRT_1.replace("1.", "4.").as_str()),
            ],
        );
        let mut doc = Documentation::default();
        let err = import_decisions(DecisionKind::AdrTools, &mut doc, &dir, &[]).expect_err("dup");
        assert_eq!(
            err.to_string(),
            "java.lang.IllegalArgumentException: A decision with an ID of 4 already exists in this scope."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn madr_strips_front_matter_and_links_to() {
        let body = concat!(
            "---\n",
            "date: 2020-05-04\n",
            "status: proposed\n",
            "---\n",
            "\n",
            "# Use MADR\n",
            "\n",
            "See [the first](0001-first.md) record.\n"
        );
        let dir = scratch(
            "madr",
            &[
                ("0001-first.md", "---\ndate: 2020-05-01\n---\n\n# First\n"),
                ("0002-madr.md", body),
                ("notes.md", "filtered out\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_decisions(DecisionKind::Madr, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.decisions.len(), 2);
        let d = &doc.decisions[1];
        assert_eq!(d.id, "2");
        assert_eq!(d.title, "Use MADR");
        assert_eq!(d.date, utc(2020, 5, 4));
        assert_eq!(d.status, "proposed");
        assert!(!d.content.starts_with("---\n"));
        assert_eq!(d.links.len(), 1);
        assert_eq!(d.links[0].id, "1");
        assert_eq!(d.links[0].description, "Links to");
        assert!(d.content.contains("(#1)"));
        // file 1: status defaults to `accepted` (no `status:` key)
        assert_eq!(doc.decisions[0].status, "accepted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn madr_title_and_status_defaults_without_front_matter() {
        let dir = scratch("madr-plain", &[("0007-p.md", "# Seven\n\nbody\n")]);
        let mut doc = Documentation::default();
        import_decisions(DecisionKind::Madr, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.decisions[0].title, "Seven");
        assert_eq!(doc.decisions[0].status, "accepted");
        // no front-matter date ⇒ file mtime (pinned): compare loosely
        let mtime = mtime_secs(&dir.join("0007-p.md"));
        assert_eq!(doc.decisions[0].date, mtime);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn madr_per_file_failure_uses_the_plus_wrap() {
        // unparseable front-matter date -> the literal-plus message
        // (cause text NOT included — message-only constructor)
        let dir = scratch(
            "madr-bad",
            &[("0001-x.md", "---\ndate: not-a-date\n---\n\n# X\n")],
        );
        let mut doc = Documentation::default();
        let err = import_decisions(DecisionKind::Madr, &mut doc, &dir, &[]).expect_err("bad date");
        let msg = err.to_string();
        assert!(
            msg.starts_with("Error importing decision from + ")
                && msg.ends_with("0001-x.md")
                && !msg.contains("ParseException"),
            "{msg}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn front_matter_strip_is_replace_first_semantics() {
        // `(?m)^---\n(^.*\n)*?---\n` + replaceFirst: the EARLIEST opening
        // pairs with its EARLIEST close (lines between are consumed) —
        // here the line-2 `---` closes at the line-5 `---`, so the
        // `date: 1`/`---` remainder survives.
        let s = "keep\n---\nno closing here\nbody\n---\ndate: 1\n---\nafter\n";
        let got = strip_front_matter_once(s);
        assert_eq!(got, "keep\ndate: 1\n---\nafter\n");
        // an opening with NO close anywhere ⇒ no removal at all
        assert_eq!(
            strip_front_matter_once("only one\n---\nbody\n"),
            "only one\n---\nbody\n"
        );
        // consecutive delimiters (zero lines between) strip fine
        assert_eq!(strip_front_matter_once("---\n---\nx\n"), "x\n");
        // `---` at EOF without a trailing newline never closes
        assert_eq!(strip_front_matter_once("---\nfoo\n---"), "---\nfoo\n---");
    }

    #[test]
    fn log4brains_sequential_ids_status_and_links() {
        let d1 = concat!(
            "# 20240110 first title\n",
            "\n",
            "- Status: superseded by [20240111-second.md](20240111-second.md)\n",
            "- Date: 2024-01-12\n",
            "\n",
            "Context here.\n",
            "\n",
            "## Links\n",
            "\n",
            "- Relates to [20240111-second.md](20240111-second.md)\n"
        );
        let d2 = "# 20240111 second title\n\n- Status: accepted\n";
        let dir = scratch(
            "l4b",
            &[
                ("20240110-first.md", d1),
                ("20240111-second.md", d2),
                ("README.md", "filtered\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_decisions(DecisionKind::Log4brains, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.decisions.len(), 2);
        let d = &doc.decisions[0];
        assert_eq!(d.id, "1");
        assert_eq!(
            d.title,
            "20240111 first title".replacen("20240111", "20240110", 1)
        );
        // the filename date is OVERRIDDEN by `- Date: `
        assert_eq!(d.date, utc(2024, 1, 12));
        assert_eq!(d.status, "superseded");
        // status-line link + `## Links` section link, both resolve
        assert_eq!(d.links.len(), 2);
        assert_eq!(d.links[0].id, "2");
        assert_eq!(d.links[0].description, "superseded by");
        assert_eq!(d.links[1].description, "Relates to");
        // ids are SEQUENTIAL from 1 (not filename-derived)
        assert_eq!(doc.decisions[1].id, "2");
        assert_eq!(doc.decisions[1].date, utc(2024, 1, 11));
        assert_eq!(doc.decisions[1].status, "accepted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log4brains_excluded_files_do_not_burn_ids() {
        let dir = scratch(
            "l4b-skip",
            &[
                ("20240110-a.md", "# a\n\n- Status: accepted\n"),
                ("20240111-b.md", "# b\n"),
                ("20240112-c.md", "# c\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_decisions(
            DecisionKind::Log4brains,
            &mut doc,
            &dir,
            &["20240111-b.md".into()],
        )
        .expect("import");
        let ids: Vec<&str> = doc.decisions.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["1", "2"]); // b is skipped WITHOUT advancing
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guard_messages_match_the_reference() {
        let mut doc = Documentation::default();
        let missing = std::path::Path::new("/nonexistent-oxidizr-p4m2f-adr");
        let err = import_decisions(DecisionKind::AdrTools, &mut doc, missing, &[]).expect_err("x");
        assert!(err.to_string().ends_with(" does not exist."), "{err}");

        let dir = scratch("adr-file", &[("notdir.md", "x\n")]);
        let err = import_decisions(DecisionKind::Madr, &mut doc, &dir.join("notdir.md"), &[])
            .expect_err("file");
        assert!(err.to_string().ends_with(" is not a directory."), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lenient_dates_roll_over_and_ignore_trailing_text() {
        assert_eq!(parse_yyyy_mm_dd("2020-05-04").unwrap(), utc(2020, 5, 4));
        assert_eq!(parse_yyyy_mm_dd("2020-13-01").unwrap(), utc(2021, 1, 1));
        assert_eq!(parse_yyyy_mm_dd("2020-02-30").unwrap(), utc(2020, 3, 1));
        assert_eq!(
            parse_yyyy_mm_dd("2020-05-04 at noon").unwrap(),
            utc(2020, 5, 4)
        );
        assert!(parse_yyyy_mm_dd("05-04").is_err());
        assert_eq!(
            parse_yyyy_mm_dd_compact("20240111-x.md").unwrap(),
            utc(2024, 1, 11)
        );
    }

    #[test]
    fn java_split_drops_trailing_empties_only() {
        assert_eq!(java_split_newlines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(java_split_newlines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(java_split_newlines(""), vec![""]);
        assert_eq!(java_split_newlines("\n\n"), Vec::<String>::new());
    }
}
