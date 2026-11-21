//! The `DefaultInspector` port: workspace, model and view phases
//! (§P4-M1 I-2/I-3).
//!
//! Run order IS the report contract (documented in `lib.rs`): workspace
//! inspections, then the model (every element in `ModelItem.compareTo`
//! order — numeric ids — per-kind blocks exactly as the reference
//! sequences them), each element's OUTGOING relationships in id order
//! (source-side ownership ⇒ every relationship inspected exactly once).
//!
//! Message strings are copied VERBATIM from the Java, including its
//! quirks: `terminologyFor(…).toLowerCase()` terms, `nameOf` = the
//! canonical name minus the `Type://` prefix (hierarchical, e.g.
//! `Payment System/Web`), and the noview message alone using
//! `element.getName()` instead. `EmptyModelInspection` fires exactly when
//! Java's `Model.isEmpty()` does — PEOPLE + SOFTWARE SYSTEMS + DEPLOYMENT
//! NODES only (custom elements and relationships do not count — verified
//! @ `ad34036`).

use std::collections::HashMap;
use std::collections::HashSet;

use oxidizr_core::builder::WorkspaceBuilder;
use oxidizr_core::configuration::Workspace;
use oxidizr_core::docs::{Documentation, Format};
use oxidizr_core::ids::Id;
use oxidizr_core::model::Component;
use oxidizr_core::model::Container;
use oxidizr_core::model::ContainerInstance;
use oxidizr_core::model::CustomElement;
use oxidizr_core::model::DeploymentNode;
use oxidizr_core::model::InfrastructureNode;
use oxidizr_core::model::ModelItem;
use oxidizr_core::model::Person;
use oxidizr_core::model::Relationship;
use oxidizr_core::model::SoftwareSystem;
use oxidizr_core::model::SoftwareSystemInstance;
use oxidizr_core::properties::Properties;
use oxidizr_core::views::ElementStyle;
use oxidizr_core::views::ViewBase;
use oxidizr_core::views::ViewContent;

use crate::severity::Severity;
use crate::strategy;
use crate::types;
use crate::violation::Violation;

/// Run every enabled inspection over `workspace` (the
/// `structurizr.inspection = "false"` kill switch returns nothing — the
/// `DefaultInspector` ctor guard). Violations keep the reference run
/// order.
#[must_use]
pub fn inspect(workspace: &Workspace) -> Vec<Violation> {
    if !strategy::enabled(&workspace.properties) {
        return Vec::new();
    }
    Engine::new(workspace).run()
}

