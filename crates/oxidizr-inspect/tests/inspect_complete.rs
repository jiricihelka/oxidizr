//! §P4-M1 I-3: a COMPLETE workspace must inspect clean. The original
//! plan item read "a fully-documented workspace yields `[]`" — this is
//! that property, over generated content: every description/technology
//! filled, systems-with-containers carrying documentation AND decisions,
//! every element related and on a view, a configured scope, no embed
//! keys, no generated view keys, no hidden-metadata styles.
#![allow(clippy::unwrap_used)]

use oxidizr_core::configuration::{Workspace, WorkspaceConfiguration, WorkspaceScope};
use oxidizr_core::docs::{Decision, Documentation, Format, Section};
use oxidizr_core::ids::Id;
use oxidizr_core::model::{
    Component, Container, Model, ModelItem, Person, Relationship, SoftwareSystem,
};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::Tags;
use oxidizr_core::views::{CustomView, ElementView, ViewBase, ViewContent, ViewSet};
use oxidizr_inspect::inspect;
use proptest::prelude::*;

fn item(id: &str, name: &str, description: &str) -> ModelItem {
    let mut i = ModelItem::new(Id::new(id), name);
    i.description = Some(description.to_owned());
    i
}

fn rel(id: &str, source: &str, destination: &str) -> Relationship {
    Relationship {
        id: Id::new(id),
        source: Id::new(source),
        destination: Id::new(destination),
        description: Some("Uses".into()),
        technology: Some("HTTPS".into()),
        tags: Tags::new(),
        url: None,
        properties: Properties::new(),
        perspectives: Vec::new(),
        interaction_style: None,
        linked_relationship_id: None,
    }
}

/// Fresh numeric id (never collides with the fixed 1–9 block).
fn mint(next: &mut i64) -> Id {
    *next += 1;
    Id::new(next.to_string())
}

/// The documentation the `has containers, but is missing …` pair
/// requires (sections AND decisions).
fn docs(sys: &str) -> Documentation {
    Documentation {
        sections: vec![Section {
            order: 1,
            filename: None,
            content: format!("The {sys} documentation."),
            format: Some(Format::Markdown),
            element_id: None,
        }],
        decisions: vec![Decision {
            id: "1".into(),
            title: "Use Rust".into(),
            status: "Accepted".into(),
            date: 1_700_000_000,
            links: Vec::new(),
            content: "We decided to use Rust.".into(),
            format: Some(Format::Markdown),
            element_id: None,
        }],
        images: Vec::new(),
    }
}

/// The container/component blocks, wiring one relationship per element
/// (endpoints and relationship ids minted from `next`, collected into
/// `rels`/`element_ids`).
fn children(
    container: &str,
    component: &str,
    containers: u8,
    components: u8,
    next: &mut i64,
    rels: &mut Vec<Relationship>,
    element_ids: &mut Vec<Id>,
) -> Vec<Container> {
    let mut out = Vec::new();
    for c in 0..i64::from(containers) {
        let cid = mint(next);
        let mut container = Container {
            item: item(
                cid.as_str(),
                &format!("{container} {c}"),
                "A complete container.",
            ),
            group: None,
            technology: Some("Rust".into()),
            components: Vec::new(),
            documentation: Documentation::default(),
            relationships: Vec::new(),
        };
        rels.push(rel(&mint(next).to_string(), cid.as_str(), "2")); // connected
        element_ids.push(cid.clone());
        for k in 0..i64::from(components) {
            let kid = mint(next);
            container.components.push(Component {
                item: item(
                    kid.as_str(),
                    &format!("{component} {c}-{k}"),
                    "A component.",
                ),
                group: None,
                technology: Some("WASM".into()),
                documentation: Documentation::default(),
                relationships: Vec::new(),
            });
            rels.push(rel(&mint(next).to_string(), kid.as_str(), cid.as_str()));
            element_ids.push(kid);
        }
        out.push(container);
    }
    out
}

/// A fully-documented workspace from generated labels: one person, one
/// software system with `containers` containers of `components`
/// components each.
fn complete_workspace(
    sys: &str,
    person: &str,
    container: &str,
    component: &str,
    containers: u8,
    components: u8,
) -> Workspace {
    // Distinct numeric ids throughout: person 1, system 2, rel 3, then
    // the minted container/component/relationship ids above 10.
    let mut next = 10i64;
    let mut rels = vec![rel("3", "1", "2")]; // person → system
    let mut element_ids = vec![Id::new("1"), Id::new("2")];
    let system = SoftwareSystem {
        item: item("2", sys, "A complete system."),
        group: None,
        containers: children(
            container,
            component,
            containers,
            components,
            &mut next,
            &mut rels,
            &mut element_ids,
        ),
        // With containers the system needs BOTH documentation and
        // decisions (the two `has containers, but is missing …` checks).
        documentation: docs(sys),
        relationships: Vec::new(),
    };

    // ONE custom view (non-generated key) carrying EVERY element id:
    // silences noview and keeps the view non-empty.
    let view = CustomView {
        base: ViewBase::new("main", 1),
        content: ViewContent {
            elements: element_ids
                .into_iter()
                .map(|e| ElementView { id: e, x: 0, y: 0 })
                .collect(),
            ..ViewContent::default()
        },
        animations: Vec::new(),
        software_system_id: None,
    };

    Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::SoftwareSystem),
            ..WorkspaceConfiguration::default()
        },
        model: Model {
            people: vec![Person {
                item: item("1", person, "A complete person."),
                group: None,
                relationships: Vec::new(),
            }],
            software_systems: vec![system],
            relationships: rels,
            ..Model::default()
        },
        views: ViewSet {
            custom_views: vec![view],
            ..ViewSet::default()
        },
        ..Workspace::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn a_complete_workspace_inspects_clean(
        sys in "[A-Za-z][A-Za-z0-9]{0,8}",
        person in "[A-Za-z][A-Za-z0-9]{0,8}",
        container in "[A-Za-z][A-Za-z0-9]{0,8}",
        component in "[A-Za-z][A-Za-z0-9]{0,8}",
        containers in 1u8..3,
        components in 1u8..3,
    ) {
        let ws = complete_workspace(&sys, &person, &container, &component, containers, components);
        let violations = inspect(&ws);
        prop_assert_eq!(violations.clone(), Vec::new(), "unexpected: {:?}", violations);
    }
}

#[test]
fn minimal_complete_workspace_is_clean() {
    // The proptest floor as a plain unit test too (fast, no RNG).
    let ws = complete_workspace("Bank", "Alice", "Web", "API", 1, 1);
    assert_eq!(inspect(&ws), Vec::new());
}
