//! Chain section: reference `StructurizrDslParser.java` arms at Java lines
//! 281–347 — the multi-line comment state machine, `}` (context end),
//! `!include` (with the full `IncludeParser` port), and the JVM-only
//! `!plugin`/`!script` (both rejected: OXZ-5002, D2).

use std::path::{Path, PathBuf};

use oxidizr_diag::codes;

use super::ctx::Ctx;
use super::error::DslError;
use super::{Parser, Sec, Stmt, eqi, tok};
use crate::compat;

/// The reference `Features` are ALL enabled in the parser ctor (V-3), so
/// `!plugin`/`!script` execute real JVM code there; oxidizr rejects both
/// (D2). The reference's disabled-feature text (`<token> is not permitted`)
/// is kept as the message tail so the shapes stay comparable.
fn jvm_only(token: &str) -> DslError {
    DslError::coded(
        codes::JVM_ONLY,
        format!(
            "{token} is not supported by oxidizr (JVM-only directive, D2): {token} is not permitted"
        ),
    )
}

pub(crate) fn section(p: &mut Parser, stmt: &Stmt) -> Sec {
    let first = stmt.first();
    let trimmed = compat::java_trim(&stmt.raw);

    // 281: multi-line comment fully contained on the line — ignore.
    if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
        return Sec::Handled;
    }
    // 283: comment block starts here (rest of the line ignored).
    if first.starts_with("/*") {
        p.start_context(Ctx::Comment);
        return Sec::Handled;
    }
    // 286/289: inside a comment block.
    if p.in_ctx(|c| matches!(c, Ctx::Comment)) {
        if trimmed.ends_with("*/") {
            return p.end_context().into();
        }
        return Sec::Handled; // swallowed
    }
    // 292: `}` — end the current context.
    if stmt.tokens.get(0) == tok::CONTEXT_END {
        return p.end_context().into();
    }
    // 295: !include <file|directory|url>
    if eqi(&first, tok::INCLUDE_FILE) {
        return p.include(stmt).into();
    }
    // 311: !plugin (JVM) — rejected; the PluginDslContext parameter arm
    // (318) is unreachable because the context is never pushed.
    if eqi(&first, tok::PLUGIN) {
        return Sec::Err(jvm_only(&first));
    }
    // 327: !script (JVM) — rejected; likewise the external-script
    // parameter arm (345).
    if eqi(&first, tok::SCRIPT) {
        return Sec::Err(jvm_only(&first));
    }

    Sec::No
}

