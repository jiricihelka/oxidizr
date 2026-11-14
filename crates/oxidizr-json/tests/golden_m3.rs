//! Byte-exact writer tests against the committed goldens (Category A).
//!
//! `testdata/golden/m3-order.json` is the WRITE-CATALOG witness (oracle
//! 2026.10.26 @ ad34036, `structurizr.inspection.*` properties removed);
//! these tests build the same workspaces through the public core API and
//! assert the writer's bytes are identical — every key table, sort,
//! omission, computed name, and `HashMap` property order at once.

use oxidizr_core::builder::WorkspaceBuilder;
use oxidizr_core::views::{
    AutomaticLayout, ElementStyle, ElementView, FilterMode, LineStyle, RelationshipStyle,
    RelationshipView, Routing, Shape,
};
use oxidizr_core::{Workspace, WorkspaceScope};

const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/golden");

/// Reference retained-source rule: `Files.readAllLines` joined with `\n`
/// (`StructurizrDslParser.parse(File)` → `getParsedDsl`), base64-encoded
/// (`DslUtils.setDsl`) — i.e. the file minus its trailing newline.
fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn el(id: &str) -> ElementView {
    ElementView {
        id: oxidizr_core::ids::Id::new(id),
        x: 0,
        y: 0,
    }
}

fn rel(
    id: &str,
    description: Option<&str>,
    order: Option<&str>,
    response: Option<bool>,
) -> RelationshipView {
    RelationshipView {
        id: oxidizr_core::ids::Id::new(id),
        description: description.map(str::to_string),
        url: None,
        order: order.map(str::to_string),
        response,
        properties: oxidizr_core::properties::Properties::new(),
        vertices: Vec::new(),
        routing: None,
        jump: None,
        position: None,
    }
}

