//! P1-M2 acceptance: every `testdata/validation/*.json` fixture (vendored from
//! the reference `structurizr-client` test resources) must produce EXACTLY the
//! single `WorkspaceValidationException` message that
//! `WorkspaceRulesValidationTests` asserts — and the "different environments"
//! fixture must produce NONE.
//!
//! The loader here is a deliberate MINI-READER over `serde_json::Value`
//! covering just what the fixtures use; `oxidizr-json` (M3) replaces it with
//! the full lenient reader. Nested `relationships` arrays are flattened into
//! the model's flat arena, like `JsonReader` + `Model.hydrate` do.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;

use oxidizr_core::DEFAULT_ENVIRONMENT;
use oxidizr_core::ids::Id;
use oxidizr_core::model::{
    Component, Container, ContainerInstance, DeploymentNode, InfrastructureNode, Model, ModelItem,
    Person, SoftwareSystem, SoftwareSystemInstance,
};
use oxidizr_core::model::{CustomElement, Relationship};
use oxidizr_core::tags::{InteractionStyle, Tags};
use oxidizr_core::views::{
    ComponentView, ContainerView, CustomView, DeploymentView, DynamicView, ElementView, FilterMode,
    FilteredView, ImageView, RelationshipView, SystemContextView, SystemLandscapeView, ViewBase,
    ViewContent, ViewSet,
};

// ---------------------------------------------------------------------------
// mini loader
// ---------------------------------------------------------------------------

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn i(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(Value::as_i64)
}

fn id(v: &Value, key: &str) -> Option<Id> {
    s(v, key).map(Id::new)
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    match v.get(key).and_then(Value::as_array) {
        Some(a) => a,
        None => &[],
    }
}

