//! §P4-M1 I-2 engine tests: per-inspection trigger snapshots (Category A,
//! `agents/plan/build/PHASE_4.md`) over hand-built fixture workspaces —
//! messages VERBATIM from the Java reference, run order = the report
//! contract. Re-bless: `UPDATE_EXPECT=1 cargo test -p oxidizr-inspect`.
#![allow(clippy::unwrap_used)]

use expect_test::expect;
use oxidizr_core::configuration::{Workspace, WorkspaceConfiguration, WorkspaceScope};
use oxidizr_core::docs::{Decision, Documentation, Format, Section};
use oxidizr_core::ids::Id;
use oxidizr_core::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, InfrastructureNode,
    Model, ModelItem, Person, Relationship, SoftwareSystem, SoftwareSystemInstance,
};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::Tags;
use oxidizr_core::views::{CustomView, ElementView, ViewBase, ViewContent};
use oxidizr_inspect::{Severity, Violation, inspect};

// -- fixture builders -----------------------------------------------------

fn item(id: &str, name: &str) -> ModelItem {
    ModelItem::new(Id::new(id), name)
}

fn person(id: &str, name: &str) -> Person {
    Person {
        item: item(id, name),
        group: None,
        relationships: Vec::new(),
    }
}

fn system(id: &str, name: &str) -> SoftwareSystem {
    SoftwareSystem {
        item: item(id, name),
        group: None,
        containers: Vec::new(),
        documentation: Documentation::default(),
        relationships: Vec::new(),
    }
}

fn container(id: &str, name: &str, technology: Option<&str>) -> Container {
    Container {
        item: item(id, name),
        group: None,
        technology: technology.map(str::to_owned),
        components: Vec::new(),
        documentation: Documentation::default(),
        relationships: Vec::new(),
    }
}

fn component(id: &str, name: &str, technology: Option<&str>) -> Component {
    Component {
        item: item(id, name),
        group: None,
        technology: technology.map(str::to_owned),
        documentation: Documentation::default(),
        relationships: Vec::new(),
    }
}

fn node(id: &str, name: &str, technology: Option<&str>) -> DeploymentNode {
    DeploymentNode {
        item: item(id, name),
        technology: technology.map(str::to_owned),
        ..DeploymentNode::default()
    }
}

fn infra(id: &str, name: &str, technology: Option<&str>) -> InfrastructureNode {
    InfrastructureNode {
        item: item(id, name),
        group: None,
        environment: "Default".into(),
        technology: technology.map(str::to_owned),
        relationships: Vec::new(),
    }
}

fn rel(
    id: &str,
    source: &str,
    destination: &str,
    description: Option<&str>,
    technology: Option<&str>,
) -> Relationship {
    Relationship {
        id: Id::new(id),
        source: Id::new(source),
        destination: Id::new(destination),
        description: description.map(str::to_owned),
        technology: technology.map(str::to_owned),
        tags: Tags::new(),
        url: None,
        properties: Properties::new(),
        perspectives: Vec::new(),
        interaction_style: None,
        linked_relationship_id: None,
    }
}

fn md(order: i32, content: &str) -> Section {
    Section {
        order,
        filename: None,
        content: content.to_owned(),
        format: Some(Format::Markdown),
        element_id: None,
    }
}

/// A custom view carrying `elements` and (optionally) a generated key.
fn custom_view(key: &str, order: i32, generated: bool, elements: &[&str]) -> CustomView {
    let mut base = ViewBase::new(key, order);
    base.generated_key = generated;
    CustomView {
        base,
        content: ViewContent {
            elements: elements
                .iter()
                .map(|e| ElementView {
                    id: Id::new(*e),
                    x: 0,
                    y: 0,
                })
                .collect(),
            relationships: Vec::new(),
            paper_size: None,
            dimensions: None,
            automatic_layout: None,
        },
        animations: Vec::new(),
        software_system_id: None,
    }
}

fn with_properties(ws: &mut Workspace, pairs: &[(&str, &str)]) {
    for (k, v) in pairs {
        ws.properties.insert(*k, *v);
    }
}

/// Snapshot format: one `type | SEVERITY | message` line per violation
/// (the reference `toString`) — the whole report is the assertion.
fn snap(violations: &[Violation]) -> String {
    let mut out = String::new();
    for v in violations {
        out.push_str(&v.to_string());
        out.push('\n');
    }
    out
}

// -- phases ---------------------------------------------------------------