/// One element, whatever its kind (the `DefaultInspector` element loop is
/// one `instanceof` ladder over `Model.getElements()`).
#[derive(Debug, Clone, Copy)]
enum Elem<'a> {
    /// `Person`.
    Person(&'a Person),
    /// `SoftwareSystem`.
    System(&'a SoftwareSystem),
    /// `Container`.
    Container(&'a Container),
    /// `Component`.
    Component(&'a Component),
    /// `DeploymentNode`.
    Node(&'a DeploymentNode),
    /// `InfrastructureNode`.
    Infra(&'a InfrastructureNode),
    /// `CustomElement`.
    Custom(&'a CustomElement),
    /// `SoftwareSystemInstance`.
    Ssi(&'a SoftwareSystemInstance),
    /// `ContainerInstance`.
    Ci(&'a ContainerInstance),
}

impl<'a> Elem<'a> {
    fn item(self) -> &'a ModelItem {
        match self {
            Elem::Person(e) => &e.item,
            Elem::System(e) => &e.item,
            Elem::Container(e) => &e.item,
            Elem::Component(e) => &e.item,
            Elem::Node(e) => &e.item,
            Elem::Infra(e) => &e.item,
            Elem::Custom(e) => &e.item,
            Elem::Ssi(e) => &e.item,
            Elem::Ci(e) => &e.item,
        }
    }

    /// `getClass().getSimpleName().toLowerCase()` — the relationship
    /// `[srcType->dstType]` operand.
    fn java_kind(self) -> &'static str {
        match self {
            Elem::Person(_) => "person",
            Elem::System(_) => "softwaresystem",
            Elem::Container(_) => "container",
            Elem::Component(_) => "component",
            Elem::Node(_) => "deploymentnode",
            Elem::Infra(_) => "infrastructurenode",
            Elem::Custom(_) => "customelement",
            Elem::Ssi(_) => "softwaresysteminstance",
            Elem::Ci(_) => "containerinstance",
        }
    }

    /// `getTechnology()` for the kinds that have one.
    fn technology(self) -> Option<&'a str> {
        match self {
            Elem::Container(c) => c.technology.as_deref(),
            Elem::Component(c) => c.technology.as_deref(),
            Elem::Node(n) => n.technology.as_deref(),
            Elem::Infra(n) => n.technology.as_deref(),
            Elem::Person(_) | Elem::System(_) | Elem::Custom(_) | Elem::Ssi(_) | Elem::Ci(_) => {
                None
            }
        }
    }

    /// The `Documentable` whose documentation the embedded-view
    /// inspections scan (`DefaultInspector` runs them for software
    /// systems, containers and components only).
    fn embedded_documentation(self) -> Option<&'a Documentation> {
        match self {
            Elem::System(s) => Some(&s.documentation),
            Elem::Container(c) => Some(&c.documentation),
            Elem::Component(c) => Some(&c.documentation),
            Elem::Person(_)
            | Elem::Node(_)
            | Elem::Infra(_)
            | Elem::Custom(_)
            | Elem::Ssi(_)
            | Elem::Ci(_) => None,
        }
    }

    /// The description inspection TYPE for the kind (`None` = the
    /// instanceof-ladder never runs one: custom elements and instances).
    fn description_type(self) -> Option<&'static str> {
        match self {
            Elem::Person(_) => Some(types::PERSON_DESCRIPTION),
            Elem::System(_) => Some(types::SOFTWARESYSTEM_DESCRIPTION),
            Elem::Container(_) => Some(types::CONTAINER_DESCRIPTION),
            Elem::Component(_) => Some(types::COMPONENT_DESCRIPTION),
            Elem::Node(_) => Some(types::DEPLOYMENTNODE_DESCRIPTION),
            Elem::Infra(_) => Some(types::INFRASTRUCTURENODE_DESCRIPTION),
            Elem::Custom(_) | Elem::Ssi(_) | Elem::Ci(_) => None,
        }
    }

    /// The technology inspection TYPE (no person/system technology
    /// exists in the reference).
    fn technology_type(self) -> Option<&'static str> {
        match self {
            Elem::Container(_) => Some(types::CONTAINER_TECHNOLOGY),
            Elem::Component(_) => Some(types::COMPONENT_TECHNOLOGY),
            Elem::Node(_) => Some(types::DEPLOYMENTNODE_TECHNOLOGY),
            Elem::Infra(_) => Some(types::INFRASTRUCTURENODE_TECHNOLOGY),
            Elem::Person(_) | Elem::System(_) | Elem::Custom(_) | Elem::Ssi(_) | Elem::Ci(_) => {
                None
            }
        }
    }
}

/// The engine: borrowed workspace + the canonical-name resolver + the
/// derived lookup tables the loops need.
struct Engine<'a> {
    ws: &'a Workspace,
    /// Canonical names over an adopted clone (the diff/reconcile
    /// precedent — `nameOf` = the canonical name minus `Type://`).
    names: WorkspaceBuilder,
    /// id → element, every kind flat (the `getElements()` merge).
    by_id: HashMap<&'a str, Elem<'a>>,
    /// id → parent id (containers→systems, components→containers,
    /// children/infra/instances→nodes — the reference `setParent` map).
    parent: HashMap<&'a str, &'a str>,
    /// element id → own properties (the severity holder chain).
    props: HashMap<&'a str, &'a Properties>,
    /// element id → outgoing relationships, id-sorted.
    outgoing: HashMap<&'a str, Vec<&'a Relationship>>,
    /// ids carrying ANY relationship, source or destination
    /// (`DisconnectedElementInspection`).
    connected: HashSet<&'a str>,
    /// ids on any MODEL view (`ElementNotIncludedInAnyViewsInspection`).
    /// OWNED ids — `View::content` borrows its local enum wrapper.
    in_views: HashSet<Id>,
    /// view key → generated-key flag (the embedded-view inspections).
    view_keys: HashMap<&'a str, bool>,
    out: Vec<Violation>,
}

/// Depth-first registration of the deployment-node tree (the reference
/// `setParent` map: every child/infra-node/instance hangs off ITS node).
fn walk_nodes<'n>(
    nodes: &'n [DeploymentNode],
    par: Option<&'n Id>,
    add: &mut impl FnMut(&'n str, Elem<'n>, Option<&'n Id>),
) {
    for n in nodes {
        add(n.item.id.as_str(), Elem::Node(n), par);
        for c in &n.children {
            walk_nodes(std::slice::from_ref(c), Some(&n.item.id), add);
        }
        for i in &n.infrastructure_nodes {
            add(i.item.id.as_str(), Elem::Infra(i), Some(&n.item.id));
        }
        for i in &n.software_system_instances {
            add(i.item.id.as_str(), Elem::Ssi(i), Some(&n.item.id));
        }
        for i in &n.container_instances {
            add(i.item.id.as_str(), Elem::Ci(i), Some(&n.item.id));
        }
    }
}

impl<'a> Engine<'a> {
    fn new(ws: &'a Workspace) -> Engine<'a> {
        let mut names = WorkspaceBuilder::new();
        names.adopt(ws.clone());

        let mut by_id = HashMap::new();
        let mut parent = HashMap::new();
        let mut props = HashMap::new();
        let mut add = |id: &'a str, elem: Elem<'a>, par: Option<&'a Id>| {
            props.insert(id, &elem.item().properties);
            if let Some(p) = par {
                parent.insert(id, p.as_str());
            }
            by_id.insert(id, elem);
        };

        for p in &ws.model.people {
            add(p.item.id.as_str(), Elem::Person(p), None);
        }
        for s in &ws.model.software_systems {
            add(s.item.id.as_str(), Elem::System(s), None);
            for c in &s.containers {
                add(c.item.id.as_str(), Elem::Container(c), Some(&s.item.id));
                for k in &c.components {
                    add(k.item.id.as_str(), Elem::Component(k), Some(&c.item.id));
                }
            }
        }
        walk_nodes(&ws.model.deployment_nodes, None, &mut add);
        for c in &ws.model.custom_elements {
            add(c.item.id.as_str(), Elem::Custom(c), None);
        }

        let mut outgoing: HashMap<&'a str, Vec<&'a Relationship>> = HashMap::new();
        let mut connected = HashSet::new();
        for r in &ws.model.relationships {
            outgoing.entry(r.source.as_str()).or_default().push(r);
            connected.insert(r.source.as_str());
            connected.insert(r.destination.as_str());
        }
        for rels in outgoing.values_mut() {
            // `Element.getRelationships()` is a TreeSet — id order.
            rels.sort_by_key(|r| r.id.clone());
        }

        let mut in_views: HashSet<Id> = HashSet::new();
        let mut view_keys = HashMap::new();
        for v in ws.views.all_views() {
            view_keys.insert(v.key(), v.base().generated_key);
            // Java: ModelView only — filtered/image views carry no entries
            // (the `View::content` `Option` is exactly that split). The
            // ids are OWNED (`View::content` borrows the local enum).
            if let Some(content) = v.content() {
                for ev in &content.elements {
                    in_views.insert(ev.id.clone());
                }
            }
        }

        Engine {
            ws,
            names,
            by_id,
            parent,
            props,
            outgoing,
            connected,
            in_views,
            view_keys,
            out: Vec::new(),
        }
    }

    fn run(mut self) -> Vec<Violation> {
        self.workspace_phase();
        self.model_phase();
        self.view_phase();
        self.out
    }

    // -- machinery -------------------------------------------------------

    fn push(&mut self, inspection: &'static str, severity: Severity, message: String) {
        self.out.push(Violation {
            inspection,
            severity,
            message,
        });
    }

    /// Workspace-level holders (`AbstractWorkspaceInspection`).
    fn ws_holders(&self) -> [Option<&'a Properties>; 1] {
        [Some(&self.ws.properties)]
    }

    /// Model-level holders (`AbstractModelInspection`: model → workspace).
    fn model_holders(&self) -> [Option<&'a Properties>; 2] {
        [Some(&self.ws.model.properties), Some(&self.ws.properties)]
    }

    /// Element holders, innermost first: element → parent → grandparent →
    /// model → workspace.
    fn element_holders(&self, id: &str) -> Vec<Option<&'a Properties>> {
        let own = self.props.get(id).copied();
        let parent = self
            .parent
            .get(id)
            .and_then(|p| self.props.get(*p).copied());
        let grand = self
            .parent
            .get(id)
            .and_then(|p| self.parent.get(*p).copied())
            .and_then(|g| self.props.get(g).copied());
        vec![
            own,
            parent,
            grand,
            Some(&self.ws.model.properties),
            Some(&self.ws.properties),
        ]
    }

    fn element_severity(&self, inspection: &'static str, id: &str) -> Severity {
        strategy::severity_from(inspection, &self.element_holders(id))
    }

    /// `nameOf`: the canonical name minus the `Type://` prefix. The
    /// fallback keeps the report total where Java's untyped model could
    /// NPE (ids dangling above the validator — the `Unresolved://`
    /// precedent in `oxidizr_api::diff`).
    fn name_of(&self, item: &ModelItem) -> String {
        self.names.canonical_name_full(&item.id).map_or_else(
            || item.name.clone(),
            |c| {
                c.find("://")
                    .map_or_else(|| c.clone(), |at| c[at + 3..].to_owned())
            },
        )
    }

    /// `terminologyFor(…).toLowerCase()` — the view-config terminology
    /// override when set, the Java default otherwise. Instances UNWRAP
    /// to their static element (`Terminology.findTerminology`).
    fn terminology(&self, e: Elem<'a>) -> String {
        let t = &self.ws.views.configuration.terminology;
        let pick = |ov: &'a Option<String>, default: &'static str| match ov {
            Some(s) if !s.is_empty() => s.as_str(),
            _ => default,
        };
        let base = match e {
            Elem::Person(_) => pick(&t.person, "Person"),
            Elem::System(_) | Elem::Ssi(_) => pick(&t.software_system, "Software System"),
            Elem::Container(_) | Elem::Ci(_) => pick(&t.container, "Container"),
            Elem::Component(_) => pick(&t.component, "Component"),
            Elem::Node(_) => pick(&t.deployment_node, "Deployment Node"),
            Elem::Infra(_) => pick(&t.infrastructure_node, "Infrastructure Node"),
            Elem::Custom(c) => match c.metadata.as_deref() {
                Some(m) if !m.is_empty() => m,
                _ => "Element",
            },
        };
        base.to_lowercase()
    }

    fn description_missing(&mut self, e: Elem<'a>) {
        let Some(ty) = e.description_type() else {
            return;
        };
        if e.item()
            .description
            .as_deref()
            .is_some_and(|d| !d.is_empty())
        {
            return;
        }
        let severity = self.element_severity(ty, e.item().id.as_str());
        let message = format!(
            "The {} \"{}\" is missing a description.",
            self.terminology(e),
            self.name_of(e.item())
        );
        self.push(ty, severity, message);
    }

    fn technology_missing(&mut self, e: Elem<'a>) {
        let Some(ty) = e.technology_type() else {
            return;
        };
        if e.technology().is_some_and(|t| !t.is_empty()) {
            return;
        }
        let severity = self.element_severity(ty, e.item().id.as_str());
        let message = format!(
            "The {} \"{}\" is missing a technology.",
            self.terminology(e),
            self.name_of(e.item())
        );
        self.push(ty, severity, message);
    }

    /// The embedded-view pair for one documentable (missing first, then
    /// generated-key — the `DefaultInspector` block order).
    fn embedded_views(&mut self, e: Elem<'a>) {
        let Some(doc) = e.embedded_documentation() else {
            return;
        };
        let keys = embedded_keys(doc);
        if keys.is_empty() {
            return;
        }
        let term = self.terminology(e);
        let name = self.name_of(e.item());
        let severity =
            self.element_severity(types::DOCUMENTATION_EMBEDDEDVIEW, e.item().id.as_str());

        let missing: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|k| !self.view_keys.contains_key(k))
            .collect();
        if !missing.is_empty() {
            self.push(
                types::DOCUMENTATION_EMBEDDEDVIEW,
                severity,
                format!(
                    "The following views are embedded into documentation for the \
                     {term} \"{name}\" but do not exist in the workspace: {}",
                    missing.join(", ")
                ),
            );
        }
        let generated: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|k| self.view_keys.get(*k).is_some_and(|g| *g))
            .collect();
        if !generated.is_empty() {
            self.push(
                types::DOCUMENTATION_EMBEDDEDVIEW,
                severity,
                format!(
                    "The following views are embedded into documentation for the \
                     {term} \"{name}\" via an automatically generated view key: {}",
                    generated.join(", ")
                ),
            );
        }
    }

    fn relationship(&mut self, r: &'a Relationship) {
        let source = self.by_id.get(r.source.as_str()).copied();
        let destination = self.by_id.get(r.destination.as_str()).copied();
        let src_kind = source.map_or("element", Elem::java_kind);
        let dst_kind = destination.map_or("element", Elem::java_kind);

        // Holder chain (inner→outer): relationship → linked → source →
        // source.parent → model → workspace.
        let linked = r
            .linked_relationship_id
            .as_ref()
            .and_then(|lid| self.ws.model.relationships.iter().find(|x| x.id == *lid));
        let mut holders: Vec<Option<&'a Properties>> = vec![Some(&r.properties)];
        if let Some(l) = linked {
            holders.push(Some(&l.properties));
        }
        if let Some(s) = source {
            holders.push(self.props.get(s.item().id.as_str()).copied());
            holders.push(
                self.parent
                    .get(s.item().id.as_str())
                    .and_then(|p| self.props.get(*p).copied()),
            );
        }
        holders.push(Some(&self.ws.model.properties));
        holders.push(Some(&self.ws.properties));

        if r.description.as_deref().is_none_or(str::is_empty) {
            let severity = strategy::relationship_severity(
                types::RELATIONSHIP_DESCRIPTION,
                src_kind,
                dst_kind,
                &holders,
            );
            let message = self.relationship_message(source, destination, "description");
            self.push(types::RELATIONSHIP_DESCRIPTION, severity, message);
        }
        if r.technology.as_ref().is_none_or(String::is_empty) {
            let severity = strategy::relationship_severity(
                types::RELATIONSHIP_TECHNOLOGY,
                src_kind,
                dst_kind,
                &holders,
            );
            let message = self.relationship_message(source, destination, "technology");
            self.push(types::RELATIONSHIP_TECHNOLOGY, severity, message);
        }
    }

    /// `The relationship between the {src} "{srcName}" and the {dst}
    /// "{dstName}" is missing a {noun}.` — dangling ends (invalid
    /// workspaces above the validator) get `element`/the raw name where
    /// Java would NPE.
    fn relationship_message(
        &self,
        src: Option<Elem<'_>>,
        dst: Option<Elem<'_>>,
        noun: &str,
    ) -> String {
        let side = |e: Option<Elem<'_>>| -> (String, String) {
            match e {
                Some(el) => (self.terminology(el), self.name_of(el.item())),
                None => ("element".to_owned(), String::new()),
            }
        };
        let (src_term, src_name) = side(src);
        let (dst_term, dst_name) = side(dst);
        format!(
            "The relationship between the {src_term} \"{src_name}\" and the \
             {dst_term} \"{dst_name}\" is missing a {noun}."
        )
    }

    // -- phases ----------------------------------------------------------

    /// `runWorkspaceInspections`.
    fn workspace_phase(&mut self) {
        // WorkspaceToolingInspection.
        if let Some(agent) = self.ws.last_modified_agent.as_deref()
            && !agent.is_empty()
            && (agent.starts_with("structurizr-cloud/dsl-editor")
                || agent.starts_with("structurizr-onpremises/dsl-editor"))
        {
            let severity = strategy::severity_from(types::WORKSPACE_TOOLING, &self.ws_holders());
            self.push(
                types::WORKSPACE_TOOLING,
                severity,
                "The browser-based DSL editor is the easiest way to get started \
                 without installing any tooling, but it does not provide access to \
                 the full feature set of the Structurizr DSL. It is recommended \
                 that you use the Structurizr DSL in conjunction with the \
                 Structurizr CLI's \"push\" command."
                    .to_owned(),
            );
        }
        // WorkspaceScopeInspection.
        if self.ws.configuration.scope.is_none() {
            let severity = strategy::severity_from(types::WORKSPACE_SCOPE, &self.ws_holders());
            self.push(
                types::WORKSPACE_SCOPE,
                severity,
                "This workspace has no defined scope. It is recommended that the \
                 workspace scope is set to \"Landscape\" or \"SoftwareSystem\"."
                    .to_owned(),
            );
        }
        // The workspace documentable pair (`run(getWorkspace())`).
        let keys = embedded_keys(&self.ws.documentation);
        if !keys.is_empty() {
            let severity =
                strategy::severity_from(types::DOCUMENTATION_EMBEDDEDVIEW, &self.ws_holders());
            let missing: Vec<&str> = keys
                .iter()
                .copied()
                .filter(|k| !self.view_keys.contains_key(k))
                .collect();
            if !missing.is_empty() {
                self.push(
                    types::DOCUMENTATION_EMBEDDEDVIEW,
                    severity,
                    format!(
                        "The following views are embedded into documentation for \
                         the workspace but do not exist in the workspace: {}",
                        missing.join(", ")
                    ),
                );
            }
            let generated: Vec<&str> = keys
                .iter()
                .copied()
                .filter(|k| self.view_keys.get(*k).is_some_and(|g| *g))
                .collect();
            if !generated.is_empty() {
                self.push(
                    types::DOCUMENTATION_EMBEDDEDVIEW,
                    severity,
                    format!(
                        "The following views are embedded into documentation for \
                         the workspace via an automatically generated view key: {}",
                        generated.join(", ")
                    ),
                );
            }
        }
    }

    /// `runModelInspections` (the element `instanceof` ladder included).
    fn model_phase(&mut self) {
        // EmptyModelInspection — Java Model.isEmpty(): people + systems +
        // deployment nodes ONLY (custom elements never count).
        if self.ws.model.people.is_empty()
            && self.ws.model.software_systems.is_empty()
            && self.ws.model.deployment_nodes.is_empty()
        {
            let severity = strategy::severity_from(types::MODEL_EMPTY, &self.model_holders());
            self.push(
                types::MODEL_EMPTY,
                severity,
                "The model is empty.".to_owned(),
            );
        }
        // MultipleSoftwareSystemsDetailedInspection (type = workspace.scope).
        let detailed = self
            .ws
            .model
            .software_systems
            .iter()
            .filter(|s| {
                // Java: hasContainers() || !documentation.isEmpty()
                !s.containers.is_empty()
                    || !s.documentation.sections.is_empty()
                    || !s.documentation.decisions.is_empty()
                    || !s.documentation.images.is_empty()
            })
            .count();
        if detailed > 1 {
            let severity = strategy::severity_from(types::WORKSPACE_SCOPE, &self.model_holders());
            self.push(
                types::WORKSPACE_SCOPE,
                severity,
                format!(
                    "This workspace describes the internal details of {detailed} \
                     software systems. It is recommended that a workspace contains \
                     the model, views, and documentation for a single software \
                     system only."
                ),
            );
        }

        // The element loop: `Model.getElements()` is a TreeSet — id order.
        let mut all: Vec<Elem<'a>> = self.by_id.values().copied().collect();
        all.sort_by_key(|e| e.item().id.clone());
        for e in all {
            self.element_inspections(e);
        }
    }

    /// The per-element block of the ladder: kind-specific inspections,
    /// then disconnected/noview, then the owned relationships — exactly
    /// the `DefaultInspector` order for one `Element`.
    fn element_inspections(&mut self, e: Elem<'a>) {
        let id = e.item().id.as_str();
        match e {
            Elem::Person(_) => self.description_missing(e),
            Elem::System(s) => {
                self.description_missing(e);
                if !s.containers.is_empty() {
                    if s.documentation.sections.is_empty() {
                        let severity =
                            self.element_severity(types::SOFTWARESYSTEM_DOCUMENTATION, id);
                        let message = format!(
                            "The {} \"{}\" has containers, but is missing \
                             documentation.",
                            self.terminology(e),
                            self.name_of(&s.item)
                        );
                        self.push(types::SOFTWARESYSTEM_DOCUMENTATION, severity, message);
                    }
                    if s.documentation.decisions.is_empty() {
                        let severity = self.element_severity(types::SOFTWARESYSTEM_DECISIONS, id);
                        let message = format!(
                            "The {} \"{}\" has containers, but is missing \
                             decisions.",
                            self.terminology(e),
                            self.name_of(&s.item)
                        );
                        self.push(types::SOFTWARESYSTEM_DECISIONS, severity, message);
                    }
                }
                self.embedded_views(e);
            }
            Elem::Container(_) | Elem::Component(_) => {
                self.description_missing(e);
                self.technology_missing(e);
                self.embedded_views(e);
            }
            Elem::Node(n) => {
                self.description_missing(e);
                self.technology_missing(e);
                if n.children.is_empty()
                    && n.software_system_instances.is_empty()
                    && n.container_instances.is_empty()
                    && n.infrastructure_nodes.is_empty()
                {
                    let severity = self.element_severity(types::DEPLOYMENTNODE_EMPTY, id);
                    let message = format!(
                        "The {} \"{}\" is empty.",
                        self.terminology(e),
                        self.name_of(&n.item)
                    );
                    self.push(types::DEPLOYMENTNODE_EMPTY, severity, message);
                }
            }
            Elem::Infra(_) => {
                self.description_missing(e);
                self.technology_missing(e);
            }
            Elem::Custom(_) | Elem::Ssi(_) | Elem::Ci(_) => {}
        }
        // DisconnectedElementInspection (deployment nodes exempt —
        // "deployment nodes typically won't have relationships").
        if !matches!(e, Elem::Node(_)) && !self.connected.contains(id) {
            let severity = self.element_severity(types::ELEMENT_DISCONNECTED, id);
            let message = format!(
                "The {} \"{}\" is disconnected - add a relationship to/from \
                 it, or consider removing it from the model.",
                self.terminology(e),
                self.name_of(e.item())
            );
            self.push(types::ELEMENT_DISCONNECTED, severity, message);
        }
        // ElementNotIncludedInAnyViewsInspection — the ONE message
        // quoting getName() (not the canonical name).
        if !self.in_views.contains(&e.item().id) {
            let severity = self.element_severity(types::ELEMENT_NOVIEW, id);
            let message = format!(
                "The {} named \"{}\" is not included on any views - add it \
                 to a view.",
                self.terminology(e),
                e.item().name
            );
            self.push(types::ELEMENT_NOVIEW, severity, message);
        }
        // Owned (outgoing) relationships, id-sorted: the Vec is COPIED
        // out so the iteration no longer borrows `self`.
        let rels: Vec<&'a Relationship> = self.outgoing.get(id).cloned().unwrap_or_default();
        for r in rels {
            self.relationship(r);
        }
    }

    // -- view phase (§P4-M1 I-3) ---------------------------------------------

    /// views-level holders (`AbstractViewsInspection`: view-config →
    /// workspace).
    fn views_holders(&self) -> [Option<&'a Properties>; 2] {
        [
            Some(&self.ws.views.configuration.properties),
            Some(&self.ws.properties),
        ]
    }

    /// per-view holders (`AbstractViewInspection`/`AbstractModelView…`:
    /// view → view-config → workspace).
    fn view_holders(&self, base: &'a ViewBase) -> [Option<&'a Properties>; 3] {
        [
            Some(&base.properties),
            Some(&self.ws.views.configuration.properties),
            Some(&self.ws.properties),
        ]
    }

    /// element-style holders (`ElementStyleMetadataInspection`: style →
    /// view-config → workspace).
    fn style_holders(&self, style: &'a ElementStyle) -> [Option<&'a Properties>; 3] {
        [
            Some(&style.properties),
            Some(&self.ws.views.configuration.properties),
            Some(&self.ws.properties),
        ]
    }

    /// `runViewInspections` — the reference order verbatim: views.empty,
    /// the per-kind model-view loops (key/empty/layout) with the two
    /// multi-system scope checks woven in at their exact positions, the
    /// generated-key-only filtered/image loops, then the element styles.
    fn view_phase(&mut self) {
        // The 'a reference is COPIED out so the loops stay out of `self`'s
        // borrow while violations are pushed.
        let ws = self.ws;
        // EmptyViewsInspection.
        if ws.views.all_views().is_empty() {
            let severity = strategy::severity_from(types::VIEWS_EMPTY, &self.views_holders());
            self.push(
                types::VIEWS_EMPTY,
                severity,
                "This workspace has no views.".to_owned(),
            );
        }
        for v in &ws.views.custom_views {
            self.model_view(&v.base, &v.content);
        }
        for v in &ws.views.system_landscape_views {
            self.model_view(&v.base, &v.content);
        }
        // SystemContextViewsForMultipleSoftwareSystemsInspection
        // (distinct `softwareSystemId`s over the SC views).
        let systems: HashSet<&Id> = ws
            .views
            .system_context_views
            .iter()
            .map(|v| &v.software_system_id)
            .collect();
        if systems.len() > 1 {
            let severity = strategy::severity_from(types::WORKSPACE_SCOPE, &self.views_holders());
            self.push(
                types::WORKSPACE_SCOPE,
                severity,
                format!(
                    "System context views exist for {} software systems. It is \
                     recommended that a workspace includes system context views \
                     for a single software system only.",
                    systems.len()
                ),
            );
        }
        for v in &ws.views.system_context_views {
            self.model_view(&v.base, &v.content);
        }
        // ContainerViewsForMultipleSoftwareSystemsInspection.
        let systems: HashSet<&Id> = ws
            .views
            .container_views
            .iter()
            .map(|v| &v.software_system_id)
            .collect();
        if systems.len() > 1 {
            let severity = strategy::severity_from(types::WORKSPACE_SCOPE, &self.views_holders());
            self.push(
                types::WORKSPACE_SCOPE,
                severity,
                format!(
                    "Container views exist for {} software systems. It is \
                     recommended that a workspace includes container views for \
                     a single software system only.",
                    systems.len()
                ),
            );
        }
        for v in &ws.views.container_views {
            self.model_view(&v.base, &v.content);
        }
        for v in &ws.views.component_views {
            self.model_view(&v.base, &v.content);
        }
        for v in &ws.views.dynamic_views {
            self.model_view(&v.base, &v.content);
        }
        for v in &ws.views.deployment_views {
            self.model_view(&v.base, &v.content);
        }
        // Filtered/image views get the generated-key inspection only.
        for v in &ws.views.filtered_views {
            self.generated_key(&v.base);
        }
        for v in &ws.views.image_views {
            self.generated_key(&v.base);
        }
        // ElementStyleMetadataInspection: fires on metadata = FALSE only
        // (`getMetadata() != null && !getMetadata()`).
        for style in &ws.views.configuration.styles.element_styles {
            if style.metadata == Some(false) {
                let severity = strategy::severity_from(
                    types::STYLE_ELEMENT_METADATA,
                    &self.style_holders(style),
                );
                self.push(
                    types::STYLE_ELEMENT_METADATA,
                    severity,
                    format!(
                        "The element style for tag \"{}\" has metadata hidden, \
                         which may introduce ambiguity on rendered diagrams.",
                        style.tag
                    ),
                );
            }
        }
    }

    /// The three inspections the reference runs for every model-view
    /// kind: `GeneratedKeyInspection`, `EmptyViewInspection`
    /// (`getElements().isEmpty()`), `ManualLayoutInspection` (generated
    /// key AND no automatic layout).
    fn model_view(&mut self, base: &'a ViewBase, content: &'a ViewContent) {
        self.generated_key(base);
        if content.elements.is_empty() {
            let severity = strategy::severity_from(types::VIEW_EMPTY, &self.view_holders(base));
            self.push(
                types::VIEW_EMPTY,
                severity,
                format!("The view with key \"{}\" is empty.", base.key),
            );
        }
        if base.generated_key && content.automatic_layout.is_none() {
            let severity = strategy::severity_from(types::VIEW_LAYOUT, &self.view_holders(base));
            self.push(
                types::VIEW_LAYOUT,
                severity,
                format!(
                    "The view with key \"{}\" has an automatically generated \
                     view key and this may cause manual layout information to \
                     be lost in the future.",
                    base.key
                ),
            );
        }
    }

    /// `GeneratedKeyInspection`.
    fn generated_key(&mut self, base: &'a ViewBase) {
        if !base.generated_key {
            return;
        }
        let severity = strategy::severity_from(types::VIEW_KEY, &self.view_holders(base));
        self.push(
            types::VIEW_KEY,
            severity,
            format!(
                "The view with key \"{}\" has an automatically generated view \
                 key and this is not guaranteed to be stable over time.",
                base.key
            ),
        );
    }
}

