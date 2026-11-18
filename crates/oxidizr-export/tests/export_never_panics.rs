//! Phase 2 M5 panic-hardening suite (`build/PHASE_2.md` §Testing): the
//! exporters never panic on valid random workspaces. Workspaces are built
//! through `WorkspaceBuilder` (valid by construction — shared id counter,
//! registered endpoints, unique view keys), with hostile names/titles
//! (quotes, slashes, HTML, groups with separators), random view kinds,
//! styles and the `mermaid.*`/layout property gates. Diagram formats may
//! still fail CLEANLY (`ExportError` values — the reference's JVM-throw
//! legs are the OXZ-5009 aborts, e.g. the null-rank-direction header), so
//! only `theme`/`json` — the error-free workspace exporters — are asserted
//! `Ok`; every `Ok` file set must carry the reference naming invariants
//! (non-empty, no path separators, the format's extension, unique).

#![allow(clippy::unwrap_used)]

use oxidizr_core::views::{
    AutomaticLayout, ElementStyle, FilterMode, LineStyle, RelationshipStyle, Shape,
};
use oxidizr_core::{Id, ViewRef, Workspace, WorkspaceBuilder};
use oxidizr_export::ExportFormat;
use proptest::prelude::*;
use std::collections::BTreeSet;

/// The formats that landed in Phase 2 (theme/json + the three diagram
/// dialects, both color modes where the reference registry has them).
const FORMATS: &[&str] = &[
    "theme",
    "json",
    "plantuml",
    "plantuml-dark",
    "plantuml/c4plantuml",
    "plantuml/c4plantuml-dark",
    "mermaid",
];

fn expected_ext(format: &str) -> &'static str {
    match format {
        "theme" | "json" => "json",
        "mermaid" => "mmd",
        _ => "puml",
    }
}

/// The always-clean exporters (no JVM-throw analog on our side).
fn always_ok(format: &str) -> bool {
    matches!(format, "theme" | "json")
}

#[derive(Clone, Debug)]
struct Recipe {
    n_people: usize,
    n_systems: usize,
    n_containers: usize,
    n_components: usize,
    rels: Vec<(u8, u8)>,
    /// Bits: context, container, component, dynamic, deployment, filtered,
    /// custom. A landscape view is always created.
    view_mask: u8,
    /// 0 = no autoLayout, 1 = default (rank direction set), 2 = rank
    /// direction `None` (the reference's abort leg — a clean Err is fine).
    layout: u8,
    /// Bit flags below.
    flags: u8,
}

