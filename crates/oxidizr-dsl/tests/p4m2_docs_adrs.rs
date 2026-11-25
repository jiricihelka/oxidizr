//! P4-M2f: `!docs`/`!adrs` run the REAL importers at context close
//! (`end()`) — the wire between `oxidizr-dsl` and `oxidizr-import`.
//! Contract: `agents/plan/build/IMPORT-DOCS-ADRS.md`; importer unit
//! coverage lives in `oxidizr-import` (`docs`/`decisions` modules).
//! Scratch fixtures (deterministic: explicit dates, svg-only images).
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use oxidizr_core::docs::Format;
use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/tmp/p4m2-dsl")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // the reference guards message `getAbsolutePath()` — `..` segments are
    // NORMALIZED there; canonicalizing the scratch keeps the expected
    // strings in the same lexical form.
    std::fs::canonicalize(&dir).unwrap()
}

/// The importer's lexical `getAbsolutePath` (mirrors the
/// `oxidizr-import` helper): `/`-rooted, `..` popped, drive/verbatim
/// prefixes dropped. The guard-message expectations run the canonicalized
/// scratch through THIS so they render in the product's shape on every
/// target — identical strings on Linux (where the scratch is already in
/// this form), and on Windows targets the product names the path through
/// the same lexical function, not through `canonicalize`'s verbatim
/// `\\?\` form (pitfall `windows-path-separator-assertions.md`).
fn java_absolute(path: &std::path::Path) -> PathBuf {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::from("/");
    for c in path.components() {
        match c {
            Component::RootDir => out.push("/"),
            Component::Normal(seg) => out.push(seg),
            Component::ParentDir => {
                if !matches!(out.components().next_back(), Some(Component::RootDir)) {
                    out.pop();
                }
            }
            Component::CurDir | Component::Prefix(_) => {}
        }
    }
    out
}

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn parse(dir: &Path, dsl: &str) -> oxidizr_dsl::engine::ParseResult {
    let file = dir.join("workspace.dsl");
    std::fs::write(&file, dsl).unwrap();
    let result = parse_dsl(
        &std::fs::read_to_string(&file).unwrap(),
        ParseOptions::file(&file.to_string_lossy()),
    )
    .unwrap_or_else(|e| panic!("fatal parse error: {}", e.message));
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors.iter().map(|e| &e.message).collect::<Vec<_>>()
    );
    result
}

const ADRT: &str = concat!(
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
    "We need to record decisions.\n"
);

