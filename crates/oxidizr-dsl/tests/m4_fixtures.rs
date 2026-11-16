//! P1-M4 fixture burn-in harness (Category A, `agents/plan/build/PHASE_1.md` M4).
//!
//! Every `.dsl` file under `testdata/dsl` is parsed as an entry with
//! file-based [`ParseOptions`] (so relative `!include`/`!docs` paths resolve
//! from the fixture's own directory). A file must either parse with ZERO
//! errors, or be registered in [`KNOWN_ERRORS`] with its expected first-error
//! OXZ code and message prefix. The register is checked both ways:
//! unregistered failures fail the gate, and registered entries that have gone
//! stale (the fixture now parses, or the code/message changed) fail it too —
//! so retired deferrals must be pruned here, in lockstep with the milestone
//! that retires them.
//!
//! Milestone tags: `D7/P4` = `!docs`/`!adrs` importers (OXZ-5003);
//! `D11/P4` = plantuml/mermaid/kroki image imports (OXZ-5005). `D2`
//! (JVM-only directives), `D5` (no network — includes, extends and image
//! URLs) and `D10` (no JVM image codecs) are permanent by design. The
//! `component:` tag marks `!include`/`extends` children — they are
//! workspace-context fragments, so parsing one standalone fails with the
//! reference "expected: workspace" error exactly as the reference parser
//! would; they are covered through their entry fixtures.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use oxidizr_diag::codes;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

fn dsl_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/dsl")
        .canonicalize()
        .unwrap()
}

/// `workspace extends <*.json>` base loader: topology keeps `oxidizr-json`
/// out of the shipped `dsl` deps (P1-D1) — the CLI injects it in production;
/// the corpus harness injects the same wiring (dev-dependency).
fn json_loader(path: &str) -> Result<oxidizr_core::Workspace, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Error reading file at {path}: {e}"))?;
    oxidizr_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// File entry options with the JSON-base loader (production wiring).
fn file_opts(path: &str) -> ParseOptions<'_> {
    ParseOptions {
        file: Some(path),
        base_dir: None,
        json_loader: Some(json_loader),
    }
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