fn groups(v: &Value) -> Vec<String> {
    arr(v, "deploymentGroups")
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

/// Shared `ModelItem` fields of any element object.
fn item(v: &Value) -> ModelItem {
    ModelItem {
        id: id(v, "id").unwrap_or_else(|| Id::new("")),
        name: s(v, "name").unwrap_or_default().to_string(),
        description: s(v, "description").map(str::to_string),
        tags: Tags::set_from_csv(s(v, "tags").unwrap_or("")),
        url: s(v, "url").map(str::to_string),
        properties: BTreeMap::new(),
        perspectives: Vec::new(),
    }
}

/// Flatten the `relationships` array nested inside one element object.
fn relationships_of(v: &Value, out: &mut Vec<Relationship>) {
    for r in arr(v, "relationships") {
        out.push(Relationship {
            id: id(r, "id").unwrap_or_else(|| Id::new("")),
            source: id(r, "sourceId").unwrap_or_else(|| Id::new("")),
            destination: id(r, "destinationId").unwrap_or_else(|| Id::new("")),
            description: s(r, "description").map(str::to_string),
            technology: s(r, "technology").map(str::to_string),
            tags: Tags::set_from_csv(s(r, "tags").unwrap_or("")),
            url: s(r, "url").map(str::to_string),
            properties: BTreeMap::new(),
            perspectives: Vec::new(),
            interaction_style: match s(r, "interactionStyle") {
                Some("Synchronous") => Some(InteractionStyle::Synchronous),
                Some("Asynchronous") => Some(InteractionStyle::Asynchronous),
                _ => None,
            },
            linked_relationship_id: id(r, "linkedRelationshipId"),
        });
    }
}

fn component(v: &Value, rels: &mut Vec<Relationship>) -> Component {
    relationships_of(v, rels);
    Component {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        technology: s(v, "technology").map(str::to_string),
        relationships: Vec::new(),
    }
}

fn container(v: &Value, rels: &mut Vec<Relationship>) -> Container {
    relationships_of(v, rels);
    Container {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        technology: s(v, "technology").map(str::to_string),
        components: arr(v, "components")
            .iter()
            .map(|c| component(c, rels))
            .collect(),
        relationships: Vec::new(),
    }
}

fn infrastructure_node(
    v: &Value,
    parent_env: &str,
    rels: &mut Vec<Relationship>,
) -> InfrastructureNode {
    relationships_of(v, rels);
    InfrastructureNode {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        environment: s(v, "environment").unwrap_or(parent_env).to_string(),
        technology: s(v, "technology").map(str::to_string),
        relationships: Vec::new(),
    }
}

fn ssi(v: &Value, parent_env: &str, rels: &mut Vec<Relationship>) -> SoftwareSystemInstance {
    relationships_of(v, rels);
    SoftwareSystemInstance {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        environment: s(v, "environment").unwrap_or(parent_env).to_string(),
        software_system_id: id(v, "softwareSystemId").unwrap_or_else(|| Id::new("")),
        instance_id: i(v, "instanceId")
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
        deployment_groups: groups(v),
        health_checks: Vec::new(),
        relationships: Vec::new(),
    }
}

fn ci(v: &Value, parent_env: &str, rels: &mut Vec<Relationship>) -> ContainerInstance {
    relationships_of(v, rels);
    ContainerInstance {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        environment: s(v, "environment").unwrap_or(parent_env).to_string(),
        container_id: id(v, "containerId").unwrap_or_else(|| Id::new("")),
        instance_id: i(v, "instanceId")
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
        deployment_groups: groups(v),
        health_checks: Vec::new(),
        relationships: Vec::new(),
    }
}

fn node(v: &Value, rels: &mut Vec<Relationship>) -> DeploymentNode {
    relationships_of(v, rels);
    let environment = s(v, "environment")
        .unwrap_or(DEFAULT_ENVIRONMENT)
        .to_string();
    DeploymentNode {
        item: item(v),
        group: s(v, "group").map(str::to_string),
        environment: environment.clone(),
        technology: s(v, "technology").map(str::to_string),
        instances: match v.get("instances") {
            Some(Value::String(t)) => t.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => oxidizr_core::DEFAULT_INSTANCES.to_string(),
        },
        deployment_groups: groups(v),
        children: arr(v, "children").iter().map(|c| node(c, rels)).collect(),
        infrastructure_nodes: arr(v, "infrastructureNodes")
            .iter()
            .map(|n| infrastructure_node(n, &environment, rels))
            .collect(),
        software_system_instances: arr(v, "softwareSystemInstances")
            .iter()
            .map(|n| ssi(n, &environment, rels))
            .collect(),
        container_instances: arr(v, "containerInstances")
            .iter()
            .map(|n| ci(n, &environment, rels))
            .collect(),
        relationships: Vec::new(),
    }
}

fn load_model(model: &Value) -> Model {
    let mut relationships = Vec::new();
    let custom_elements = arr(model, "customElements")
        .iter()
        .map(|e| {
            relationships_of(e, &mut relationships);
            CustomElement {
                item: item(e),
                group: s(e, "group").map(str::to_string),
                relationships: Vec::new(),
            }
        })
        .collect();
    let people = arr(model, "people")
        .iter()
        .map(|e| {
            relationships_of(e, &mut relationships);
            Person {
                item: item(e),
                group: s(e, "group").map(str::to_string),
                relationships: Vec::new(),
            }
        })
        .collect();
    let software_systems = arr(model, "softwareSystems")
        .iter()
        .map(|e| {
            relationships_of(e, &mut relationships);
            SoftwareSystem {
                item: item(e),
                group: s(e, "group").map(str::to_string),
                containers: arr(e, "containers")
                    .iter()
                    .map(|c| container(c, &mut relationships))
                    .collect(),
                relationships: Vec::new(),
            }
        })
        .collect();
    let deployment_nodes = arr(model, "deploymentNodes")
        .iter()
        .map(|n| node(n, &mut relationships))
        .collect();
    Model {
        custom_elements,
        people,
        software_systems,
        deployment_nodes,
        relationships,
        properties: BTreeMap::new(),
    }
}

// ---------------------------------------------------------------------------
// views mini loader
// ---------------------------------------------------------------------------

fn base(v: &Value) -> ViewBase {
    ViewBase {
        key: s(v, "key").unwrap_or_default().to_string(),
        order: i(v, "order")
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0),
        title: s(v, "title").map(str::to_string),
        description: s(v, "description").map(str::to_string),
        properties: BTreeMap::new(),
    }
}

