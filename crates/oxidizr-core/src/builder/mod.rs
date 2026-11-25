//! The checked workspace accumulator: `WorkspaceBuilder` mirrors the
//! official add-API surface with Rust `Result`s — the DSL builder drives
//! this, the JSON reader constructs the model directly and only shares
//! [`mod@crate::validate`].
//!
//! Ids: one shared sequential counter, allocated at creation time
//! inside the add operations;
//! `found` bumps it for externally-assigned ids.

// The official add-APIs are throw-based (they raise on duplicate
// names/keys; oxidizr errors as values — every failure mode
// here is enumerated by the shared [`CoreError`] enum instead of repeated
// as per-method `# Errors`/`# Panics` sections.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod deployment;
mod elements;
mod lookups;
mod relationships;
mod setters;
mod views;

use crate::Workspace;
use crate::configuration::{Role, User, Visibility, WorkspaceScope};
use crate::error::CoreError;
use crate::ids::{Id, IdGenerator};
use crate::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, InfrastructureNode,
    Model, ModelItem, Person, Relationship, SoftwareSystem, SoftwareSystemInstance,
};
use crate::validate::ElementKind;
use crate::views::ViewSet;

/// The implied-relationship strategies the DSL can select
/// (`!impliedRelationships true|false|<fqcn>`); `Default` creates nothing and
/// is what the JSON load path uses (the strategy is a DSL-workspace default,
/// applied by the DSL parser).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImpliedRelationshipsStrategy {
    /// The default strategy — no implied
    /// relationships.
    #[default]
    Default,
    /// `CreateImpliedRelationshipsUnlessAnyRelationshipExistsStrategy` — the
    /// DSL-workspace default.
    UnlessAnyRelationshipExists,
    /// `CreateImpliedRelationshipsUnlessSameRelationshipExistsStrategy`.
    UnlessSameRelationshipExists,
}

/// Checked builder producing a [`Workspace`].
#[derive(Debug)]
pub struct WorkspaceBuilder {
    /// The workspace under construction.
    workspace: Workspace,
    /// The shared element+relationship id counter.
    id_gen: IdGenerator,
    /// Strategy applied when a static↔static relationship is added.
    implied_strategy: ImpliedRelationshipsStrategy,
}

impl Default for WorkspaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Ids of a deployment-node subtree (node, infra nodes, instances, children)
/// for [`WorkspaceBuilder::adopt`] counter seeding.
fn collect_deployment_ids(n: &crate::model::DeploymentNode, out: &mut Vec<Id>) {
    out.push(n.item.id.clone());
    out.extend(n.infrastructure_nodes.iter().map(|i| i.item.id.clone()));
    out.extend(
        n.software_system_instances
            .iter()
            .map(|i| i.item.id.clone()),
    );
    out.extend(n.container_instances.iter().map(|i| i.item.id.clone()));
    for child in &n.children {
        collect_deployment_ids(child, out);
    }
}

impl WorkspaceBuilder {
    /// A workspace with parser defaults (`name` "Name" / description
    /// "Description" are applied by the DSL layer, not here; JSON defaults the
    /// id to 0).
    #[must_use]
    pub fn new() -> Self {
        WorkspaceBuilder {
            workspace: Workspace::default(),
            id_gen: IdGenerator::new(),
            implied_strategy: ImpliedRelationshipsStrategy::Default,
        }
    }

    /// Select the implied-relationships strategy (`!impliedRelationships`;
    /// the DSL parser applies the DSL-workspace default up front).
    pub fn set_implied_strategy(&mut self, strategy: ImpliedRelationshipsStrategy) {
        self.implied_strategy = strategy;
    }

    /// The current implied-relationships strategy.
    // Official-parity surface (strategy getter/setter pair); no in-workspace
    // consumer yet — kept deliberately, not dead code to delete.
    #[allow(dead_code)]
    #[must_use]
    pub const fn implied_strategy(&self) -> ImpliedRelationshipsStrategy {
        self.implied_strategy
    }

    /// Finish: hand over the workspace.
    #[must_use]
    pub fn build(self) -> Workspace {
        self.workspace
    }