/// `findEmbeddedViewKeys` — the `LinkedHashSet` scan: sections then
/// decisions, line by line, `![](embed:KEY)` (Markdown) /
/// `image::embed:KEY[]` (`AsciiDoc`), first-seen order, deduped. A `null`
/// format (the nullable bean field) matches nothing, exactly like the
/// reference `if/else if` on the enum.
fn embedded_keys<'d>(doc: &'d Documentation) -> Vec<&'d str> {
    let mut keys: Vec<&'d str> = Vec::new();
    let mut scan = |content: &'d str, format: &Option<Format>| {
        for line in content.split('\n') {
            let key = match format {
                Some(Format::Markdown) => markdown_embed_key(line),
                Some(Format::AsciiDoc) => asciidoc_embed_key(line),
                None => None,
            };
            if let Some(k) = key
                && !keys.contains(&k)
            {
                keys.push(k);
            }
        }
    };
    for s in &doc.sections {
        scan(&s.content, &s.format);
    }
    for d in &doc.decisions {
        scan(&d.content, &d.format);
    }
    keys
}

/// `!\[.*?]\(embed:(.+?)\)` under `Matcher.matches()` (whole line): the
/// line starts with `![`, ends with `)`, and the KEY is everything from
/// the FIRST `](embed:` to the last `)` — the non-greedy groups only ever
/// resolve to that, because the tail `)` must consume the final
/// character.
fn markdown_embed_key(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("![")?.strip_suffix(')')?;
    let at = inner.find("](embed:")?;
    let key = &inner[at + "](embed:".len()..];
    (!key.is_empty()).then_some(key)
}