fn content(v: &Value) -> ViewContent {
    ViewContent {
        elements: arr(v, "elements")
            .iter()
            .map(|e| ElementView {
                id: id(e, "id").unwrap_or_else(|| Id::new("")),
                x: i(e, "x").and_then(|n| i32::try_from(n).ok()).unwrap_or(0),
                y: i(e, "y").and_then(|n| i32::try_from(n).ok()).unwrap_or(0),
            })
            .collect(),
        relationships: arr(v, "relationships")
            .iter()
            .map(|r| RelationshipView {
                id: id(r, "id").unwrap_or_else(|| Id::new("")),
                description: s(r, "description").map(str::to_string),
                url: s(r, "url").map(str::to_string),
                order: s(r, "order").map(str::to_string),
                response: r.get("response").and_then(Value::as_bool),
                technology: s(r, "technology").map(str::to_string),
                properties: BTreeMap::new(),
                vertices: Vec::new(),
                routing: None,
                jump: r.get("jump").and_then(Value::as_bool),
                position: r
                    .get("position")
                    .and_then(Value::as_i64)
                    .and_then(|n| i32::try_from(n).ok()),
            })
            .collect(),
        paper_size: None,
        dimensions: None,
        automatic_layout: None,
    }
}

fn load_views(views: &Value) -> ViewSet {
    let mut out = ViewSet::default();
    for v in arr(views, "customViews") {
        out.custom_views.push(CustomView {
            base: base(v),
            content: content(v),
            animations: Vec::new(),
        });
    }
    for v in arr(views, "systemLandscapeViews") {
        out.system_landscape_views.push(SystemLandscapeView {
            base: base(v),
            content: content(v),
        });
    }
    for v in arr(views, "systemContextViews") {
        out.system_context_views.push(SystemContextView {
            base: base(v),
            content: content(v),
            software_system_id: id(v, "softwareSystemId").unwrap_or_else(|| Id::new("")),
            enterprise_boundary_visible: v
                .get("enterpriseBoundaryVisible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    for v in arr(views, "containerViews") {
        out.container_views.push(ContainerView {
            base: base(v),
            content: content(v),
            software_system_id: id(v, "softwareSystemId").unwrap_or_else(|| Id::new("")),
            external_software_system_boundaries_visible: v
                .get("externalSoftwareSystemBoundariesVisible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    for v in arr(views, "componentViews") {
        out.component_views.push(ComponentView {
            base: base(v),
            content: content(v),
            container_id: id(v, "containerId").unwrap_or_else(|| Id::new("")),
            external_container_boundaries_visible: v
                .get("externalContainerBoundariesVisible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    for v in arr(views, "dynamicViews") {
        out.dynamic_views.push(DynamicView {
            base: base(v),
            content: content(v),
            element_id: id(v, "elementId"),
            deployment_environment: s(v, "deploymentEnvironment").map(str::to_string),
            external_boundaries_visible: v
                .get("externalBoundariesVisible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    for v in arr(views, "deploymentViews") {
        out.deployment_views.push(DeploymentView {
            base: base(v),
            content: content(v),
            environment: s(v, "deploymentEnvironment")
                .unwrap_or(DEFAULT_ENVIRONMENT)
                .to_string(),
            software_system_id: id(v, "softwareSystemId"),
            animations: Vec::new(),
        });
    }
    for v in arr(views, "filteredViews") {
        out.filtered_views.push(FilteredView {
            base: base(v),
            base_view_key: s(v, "baseViewKey").unwrap_or_default().to_string(),
            mode: if s(v, "mode") == Some("Exclude") {
                FilterMode::Exclude
            } else {
                FilterMode::Include
            },
            tags: arr(v, "tags")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            legacy_filter: s(v, "filter").map(str::to_string),
        });
    }
    for v in arr(views, "imageViews") {
        out.image_views.push(ImageView {
            base: base(v),
            element_id: id(v, "elementId"),
            content: s(v, "content").map(str::to_string),
            content_light: s(v, "contentLight").map(str::to_string),
            content_dark: s(v, "contentDark").map(str::to_string),
            content_type: s(v, "contentType").map(str::to_string),
        });
    }
    out
}

fn load(doc: &Value) -> (Model, ViewSet) {
    (
        load_model(doc.get("model").unwrap_or(&Value::Null)),
        load_views(doc.get("views").unwrap_or(&Value::Null)),
    )
}

// ---------------------------------------------------------------------------
// expectations — verbatim from `WorkspaceRulesValidationTests`
// ---------------------------------------------------------------------------

const CASES: &[(&str, Option<&str>)] = &[
    (
        "ChildDeploymentNodeNamesAreNotUnique",
        Some("A deployment node named \"Child\" already exists within \"Deployment Node\"."),
    ),
    (
        "ComponentNamesAreNotUnique",
        Some("A component named \"Component\" already exists within \"Container\"."),
    ),
    (
        "ContainerAssociatedWithComponentViewIsMissingFromTheModel",
        Some(
            "The component view with key \"Components\" is associated with a container (id=3), but that element does not exist in the model.",
        ),
    ),
    (
        "ContainerNamesAreNotUnique",
        Some("A container named \"Container\" already exists within \"Software System\"."),
    ),
    (
        "ElementAssociatedWithDynamicViewIsMissingFromTheModel",
        Some(
            "The dynamic view with key \"Dynamic\" is associated with an element (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "ElementReferencedByViewIsMissingFromTheModel",
        Some(
            "The view with key \"SystemLandscape\" references an element (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "PeopleAndSoftwareSystemNamesAreNotUnique",
        Some("A person or software system named \"Name\" already exists."),
    ),
    (
        "RelationshipDescriptionsAreNotUnique",
        Some(
            "A relationship with the description \"Uses\" already exists between \"User\" and \"Software System\".",
        ),
    ),
    (
        "RelationshipReferencedByViewIsMissingFromTheModel",
        Some(
            "The view with key \"SystemLandscape\" references a relationship (id=4), but that relationship does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithContainerViewIsMissingFromTheModel",
        Some(
            "The container view with key \"Containers\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithDeploymentViewIsMissingFromTheModel",
        Some(
            "The deployment view with key \"Deployment\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithSystemContextViewIsMissingFromTheModel",
        Some(
            "The system context view with key \"SystemContext\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    // Same name, DIFFERENT environments — the reference loads this cleanly.
    (
        "TopLevelDeploymentNodeNamesAreNotUniqueButTheyExistInDifferentEnvironments",
        None,
    ),
    (
        "TopLevelDeploymentNodeNamesAreNotUnique",
        Some(
            "A top-level deployment node named \"Deployment Node\" already exists for the environment named \"Default\".",
        ),
    ),
    (
        "ViewAssociatedWithFilteredViewIsMissingFromTheWorkspace",
        Some(
            "The filtered view with key \"Filtered\" is based upon a view (key=SystemContext), but that view does not exist in the workspace.",
        ),
    ),
    (
        "ViewKeysAreNotUnique",
        Some("A view with the key key already exists."),
    ),
];

#[test]
fn validation_catalog_matches_reference_messages() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/validation");
    for (name, expected) in CASES {
        let raw = std::fs::read_to_string(dir.join(format!("{name}.json")))
            .unwrap_or_else(|e| panic!("{name}.json: {e}"));
        let doc: Value = serde_json::from_str(&raw).expect("valid JSON");
        let (model, views) = load(&doc);
        let errors = oxidizr_core::validate(&model, &views);
        let messages: Vec<&str> = errors.iter().map(|e| e.message.as_str()).collect();
        match expected {
            None => assert!(
                messages.is_empty(),
                "{name}: expected no violations, got {messages:?}"
            ),
            Some(want) => assert_eq!(messages, [*want], "{name}"),
        }
    }
}

#[test]
fn every_fixture_is_covered() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/validation");
    let mut missing: Vec<String> = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("fixtures dir")
        .map(|e| e.expect("dir entry"))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in &entries {
        let stem = entry
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if entry.path().extension().is_some_and(|e| e == "json")
            && !CASES.iter().any(|(name, _)| *name == stem)
        {
            missing.push(stem);
        }
    }
    assert!(
        missing.is_empty(),
        "fixtures without expectations: {missing:?}"
    );
}
