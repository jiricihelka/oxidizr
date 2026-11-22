//! §P4T gap 1 (T-2): Category A golden FILE suite for the importers —
//! corpus-style probe fixtures under `tests/fixtures/` (authored
//! originals, LF-pinned by `.gitattributes`), imported through the public
//! API and pinned as `expect!` model digests (re-bless:
//! `UPDATE_EXPECT=1 cargo test -p oxidizr-import --test golden_files`).
//! The unit suites prove the rules; this suite proves the real files —
//! byte-stable fixtures round into byte-stable digests on every checkout.
#![allow(clippy::unwrap_used)]
#![allow(clippy::format_push_string)] // test digest: format! reads best here (testutil.rs)

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use oxidizr_core::Workspace;
use oxidizr_core::docs::Documentation;
use oxidizr_core::tags::TagKind;
use oxidizr_import::decisions::{self, DecisionKind};
use oxidizr_import::docs::{self, DocsKind};
use oxidizr_import::{Imported, c4plantuml, image, mermaid};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(rel)
}

fn read(rel: &str) -> String {
    fs::read_to_string(fixture(rel)).unwrap_or_else(|e| panic!("fixture {rel}: {e}"))
}

/// id → display name (path-qualified below system level) — the same
/// compact digest the unit goldens use (`src/testutil.rs` is unit-only,
/// so the integration view re-states it; keep the two in step when the
/// model shape grows).
fn names(ws: &Workspace) -> HashMap<&str, String> {
    let mut map = HashMap::new();
    for p in &ws.model.people {
        map.insert(p.item.id.as_str(), p.item.name.clone());
    }
    for s in &ws.model.software_systems {
        map.insert(s.item.id.as_str(), s.item.name.clone());
        for c in &s.containers {
            map.insert(
                c.item.id.as_str(),
                format!("{} / {}", s.item.name, c.item.name),
            );
        }
    }
    map
}

fn digest(im: &Imported) -> String {
    let ws = &im.workspace;
    let n = names(ws);
    let mut out = String::new();
    for p in &ws.model.people {
        out.push_str(&format!(
            "person {} [{}] {:?}\n",
            p.item.name,
            p.item.tags.serialized_for(TagKind::Person),
            p.item.description
        ));
    }
    for s in &ws.model.software_systems {
        out.push_str(&format!(
            "system {} [{}] {:?}\n",
            s.item.name,
            s.item.tags.serialized_for(TagKind::SoftwareSystem),
            s.item.description
        ));
        for c in &s.containers {
            out.push_str(&format!(
                "  container {} [{}] tech={:?}\n",
                c.item.name,
                c.item.tags.serialized_for(TagKind::Container),
                c.technology
            ));
        }
    }
    for r in &ws.model.relationships {
        out.push_str(&format!(
            "rel {} -> {} \"{}\" tech={:?} [{}]\n",
            n[r.source.as_str()],
            n[r.destination.as_str()],
            r.description.clone().unwrap_or_default(),
            r.technology,
            r.tags.stored().join("+")
        ));
    }
    for w in &im.warnings {
        out.push_str(&format!("warn: {w}\n"));
    }
    out
}

fn docs_digest(doc: &Documentation) -> String {
    let mut out = String::new();
    for s in &doc.sections {
        out.push_str(&format!(
            "section order={} file={:?} format={:?}\n{}\n--\n",
            s.order, s.filename, s.format, s.content
        ));
    }
    for im in &doc.images {
        out.push_str(&format!(
            "image {:?} type={:?} len={}\n",
            im.name,
            im.content_type,
            im.content.len()
        ));
    }
    out
}

fn decisions_digest(doc: &Documentation) -> String {
    let mut out = String::new();
    for d in &doc.decisions {
        out.push_str(&format!(
            "decision id={} title={:?} status={:?} date={} format={:?}\n{}\n--\n",
            d.id, d.title, d.status, d.date, d.format, d.content
        ));
    }
    out
}