impl Parser<'_> {
    /// Chain 295–304 + `IncludeParser.parse` + the recursion
    /// (`parse(paddedLines, file, true, false)` — fragments are not
    /// source-tracked; per-line errors inside report their own file/line).
    fn include(&mut self, stmt: &Stmt) -> Result<(), DslError> {
        const GRAMMAR: &str = "!include <file|directory|url>";
        const SOURCE_INDEX: usize = 1;
        if stmt.tokens.has_more_than(SOURCE_INDEX) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {GRAMMAR}"
            )));
        }
        if !stmt.tokens.includes(SOURCE_INDEX) {
            return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
        }
        let source = stmt.tokens.get(SOURCE_INDEX);

        // Phase 1 policy (D5 hardened, DEVIATIONS §network): ALL network
        // input is rejected (OXZ-5001); the reference's feature-gate texts
        // are kept verbatim.
        if source.starts_with("https://") {
            return Err(DslError::coded(
                codes::NET_DISABLED,
                "Includes via HTTPS are not permitted",
            ));
        }
        if source.starts_with("http://") {
            return Err(DslError::coded(
                codes::NET_DISABLED,
                "Includes via HTTP are not permitted",
            ));
        }

        // File-system include (`Features.FILE_SYSTEM` is on). Java skips
        // the whole branch when dslFile == null: a fragment parse silently
        // IGNORES file includes — not even portability changes.
        let Some(dsl_file) = self.current_file_raw() else {
            return Ok(());
        };
        let base = Path::new(&dsl_file)
            .parent()
            .map_or_else(PathBuf::new, PathBuf::from);
        let path = base.join(&source);
        if !path.exists() {
            return Err(DslError::stmt(format!(
                "{} could not be found",
                java_canonical(&path)
            )));
        }
        let files = read_files(&path).map_err(|e| {
            DslError::stmt(format!("Error including {}: {e}", java_absolute(&path)))
        })?;
        // `context.setDslPortable(false)` — on the TOP context, only AFTER
        // the reads succeeded.
        if let Some(top) = self.frame_portable.last_mut() {
            *top = false;
        }

        // Chain 296: `line.substring(0, line.indexOf("!include"))` —
        // everything before the FIRST `!include` occurrence in the logical
        // line pads every included line (they are re-tokenized, so an
        // `id = x ` prefix on the including statement is re-applied to each
        // included line — a reference quirk, preserved).
        let leading = &stmt.raw[..stmt.raw.find(tok::INCLUDE_FILE).unwrap_or(0)];
        for file in files {
            let Ok(bytes) = std::fs::read(&file) else {
                return Err(DslError::stmt(format!(
                    "Error reading file at {}: stream read failed",
                    java_absolute(&file)
                )));
            };
            // Files.readAllLines(UTF_8): invalid bytes become U+FFFD, no
            // trailing empty line for a final terminator.
            let text = String::from_utf8_lossy(&bytes);
            let padded: Vec<String> = read_all_lines(&text)
                .into_iter()
                .map(|unpadded| {
                    let unpadded = unpadded.strip_prefix('\u{feff}').unwrap_or(&unpadded);
                    format!("{leading}{unpadded}")
                })
                .collect();
            self.last_include_path = Some(file.display().to_string());
            self.parse_included(&padded.join("\n"));
        }
        Ok(())
    }
}

/// Java `File.getAbsolutePath()`: lexical cwd-join (the reference only
/// prints it when `isFile()` holds of the path it formats).
pub(crate) fn java_absolute(path: &Path) -> String {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    joined.to_string_lossy().into_owned()
}

/// Java `File.getCanonicalPath()` — used only in the "could not be found"
/// text, where the path does NOT exist, so `std::fs::canonicalize` is
/// unusable. Lexical approximation: cwd-join, drop `.` segments, each `..`
/// pops one segment (no symlink resolution; symlink-free corpora match).
pub(crate) fn java_canonical(path: &Path) -> String {
    let mut parts: Vec<std::path::PathBuf> = Vec::new();
    for comp in Path::new(&java_absolute(path)).components() {
        use std::path::Component as C;
        match comp {
            C::CurDir => {}
            C::ParentDir => {
                parts.pop();
            }
            c => parts.push(PathBuf::from(c.as_os_str())),
        }
    }
    let mut out = PathBuf::new();
    for p in &parts {
        out.push(p);
    }
    out.to_string_lossy().into_owned()
}

/// `IncludeParser.readFiles`: skip hidden/dot entries, recurse directories
/// in `Arrays.sort` (path-string) order, read plain files as UTF-8 lines.
fn read_files(path: &Path) -> Result<Vec<PathBuf>, DslError> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.starts_with('.') {
        return Ok(Vec::new()); // Java isHidden()/startsWith(".")
    }
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = match std::fs::read_dir(path) {
            Ok(rd) => rd.filter_map(Result::ok).map(|e| e.path()).collect(),
            Err(_) => return Ok(Vec::new()), // listFiles() == null
        };
        entries.sort();
        let mut out = Vec::new();
        for e in entries {
            out.extend(read_files(&e)?);
        }
        return Ok(out);
    }
    Ok(vec![path.to_path_buf()])
}

/// Java `Files.readAllLines`: split on line terminators, no trailing empty
/// string after a final terminator; `\r\n` collapses.
fn read_all_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    if lines.last().is_some_and(std::string::String::is_empty) {
        lines.pop();
    }
    lines
}
