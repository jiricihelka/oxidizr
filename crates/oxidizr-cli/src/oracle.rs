//! The shared `oracle` engine (§P4-M5 E-1, F4, P4-D6): reference-binary
//! discovery, the JSON export prologue, and golden normalization. It backs
//! BOTH the `oxidizr oracle gen|verify` user command and the `cargo xtask
//! oracle …` dev legs — xtask stays a dev shim (P4-D6) and delegates here,
//! so the normalization rules live exactly once. The reference CLI is an
//! EXTERNAL prerequisite (`$OXIDZR_STRUCTURIZR_BIN` → PATH); oxidizr never
//! downloads anything (D5).

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use oxidizr_api::{ExitClass, Issue, LogItem, Outcome};
use oxidizr_diag::codes;

/// What `oxidizr oracle` does per workspace (the clap arg maps onto this).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Write the normalized reference output to the golden file.
    Gen,
    /// Re-run the reference and byte-compare against the committed golden.
    Verify,
}

/// Engine errors, CLASSIFIED because the exit family splits on the class
/// (§P4-M5 E-1): discovery/spawn/I-O abort with exit 3, while a reference
/// run that merely FAILS on one workspace is that leg's `error` status
/// (exit 1, with the report).
#[derive(Debug)]
pub enum OracleError {
    /// Binary not found, or an explicit `--bin` that is not a file.
    NotFound(String),
    /// Spawn/exec or filesystem failure (exit class 3).
    Io(String),
    /// The reference ran and rejected the workspace (leg-level `error`).
    Failed(String),
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            OracleError::NotFound(message)
            | OracleError::Io(message)
            | OracleError::Failed(message) => message,
        };
        f.write_str(message)
    }
}

impl std::error::Error for OracleError {}