/// `image::embed:(.+?)\[\]` under `matches()`: the key is everything
/// from the fixed prefix to the trailing `[]`.
fn asciidoc_embed_key(line: &str) -> Option<&str> {
    let key = line.strip_prefix("image::embed:")?.strip_suffix("[]")?;
    (!key.is_empty()).then_some(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::docs::{Decision, Section};

    #[test]
    fn markdown_key_matches_the_java_regex_semantics() {
        // Plain.
        assert_eq!(markdown_embed_key("![](embed:My-Key)"), Some("My-Key"));
        assert_eq!(markdown_embed_key("![alt text](embed:K1)"), Some("K1"));
        // (.+?) needs a non-empty key.
        assert_eq!(markdown_embed_key("![](embed:)"), None);
        // matches() is whole-line: prefix/suffix text never matches.
        assert_eq!(markdown_embed_key("see ![](embed:K) here"), None);
        assert_eq!(markdown_embed_key("image::embed:K[]"), None);
        // The tail `)` must consume the final character: an earlier `)`
        // stays INSIDE the key (the non-greedy group extends).
        assert_eq!(markdown_embed_key("![](embed:a)b)"), Some("a)b"));
        // The FIRST `](embed:` wins (non-greedy `.*?`).
        assert_eq!(
            markdown_embed_key("![](embed:x](embed:y)"),
            Some("x](embed:y")
        );
        // No embed marker at all.
        assert_eq!(markdown_embed_key("![alt](other)"), None);
        // The suffix check runs BEFORE the marker search.
        assert_eq!(markdown_embed_key("](embed:K"), None);
    }

    #[test]
    fn asciidoc_key_matches_the_java_regex_semantics() {
        assert_eq!(asciidoc_embed_key("image::embed:My-Key[]"), Some("My-Key"));
        assert_eq!(asciidoc_embed_key("image::embed:[]"), None);
        assert_eq!(asciidoc_embed_key("image::embed:K[] trailing"), None);
        assert_eq!(asciidoc_embed_key("text image::embed:K[]"), None);
        // The `[]` group must land on the trailing characters: an earlier
        // `[]` stays inside the key.
        assert_eq!(asciidoc_embed_key("image::embed:a[]b[]"), Some("a[]b"));
    }

    #[test]
    fn embedded_keys_scans_sections_then_decisions_deduped() {
        let doc = Documentation {
            sections: vec![Section {
                order: 1,
                filename: None,
                content: "![](embed:A)\ntext\n![](embed:B)".into(),
                format: Some(Format::Markdown),
                element_id: None,
            }],
            decisions: vec![Decision {
                id: "1".into(),
                title: String::new(),
                status: String::new(),
                date: 0,
                links: Vec::new(),
                // Re-emits A (dedup keeps first position) and adds C.
                content: "image::embed:C[]\n![](embed:A)".into(),
                // One format per content: this decision is AsciiDoc, so
                // its Markdown line never matches.
                format: Some(Format::AsciiDoc),
                element_id: None,
            }],
            images: Vec::new(),
        };
        assert_eq!(embedded_keys(&doc), ["A", "B", "C"]);
    }

    #[test]
    fn embedded_keys_null_format_matches_nothing() {
        // The nullable bean field (unknown JSON `format`): the reference
        // `if/else if` on the enum skips it entirely.
        let doc = Documentation {
            sections: vec![Section {
                order: 1,
                filename: None,
                content: "![](embed:A)".into(),
                format: None,
                element_id: None,
            }],
            decisions: Vec::new(),
            images: Vec::new(),
        };
        assert_eq!(embedded_keys(&doc), [] as [&str; 0]);
    }
}
