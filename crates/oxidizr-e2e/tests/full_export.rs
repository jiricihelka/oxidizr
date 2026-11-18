//! Full-tier export invariants (P2T-D5): the export engine checked against
//! ITSELF over every export-applicable valid entry — self-consistency, immune
//! to future blessed deviations (P1T-D6 philosophy). Same env gate as
//! `full.rs` (`OXIDZR_E2E_FULL=1`, nightly `e2e.yml`).
//!
//! 1. **Determinism** — two independent exports of the same input through the
//!    same format produce the identical file set and byte-identical content
//!    (dialects must stay clock/random/path-free; extends V-T7 to the corpus).
//! 2. **Path parity** — `export(dsl) ≡ export(json2json(dsl))` per output
//!    file: the export twin of P1T's round-trip stability, catching DSL-vs-JSON
//!    engine drift (a violation is a REAL bug — witness verdict, fix the tool,
//!    never bless; V-T8 pre-probed this green before the invariant landed).
//! 3. **Format-fixpoint parity** — `export(fmt(x)) ≡ export(x)`: the formatter
//!    may move whitespace, never semantics the dialects render.
//!
//! Theme-context fidelity: the JSON/formatted replay runs from a scratch copy
//! of the entry directory (children + `themes/` included), so the reference
//! theme chain (`<workspace-parent>/themes`) and `!include`/`extends`
//! resolution see exactly what the original input sees.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use oxidizr_api::{Input, LogItem, Request, execute};
use oxidizr_e2e::corpus::{self, Entry, Kind};
use oxidizr_e2e::manifest::Manifest;

/// Skip-with-note unless the full tier is requested (same contract as full.rs).
fn full_tier() -> bool {
    if env::var("OXIDZR_E2E_FULL").is_ok() {
        true
    } else {
        eprintln!("skipped: set OXIDZR_E2E_FULL=1 to run the full tier (P2T-D5)");
        false
    }
}

/// Valid entries whose blessed sidecar carries an `export` section — the
/// applicability contract the runner enforces both ways (P2T-D2).
fn export_entries() -> Vec<Entry> {
    corpus::discover()
        .expect("corpus discovers")
        .into_iter()
        .filter(|e| e.kind == Kind::Valid)
        .filter(|e| {
            Manifest::read(&e.dir.join(oxidizr_e2e::manifest::FILE_NAME))
                .is_ok_and(|m| m.export.is_some())
        })
        .collect()
}

/// Fan `f` out over `items` (std-only), collecting `(label, Err(msg))`s
/// (same helper shape as `full.rs` — tests are separate binaries).
fn fan_out<T: Send + Clone, F>(items: &[T], f: F) -> Vec<String>
where
    F: Fn(&T) -> Result<(), String> + Send + Sync,
{
    let f = &f;
    let mut results: Vec<Option<String>> = vec![None; items.len()];
    std::thread::scope(|scope| {
        let slots: Vec<(&T, &mut Option<String>)> = items.iter().zip(results.iter_mut()).collect();
        for (item, slot) in slots {
            let item = item.clone();
            scope.spawn(move || {
                if let Err(e) = f(&item) {
                    *slot = Some(e);
                }
            });
        }
    });
    results.into_iter().flatten().collect()
}

fn codes_of(out: &oxidizr_api::Outcome) -> Vec<u16> {
    let mut codes: Vec<u16> = out
        .log
        .iter()
        .filter_map(|item| match item {
            LogItem::Issue(issue) => Some(issue.code.get()),
            LogItem::OkLine(_) | LogItem::NoteLine(_) => None,
        })
        .collect();
    codes.sort_unstable();
    codes.dedup();
    codes
}

/// Human-readable diff of two scratch captures (name/bytes deltas, sorted).
fn diff_files(a: &BTreeMap<String, Vec<u8>>, b: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut d: Vec<String> = Vec::new();
    for (k, v) in a {
        match b.get(k) {
            None => d.push(format!("{k} (left only)")),
            Some(w) if w != v => d.push(format!("{k} (bytes)")),
            Some(_) => {}
        }
    }
    for k in b.keys() {
        if !a.contains_key(k) {
            d.push(format!("{k} (right only)"));
        }
    }
    d.sort();
    d
}