#[test]
fn empty_workspace_snapshot() {
    // The bare workspace trips exactly the workspace-phase scope
    // inspection and the model-phase empty inspection, IN RUN ORDER.
    let violations = inspect(&Workspace::default());
    expect![[r#"
        workspace.scope | ERROR | This workspace has no defined scope. It is recommended that the workspace scope is set to "Landscape" or "SoftwareSystem".
        model.empty | ERROR | The model is empty.
    "#]]
    .assert_eq(&snap(&violations));
}

#[test]
fn kill_switch_and_scope_configuration() {
    // `structurizr.inspection = "false"` skips the whole run (ctor guard)…
    let mut ws = Workspace::default();
    with_properties(&mut ws, &[("structurizr.inspection", "FALSE")]);
    assert_eq!(inspect(&ws), Vec::new());
    // …and a configured scope silences the scope inspection only.
    let ws = Workspace {
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        ..Workspace::default()
    };
    let violations = inspect(&ws);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].inspection, "model.empty");
}

#[test]
fn workspace_tooling_and_documentation_snapshot() {
    let mut ws = Workspace {
        last_modified_agent: Some("structurizr-onpremises/dsl-editor/1.0".into()),
        ..Workspace::default()
    };
    ws.documentation.sections = vec![md(
        1,
        "# Title\n![alt](embed:Missing-1)\n![](embed:Gen-1)\nunrelated text\n",
    )];
    ws.views.custom_views = vec![custom_view("Gen-1", 1, true, &[])];
    let violations = inspect(&ws);
    expect![[r#"
        workspace.tooling | ERROR | The browser-based DSL editor is the easiest way to get started without installing any tooling, but it does not provide access to the full feature set of the Structurizr DSL. It is recommended that you use the Structurizr DSL in conjunction with the Structurizr CLI's "push" command.
        workspace.scope | ERROR | This workspace has no defined scope. It is recommended that the workspace scope is set to "Landscape" or "SoftwareSystem".
        documentation.embeddedview | ERROR | The following views are embedded into documentation for the workspace but do not exist in the workspace: Missing-1
        documentation.embeddedview | ERROR | The following views are embedded into documentation for the workspace via an automatically generated view key: Gen-1
        model.empty | ERROR | The model is empty.
    "#]]
    .assert_eq(&snap(&violations));
}

// -- the element ladder -----------------------------------------------------

/// The kitchen-sink model: ids arrive SHUFFLED in the vectors — the loop
/// runs in numeric-id order (`Model.getElements()` `TreeSet`), the
/// canonical names nest (`Bank/Web`, `Bank/Web/API`), and every message
/// family fires at least once.
fn kitchen_sink() -> Workspace {
    let mut alice = person("5", "Alice");
    alice.relationships = vec![Id::new("8")];
    let mut bank = system("2", "Bank");
    let mut web = container("3", "Web", None);
    let api = component("4", "API", None);
    web.components = vec![api];
    bank.containers = vec![web];
    let mail = system("6", "Mail");
    let mut node = node("7", "Server", None);
    let db = infra("9", "DB", None);
    node.infrastructure_nodes = vec![db];

    let mut ws = Workspace {
        model: Model {
            // Intentionally not id-sorted: the engine must sort.
            people: vec![alice],
            software_systems: vec![bank, mail],
            deployment_nodes: vec![node],
            relationships: vec![
                rel("8", "5", "2", None, None),
                rel("10", "3", "9", Some("Reads"), None),
            ],
            ..Model::default()
        },
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        ..Workspace::default()
    };
    // Silence the two view-side sweeps for this focused snapshot.
    ws.views.custom_views.push(custom_view(
        "v1",
        1,
        false,
        &["5", "2", "6", "3", "4", "7", "9"],
    ));
    ws
}

#[test]
fn element_ladder_run_order_snapshot() {
    let violations = inspect(&kitchen_sink());
    expect![[r#"
        model.softwaresystem.description | ERROR | The software system "Bank" is missing a description.
        model.softwaresystem.documentation | ERROR | The software system "Bank" has containers, but is missing documentation.
        model.softwaresystem.decisions | ERROR | The software system "Bank" has containers, but is missing decisions.
        model.container.description | ERROR | The container "Bank.Web" is missing a description.
        model.container.technology | ERROR | The container "Bank.Web" is missing a technology.
        model.relationship.technology | ERROR | The relationship between the container "Bank.Web" and the infrastructure node "Default/Server/DB" is missing a technology.
        model.component.description | ERROR | The component "Bank.Web.API" is missing a description.
        model.component.technology | ERROR | The component "Bank.Web.API" is missing a technology.
        model.element.disconnected | ERROR | The component "Bank.Web.API" is disconnected - add a relationship to/from it, or consider removing it from the model.
        model.person.description | ERROR | The person "Alice" is missing a description.
        model.relationship.description | ERROR | The relationship between the person "Alice" and the software system "Bank" is missing a description.
        model.relationship.technology | ERROR | The relationship between the person "Alice" and the software system "Bank" is missing a technology.
        model.softwaresystem.description | ERROR | The software system "Mail" is missing a description.
        model.element.disconnected | ERROR | The software system "Mail" is disconnected - add a relationship to/from it, or consider removing it from the model.
        model.deploymentnode.description | ERROR | The deployment node "Default/Server" is missing a description.
        model.deploymentnode.technology | ERROR | The deployment node "Default/Server" is missing a technology.
        model.infrastructurenode.description | ERROR | The infrastructure node "Default/Server/DB" is missing a description.
        model.infrastructurenode.technology | ERROR | The infrastructure node "Default/Server/DB" is missing a technology.
    "#]]
    .assert_eq(&snap(&violations));
}

#[test]
fn disconnected_and_noview_fire_for_unlinked_elements() {
    // One bare person: disconnected AND noview, in that block order.
    let mut ws = Workspace::default();
    ws.model.people = vec![person("1", "Bob")];
    ws.configuration.scope = Some(WorkspaceScope::SoftwareSystem);
    let violations = inspect(&ws);
    let kinds: Vec<&str> = violations.iter().map(|v| v.inspection).collect();
    assert_eq!(
        kinds,
        [
            "model.person.description",
            "model.element.disconnected",
            "model.element.noview",
        ]
    );
    assert_eq!(
        violations[1].message,
        "The person \"Bob\" is disconnected - add a relationship to/from it, \
         or consider removing it from the model."
    );
}

#[test]
fn terminology_override_and_instance_unwrap() {
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::Landscape);
    let mut s = system("1", "Bank");
    s.containers = vec![container("2", "Web", Some("nginx"))];
    ws.model.software_systems = vec![s];
    let mut n = node("3", "Node", Some("Linux"));
    n.software_system_instances = vec![SoftwareSystemInstance {
        item: item("4", "Bank [Live]"),
        group: None,
        environment: "Default".into(),
        software_system_id: Id::new("1"),
        instance_id: 1,
        deployment_groups: Vec::new(),
        health_checks: Vec::new(),
        relationships: Vec::new(),
    }];
    n.container_instances = vec![ContainerInstance {
        item: item("5", "Web [Live]"),
        group: None,
        environment: "Default".into(),
        container_id: Id::new("2"),
        instance_id: 1,
        deployment_groups: Vec::new(),
        health_checks: Vec::new(),
        relationships: Vec::new(),
    }];
    ws.model.deployment_nodes = vec![n];
    ws.model.relationships = vec![rel("6", "4", "5", Some("Serves"), Some("HTTP"))];
    ws.views.configuration.terminology.person = Some("Actor".into());
    ws.views.configuration.terminology.software_system = Some("Application".into());

    let violations = inspect(&ws);
    // The system description message uses the OVERRIDE, lowercased; the
    // instance messages UNWRAP to the static element's terminology
    // (`Terminology.findTerminology`) — instance "4" is a software system.
    let messages: Vec<&str> = violations.iter().map(|v| v.message.as_str()).collect();
    assert!(
        messages.contains(&"The application \"Bank\" is missing a description."),
        "{messages:?}"
    );
    assert!(
        messages.contains(&"The application named \"Bank [Live]\" is not included on any views - add it to a view."),
        "{messages:?}"
    );
    assert!(
        messages.contains(
            &"The container named \"Web [Live]\" is not included on any views - add it to a view."
        ),
        "{messages:?}"
    );
}

#[test]
fn custom_element_terminology_is_metadata_or_element() {
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::Landscape);
    let custom = CustomElement {
        item: item("1", "Queue"),
        group: None,
        metadata: Some("Message Queue".into()),
        relationships: Vec::new(),
    };
    let plain = CustomElement {
        item: item("2", "Thing"),
        group: None,
        metadata: None,
        relationships: Vec::new(),
    };
    ws.model.custom_elements = vec![custom, plain];
    let violations = inspect(&ws);
    let messages: Vec<&str> = violations.iter().map(|v| v.message.as_str()).collect();
    // Disconnected uses the canonical-name term; noview uses getName().
    assert!(
        messages.contains(&"The message queue \"Queue\" is disconnected - add a relationship to/from it, or consider removing it from the model."),
        "{messages:?}"
    );
    assert!(
        messages.contains(
            &"The element named \"Thing\" is not included on any views - add it to a view."
        ),
        "{messages:?}"
    );
    // QUIRK (faithful): Java `Model.isEmpty()` counts people + systems +
    // deployment nodes ONLY — a custom-elements-only model is "empty".
    assert!(messages.contains(&"The model is empty."));
}

// -- severity plumbing ------------------------------------------------------

#[test]
fn property_severity_overrides_and_ignore() {
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::SoftwareSystem);
    ws.model.people = vec![person("1", "Bob")];
    with_properties(
        &mut ws,
        &[
            // The exact type key downgrades; the wildcard silences.
            ("structurizr.inspection.model.person.description", "warning"),
            ("structurizr.inspection.model.element.*", "ignore"),
        ],
    );
    let violations = inspect(&ws);
    let got: Vec<(&str, Severity)> = violations
        .iter()
        .map(|v| (v.inspection, v.severity))
        .collect();
    assert_eq!(
        got,
        [
            ("model.person.description", Severity::Warning),
            ("model.element.disconnected", Severity::Ignore),
            ("model.element.noview", Severity::Ignore),
        ]
    );
    // IGNORE violations are still REPORTED by the engine (the reference
    // counts them); it is the CLI threshold that filters (I-4).
}

#[test]
fn holder_chain_element_beats_workspace() {
    // The property lives on the ELEMENT (innermost holder) — it must win
    // over the workspace-level key.
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::SoftwareSystem);
    let mut bob = person("1", "Bob");
    bob.item
        .properties
        .insert("structurizr.inspection.model.person.description", "info");
    ws.model.people = vec![bob];
    with_properties(
        &mut ws,
        &[("structurizr.inspection.model.person.description", "warning")],
    );
    let violations = inspect(&ws);
    let desc = violations
        .iter()
        .find(|v| v.inspection == "model.person.description")
        .unwrap();
    assert_eq!(desc.severity, Severity::Info);
}