/// The `testdata/probes/m3-order.dsl` workspace, built through the builder
/// exactly as the DSL defines it (id counter 1..14).
// One-binding-per-DSL-statement keeps the test auditable against the probe
// file and the golden side by side — single-char element names mirror the
// DSL identifiers (`p`, `s`, `z`, `k`, `a`), and the whole DSL is one
// straight-line function.
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
fn probe_workspace() -> Workspace {
    let mut b = WorkspaceBuilder::new();
    b.set_metadata(
        Some("Probe".to_string()),
        // Reference `WorkspaceParser`: `new Workspace("Name", "Description")`
        // — an unset description survives as "Description".
        Some("Description".to_string()),
        None,
    );
    b.set_scope(WorkspaceScope::SoftwareSystem);

    // -- model ------------------------------------------------------------
    let p = b
        .add_person("P", Some("p desc".into()), Some("Tag A,Tag B"))
        .unwrap(); // id 1
    b.set_url(&p, "https://example.com/p").unwrap();
    // Insertion order matters (the write reorders by Java `HashMap` bucket,
    // which depends on it): the reference stores the DSL identifier when the
    // element is created, BEFORE the `properties { ... }` block runs.
    b.add_item_property(&p, "structurizr.dsl.identifier", "p")
        .unwrap();
    for (i, k) in [
        "aaa", "bbb", "ccc", "ddd", "eee", "fff", "ggg", "hhh", "iii", "jjj", "kkk", "lll", "mmm",
    ]
    .iter()
    .enumerate()
    {
        b.add_item_property(&p, *k, (i + 1).to_string()).unwrap();
    }
    let s = b.add_software_system("S", None, None).unwrap(); // id 2
    b.add_item_property(&s, "structurizr.dsl.identifier", "s")
        .unwrap();
    let z = b
        .add_container(&s, "Z-first", None, Some("Container,X".into()), None)
        .unwrap(); // id 3
    b.add_item_property(&z, "structurizr.dsl.identifier", "z")
        .unwrap();
    let k = b.add_component(&z, "K1", None, None, None).unwrap(); // id 4
    b.add_item_property(&k, "structurizr.dsl.identifier", "k")
        .unwrap();
    let a = b.add_container(&s, "A-second", None, None, None).unwrap(); // id 5
    b.add_item_property(&a, "structurizr.dsl.identifier", "a")
        .unwrap();
    let r6 = b
        .add_relationship(
            &p,
            &s,
            Some("Uses".into()),
            Some("HTTP".into()),
            Some("customrel"),
        )
        .unwrap(); // id 6
    // id 7 — the shared counter advances even though the id is not reused.
    b.add_relationship(
        &k,
        &p,
        Some("Returns data to".into()),
        Some("JSON".into()),
        None,
    )
    .unwrap();
    for v in 1..=3 {
        b.add_item_property(&r6, format!("r{v}"), format!("rv{v}"))
            .unwrap();
    }
    b.model_mut().properties.insert("m1", "mv1");
    b.model_mut().properties.insert("m2", "mv2");

    // Implied (linked) relationships, exactly as `Model.addRelationship`
    // propagates them (ids 8: container, 9: system) — allocated through the
    // shared counter, as the reference does at `k -> p` parse time.
    let r8 = b.alloc_id();
    let r9 = b.alloc_id();
    {
        let m = b.model_mut();
        m.relationships.push(oxidizr_core::model::Relationship {
            id: r8.clone(),
            source: z.clone(),
            destination: p.clone(),
            description: Some("Returns data to".into()),
            technology: Some("JSON".into()),
            tags: oxidizr_core::tags::Tags::new(),
            url: None,
            properties: oxidizr_core::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: Some(oxidizr_core::ids::Id::new("7")),
        });
        m.relationships.push(oxidizr_core::model::Relationship {
            id: r9.clone(),
            source: s.clone(),
            destination: p.clone(),
            description: Some("Returns data to".into()),
            technology: Some("JSON".into()),
            tags: oxidizr_core::tags::Tags::new(),
            url: None,
            properties: oxidizr_core::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: Some(oxidizr_core::ids::Id::new("7")),
        });
        let sys = m
            .software_systems
            .iter_mut()
            .find(|x| x.item.id == s)
            .unwrap();
        sys.containers
            .iter_mut()
            .find(|c| c.item.id == z)
            .unwrap()
            .relationships
            .push(r8);
        sys.relationships.push(r9);
    }

    // Deployment environment "Live" (ids 10..14).
    let aws = b
        .add_deployment_node(
            "Live",
            "AWS",
            Some("us-east-1".into()),
            Some("Region".into()),
            None,
        )
        .unwrap(); // id 10
    b.node_mut(&aws).unwrap().deployment_groups = vec!["green group".into(), "blue group".into()];
    b.add_item_property(&aws, "structurizr.dsl.identifier", "aws")
        .unwrap();
    let ssi = b
        .add_software_system_instance(&aws, &s, None, Vec::new())
        .unwrap(); // id 11
    b.add_item_property(&ssi, "structurizr.dsl.identifier", "ssi")
        .unwrap();
    let child = b
        .add_child_deployment_node(
            &aws,
            "EC2",
            Some("t3.medium".into()),
            Some("VM".into()),
            None,
        )
        .unwrap(); // id 12
    b.add_item_property(&child, "structurizr.dsl.identifier", "child")
        .unwrap();
    let ci = b
        .add_container_instance(&child, &z, None, Vec::new())
        .unwrap(); // id 13
    b.add_item_property(&ci, "structurizr.dsl.identifier", "ci")
        .unwrap();
    let infra = b
        .add_infrastructure_node(
            &aws,
            "ALB",
            Some("Load balancer".into()),
            Some("AWS ALB".into()),
            None,
        )
        .unwrap(); // id 14
    b.add_item_property(&infra, "structurizr.dsl.identifier", "infra")
        .unwrap();

    // -- views (DSL definition order = TreeSet order 1..5) ----------------
    b.create_landscape_view("land").unwrap();
    b.create_container_view(&s, "cont").unwrap();
    b.create_filtered_view(
        "land",
        "filt",
        FilterMode::Include,
        vec!["Element".into(), "Person".into()],
    )
    .unwrap();
    b.create_dynamic_view(Some(z.clone()), "dyn").unwrap();
    b.create_deployment_view("Live", "dep").unwrap();
    {
        let views = b.views_mut();
        views.system_landscape_views[0].base.description = Some("Landscape".into());
        views.system_landscape_views[0].content.elements = vec![el("1"), el("2")];
        views.system_landscape_views[0].content.relationships =
            vec![rel("6", None, None, None), rel("9", None, None, None)];
        views.container_views[0].base.description = Some("Containers".into());
        views.container_views[0].content.elements = vec![el("1"), el("3"), el("5")];
        views.container_views[0].content.relationships = vec![rel("8", None, None, None)];
        views.container_views[0].content.automatic_layout = Some(AutomaticLayout::default());
        views.filtered_views[0].base.description = Some("Filtered desc".into());
        views.dynamic_views[0].base.description = Some("Dynamic desc".into());
        views.dynamic_views[0].content.elements = vec![el("1"), el("4")];
        views.dynamic_views[0].content.relationships =
            vec![rel("7", Some("Returns data to"), Some("1"), Some(false))];
        views.deployment_views[0].base.description = Some("Deployment desc".into());
        views.deployment_views[0].content.elements = vec![el("10"), el("11"), el("14")];

        // Styles (DSL order; the writer re-sorts by `AbstractStyle.compareTo`).
        views.configuration.styles.element_styles = vec![
            ElementStyle {
                tag: "Tag B".into(),
                color: Some("#ff0000".into()),
                ..Default::default()
            },
            ElementStyle {
                tag: "Element".into(),
                background: Some("#eeeeee".into()),
                stroke: Some("#000000".into()),
                stroke_width: Some(2),
                shape: Some(Shape::Ellipse),
                border: Some(LineStyle::Dashed),
                opacity: Some(90),
                font_size: Some(22),
                metadata: Some(false),
                description: Some(false),
                width: Some(100),
                height: Some(80),
                ..Default::default()
            },
        ];
        views.configuration.styles.relationship_styles = vec![RelationshipStyle {
            tag: "Relationship".into(),
            color: Some("#123456".into()),
            thickness: Some(3),
            dashed: Some(true),
            routing: Some(Routing::Orthogonal),
            jump: Some(true),
            style: Some(LineStyle::Dotted),
            font_size: Some(11),
            ..Default::default()
        }];
    }

    // Workspace properties: the DSL `configuration { properties { ... } }`
    // block (cfg/cfg2 — these land in the WORKSPACE properties, not in
    // `configuration`, golden-proven) precedes the `properties` block, and
    // the retained DSL source is added after parsing finished.
    b.set_property("cfg", "1");
    b.set_property("cfg2", "2");
    for (i, k) in [
        "aaa", "bbb", "ccc", "ddd", "eee", "fff", "ggg", "hhh", "iii", "jjj", "kkk", "lll", "mmm",
    ]
    .iter()
    .enumerate()
    {
        b.set_property(*k, (i + 1).to_string());
    }
    let dsl_path = format!(
        "{}/../../testdata/probes/m3-order.dsl",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&dsl_path).expect("probe fixture");
    let source = raw.strip_suffix('\n').unwrap_or(&raw);
    b.set_property("structurizr.dsl", base64(source.as_bytes()));

    b.build()
}