/// Recursive dir copy (std-only, best effort on symlinks — the corpus has
/// none): entry children (`!include`/`extends` files, `themes/`) preserve
/// their relative layout; sidecars and outputs are skipped.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("mkdir {}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("readdir: {e}"))? {
        let entry = entry.map_err(|e| format!("readdir entry: {e}"))?;
        let name = entry.file_name();
        if name == oxidizr_e2e::manifest::FILE_NAME {
            continue;
        }
        let target = to.join(&name);
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)
                .map_err(|e| format!("copy {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// `dsl2json`/`json2json` bytes for the replay feeds (the same reduction the
/// runner uses for its json2json section).
fn json_bytes(entry: &Entry, name: &str) -> Result<Vec<u8>, String> {
    let input = match &entry.dsl {
        Some(dsl) => {
            let out = execute(
                Request::Dsl2Json {
                    input: Input::File(dsl.clone()),
                    output: None,
                },
                &oxidizr_e2e::hermetic_options(),
            );
            out.bytes
                .ok_or_else(|| format!("{name}: dsl2json produced no bytes"))?
        }
        None => std::fs::read(entry.json.as_ref().expect("json input exists"))
            .map_err(|e| format!("{name}: read json: {e}"))?,
    };
    let out = execute(
        Request::Json2Json {
            input: Input::Bytes(input),
            output: None,
        },
        &oxidizr_e2e::hermetic_options(),
    );
    out.bytes
        .ok_or_else(|| format!("{name}: json2json over own model produced no bytes"))
}

/// `fmt` (Print) bytes of the DSL input.
fn fmt_bytes(entry: &Entry, name: &str) -> Result<Vec<u8>, String> {
    let out = execute(
        Request::Fmt {
            inputs: vec![Input::File(
                entry.dsl.as_ref().expect("fmt implies dsl").clone(),
            )],
            mode: oxidizr_api::FmtMode::Print,
        },
        &oxidizr_e2e::hermetic_options(),
    );
    out.bytes
        .ok_or_else(|| format!("{name}: fmt produced no bytes"))
}

/// One invariant run per entry × format; returns problems.
fn check_format(entry: &Entry, format: &str) -> Vec<String> {
    let name = entry.full_name();
    let mut problems = Vec::new();
    let input = match &entry.dsl {
        Some(d) => d.clone(),
        None => entry
            .json
            .as_ref()
            .expect("export implies an input")
            .clone(),
    };

    // Baseline + determinism (two independent scratch exports).
    let (base_out, base_files) = oxidizr_e2e::runner::export_capture(
        &input,
        format,
        &oxidizr_e2e::hermetic_options(),
        &format!("inv-a-{name}"),
    );
    let (again_out, again_files) = oxidizr_e2e::runner::export_capture(
        &input,
        format,
        &oxidizr_e2e::hermetic_options(),
        &format!("inv-b-{name}"),
    );
    if base_out.exit != again_out.exit || codes_of(&base_out) != codes_of(&again_out) {
        problems.push(format!(
            "{name} [{format}]: determinism — exit/codes differ between runs"
        ));
    }
    if base_files != again_files {
        problems.push(format!(
            "{name} [{format}]: determinism — runs differ: {:?}",
            diff_files(&base_files, &again_files)
        ));
    }

    // Replay both sides from a scratch copy of the entry context (children +
    // themes/), so the theme chain and include/extends resolution match.
    let ctx = oxidizr_e2e::runner::scratch_dir(&format!("inv-ctx-{name}"));
    let r = (|| -> Result<(), String> {
        copy_tree(&entry.dir, &ctx)?;

        // (2) Path parity: json2json feed.
        let json_file = ctx.join("workspace.json");
        std::fs::write(&json_file, json_bytes(entry, &name)?)
            .map_err(|e| format!("write replay feed: {e}"))?;
        let (json_out, json_files) = oxidizr_e2e::runner::export_capture(
            &json_file,
            format,
            &oxidizr_e2e::hermetic_options(),
            &format!("inv-j-{name}"),
        );
        if json_out.exit != base_out.exit || codes_of(&json_out) != codes_of(&base_out) {
            problems.push(format!(
                "{name} [{format}]: path parity — exit/codes via JSON {:?} vs DSL {:?}",
                codes_of(&json_out),
                codes_of(&base_out)
            ));
        }
        if json_files != base_files {
            problems.push(format!(
                "{name} [{format}]: path parity — differ: {:?}",
                diff_files(&base_files, &json_files)
            ));
        }

        // (3) Format-fixpoint parity (DSL inputs only).
        if entry.dsl.is_some() {
            let dsl_file = ctx.join("formatted.dsl");
            std::fs::write(&dsl_file, fmt_bytes(entry, &name)?)
                .map_err(|e| format!("write formatted feed: {e}"))?;
            let (fmt_out, fmt_files) = oxidizr_e2e::runner::export_capture(
                &dsl_file,
                format,
                &oxidizr_e2e::hermetic_options(),
                &format!("inv-f-{name}"),
            );
            if fmt_out.exit != base_out.exit || codes_of(&fmt_out) != codes_of(&base_out) {
                problems.push(format!(
                    "{name} [{format}]: fmt parity — exit/codes via fmt {:?} vs raw {:?}",
                    codes_of(&fmt_out),
                    codes_of(&base_out)
                ));
            }
            if fmt_files != base_files {
                problems.push(format!(
                    "{name} [{format}]: fmt parity — differ: {:?}",
                    diff_files(&base_files, &fmt_files)
                ));
            }
        }
        Ok(())
    })();
    if let Err(e) = r {
        problems.push(format!("{name} [{format}]: scratch context: {e}"));
    }
    let _ = std::fs::remove_dir_all(&ctx);
    problems
}

fn run_all() -> Vec<String> {
    let entries = export_entries();
    assert!(
        !entries.is_empty(),
        "no export-applicable valid entries — the invariant would pass vacuously"
    );
    let formats = ["plantuml", "plantuml/c4plantuml", "mermaid"];
    let jobs: Vec<(Entry, &'static str)> = entries
        .iter()
        .flat_map(|e| formats.iter().map(move |f| (e.clone(), *f)))
        .collect();
    fan_out(&jobs, |(entry, format)| {
        let problems = check_format(entry, format);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    })
}

#[test]
fn export_is_deterministic_json_parity_and_fmt_parity() {
    if !full_tier() {
        return;
    }
    let problems = run_all();
    assert!(
        problems.is_empty(),
        "export invariants ({} entry x format jobs):\n{}",
        export_entries().len() * 3,
        problems.join("\n")
    );
}

/// Scratch-context helper kept honest on its own (also used by future tests):
/// copying an entry directory preserves children, the sidecar stays out.
#[test]
fn copy_tree_preserves_children_without_the_sidecar() {
    let entries = export_entries();
    let with_children = entries.iter().find(|e| {
        std::fs::read_dir(&e.dir).is_ok_and(|rd| {
            rd.flatten()
                .any(|de| de.path().is_dir() && de.file_name() != oxidizr_e2e::manifest::FILE_NAME)
        })
    });
    let Some(entry) = with_children else {
        eprintln!("no entry with child dirs — probe skipped");
        return;
    };
    let dst: PathBuf = oxidizr_e2e::runner::scratch_dir(&format!("copychk-{}", entry.name));
    copy_tree(&entry.dir, &dst).expect("copy succeeds");
    assert!(
        !dst.join(oxidizr_e2e::manifest::FILE_NAME).exists(),
        "sidecar must not be copied"
    );
    for child in std::fs::read_dir(&entry.dir).expect("src readable") {
        let child = child.expect("entry");
        if child.path().is_dir() {
            assert!(dst.join(child.file_name()).is_dir(), "child dir missing");
        }
    }
    let _ = std::fs::remove_dir_all(&dst);
}