#[test]
fn relationship_specific_property_beats_generic() {
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::Landscape);
    let mut a = person("1", "Alice");
    a.relationships = vec![Id::new("3"), Id::new("4")];
    ws.model.people = vec![a];
    ws.model.software_systems = vec![system("2", "Bank")];
    ws.model.custom_elements = vec![CustomElement {
        item: item("9", "Peer"),
        group: None,
        metadata: None,
        relationships: Vec::new(),
    }];
    ws.model.relationships = vec![
        rel("3", "1", "2", None, Some("HTTPS")),
        rel("4", "1", "9", None, Some("HTTPS")),
    ];
    with_properties(
        &mut ws,
        &[
            (
                "structurizr.inspection.model.relationship[person->softwaresystem].description",
                "warning",
            ),
            (
                "structurizr.inspection.model.relationship.description",
                "info",
            ),
        ],
    );
    let violations = inspect(&ws);
    let descs: Vec<Severity> = violations
        .iter()
        .filter(|v| v.inspection == "model.relationship.description")
        .map(|v| v.severity)
        .collect();
    // person→softwaresystem hits the SPECIFIC key; person→customelement
    // misses it and falls through to the generic type key.
    assert_eq!(descs, [Severity::Warning, Severity::Info]);
}

// -- embedded documentation edges -------------------------------------------