fn assert_golden(workspace: &Workspace, name: &str) {
    let want = std::fs::read(format!("{GOLDEN_DIR}/{name}")).expect("golden fixture");
    let got = oxidizr_json::to_vec(workspace).expect("write");
    if got != want {
        // Write the received bytes out for local diffing, fail with a
        // readable unified preview.
        let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/writer-got.json");
        std::fs::write(out, &got).unwrap();
        let w = String::from_utf8_lossy(&want);
        let g = String::from_utf8_lossy(&got);
        let at = want
            .iter()
            .zip(got.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| want.len().min(got.len()));
        let ctx = |s: &str| {
            // MSRV-safe `floor_char_boundary` (stable only since 1.91).
            let mut cut = at.min(s.len());
            while cut > 0 && !s.is_char_boundary(cut) {
                cut -= 1;
            }
            s[..cut].chars().rev().take(60).collect::<String>()
        };
        panic!(
            "golden mismatch for {name} (len want {} got {}), first difference at byte {at}:\n  want: ...{}\n  got:  ...{}\n(full output: {out})",
            want.len(),
            got.len(),
            ctx(&w),
            ctx(&g),
        );
    }
}

#[test]
fn m3_probe_workspace_writes_golden_bytes() {
    assert_golden(&probe_workspace(), "m3-order.json");
}

#[test]
fn empty_named_workspace_writes_golden_bytes() {
    // `workspace-properties.json`: the bare `workspace { properties {
    // "structurizr.dslEditor" "false" } }` probe — exercises the always-on
    // empty beans (`configuration`, `documentation`, `model`,
    // `views.configuration.styles`/`terminology` → `{ }`).
    let mut b = WorkspaceBuilder::new();
    b.set_metadata(
        Some("Name".to_string()),
        Some("Description".to_string()),
        None,
    );
    let source =
        "workspace {\n\n    properties {\n        \"structurizr.dslEditor\" \"false\"\n    }\n\n}";
    b.set_property("structurizr.dsl", base64(source.as_bytes()));
    b.set_property("structurizr.dslEditor", "false");
    assert_golden(&b.build(), "workspace-properties.json");
}