    /// The workspace under construction (read-only).
    #[must_use]
    pub const fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Mutable workspace access for the crate-internal loader layers
    /// ([`crate::viewops`]); prefer the checked methods here otherwise.
    pub(crate) fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    /// The documentation of one DSL holder — the seam the
    /// `!docs`/`!adrs` importers append to (the official contexts hold
    /// the documentation object directly).
    /// `None` id ⇒ the workspace; `Some` ⇒ the software system / container
    /// / component with that id (persons and relationships are not
    /// documented in the official model — no arm matches them).
    pub fn documentation_mut(
        &mut self,
        id: Option<&Id>,
    ) -> Option<&mut crate::docs::Documentation> {
        match id {
            None => Some(&mut self.workspace.documentation),
            Some(id) => {
                for s in &mut self.workspace.model.software_systems {
                    if &s.item.id == id {
                        return Some(&mut s.documentation);
                    }
                    for c in &mut s.containers {
                        if &c.item.id == id {
                            return Some(&mut c.documentation);
                        }
                        for k in &mut c.components {
                            if &k.item.id == id {
                                return Some(&mut k.documentation);
                            }
                        }
                    }
                }
                None
            }
        }
    }

    /// Register an externally assigned id (JSON load / `!identifiers` import).
    // Official-parity surface (external-id registration); no other consumer
    // exists — kept deliberately.
    #[allow(dead_code)]
    pub fn found(&mut self, id: &Id) {
        self.id_gen.found(id);
    }

    /// Adopt a pre-built workspace as the one under construction
    /// (`workspace extends`) — the base REPLACES the workspace object under
    /// construction; later statements layer over it. Every element and
    /// relationship id of the base is replayed through [`Self::found`] — the
    /// loaders seed the id counter on every add, JSON loads included — so
    /// subsequent [`Self::alloc_id`] continues after the base's highest id. Implied-relationship strategy
    /// and all other builder state stay as set; the DSL layer applies its
    /// workspace default after adoption.
    pub fn adopt(&mut self, workspace: Workspace) {
        let mut ids: Vec<Id> = Vec::new();
        let model = &workspace.model;
        ids.extend(model.custom_elements.iter().map(|e| e.item.id.clone()));
        ids.extend(model.people.iter().map(|e| e.item.id.clone()));
        for s in &model.software_systems {
            ids.push(s.item.id.clone());
            for c in &s.containers {
                ids.push(c.item.id.clone());
                ids.extend(c.components.iter().map(|k| k.item.id.clone()));
            }
        }
        for n in &model.deployment_nodes {
            collect_deployment_ids(n, &mut ids);
        }
        ids.extend(model.relationships.iter().map(|r| r.id.clone()));
        for id in &ids {
            self.id_gen.found(id);
        }
        self.workspace = workspace;
    }

    /// Allocate the next shared id.
    pub fn alloc_id(&mut self) -> Id {
        Id::from_num(self.id_gen.generate())
    }

    /// Set the numeric workspace id (JSON `id`).
    // Official-parity surface; the JSON reader assigns
    // `workspace.id` through `adopt` instead — kept deliberately.
    #[allow(dead_code)]
    pub fn set_workspace_id(&mut self, id: i64) {
        self.workspace.id = id;
    }

    /// DSL `workspace [name] …` / `name <name>` — `Workspace.setName`.
    pub fn set_workspace_name(&mut self, name: impl Into<String>) {
        self.workspace.name = Some(name.into());
    }

    /// DSL `description <description>` at the workspace scope —
    /// `Workspace.setDescription`.
    pub fn set_workspace_description(&mut self, description: impl Into<String>) {
        self.workspace.description = Some(description.into());
    }

    /// Set name/description/version metadata.
    pub fn set_metadata(
        &mut self,
        name: Option<String>,
        description: Option<String>,
        version: Option<String>,
    ) {
        self.workspace.name = name;
        self.workspace.description = description;
        self.workspace.version = version;
    }

    /// Set the workspace scope (`workspaceScope` / DSL `!scope`-ish loaders).
    pub fn set_scope(&mut self, scope: WorkspaceScope) {
        self.workspace.configuration.scope = Some(scope);
    }

    /// `scope none` — clears the scope.
    pub fn clear_scope(&mut self) {
        self.workspace.configuration.scope = None;
    }