#[test]
fn docs_import_success_with_holders_exclude_and_images() {
    let dir = scratch("success");
    write(&dir, "docs/a.md", "# A\n\n![Logo](logo.svg)\n");
    write(&dir, "docs/b.adoc", "= B\n");
    write(&dir, "docs/README.md", "excluded\n");
    write(
        &dir,
        "docs/logo.svg",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    );
    write(&dir, "sdocs/s.md", "# S\n");
    let result = parse(
        &dir,
        concat!(
            "workspace {\n",
            "  !docs docs {\n    exclude \"README.md\"\n  }\n",
            "  model {\n",
            "    s = softwareSystem \"S\" {\n      !docs sdocs\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    );
    let ws = &result.workspace;
    // default importer: sorted top-level files, FormatFinder, trim of the
    // base path (the trim removes the PLATFORM separator, so the names are
    // bare on every target), the exclude token applied by NAME.
    let names: Vec<&str> = ws
        .documentation
        .sections
        .iter()
        .map(|s| s.filename.as_deref().unwrap_or_default())
        .collect();
    assert_eq!(names, ["a.md", "b.adoc"]);
    assert_eq!(ws.documentation.sections[0].order, 1);
    assert_eq!(ws.documentation.sections[0].format, Some(Format::Markdown));
    assert_eq!(ws.documentation.sections[1].format, Some(Format::AsciiDoc));
    // trailing RELUCTANT image pass: only the REFERENCED svg, raw bytes
    // base64'd (D16), content type image/svg+xml.
    assert_eq!(ws.documentation.images.len(), 1);
    let img = &ws.documentation.images[0];
    assert_eq!(img.name, "logo.svg");
    assert_eq!(img.content_type.as_deref(), Some("image/svg+xml"));
    assert!(
        !img.content.is_empty(),
        "reluctant import embeds the svg bytes"
    );
    // holder routing: the softwareSystem's own documentation.
    let s = ws
        .model
        .software_systems
        .iter()
        .find(|x| x.item.name == "S")
        .unwrap();
    assert_eq!(s.documentation.sections.len(), 1);
    assert_eq!(
        s.documentation.sections[0].filename.as_deref(),
        Some("s.md")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adrs_default_is_adrtools_and_dates_parse_in_utc() {
    let dir = scratch("adrs-default");
    write(&dir, "adrs/0001-record.md", ADRT);
    let result = parse(
        &dir,
        concat!(
            "workspace {\n",
            "  !adrs adrs\n",
            "  model {\n",
            "    s = softwareSystem \"S\" {\n",
            "      !decisions adrs\n",
            "    }\n",
            "  }\n",
            "}\n",
        ),
    );
    let ws = &result.workspace;
    assert_eq!(ws.documentation.decisions.len(), 1);
    let d = &ws.documentation.decisions[0];
    assert_eq!(d.id, "1");
    assert_eq!(d.title, "Record architecture decisions");
    assert_eq!(d.status, "Accepted");
    // 2016-02-12T00:00:00Z — deviation D15 pins the PARSE zone to UTC.
    assert_eq!(d.date, 1_455_235_200);
    assert_eq!(d.format, Some(Format::Markdown));
    // the softwareSystem got its OWN copy (a second import into its tree).
    let s = ws
        .model
        .software_systems
        .iter()
        .find(|x| x.item.name == "S")
        .unwrap();
    assert_eq!(s.documentation.decisions.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_importer_fqn_is_the_end_error() {
    let dir = scratch("unknown-fqn");
    write(&dir, "docs/a.md", "# A\n");
    let file = dir.join("workspace.dsl");
    std::fs::write(
        &file,
        "workspace {\n  !docs docs com.example.MissingImporter\n}\n",
    )
    .unwrap();
    let result = parse_dsl(
        &std::fs::read_to_string(&file).unwrap(),
        ParseOptions::file(&file.to_string_lossy()),
    )
    .unwrap();
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].code.get(), 2002);
    assert_eq!(
        result.errors[0].message,
        "Error importing documentation: com.example.MissingImporter was not found"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn decisions_alias_pass_through_and_missing_path() {
    let dir = scratch("decisions-guards");
    // unknown "type" tokens fall through to loadClass — "was not found"
    let file = dir.join("workspace.dsl");
    std::fs::write(&file, "workspace {\n  !adrs adrs com.example.X\n}\n").unwrap();
    let result = parse_dsl(
        &std::fs::read_to_string(&file).unwrap(),
        ParseOptions::file(&file.to_string_lossy()),
    )
    .unwrap();
    assert_eq!(
        result.errors[0].message,
        "Error importing decisions: com.example.X was not found"
    );
    // the three aliases DO resolve — `!adrs adrs madr` runs MadrDecisionImporter
    std::fs::write(&file, "workspace {\n  !adrs adrs madr\n}\n").unwrap();
    write(
        &dir,
        "adrs/0001-madr.md",
        "---\ndate: 2020-05-04\n---\n\n# Use MADR\n",
    );
    let result = parse_dsl(
        &std::fs::read_to_string(&file).unwrap(),
        ParseOptions::file(&file.to_string_lossy()),
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.workspace.documentation.decisions[0].status,
        "accepted"
    );
    // missing path: the AdrTools guard, wrapped with the decisions prefix
    let file2 = dir.join("w2.dsl");
    std::fs::write(&file2, "workspace {\n  !adrs nosuchdir\n}\n").unwrap();
    let result = parse_dsl(
        &std::fs::read_to_string(&file2).unwrap(),
        ParseOptions::file(&file2.to_string_lossy()),
    )
    .unwrap();
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(
        result.errors[0].message,
        format!(
            "Error importing decisions: {} does not exist.",
            java_absolute(&dir.join("nosuchdir")).display()
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_docs_path_and_string_source_skips() {
    let dir = scratch("guards");
    // single-line default importer on a missing path: trailing period
    let file = dir.join("workspace.dsl");
    std::fs::write(&file, "workspace {\n  !docs nosuch\n}\n").unwrap();
    let result = parse_dsl(
        &std::fs::read_to_string(&file).unwrap(),
        ParseOptions::file(&file.to_string_lossy()),
    )
    .unwrap();
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert_eq!(
        result.errors[0].message,
        format!(
            "Error importing documentation: {} does not exist.",
            java_absolute(&dir.join("nosuch")).display()
        )
    );
    // `dslFile == null` (string/fragment source): BOTH contexts silently
    // skip — even an unknown FQN produces no error here.
    let result = parse_dsl(
        "workspace {\n  !docs nosuch com.example.X\n  !adrs nosuch com.example.Y\n}\n",
        ParseOptions::fragment(),
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        result.workspace.documentation.sections.is_empty(),
        "no dslFile ⇒ no import"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