#[test]
fn c4plantuml_context_file_golden() {
    let im = c4plantuml::import(&read("c4/context.puml")).expect("import");
    expect_test::expect![[r#"
        person Ops Engineer [Element,Person] Some("Runs the oxidizr stack.")
        system oxidizr [Element,Software System] None
        system oxidizr CLI [Element,Software System,Critical] Some("The models-as-code CLI.")
        system Structurizr [Element,Software System,External] Some("The reference implementation this project is compatible with.")
        system Change Feed [Element,Software System,Queue] Some("File watcher")
        rel Ops Engineer -> oxidizr CLI "Drives" tech=Some("CLI") []
        rel oxidizr CLI -> Structurizr "Mirrors bytes" tech=Some("JSON") []
        warn: unknown macro "Mystery_Box" — skipped
    "#]].assert_eq(&digest(&im));
}

#[test]
fn c4plantuml_containers_file_golden() {
    let im = c4plantuml::import(&read("c4/containers.puml")).expect("import");
    expect_test::expect![[r#"
        system Internet Banking System [Element,Software System] None
          container Web Application [Element,Container] tech=Some("Rust")
        warn: unknown macro "Container_Stub" — skipped
        warn: skipped ContainerDb "Ledger": declared outside a System_Boundary
        warn: skipped BiRel "Reads and writes": unknown element alias "api"
        warn: skipped Rel_D "Writes": unknown element alias "api"
        warn: skipped Rel_Index "": unknown element alias "api"
    "#]]
    .assert_eq(&digest(&im));
}

#[test]
fn mermaid_context_file_golden() {
    let im = mermaid::import(&read("mermaid/context.mmd")).expect("import");
    expect_test::expect![[r#"
        person Doc Reader [Element,Person] Some("Reads the rendered docs.")
        person CI Bot [Element,Person,Automated] None
        system Docs Site [Element,Software System,Public] Some("Rendered docs.")
        system GitHub [Element,Software System,External] Some("Source hosting.")
        rel Doc Reader -> Docs Site "Reads" tech=Some("HTTPS") []
        rel CI Bot -> GitHub "Triggers" tech=None []
        rel Docs Site -> GitHub "Syncs" tech=None []
        rel GitHub -> Docs Site "Syncs" tech=None []
        warn: skipped ContainerDb "Asset Cache": declared outside a System_Boundary
    "#]]
    .assert_eq(&digest(&im));
}

#[test]
fn image_png_and_svg_file_golden() {
    let opts = image::ImageImport::default();
    let png = image::import_image(&fixture("image/tiny.png"), &opts).expect("png");
    let svg = image::import_image(&fixture("image/icon.svg"), &opts).expect("svg");
    let view = |im: &Imported| {
        let v = &im.workspace.views.image_views[0];
        let content = v.content.clone().unwrap_or_default();
        format!(
            "key={:?} generated={} title={:?} type={:?} head={:?} len={}",
            v.base.key,
            v.base.generated_key,
            v.base.title,
            v.content_type,
            content.chars().take(32).collect::<String>(),
            content.len()
        )
    };
    expect_test::expect![[r#"
        key="Image-001" generated=true title=Some("tiny.png") type=Some("image/png") head="data:image/png;base64,iVBORw0KGg" len=118
        key="Image-001" generated=true title=Some("icon.svg") type=Some("image/svg+xml") head="data:image/svg+xml;base64,PHN2Zy" len=182"#]].assert_eq(&format!("{}\n{}", view(&png), view(&svg)));
}

#[test]
fn docs_default_walk_file_golden() {
    let mut doc = Documentation::default();
    docs::import_docs(DocsKind::Default, &mut doc, &fixture("docs"), &[]).expect("docs");
    expect_test::expect![[r#"
        section order=1 file=Some("01-intro.md") format=Some(Markdown)
        # Intro

        oxidizr models as code.

        --
        section order=2 file=Some("02-guide.adoc") format=Some(AsciiDoc)
        = Guide

        Import, export, view.

        --
    "#]]
    .assert_eq(&docs_digest(&doc));
}

#[test]
fn docs_recursive_walk_file_golden() {
    let mut doc = Documentation::default();
    docs::import_docs(DocsKind::Recursive, &mut doc, &fixture("docs"), &[]).expect("docs");
    expect_test::expect![[r#"
        section order=1 file=Some("01-intro.md") format=Some(Markdown)
        # Intro

        oxidizr models as code.

        --
        section order=2 file=Some("02-guide.adoc") format=Some(AsciiDoc)
        = Guide

        Import, export, view.

        --
        section order=3 file=Some("sub/03-nested.md") format=Some(Markdown)
        # Nested

        Deeper page.

        --
    "#]]
    .assert_eq(&docs_digest(&doc));
}

#[test]
fn adrs_adrtools_file_golden() {
    let mut doc = Documentation::default();
    decisions::import_decisions(
        DecisionKind::AdrTools,
        &mut doc,
        &fixture("adrs/adrtools"),
        &[],
    )
    .expect("adrs");
    expect_test::expect![[r#"
        decision id=1 title="Adopt Rust" status="Accepted" date=1767571200 format=Some(Markdown)
        # 1. Adopt Rust

        Date: 2026-01-05

        ## Status

        Accepted

        ## Context

        We need memory safety without a GC.

        --
        decision id=2 title="Stay DSL compatible" status="Accepted" date=1770595200 format=Some(Markdown)
        # 2. Stay DSL compatible

        Date: 2026-02-09

        ## Status

        Accepted

        See the compatibility charter.

        --
    "#]].assert_eq(&decisions_digest(&doc));
}

#[test]
fn adrs_madr_file_golden() {
    let mut doc = Documentation::default();
    decisions::import_decisions(DecisionKind::Madr, &mut doc, &fixture("adrs/madr"), &[])
        .expect("adrs");
    expect_test::expect![[r#"
        decision id=1 title="1. Choose a layered layout engine" status="accepted" date=1772323200 format=Some(Markdown)

        # 1. Choose a layered layout engine

        ## Status

        Accepted

        ## Context

        The viewer needs auto-layout.

        ## Consequences

        The engine is a Sugiyama-family port.

        --
        decision id=2 title="2. Front-matter decisions" status="on-hold" date=1775088000 format=Some(Markdown)

        # 2. Front-matter decisions

        ## Status

        Proposed

        --
    "#]].assert_eq(&decisions_digest(&doc));
}

/// The suite fails loudly if a fixture file is added without a test
/// consuming it — the matrix row stays honest.
#[test]
fn every_fixture_is_consumed() {
    let mut files = Vec::new();
    let mut stack = vec![fixture("")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("fixtures readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let rel = path
                    .strip_prefix(fixture(""))
                    .expect("under fixtures")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push(rel);
            }
        }
    }
    files.sort();
    let consumed = [
        "README.md",
        "adrs/adrtools/0001-adopt-rust.md",
        "adrs/adrtools/0002-stay-dsl-compatible.md",
        "adrs/madr/0001-choose-a-layered-layout-engine.md",
        "adrs/madr/0002-front-matter-decisions.md",
        "c4/containers.puml",
        "c4/context.puml",
        "docs/01-intro.md",
        "docs/02-guide.adoc",
        "docs/sub/03-nested.md",
        "image/icon.svg",
        "image/tiny.png",
        "mermaid/context.mmd",
    ];
    assert_eq!(files, consumed, "fixture inventory drifted");
}