    /// `visibility <private|public>` (`ConfigurationParser`).
    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.workspace.configuration.visibility = Some(visibility);
    }

    /// `users { … }` role statements: the official user set is ordered
    /// by username and silently
    /// KEEPS THE FIRST user for a repeated (exact-string) username.
    pub fn add_user(&mut self, username: &str, role: Role) {
        if !self
            .workspace
            .configuration
            .users
            .iter()
            .any(|u| u.username == username)
        {
            self.workspace.configuration.users.push(User {
                username: username.to_string(),
                role: Some(role),
            });
        }
    }

    /// Add a top-level property.
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.workspace.properties.insert(name.into(), value.into());
    }

    /// Drop a top-level property (`workspace extends` discards the base's
    /// retained `structurizr.dsl`).
    pub fn remove_property(&mut self, name: &str) {
        self.workspace.properties.remove(name);
    }

    /// Mutable access to the model (styles-of-validation escape hatch for
    /// loaders, e.g. direct `!identifiers` imports).
    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.workspace.model
    }

    // ------------------------------------------------------------------
    // lookups
    // ------------------------------------------------------------------

    fn new_item(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> ModelItem {
        let id = self.alloc_id();
        let mut item = ModelItem::new(id, name);
        item.description = filter_default(description);
        if let Some(csv) = tags_csv {
            // DSL path: `addTags(tags.split(","))` — trimmed adds.
            for t in java_split_csv(csv) {
                item.tags.add(t);
            }
        }
        item
    }

    /// Top-level duplicate-name guard: the same-name lookup covers ONLY
    /// the same kind, so a person and a software system MAY share a name
    /// (all three add paths throw the same text).
    fn check_top_level_name(
        &self,
        name: &str,
        kind: crate::validate::ElementKind,
    ) -> Result<(), CoreError> {
        use crate::validate::ElementKind as K;
        let model = &self.workspace.model;
        let taken = match kind {
            K::Person => model.people.iter().any(|p| p.item.name == name),
            K::SoftwareSystem => model.software_systems.iter().any(|s| s.item.name == name),
            K::CustomElement => model.custom_elements.iter().any(|c| c.item.name == name),
            _ => false,
        };
        if taken {
            return Err(CoreError::DuplicateTopLevelName {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// Whether an id names an element of any kind.
    #[must_use]
    pub(crate) fn has_element(&self, id: &Id) -> bool {
        crate::validate::flattened_ids(&self.workspace.model)
            .into_iter()
            .any(|(eid, _, _)| eid == id)
    }

    /// The kind label of an element id.
    #[must_use]
    pub fn element_kind(&self, id: &Id) -> Option<crate::validate::ElementKind> {
        crate::validate::flattened_ids(&self.workspace.model)
            .into_iter()
            .find(|&(eid, _, _)| eid == id)
            .map(|(_, kind, _)| kind)
    }

    /// The shared item of any element.
    #[must_use]
    pub fn item(&self, id: &Id) -> Option<&ModelItem> {
        let model = &self.workspace.model;
        if let Some(p) = model.people.iter().find(|p| &p.item.id == id) {
            return Some(&p.item);
        }
        for s in &model.software_systems {
            if &s.item.id == id {
                return Some(&s.item);
            }
            for c in &s.containers {
                if &c.item.id == id {
                    return Some(&c.item);
                }
                for k in &c.components {
                    if &k.item.id == id {
                        return Some(&k.item);
                    }
                }
            }
        }
        if let Some(c) = model.custom_elements.iter().find(|c| &c.item.id == id) {
            return Some(&c.item);
        }
        node_item(&model.deployment_nodes, id)
    }

    /// Mutable shared item of any element.
    pub(crate) fn item_mut(&mut self, id: &Id) -> Option<&mut ModelItem> {
        let id_cloned = id.clone();
        macro_rules! by_id {
            ($v:expr) => {
                $v.iter_mut()
                    .find(|x| x.item.id == id_cloned)
                    .map(|x| &mut x.item)
            };
        }
        let model = &mut self.workspace.model;
        if let Some(item) = by_id!(model.people) {
            return Some(item);
        }
        for s in &mut model.software_systems {
            if s.item.id == id_cloned {
                return Some(&mut s.item);
            }
            if let Some(item) = by_id!(s.containers) {
                return Some(item);
            }
            for c in &mut s.containers {
                if c.item.id == id_cloned {
                    return Some(&mut c.item);
                }
                if let Some(item) = by_id!(c.components) {
                    return Some(item);
                }
            }
        }
        if let Some(item) = by_id!(model.custom_elements) {
            return Some(item);
        }
        node_item_mut(&mut model.deployment_nodes, &id_cloned)
    }

    fn relationship_mut(&mut self, id: &Id) -> Option<&mut Relationship> {
        self.workspace
            .model
            .relationships
            .iter_mut()
            .find(|r| &r.id == id)
    }

    /// Borrow a model relationship by id — the read-side twin of
    /// [`Self::relationship_mut`] (was hand-rolled at ~15 call sites).
    #[must_use]
    pub(crate) fn relationship(&self, id: &Id) -> Option<&Relationship> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
    }

    fn source_relationships_mut(&mut self, source: &Id) -> Option<&mut Vec<Id>> {
        let id_cloned = source.clone();
        let model = &mut self.workspace.model;
        if let Some(p) = model.people.iter_mut().find(|p| p.item.id == id_cloned) {
            return Some(&mut p.relationships);
        }
        for s in &mut model.software_systems {
            if s.item.id == id_cloned {
                return Some(&mut s.relationships);
            }
            for c in &mut s.containers {
                if c.item.id == id_cloned {
                    return Some(&mut c.relationships);
                }
                for k in &mut c.components {
                    if k.item.id == id_cloned {
                        return Some(&mut k.relationships);
                    }
                }
            }
        }
        if let Some(c) = model
            .custom_elements
            .iter_mut()
            .find(|c| c.item.id == id_cloned)
        {
            return Some(&mut c.relationships);
        }
        node_relationships_mut(&mut model.deployment_nodes, &id_cloned)
    }

    /// Mutable access to a software system.
    pub(crate) fn system_mut(&mut self, id: &Id) -> Option<&mut SoftwareSystem> {
        self.workspace
            .model
            .software_systems
            .iter_mut()
            .find(|s| &s.item.id == id)
    }

    /// Read access to a software system.
    #[must_use]
    pub(crate) fn system_ref(&self, id: &Id) -> Option<&SoftwareSystem> {
        self.workspace
            .model
            .software_systems
            .iter()
            .find(|s| &s.item.id == id)
    }

    /// Read access to a container (any system).
    #[must_use]
    pub(crate) fn container_ref(&self, id: &Id) -> Option<&Container> {
        self.workspace
            .model
            .software_systems
            .iter()
            .find_map(|s| s.containers.iter().find(|c| &c.item.id == id))
    }

    /// Read access to a deployment node (any depth).
    #[must_use]
    pub fn node_ref(&self, id: &Id) -> Option<&DeploymentNode> {
        fn find<'a>(nodes: &'a [DeploymentNode], id: &Id) -> Option<&'a DeploymentNode> {
            for n in nodes {
                if &n.item.id == id {
                    return Some(n);
                }
                if let Some(found) = find(&n.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find(&self.workspace.model.deployment_nodes, id)
    }

    /// Mutable access to a container (any system).
    pub(crate) fn container_mut(&mut self, id: &Id) -> Option<&mut Container> {
        fn find<'a>(systems: &'a mut [SoftwareSystem], id: &Id) -> Option<&'a mut Container> {
            for s in systems {
                if let Some(c) = s.containers.iter_mut().find(|c| &c.item.id == id) {
                    return Some(c);
                }
            }
            None
        }
        find(&mut self.workspace.model.software_systems, id)
    }

    /// Mutable access to a deployment node (any depth).
    pub fn node_mut(&mut self, id: &Id) -> Option<&mut DeploymentNode> {
        fn find<'a>(nodes: &'a mut [DeploymentNode], id: &Id) -> Option<&'a mut DeploymentNode> {
            for n in nodes {
                if &n.item.id == id {
                    return Some(n);
                }
                if let Some(found) = find(&mut n.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find(&mut self.workspace.model.deployment_nodes, id)
    }

    /// Mutable view set (content operations — the view statements).
    pub fn views_mut(&mut self) -> &mut ViewSet {
        &mut self.workspace.views
    }

    /// Whether `id` is a software system.
    #[must_use]
    pub fn is_system(&self, id: &Id) -> bool {
        self.element_kind(id) == Some(crate::validate::ElementKind::SoftwareSystem)
    }

    /// Whether `id` is a container.
    #[must_use]
    pub fn is_container(&self, id: &Id) -> bool {
        self.element_kind(id) == Some(crate::validate::ElementKind::Container)
    }

    /// Mutable element wrapper (group/technology polymorphism).
    pub fn element_mut(&mut self, id: &Id) -> Option<ElementMut<'_>> {
        let id_cloned = id.clone();
        let model = &mut self.workspace.model;
        if let Some(p) = model.people.iter_mut().find(|p| p.item.id == id_cloned) {
            return Some(ElementMut::Person(p));
        }
        for s in &mut model.software_systems {
            if s.item.id == id_cloned {
                return Some(ElementMut::System(s));
            }
            for c in &mut s.containers {
                if c.item.id == id_cloned {
                    return Some(ElementMut::Container(c));
                }
                for k in &mut c.components {
                    if k.item.id == id_cloned {
                        return Some(ElementMut::Component(k));
                    }
                }
            }
        }
        if let Some(c) = model
            .custom_elements
            .iter_mut()
            .find(|c| c.item.id == id_cloned)
        {
            return Some(ElementMut::Custom(c));
        }
        node_element_mut(&mut model.deployment_nodes, &id_cloned)
    }
}

/// Polymorphic mutable element view for kind-specific fields.
#[derive(Debug)]
pub enum ElementMut<'a> {
    /// Person.
    Person(&'a mut Person),
    /// Software system.
    System(&'a mut SoftwareSystem),
    /// Container.
    Container(&'a mut Container),
    /// Component.
    Component(&'a mut Component),
    /// Custom element.
    Custom(&'a mut CustomElement),
    /// Deployment node.
    Node(&'a mut DeploymentNode),
    /// Infrastructure node.
    Infra(&'a mut InfrastructureNode),
    /// Software-system instance.
    Ssi(&'a mut SoftwareSystemInstance),
    /// Container instance.
    Ci(&'a mut ContainerInstance),
}

impl ElementMut<'_> {
    /// Set the group — every element kind is groupable
    /// (deployment elements included).
    pub fn set_group(&mut self, group: Option<String>) {
        match self {
            ElementMut::Person(p) => p.group = group,
            ElementMut::System(s) => s.group = group,
            ElementMut::Container(c) => c.group = group,
            ElementMut::Component(c) => c.group = group,
            ElementMut::Custom(c) => c.group = group,
            ElementMut::Node(n) => n.group = group,
            ElementMut::Infra(i) => i.group = group,
            ElementMut::Ssi(s) => s.group = group,
            ElementMut::Ci(c) => c.group = group,
        }
    }

    /// The stored group name.
    #[must_use]
    pub fn group(&self) -> Option<&str> {
        match self {
            ElementMut::Person(p) => p.group.as_deref(),
            ElementMut::System(s) => s.group.as_deref(),
            ElementMut::Container(c) => c.group.as_deref(),
            ElementMut::Component(c) => c.group.as_deref(),
            ElementMut::Custom(c) => c.group.as_deref(),
            ElementMut::Node(n) => n.group.as_deref(),
            ElementMut::Infra(i) => i.group.as_deref(),
            ElementMut::Ssi(s) => s.group.as_deref(),
            ElementMut::Ci(c) => c.group.as_deref(),
        }
    }

    /// Set the technology (no-op on kinds without one).
    pub fn set_technology(&mut self, technology: Option<String>) {
        match self {
            ElementMut::Container(c) => c.technology = technology,
            ElementMut::Component(c) => c.technology = technology,
            ElementMut::Node(n) => n.technology = technology,
            ElementMut::Infra(i) => i.technology = technology,
            _ => {}
        }
    }

    /// Set the custom-element `metadata` string (`CustomElementParser`).
    pub fn set_metadata(&mut self, metadata: Option<String>) {
        if let ElementMut::Custom(c) = self {
            c.metadata = metadata;
        }
    }
}

/// DSL convention: omitted positional args arrive as `""`, and empty
/// descriptions are stored as null (omitted on write).
fn filter_default(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// The format's CSV split (limit-0 semantics): an EMPTY input yields
/// `[""]`; otherwise all TRAILING empty parts are dropped (leading/middle
/// empties survive, and `","` splits to the empty array). Tag parts are
/// then added via [`crate::tags::Tags::add`] which trims — so
/// `tags "a, ,"` stores `["a", ""]` exactly like the official tooling.
pub(crate) fn java_split_csv(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let parts: Vec<&str> = s.split(',').collect();
    let end = parts
        .iter()
        .rposition(|p| !p.is_empty())
        .map_or(0, |i| i + 1);
    parts[..end].to_vec()
}

/// Model id ordering:
/// ids come from one sequential counter, so
/// creation order is ascending numeric id order. Non-numeric ids (JSON
/// loads) sort last, lexicographically.
pub(crate) fn id_sort_key(s: &str) -> (u64, String) {
    match s.parse::<u64>() {
        Ok(n) => (n, String::new()),
        Err(_) => (u64::MAX, s.to_string()),
    }
}

/// Kinds eligible for implied relationships (the
/// kind guard).
fn is_implied_kind(kind: Option<ElementKind>) -> bool {
    matches!(
        kind,
        Some(
            ElementKind::CustomElement
                | ElementKind::Person
                | ElementKind::SoftwareSystem
                | ElementKind::Container
                | ElementKind::Component
        )
    )
}

/// Deployment nodes depth-first in model order (parents before children),
/// paired with their inherited deployment-group set (`StaticStructureElement
/// Instance.getAllDeploymentGroups` reads the whole ancestor chain).
pub(crate) fn flatten_nodes(nodes: &[DeploymentNode]) -> Vec<(&DeploymentNode, Vec<String>)> {
    let mut out = Vec::new();
    walk_nodes(nodes, &[], &mut out);
    out
}

fn walk_nodes<'a>(
    nodes: &'a [DeploymentNode],
    inherited: &[String],
    out: &mut Vec<(&'a DeploymentNode, Vec<String>)>,
) {
    for n in nodes {
        let mut own = inherited.to_vec();
        for g in &n.deployment_groups {
            if !own.contains(g) {
                own.push(g.clone());
            }
        }
        out.push((n, own.clone()));
        walk_nodes(&n.children, &own, out);
    }
}

/// `DeploymentNode` tree search for the node PARENT of `id` (the parent of a
/// top-level node is `None`).
fn node_parent(nodes: &[DeploymentNode], id: &Id) -> Option<Id> {
    for n in nodes {
        if n.children.iter().any(|c| &c.item.id == id)
            || n.infrastructure_nodes.iter().any(|i| &i.item.id == id)
            || n.software_system_instances.iter().any(|i| &i.item.id == id)
            || n.container_instances.iter().any(|i| &i.item.id == id)
        {
            return Some(n.item.id.clone());
        }
        if let Some(found) = node_parent(&n.children, id) {
            return Some(found);
        }
    }
    None
}

/// `StaticStructureElementInstance.getAllDeploymentGroups`: own groups plus
/// the accumulated ancestor-node groups.
pub(crate) fn all_deployment_groups(inherited: &[String], own: &[String]) -> Vec<String> {
    let mut groups = inherited.to_vec();
    for g in own {
        if !groups.contains(g) {
            groups.push(g.clone());
        }
    }
    groups
}

/// `StaticStructureElementInstance.inSameDeploymentGroup`: both without
/// groups match; otherwise a shared group is required.
pub(crate) fn in_same_deployment_group(mine: &[String], other: &[String]) -> bool {
    if mine.is_empty() && other.is_empty() {
        return true;
    }
    mine.iter().any(|g| other.contains(g))
}

fn check_name(name: &str) -> Result<(), CoreError> {
    if name.is_empty() {
        Err(CoreError::NameRequired)
    } else {
        Ok(())
    }
}

/// `StaticStructureElementInstance` ctor: deployment groups are TRIMMED and
/// empty ones dropped.
fn clean_groups(groups: Vec<String>) -> Vec<String> {
    groups
        .into_iter()
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect()
}

fn new_node(item: ModelItem, environment: &str, technology: Option<String>) -> DeploymentNode {
    DeploymentNode {
        item,
        group: None,
        environment: if environment.is_empty() {
            crate::DEFAULT_ENVIRONMENT.to_string()
        } else {
            environment.to_string()
        },
        technology,
        instances: crate::DEFAULT_INSTANCES.to_string(),
        deployment_groups: Vec::new(),
        children: Vec::new(),
        infrastructure_nodes: Vec::new(),
        software_system_instances: Vec::new(),
        container_instances: Vec::new(),
        relationships: Vec::new(),
    }
}

/// Read-side counterpart of [`node_item_mut`]: the [`ModelItem`] of any
/// deployment element (node, infrastructure node, instances) at any depth.
fn node_item<'a>(nodes: &'a [DeploymentNode], id: &Id) -> Option<&'a ModelItem> {
    for n in nodes {
        if &n.item.id == id {
            return Some(&n.item);
        }
        for i in &n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(&i.item);
            }
        }
        for s in &n.software_system_instances {
            if &s.item.id == id {
                return Some(&s.item);
            }
        }
        for c in &n.container_instances {
            if &c.item.id == id {
                return Some(&c.item);
            }
        }
        if let Some(found) = node_item(&n.children, id) {
            return Some(found);
        }
    }
    None
}

fn node_item_mut<'a>(nodes: &'a mut [DeploymentNode], id: &Id) -> Option<&'a mut ModelItem> {
    for n in nodes {
        if &n.item.id == id {
            return Some(&mut n.item);
        }
        for i in &mut n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(&mut i.item);
            }
        }
        for s in &mut n.software_system_instances {
            if &s.item.id == id {
                return Some(&mut s.item);
            }
        }
        for c in &mut n.container_instances {
            if &c.item.id == id {
                return Some(&mut c.item);
            }
        }
        if let Some(found) = node_item_mut(&mut n.children, id) {
            return Some(found);
        }
    }
    None
}

fn node_relationships_mut<'a>(nodes: &'a mut [DeploymentNode], id: &Id) -> Option<&'a mut Vec<Id>> {
    for n in nodes {
        if &n.item.id == id {
            return Some(&mut n.relationships);
        }
        for i in &mut n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(&mut i.relationships);
            }
        }
        for s in &mut n.software_system_instances {
            if &s.item.id == id {
                return Some(&mut s.relationships);
            }
        }
        for c in &mut n.container_instances {
            if &c.item.id == id {
                return Some(&mut c.relationships);
            }
        }
        if let Some(found) = node_relationships_mut(&mut n.children, id) {
            return Some(found);
        }
    }
    None
}