impl Recipe {
    const NASTY: u8 = 1;
    const GROUPS: u8 = 2;
    const STYLES: u8 = 4;
    const SEQUENCE: u8 = 8;
    fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

fn recipe() -> impl Strategy<Value = Recipe> {
    (
        0..=3usize,
        1..=3usize,
        0..=3usize,
        0..=2usize,
        proptest::collection::vec((0..64u8, 0..64u8), 0..6),
        any::<u8>(),
        0..3u8,
        any::<u8>(),
    )
        .prop_map(
            |(n_people, n_systems, n_containers, n_components, rels, view_mask, layout, flags)| {
                Recipe {
                    n_people,
                    n_systems,
                    n_containers,
                    n_components,
                    rels,
                    view_mask,
                    layout,
                    flags,
                }
            },
        )
}

/// Hostile name pool (quotes, backslashes, HTML, separators, unicode).
const NASTY_NAMES: &[&str] = &[
    "A\"B",
    "A/B",
    "A\\B",
    "<b>bold</b>",
    "sp ace",
    "'quote'",
    "&amp;",
    "e\u{e9}",
    "[bracket]",
    "G1/G2",
];

fn name(kind: &str, i: usize, nasty: bool) -> String {
    if nasty {
        format!("{}-{i}-{}", kind, NASTY_NAMES[i % NASTY_NAMES.len()])
    } else {
        format!("{kind} {i}")
    }
}

fn desc(i: usize, nasty: bool) -> String {
    if nasty {
        format!("d<i>{i} & \"q\" \\ / ")
    } else {
        format!("description {i}")
    }
}

/// The element ids the view/dcorrelation phases reference.
#[derive(Default)]
struct Parts {
    systems: Vec<Id>,
    containers: Vec<Id>,
    rels: Vec<Id>,
}

/// Elements, deployment nodes and the relationship fan-out.
fn add_model(r: &Recipe, b: &mut WorkspaceBuilder) -> Parts {
    let nasty = r.has(Recipe::NASTY);
    let mut parts = Parts::default();
    for i in 0..r.n_people {
        let _ = b.add_person(&name("Person", i, nasty), Some(desc(i, nasty)), None);
    }
    let mut nodes = Vec::new();
    for i in 0..r.n_systems {
        let Ok(sys) = b.add_software_system(&name("System", i, nasty), None, None) else {
            continue;
        };
        parts.systems.push(sys.clone());
        for j in 0..r.n_containers {
            let Ok(c) = b.add_container(
                &sys,
                &name("Container", j, nasty),
                None,
                Some("Tech".into()),
                None,
            ) else {
                continue;
            };
            parts.containers.push(c.clone());
            for k in 0..r.n_components {
                let _ = b.add_component(
                    &c,
                    &name("Component", k, nasty),
                    None,
                    None,
                    Some("Database"),
                );
            }
        }
    }
    for i in 0..2 {
        let _ = b.add_custom_element(&name("Custom", i, nasty), None, None);
    }
    if r.view_mask & 16 == 16 {
        for i in 0..2 {
            if let Ok(id) = b.add_deployment_node(
                "Default",
                &name("Node", i, nasty),
                None,
                Some("Docker".into()),
                None,
            ) {
                nodes.push(id);
            }
        }
        if let Some(parent) = nodes.first() {
            let _ = b.add_child_deployment_node(parent, &name("Child", 0, nasty), None, None, None);
        }
    }
    let all: Vec<Id> = b.all_element_ids();
    if all.len() >= 2 {
        for (s, d) in &r.rels {
            let (s, d) = (
                &all[usize::from(*s) % all.len()],
                &all[usize::from(*d) % all.len()],
            );
            if s == d {
                continue;
            }
            if let Ok(id) =
                b.add_relationship(s, d, Some(desc(1, nasty)), Some("HTTP".into()), None)
            {
                parts.rels.push(id);
            }
        }
    }
    parts
}

/// One view per recipe bit (landscape always); adds default content where
/// the kind supports it.
fn add_views(r: &Recipe, b: &mut WorkspaceBuilder, parts: &Parts) {
    let nasty = r.has(Recipe::NASTY);
    let mut views: Vec<ViewRef> = Vec::new();
    if let Ok(v) = b.create_landscape_view("L1") {
        let _ = b.view_add_default_elements(v, false);
        views.push(v);
    }
    if r.view_mask & 1 == 1
        && let Some(sys) = parts.systems.first()
        && let Ok(v) = b.create_context_view(sys, "C1")
    {
        let _ = b.view_add_default_elements(v, false);
        views.push(v);
    }
    if r.view_mask & 2 == 2
        && let Some(sys) = parts.systems.first()
        && let Ok(v) = b.create_container_view(sys, "CV1")
    {
        let _ = b.view_add_default_elements(v, false);
        views.push(v);
    }
    if r.view_mask & 4 == 4
        && let Some(c) = parts.containers.first()
        && let Ok(v) = b.create_component_view(c, "CP1")
    {
        let _ = b.view_add_default_elements(v, false);
        views.push(v);
    }
    if r.view_mask & 8 == 8
        && let Ok(v) = b.create_dynamic_view(parts.systems.first().cloned(), "D1")
    {
        for rel in parts.rels.iter().take(3) {
            let _ = b.dynamic_add_relationship(v, rel, &desc(2, nasty), "1");
        }
        views.push(v);
    }
    if r.view_mask & 16 == 16
        && let Ok(v) = b.create_deployment_view("Default", "DV1")
    {
        let _ = b.view_add_default_elements(v, false);
        views.push(v);
    }
    if r.view_mask & 32 == 32
        && let Ok(v) = b.create_filtered_view(
            "L1",
            "F1",
            if nasty {
                FilterMode::Exclude
            } else {
                FilterMode::Include
            },
            vec!["Person".into(), "Group".into()],
        )
    {
        views.push(v);
    }
    if r.view_mask & 64 == 64
        && let Ok(v) = b.create_custom_view("X1", parts.systems.first().cloned())
    {
        views.push(v);
    }
    if nasty {
        for (i, v) in views.iter().enumerate() {
            if let Some(base) = b.view_base_mut(*v) {
                base.title = Some(desc(i, true));
            }
        }
    }
}

/// Post-build decoration: group separators, autoLayout (incl. the
/// null-rank-direction abort leg), the `mermaid.*` gates and styles.
fn decorate(r: &Recipe, mut ws: Workspace) -> Workspace {
    if r.has(Recipe::GROUPS) {
        ws.model
            .properties
            .insert("structurizr.groupSeparator", "/");
        for p in &mut ws.model.people {
            p.group = Some("G1/G2".into());
        }
        for s in &mut ws.model.software_systems {
            s.group = Some("G1".into());
        }
    }
    let layout = match r.layout {
        1 => Some(AutomaticLayout::default()),
        2 => Some(AutomaticLayout {
            rank_direction: None,
            ..AutomaticLayout::default()
        }),
        _ => None,
    };
    for v in &mut ws.views.system_landscape_views {
        v.content.automatic_layout = layout;
    }
    for v in &mut ws.views.dynamic_views {
        v.content.automatic_layout = layout;
    }
    if r.has(Recipe::SEQUENCE) {
        for v in &mut ws.views.dynamic_views {
            v.base.properties.insert("mermaid.sequenceDiagram", "true");
        }
    }
    if r.has(Recipe::NASTY)
        && let Some(v) = ws.views.system_landscape_views.first_mut()
    {
        v.base.properties.insert("Mermaid.Title", "true");
    }
    if r.has(Recipe::STYLES) {
        ws.views
            .configuration
            .styles
            .element_styles
            .push(ElementStyle {
                tag: "Person".into(),
                background: Some("#112233".into()),
                stroke: Some("#445566".into()),
                color: Some("#778899".into()),
                shape: Some(Shape::Hexagon),
                border: Some(LineStyle::Dashed),
                ..ElementStyle::default()
            });
        ws.views
            .configuration
            .styles
            .element_styles
            .push(ElementStyle {
                tag: "Group:G1".into(),
                color: Some("#00ff00".into()),
                ..ElementStyle::default()
            });
        ws.views
            .configuration
            .styles
            .relationship_styles
            .push(RelationshipStyle {
                tag: "Relationship".into(),
                color: Some("#aabbcc".into()),
                dashed: Some(true),
                ..RelationshipStyle::default()
            });
    }
    ws
}

fn build(r: &Recipe) -> Workspace {
    let mut b = WorkspaceBuilder::new();
    b.set_workspace_name("proptest");
    b.set_workspace_description("generated");
    let parts = add_model(r, &mut b);
    add_views(r, &mut b, &parts);
    decorate(r, b.build())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn export_never_panics(recipe in recipe()) {
        let ws = build(&recipe);
        for format in FORMATS {
            let format = ExportFormat::parse(format);
            // A panic here fails the property (proptest shrinks).
            let result = format.export(&ws, "ws.dsl");
            let label = format.label();
            if always_ok(&label) {
                prop_assert!(result.is_ok(), "{label} errored: {:?}", result.err());
            }
            let Ok(files) = result else {
                continue; // the reference-abort legs (e.g. null rankDirection) stay clean Errs
            };
            let mut seen = BTreeSet::new();
            for file in &files {
                prop_assert!(!file.name.is_empty(), "empty file name");
                prop_assert!(
                    !file.name.contains('/') && !file.name.contains('\\'),
                    "path separator in file name: {}",
                    file.name
                );
                prop_assert!(
                    file.name.ends_with(&format!(".{}", expected_ext(&label))),
                    "wrong extension for {}: {}",
                    label,
                    file.name
                );
                prop_assert!(seen.insert(file.name.clone()), "duplicate file name {}", file.name);
            }
        }
    }
}