#[test]
fn embedded_views_per_documentable_and_formats() {
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::SoftwareSystem);
    let mut bank = system("1", "Bank");
    bank.item.description = Some("Core".into());
    bank.documentation.sections = vec![Section {
        order: 1,
        filename: None,
        content: "image::embed:Context[]\nnot-an-embed\n".into(),
        format: Some(Format::AsciiDoc),
        element_id: None,
    }];
    bank.documentation.decisions = vec![Decision {
        id: "1".into(),
        title: "T".into(),
        status: "Accepted".into(),
        date: 0,
        links: Vec::new(),
        content: "![](embed:Context)\n![](embed:Context)".into(), // dup dedupes
        format: Some(Format::Markdown),
        element_id: None,
    }];
    ws.model.software_systems = vec![bank];
    let violations = inspect(&ws);
    // Context is embedded (twice, deduped) but no view exists — exactly
    // ONE element-scoped violation, AsciiDoc and Markdown both parsed,
    // decisions scanned after sections.
    let embedded: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.inspection == "documentation.embeddedview")
        .collect();
    assert_eq!(embedded.len(), 1);
    assert_eq!(
        embedded[0].message,
        "The following views are embedded into documentation for the software \
         system \"Bank\" but do not exist in the workspace: Context"
    );
}

#[test]
fn embedded_key_parsing_edges() {
    // The reference regexes under Matcher.matches(): whole-line only,
    // format-gated. (Private-surface test lives in `run.rs`; here the
    // public proof is that a wrong-format line yields nothing.)
    let mut ws = Workspace::default();
    ws.configuration.scope = Some(WorkspaceScope::SoftwareSystem);
    let mut bank = system("1", "Bank");
    bank.item.description = Some("Core".into());
    bank.documentation.sections = vec![
        // AsciiDoc syntax in a MARKDOWN section (and prefix text before
        // the embed) — must NOT match.
        md(1, "see image::embed:Nope[]\ntext ![](embed:Nope2) tail\n"),
    ];
    ws.model.software_systems = vec![bank];
    let violations = inspect(&ws);
    assert!(
        !violations
            .iter()
            .any(|v| v.inspection == "documentation.embeddedview")
    );
}