fn node_element_mut<'a>(nodes: &'a mut [DeploymentNode], id: &Id) -> Option<ElementMut<'a>> {
    for n in nodes {
        if &n.item.id == id {
            return Some(ElementMut::Node(n));
        }
        for i in &mut n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(ElementMut::Infra(i));
            }
        }
        for s in &mut n.software_system_instances {
            if &s.item.id == id {
                return Some(ElementMut::Ssi(s));
            }
        }
        for c in &mut n.container_instances {
            if &c.item.id == id {
                return Some(ElementMut::Ci(c));
            }
        }
        if let Some(found) = node_element_mut(&mut n.children, id) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_share_one_counter() {
        let mut b = WorkspaceBuilder::new();
        let person = b.add_person("User", None, None).unwrap();
        let system = b.add_software_system("System", None, None).unwrap();
        let rel = b
            .add_relationship(&person, &system, Some("Uses".into()), None, None)
            .unwrap();
        assert_eq!(person.as_str(), "1");
        assert_eq!(system.as_str(), "2");
        assert_eq!(
            rel.as_str(),
            "3",
            "relationship id continues the element counter"
        );
        let w = b.build();
        assert_eq!(w.model.relationships.len(), 1);
        assert_eq!(w.model.people[0].relationships, vec![rel]);
    }

    #[test]
    fn dsl_uses_gates_duplicates_on_destination_plus_description() {
        // The DSL relationship add and the core duplicate check:
        // the duplicate throw fires only when the core add returns null —
        // same destination AND same description. Same pair with a different
        // description ADDS (`filteredviews.dsl`,
        // `dynamic-view-with-explicit-relationships.dsl`).
        let mut b = WorkspaceBuilder::new();
        let a = b.add_software_system("A", None, None).unwrap();
        let c = b.add_software_system("B", None, None).unwrap();
        b.dsl_uses(&a, &c, Some("Sync".into()), None).unwrap();
        b.dsl_uses(&a, &c, Some("Async".into()), None).unwrap();
        assert_eq!(b.build().model.relationships.len(), 2);

        let mut b = WorkspaceBuilder::new();
        let a = b.add_software_system("A", None, None).unwrap();
        let c = b.add_software_system("B", None, None).unwrap();
        b.dsl_uses(&a, &c, Some("Sync".into()), None).unwrap();
        assert_eq!(
            b.dsl_uses(&a, &c, Some("Sync".into()), None),
            Err(CoreError::DuplicateRelationship {
                source: "SoftwareSystem://A".into(),
                destination: "SoftwareSystem://B".into(),
            })
        );
    }

    #[test]
    fn top_level_names_are_per_kind_namespaces() {
        // Each top-level add guards ONLY against a same-kind collision
        // (getPersonWithName, …), all throwing the same text.
        let mut b = WorkspaceBuilder::new();
        b.add_software_system("Dup", None, None).unwrap();
        assert_eq!(
            b.add_software_system("Dup", None, None),
            Err(CoreError::DuplicateTopLevelName { name: "Dup".into() })
        );
        b.add_person("Dup", None, None).unwrap();
        b.add_custom_element("Dup", None, None).unwrap();
        assert_eq!(
            b.add_person("Dup", None, None),
            Err(CoreError::DuplicateTopLevelName { name: "Dup".into() })
        );
        assert_eq!(
            b.add_custom_element("Dup", None, None),
            Err(CoreError::DuplicateTopLevelName { name: "Dup".into() })
        );
    }

    #[test]
    fn containers_scoped_per_system() {
        let mut b = WorkspaceBuilder::new();
        let s1 = b.add_software_system("S1", None, None).unwrap();
        let s2 = b.add_software_system("S2", None, None).unwrap();
        b.add_container(&s1, "Web", None, None, None).unwrap();
        // same container name in a DIFFERENT system is fine
        b.add_container(&s2, "Web", None, None, None).unwrap();
        assert!(b.add_container(&s1, "Web", None, None, None).is_err());
    }

    #[test]
    fn dsl_tags_are_trimmed_adds() {
        let mut b = WorkspaceBuilder::new();
        let p = b.add_person("U", None, Some("Foo, Bar")).unwrap();
        assert_eq!(b.item(&p).unwrap().tags.stored(), ["Foo", "Bar"]);
    }

    #[test]
    fn instance_copies_system_name_and_adds_kind_tag() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("Bank", None, None).unwrap();
        let n = b
            .add_deployment_node("Live", "AWS", None, None, None)
            .unwrap();
        let i = b
            .add_software_system_instance(&n, &s, None, Vec::new())
            .unwrap();
        let item = b.item(&i).unwrap();
        assert_eq!(item.name, "Bank");
        assert_eq!(
            item.tags.stored(),
            [crate::tags::tag::SOFTWARE_SYSTEM_INSTANCE]
        );
    }

    #[test]
    fn view_keys_unique() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        b.create_landscape_view("L1").unwrap();
        assert_eq!(
            b.create_context_view(&s, "L1"),
            Err(CoreError::DuplicateViewKey { key: "L1".into() })
        );
    }

    #[test]
    fn adopt_replaces_workspace_and_seeds_id_counter() {
        // `workspace extends`: the base becomes the workspace under
        // construction and its ids seed the counter (the loaders seed it on
        // every add, JSON loads included).
        let mut base = WorkspaceBuilder::new();
        base.set_workspace_name("Base");
        let user = base.add_person("User", None, None).unwrap();
        let system = base.add_software_system("System", None, None).unwrap();
        let rel = base
            .add_relationship(&user, &system, Some("Uses".into()), None, None)
            .unwrap();
        assert_eq!(rel, Id::from_num(3)); // base consumed ids 1..=3
        let ws = base.build();

        let mut b = WorkspaceBuilder::new();
        b.set_workspace_name("Standalone");
        b.adopt(ws);
        assert_eq!(b.workspace().name.as_deref(), Some("Base"));
        assert_eq!(b.workspace().model.people.len(), 1);
        assert_eq!(b.workspace().model.relationships.len(), 1);
        assert_eq!(b.alloc_id(), Id::from_num(4));
        assert_eq!(b.add_person("Later", None, None).unwrap(), Id::from_num(5));
    }
}