/// (relative path, expected first-error code, expected message prefix).
const KNOWN_ERRORS: &[(&str, u16, &str)] = &[
    // -- D7/P4: `!docs`/`!adrs` importers deferred (OXZ-5003) --------------
    (
        "big-bank-plc/internet-banking-system.dsl",
        codes::DOCS_DEFERRED.get(),
        "!docs documentation import arrives in Phase 4",
    ),
    (
        "decisions/workspace.dsl",
        codes::DOCS_DEFERRED.get(),
        "!adrs decisions import arrives in Phase 4",
    ),
    (
        "docs/workspace.dsl",
        codes::DOCS_DEFERRED.get(),
        "!docs documentation import arrives in Phase 4",
    ),
    (
        "include-directory.dsl",
        codes::DOCS_DEFERRED.get(),
        "!docs documentation import arrives in Phase 4",
    ),
    // -- D11/P4: plantuml/mermaid/kroki image imports deferred (OXZ-5005) --
    (
        "image-views/workspace-via-file.dsl",
        codes::IMPORT_DEFERRED.get(),
        "plantuml <file> import arrives in Phase 4",
    ),
    (
        "image-views/workspace-via-source.dsl",
        codes::IMPORT_DEFERRED.get(),
        "plantuml <source> import arrives in Phase 4",
    ),
    (
        "text-block.dsl",
        codes::IMPORT_DEFERRED.get(),
        "plantuml <source> import arrives in Phase 4",
    ),
    // -- D5: no network (OXZ-5001, `--allow-net` only) ----------------------
    (
        "image-views/workspace-via-url.dsl",
        codes::NET_DISABLED.get(),
        "Image views via HTTP",
    ),
    (
        "include-url.dsl",
        codes::NET_DISABLED.get(),
        "Includes via HTTPS are not permitted",
    ),
    (
        "extend/extend-workspace-from-dsl-url.dsl",
        codes::NET_DISABLED.get(),
        "Extends via HTTPS are not permitted",
    ),
    (
        "extend/extend-workspace-from-json-url.dsl",
        codes::NET_DISABLED.get(),
        "Extends via HTTPS are not permitted",
    ),
    // -- D2/D10: JVM-only directives and codecs (OXZ-5002) -----------------
    (
        "plugin-with-parameters.dsl",
        codes::JVM_ONLY.get(),
        "!plugin is not supported by oxidizr",
    ),
    (
        "plugin-without-parameters.dsl",
        codes::JVM_ONLY.get(),
        "!plugin is not supported by oxidizr",
    ),
    (
        "script-external.dsl",
        codes::JVM_ONLY.get(),
        "!script is not supported by oxidizr",
    ),
    (
        "script-external-with-parameters.dsl",
        codes::JVM_ONLY.get(),
        "!script is not supported by oxidizr",
    ),
    (
        "script-in-dynamic-view.dsl",
        codes::JVM_ONLY.get(),
        "!script is not supported by oxidizr",
    ),
    (
        "script-inline.dsl",
        codes::JVM_ONLY.get(),
        "!script is not supported by oxidizr",
    ),
    (
        "spring-petclinic/workspace.dsl",
        codes::JVM_ONLY.get(),
        "!components is not supported by oxidizr",
    ),
    (
        "test.dsl",
        codes::JVM_ONLY.get(),
        "image logo.png is not supported by oxidizr",
    ),
    // -- fixtures the reference test suite asserts FAIL (pinned messages) --
    (
        "constant.dsl",
        codes::STATEMENT.get(),
        "!constant was previously deprecated",
    ),
    (
        "enterprise.dsl",
        codes::STATEMENT.get(),
        "The enterprise keyword was previously deprecated",
    ),
    (
        "group-url.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: !docs, !decisions, group, container",
    ),
    (
        "group-without-brace.dsl",
        codes::STATEMENT.get(),
        "Expected: group <name> {",
    ),
    (
        "multi-line-with-error.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: !docs, !decisions, group, container",
    ),
    (
        "multiple-model-tokens.dsl",
        codes::STATEMENT.get(),
        "Multiple models are not permitted",
    ),
    (
        "multiple-view-tokens.dsl",
        codes::STATEMENT.get(),
        "Multiple view sets are not permitted",
    ),
    (
        "multiple-workspace-tokens.dsl",
        codes::STATEMENT.get(),
        "Multiple workspaces are not permitted",
    ),
    (
        "relationship-already-exists.dsl",
        codes::STATEMENT.get(),
        "A relationship between \"SoftwareSystem://B\" and \"SoftwareSystem://A\" already exists",
    ),
    (
        "unexpected-tokens-after-workspace.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens",
    ),
    (
        "unexpected-tokens-before-workspace.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "unexpected-tokens-in-workspace.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: name, description",
    ),
    // -- image asset absent (the corpus vendors no image.png; the SVG path
    //    is unit-tested, raster content is D10-rejected anyway) ------------
    ("image-view.dsl", codes::STATEMENT.get(), "The file at "),
    // -- component: `!include`/`extends` children, not entry fixtures ------
    (
        "big-bank-plc/model/internet-banking-system/details.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "big-bank-plc/model/internet-banking-system/summary.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "big-bank-plc/model/people-and-software-systems.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "big-bank-plc/views/styles-people.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "include/model.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "include/model/software-system/model.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
    (
        "workspace-with-bom-model.dsl",
        codes::UNEXPECTED_TOKEN.get(),
        "Unexpected tokens (expected: workspace)",
    ),
];

fn rel(path: &Path) -> String {
    path.strip_prefix(dsl_dir())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// Non-UTF-8 corpus members are handled by their own encoding test.
fn latin1_only(relpath: &str) -> bool {
    relpath == "iso-8859.dsl"
}

#[test]
fn corpus_matches_register() {
    let mut problems: Vec<String> = Vec::new();
    let mut clean = 0;
    let mut matched = 0;
    let mut seen: Vec<String> = Vec::new();

    for path in corpus() {
        let name = rel(&path);
        if latin1_only(&name) {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{name} must be valid UTF-8 for the default read: {e}"));
        let result = parse_dsl(&source, file_opts(&name_to_full(&path)))
            .unwrap_or_else(|e| panic!("{name}: fatal parse error: {}", e.message));
        if result.errors.is_empty() {
            clean += 1;
            if KNOWN_ERRORS.iter().any(|(f, _, _)| *f == name) {
                problems.push(format!(
                    "{name}: REGISTERED but parses CLEAN — prune the entry"
                ));
            }
            continue;
        }
        let Some((_, code, prefix)) = KNOWN_ERRORS.iter().find(|(f, _, _)| *f == name) else {
            problems.push(format!(
                "{name}: UNEXPECTED FAILURE — first error [OXZ-{}] {}",
                result.errors[0].code.get(),
                result.errors[0].message
            ));
            continue;
        };
        seen.push(name.clone());
        let first = &result.errors[0];
        if first.code.get() != *code || !first.message.starts_with(prefix) {
            problems.push(format!(
                "{name}: first error [OXZ-{}] {} does not match register [OXZ-{code}] \"{prefix}…\"",
                first.code.get(),
                first.message
            ));
        } else {
            matched += 1;
        }
    }

    for (f, _, _) in KNOWN_ERRORS {
        if !seen.iter().any(|s| s == f) && !latin1_only(f) {
            problems.push(format!("{f}: REGISTERED but not in the corpus"));
        }
    }

    assert!(
        problems.is_empty(),
        "{} clean, {} registered failures, {} problems:\n{}",
        clean,
        matched,
        problems.len(),
        problems.join("\n")
    );
}

fn name_to_full(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `test_ISO8859Encoding` mirror: read the fixture as ISO-8859-1 (the
/// reference test sets that charset), then it must load with `Namé`.
#[test]
fn iso8859_fixture_decodes_as_latin1() {
    let path = dsl_dir().join("iso-8859.dsl");
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        std::str::from_utf8(&bytes).is_err(),
        "the fixture must stay non-UTF-8 for this test to mean anything"
    );
    let source: String = bytes.iter().map(|b| char::from(*b)).collect();
    let name = name_to_full(&path);
    let result = parse_dsl(&source, file_opts(&name)).unwrap();
    assert!(
        result.errors.is_empty(),
        "{:?}",
        result.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
    assert!(
        result
            .workspace
            .model
            .software_systems
            .iter()
            .any(|s| s.item.name == "Namé"),
        "Namé missing (mojibake?)"
    );
}