/// Reference CLI discovery: an explicit `--bin` (a non-file is a hard
/// miss — explicit means explicit), else `$OXIDZR_STRUCTURIZR_BIN`, else
/// `structurizr` on PATH (the `.bat`/`.cmd` wrappers cover Windows). The
/// message stays xtask-verbatim (agents/plan/TESTING.md documents setup).
///
/// # Errors
///
/// [`OracleError::NotFound`] when no candidate resolves to a file.
pub fn find_oracle(explicit: Option<&Path>) -> Result<PathBuf, OracleError> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(OracleError::NotFound(format!(
            "`--bin` is not a file: {}",
            path.display()
        )));
    }
    if let Some(explicit) = std::env::var_os("OXIDZR_STRUCTURIZR_BIN") {
        let path = PathBuf::from(&explicit);
        if !path.is_file() {
            return Err(OracleError::NotFound(format!(
                "$OXIDZR_STRUCTURIZR_BIN={}",
                explicit.to_string_lossy()
            )));
        }
        return Ok(path);
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        // Windows ships .bat/.cmd wrappers; plain name covers unix.
        for name in ["structurizr", "structurizr.bat", "structurizr.cmd"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(OracleError::NotFound(
        "structurizr reference binary not found (--bin → $OXIDZR_STRUCTURIZR_BIN → PATH); \
         it is an external prerequisite the nix dev flake provides \
         (agents/plan/TESTING.md) — oxidizr never downloads it"
            .to_string(),
    ))
}

/// The oracle export prologue (the create-dirs + run + exit-status leg;
/// moved from `xtask` unchanged — the dev shim now delegates here). The
/// failure message names the format on every leg.
///
/// # Errors
///
/// [`OracleError::Io`] on directory/spawn failure, [`OracleError::Failed`]
/// when the reference exits non-zero.
pub fn run_export(
    oracle: &Path,
    workspace: &Path,
    outdir: &Path,
    format: &str,
) -> Result<(), OracleError> {
    std::fs::create_dir_all(outdir)
        .map_err(|e| OracleError::Io(format!("failed to create `{}`: {e}", outdir.display())))?;
    let output = Command::new(oracle)
        .arg("export")
        .arg("--workspace")
        .arg(workspace)
        .arg("--format")
        .arg(format)
        .arg("--output")
        .arg(outdir)
        .output()
        .map_err(|e| OracleError::Io(format!("failed to run {}: {e}", oracle.display())))?;
    if !output.status.success() {
        return Err(OracleError::Failed(format!(
            "structurizr {format} export failed ({}):\n{}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

/// `export --format json`: the reference writes `<stem>.json` into the
/// output dir; return the produced path (moved from `xtask`).
///
/// # Errors
///
/// As [`run_export`], plus [`OracleError::Io`] when the expected output
/// file is missing or the workspace path has no usable stem.
pub fn reference_export_json(
    oracle: &Path,
    workspace: &Path,
    outdir: &Path,
) -> Result<PathBuf, OracleError> {
    run_export(oracle, workspace, outdir, "json")?;
    let stem = workspace
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| OracleError::Io("fixture without usable file stem".to_string()))?;
    let produced = outdir.join(format!("{stem}.json"));
    if !produced.is_file() {
        return Err(OracleError::Io(format!(
            "expected output missing: {}",
            produced.display()
        )));
    }
    Ok(produced)
}

/// Golden normalization: drop the four violation-counter properties the reference
/// **application harness** injects into `workspace.properties` on every load
/// (`AbstractCommand.loadWorkspace` → `new DefaultInspector(workspace)`;
/// `structurizr-inspection/DefaultInspector.java`). They are harness artifacts, not
/// workspace content — oxidizr models them as ordinary (round-tripped) properties but
/// never computes counts (that would mean reimplementing `structurizr-inspection`).
/// Verified against 2026.10.26 (2026-11-14, `agents/plan/PROGRESS.md`).
///
/// Line deletion must fix up the preceding comma when a removed entry was the object's
/// LAST member (the reference pretty-prints one entry per line, so a comma-less removed
/// line means the previous line's trailing comma must go too); when the removed entries
/// were the object's ONLY members, the emptied `"properties" : { }` shell goes too — the
/// `NON_EMPTY` writer can never produce an empty properties object, so its braces are a
/// deletion artifact (M5: `workspace extends` clears the retained source, leaving the
/// inspection entries alone). Note the export output has NO volatile members
/// (`lastModifiedDate*` only round-trips from JSON inputs, deterministically) — nothing
/// else is stripped.
#[must_use]
pub fn normalize_reference_output(raw: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    // Byte parity includes EOF: the reference pretty-printer emits NO trailing
    // newline; `lines()` would invent one (M0 goldens carried that artifact).
    let raw_trailing_newline = text.ends_with('\n');
    let drop_trailing_comma = |lines: &mut [String]| {
        // removed entry was the last in its object: drop the previous comma
        if let Some(last) = lines.last_mut()
            && let Some(pos) = last.rfind(',')
            && last[pos + 1..].chars().all(char::is_whitespace)
        {
            last.remove(pos);
        }
    };
    // Pass 1: delete the inspection entry lines (comma fix-up included).
    let mut kept: Vec<String> = Vec::new();
    for line in text.lines() {
        let t = line.trim_start();
        let removed = (t.starts_with("\"structurizr.inspection.error\"")
            || t.starts_with("\"structurizr.inspection.warning\"")
            || t.starts_with("\"structurizr.inspection.info\"")
            || t.starts_with("\"structurizr.inspection.ignore\""))
            && t.contains("\" :");
        if removed {
            if !t.ends_with(',') {
                drop_trailing_comma(&mut kept);
            }
            continue;
        }
        kept.push(line.to_string());
    }
    // Pass 2: when the inspection entries were the ONLY members of the
    // workspace-level `properties` object (`workspace extends` clears the
    // retained source, M5), pass 1 emptied it — but the library writer uses
    // NON_EMPTY inclusion, so an EMPTY `"properties" : { }` can only be a
    // deletion artifact: drop the braces too (comma rules as in pass 1).
    let mut out_lines: Vec<String> = Vec::new();
    let mut i = 0;
    while i < kept.len() {
        let t = kept[i].trim_start();
        if t.ends_with("\"properties\" : {")
            && kept
                .get(i + 1)
                .is_some_and(|n| matches!(n.trim(), "}," | "}"))
        {
            if kept[i + 1].trim() == "}" {
                drop_trailing_comma(&mut out_lines);
            }
            i += 2;
            continue;
        }
        out_lines.push(std::mem::take(&mut kept[i]));
        i += 1;
    }
    let mut out = out_lines.join("\n");
    if raw_trailing_newline {
        out.push('\n');
    }
    out.into_bytes()
}

/// One workspace's outcome. `status` is the report vocabulary (§P4-M5 E-1:
/// `generated|match|mismatch|missing|error`); `io` marks the I/O class of
/// failure that raises the whole command's exit code to 3.
struct Leg {
    workspace: PathBuf,
    golden: PathBuf,
    status: &'static str,
    error: Option<String>,
    io: bool,
}

/// Run `gen`/`verify` over user fixture paths and return the [`Outcome`]
/// for the CLI `replay` (report = stdout payload; `--json` switches it to
/// the machine document; `--quiet` carries no bytes — exit code only).
/// Exit family: 0 all ok · 1 any mismatch/missing/error · 3 discovery,
/// spawn or I-O failure. Goldens are `<stem>.golden.json` next to the
/// workspace, or under `--goldens DIR`.
#[must_use]
pub fn run(
    action: Action,
    paths: &[PathBuf],
    bin: Option<&Path>,
    goldens: Option<&Path>,
    json: bool,
    quiet: bool,
) -> Outcome {
    let mut out = Outcome::default();
    let oracle = match find_oracle(bin) {
        Ok(path) => path,
        Err(e) => {
            out.exit = ExitClass::Io;
            out.log
                .push(LogItem::Issue(Issue::error(codes::CLI_IO, e.to_string())));
            return out;
        }
    };
    // pid + clock nonce: parallel invocations (and pid reuse after an old
    // run's cleanup) can never collide on the scratch tree.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let tmp = std::env::temp_dir().join(format!("oxidizr-oracle-{}-{nonce}", std::process::id()));
    let legs: Vec<Leg> = paths
        .iter()
        .enumerate()
        .map(|(i, ws)| one(&oracle, action, ws, goldens, &tmp.join(i.to_string())))
        .collect();
    let _ = std::fs::remove_dir_all(&tmp);
    out.exit = if legs.iter().any(|leg| leg.io) {
        ExitClass::Io
    } else if legs
        .iter()
        .any(|leg| !matches!(leg.status, "generated" | "match"))
    {
        ExitClass::Diagnostics
    } else {
        ExitClass::Ok
    };
    if !quiet {
        out.bytes = Some(if json {
            json_report(&legs)
        } else {
            human_report(action, &legs)
        });
    }
    out
}

/// One fixture path: prechecks (stem, existence, missing golden), the
/// export + normalize round, then the gen write or verify compare. The
/// per-leg scratch dir lives under the run's tmp dir (removed whole).
fn one(oracle: &Path, action: Action, ws: &Path, goldens: Option<&Path>, tmp: &Path) -> Leg {
    let _ = std::fs::remove_dir_all(tmp);
    let stem = ws
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    let golden = goldens
        .unwrap_or_else(|| ws.parent().unwrap_or(Path::new(".")))
        .join(format!("{stem}.golden.json"));
    let mut leg = Leg {
        workspace: ws.to_path_buf(),
        golden: golden.clone(),
        status: "error",
        error: None,
        io: false,
    };
    if stem.is_empty() {
        leg.error = Some("workspace path has no file stem".to_string());
        return leg;
    }
    if !ws.is_file() {
        leg.error = Some(format!(
            "cannot read `{}` (no such file or directory)",
            ws.display()
        ));
        leg.io = true;
        return leg;
    }
    if action == Action::Verify && !golden.is_file() {
        leg.status = "missing";
        return leg;
    }
    let fresh = match reference_export_json(oracle, ws, tmp)
        .and_then(|produced| {
            std::fs::read(&produced).map_err(|e| {
                OracleError::Io(format!("failed to read `{}`: {e}", produced.display()))
            })
        })
        .map(|raw| normalize_reference_output(&raw))
    {
        Ok(bytes) => bytes,
        Err(e) => {
            leg.error = Some(e.to_string());
            leg.io = !matches!(e, OracleError::Failed(_));
            return leg;
        }
    };
    leg.status = match action {
        Action::Gen => match golden
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&golden, &fresh))
        {
            Ok(()) => "generated",
            Err(e) => {
                leg.error = Some(format!("failed to write `{}`: {e}", golden.display()));
                leg.io = true;
                return leg;
            }
        },
        Action::Verify => match std::fs::read(&golden) {
            Ok(committed) if committed == fresh => "match",
            Ok(_) => "mismatch",
            Err(e) => {
                leg.error = Some(format!("failed to read `{}`: {e}", golden.display()));
                leg.io = true;
                return leg;
            }
        },
    };
    leg
}

fn human_report(action: Action, legs: &[Leg]) -> Vec<u8> {
    use std::fmt::Write as _;
    let word = match action {
        Action::Gen => "gen",
        Action::Verify => "verify",
    };
    let mut text = String::new();
    let mut ok = 0_usize;
    for leg in legs {
        match leg.status {
            "generated" => {
                ok += 1;
                let _ = writeln!(text, "generated {}", leg.golden.display());
            }
            "match" => {
                ok += 1;
                let _ = writeln!(text, "ok {}", leg.workspace.display());
            }
            "mismatch" => {
                let _ = writeln!(text, "MISMATCH {}", leg.workspace.display());
            }
            "missing" => {
                let _ = writeln!(text, "missing {}", leg.golden.display());
            }
            _ => {
                let _ = writeln!(
                    text,
                    "error {}: {}",
                    leg.workspace.display(),
                    leg.error.as_deref().unwrap_or("oracle failure")
                );
            }
        }
    }
    let _ = writeln!(text, "oracle {word}: {ok}/{} ok", legs.len());
    text.into_bytes()
}

fn json_report(legs: &[Leg]) -> Vec<u8> {
    use std::fmt::Write as _;
    let mut text = String::from("{\"schema\":1,\"results\":[");
    for (i, leg) in legs.iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        let _ = write!(
            text,
            "{{\"workspace\":\"{}\",\"golden\":\"{}\",\"status\":\"{}\"",
            esc(&leg.workspace.to_string_lossy()),
            esc(&leg.golden.to_string_lossy()),
            leg.status
        );
        if let Some(error) = &leg.error {
            let _ = write!(text, ",\"error\":\"{}\"", esc(error));
        }
        text.push('}');
    }
    text.push_str("]}\n");
    text.into_bytes()
}

/// The minimal JSON string escape (report payloads are our own strings —
/// paths and error texts; std-only, §P4-M5 E-1).
fn esc(s: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{OracleError, esc, find_oracle, normalize_reference_output};

    fn norm(s: &str) -> String {
        String::from_utf8(normalize_reference_output(s.as_bytes())).unwrap()
    }

    #[test]
    fn removes_middle_entries_untouched() {
        let src = "{\n  \"properties\" : {\n    \"a\" : \"1\",\n    \"structurizr.inspection.error\" : \"3\",\n    \"structurizr.inspection.info\" : \"0\",\n    \"z\" : \"9\"\n  }\n}\n";
        assert_eq!(
            norm(src),
            "{\n  \"properties\" : {\n    \"a\" : \"1\",\n    \"z\" : \"9\"\n  }\n}\n"
        );
    }

    #[test]
    fn preserves_missing_trailing_newline() {
        // The reference pretty-printer emits no trailing newline; goldens must
        // not invent one (byte parity includes EOF).
        let src = "{\n  \"a\" : \"1\"\n}";
        assert_eq!(norm(src), "{\n  \"a\" : \"1\"\n}");
    }

    #[test]
    fn removes_last_entry_and_fixes_comma() {
        // inspection entries sorted last (as in workspace-properties.dsl, 2026-11-14):
        // the surviving `structurizr.dslEditor` line must lose its trailing comma.
        let src = "{\n  \"properties\" : {\n    \"structurizr.dslEditor\" : \"false\",\n    \"structurizr.inspection.warning\" : \"0\"\n  }\n}\n";
        assert_eq!(
            norm(src),
            "{\n  \"properties\" : {\n    \"structurizr.dslEditor\" : \"false\"\n  }\n}\n"
        );
    }

    #[test]
    fn leaves_other_keys_alone() {
        let src = "{\n  \"structurizr.inspectionUrl\" : \"x\",\n  \"structurizr.dsl\" : \"y\"\n}\n";
        assert_eq!(norm(src), src);
    }

    #[test]
    fn explicit_bin_must_be_a_file() {
        let missing = std::env::temp_dir().join("oxidizr-oracle-unit-nope");
        let error = find_oracle(Some(&missing)).unwrap_err();
        assert!(matches!(error, OracleError::NotFound(_)), "{error:?}");
        assert!(error.to_string().contains("`--bin` is not a file"));
    }

    #[test]
    fn esc_covers_the_report_strings() {
        assert_eq!(esc("a\"b\\c\nd\te\u{1}"), "a\\\"b\\\\c\\nd\\te\\u0001");
    }
}
