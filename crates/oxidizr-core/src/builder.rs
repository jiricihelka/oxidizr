//! The checked workspace accumulator (`P1-D4`): `WorkspaceBuilder` mirrors the
//! reference `Model.add*` / `ViewSet.add*` API surface with Rust `Result`s —
//! the DSL builder (M4) drives this, the JSON reader (M3) constructs the model
//! directly and only shares [`crate::validate`].
//!
//! Ids: one shared sequential counter (Locked), allocated at creation time
//! exactly like `Model.addRelationship`/`addElementToInternalStructures`;
//! `found` bumps it for externally-assigned ids.

// The reference `Model`/`ViewSet` add-APIs are throw-based (they raise
// `IllegalArgumentException` on duplicate names/keys); every failure mode
// here is enumerated by the shared [`CoreError`] enum instead of repeated
// as per-method `# Errors`/`# Panics` sections.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::Workspace;
use crate::configuration::WorkspaceScope;
use crate::error::CoreError;
use crate::ids::{Id, IdGenerator};
use crate::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, InfrastructureNode,
    Model, ModelItem, Person, Perspective, Relationship, SoftwareSystem, SoftwareSystemInstance,
};
use crate::tags::Tags;
use crate::validate::ElementKind;
use crate::viewops::ViewRef;
use crate::views::{
    ContainerView, CustomView, DeploymentView, DynamicView, FilteredView, ImageView,
    SystemContextView, SystemLandscapeView, ViewBase, ViewContent, ViewSet,
};

/// The `Model.ImpliedRelationshipsStrategy` variants the DSL can select
/// (`!impliedRelationships true|false|<fqcn>`); `Default` creates nothing and
/// is what the JSON load path uses (the strategy is a DSL-workspace default,
/// `WorkspaceParser.parse`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImpliedRelationshipsStrategy {
    /// Reference `DefaultImpliedRelationshipsStrategy` — no implied
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

impl WorkspaceBuilder {
    /// A workspace with reference parser defaults (`name` "Name" / description
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

    /// Register an externally assigned id (JSON load / `!identifiers` import).
    pub fn found(&mut self, id: &Id) {
        self.id_gen.found(id);
    }

    /// Allocate the next shared id.
    pub fn alloc_id(&mut self) -> Id {
        Id::from_num(self.id_gen.generate())
    }

    /// Set the numeric workspace id (JSON `id`).
    pub fn set_workspace_id(&mut self, id: i64) {
        self.workspace.id = id;
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

    /// Add a top-level property.
    pub fn set_property(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.workspace.properties.insert(name.into(), value.into());
    }

    // ------------------------------------------------------------------
    // elements
    // ------------------------------------------------------------------

    /// `person "Name" ["Desc"] ["Tags"]`.
    pub fn add_person(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name)?;
        let item = self.new_item(name, description, tags_csv);
        self.workspace.model.people.push(Person {
            item: item.clone(),
            group: None,
            relationships: Vec::new(),
        });
        Ok(item.id)
    }

    /// `softwareSystem "Name" ["Desc"] ["Tags"]`.
    pub fn add_software_system(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name)?;
        let item = self.new_item(name, description, tags_csv);
        self.workspace.model.software_systems.push(SoftwareSystem {
            item: item.clone(),
            group: None,
            containers: Vec::new(),
            documentation: crate::docs::Documentation::default(),
            relationships: Vec::new(),
        });
        Ok(item.id)
    }

    /// `element "Name" ["metadata"] ["Desc"] ["Tags"]` — custom element.
    pub fn add_custom_element(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name)?;
        let item = self.new_item(name, description, tags_csv);
        self.workspace.model.custom_elements.push(CustomElement {
            item: item.clone(),
            group: None,
            metadata: None,
            relationships: Vec::new(),
        });
        Ok(item.id)
    }

    /// `container "Name" ["Desc"] ["Tech"] ["Tags"]` in `system`.
    pub fn add_container(
        &mut self,
        system: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let system_name = self
            .system_ref(system)
            .map(|s| s.item.name.clone())
            .ok_or_else(|| CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            })?;
        if self
            .system_ref(system)
            .is_some_and(|s| s.containers.iter().any(|c| c.item.name == name))
        {
            return Err(CoreError::DuplicateContainerName {
                name: name.to_string(),
                system: system_name,
            });
        }
        let item = self.new_item(name, description, tags_csv);
        self.system_mut(system)
            .expect("checked above")
            .containers
            .push(Container {
                item: item.clone(),
                group: None,
                technology,
                components: Vec::new(),
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(item.id)
    }

    /// `component "Name" ["Desc"] ["Tech"] ["Tags"]` in `container`.
    pub fn add_component(
        &mut self,
        container: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let container_name = self
            .container_ref(container)
            .map(|c| c.item.name.clone())
            .ok_or_else(|| CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            })?;
        if self
            .container_ref(container)
            .is_some_and(|c| c.components.iter().any(|k| k.item.name == name))
        {
            return Err(CoreError::DuplicateComponentName {
                name: name.to_string(),
                container: container_name,
            });
        }
        let item = self.new_item(name, description, tags_csv);
        self.container_mut(container)
            .expect("checked above")
            .components
            .push(Component {
                item: item.clone(),
                group: None,
                technology,
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(item.id)
    }

    /// `deploymentNode "Name" …` as a top-level node in `environment`.
    pub fn add_deployment_node(
        &mut self,
        environment: &str,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        if self
            .workspace
            .model
            .deployment_nodes
            .iter()
            .any(|n| n.item.name == name && n.environment == environment)
        {
            return Err(CoreError::DuplicateDeploymentNodeName {
                name: name.to_string(),
                environment: environment.to_string(),
            });
        }
        let item = self.new_item(name, description, tags_csv);
        self.workspace
            .model
            .deployment_nodes
            .push(new_node(&item, environment, technology));
        Ok(item.id)
    }

    /// A child `deploymentNode` under `parent`.
    pub fn add_child_deployment_node(
        &mut self,
        parent: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let env = {
            let p = self
                .node_mut(parent)
                .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: parent.clone() })?;
            // `Model.addDeploymentNode(parent, …): the child name must be
            // unique among the parent's child nodes AND infrastructure nodes.
            if p.children.iter().any(|c| c.item.name == name)
                || p.infrastructure_nodes.iter().any(|i| i.item.name == name)
            {
                return Err(CoreError::DuplicateChildDeploymentNodeName {
                    name: name.to_string(),
                    parent: p.item.name.clone(),
                });
            }
            p.environment.clone()
        };
        let item = self.new_item(name, description, tags_csv);
        self.node_mut(parent)
            .expect("just found")
            .children
            .push(new_node(&item, &env, technology));
        Ok(item.id)
    }

    /// `infrastructureNode "Name" ["Desc"] ["Tech"] ["Tags"]` under `node`.
    pub fn add_infrastructure_node(
        &mut self,
        node: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        let environment = {
            let parent = self
                .node_ref(node)
                .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
            // `Model.addInfrastructureNode`: the name must be unique among the
            // parent's CHILD NODES and INFRASTRUCTURE NODES alike.
            if parent.children.iter().any(|c| c.item.name == name)
                || parent
                    .infrastructure_nodes
                    .iter()
                    .any(|i| i.item.name == name)
            {
                return Err(CoreError::DuplicateInfrastructureNodeName {
                    name: name.to_string(),
                    parent: parent.item.name.clone(),
                });
            }
            parent.environment.clone()
        };
        let item = self.new_item(name, description, tags_csv);
        self.node_mut(node)
            .expect("checked above")
            .infrastructure_nodes
            .push(InfrastructureNode {
                item: item.clone(),
                group: None,
                environment,
                technology,
                relationships: Vec::new(),
            });
        Ok(item.id)
    }

    /// `softwareSystemInstance <systemId> …` under `node`.
    pub fn add_software_system_instance(
        &mut self,
        node: &Id,
        system: &Id,
        tags_csv: Option<&str>,
        deployment_groups: Vec<String>,
    ) -> Result<Id, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        // Reference: the instance NAME is derived (getElement().getName(),
        // @JsonIgnore — never serialized); we mirror it for diagnostics.
        let name = self.item(system).expect("system exists").name.clone();
        let mut item = self.new_item(&name, None, tags_csv);
        item.tags.add(crate::tags::tag::SOFTWARE_SYSTEM_INSTANCE);
        let id = item.id.clone();
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        // `Model.addSoftwareSystemInstance`: instanceId counts instances of the
        // SAME system within this node (1-based: reference starts at 1).
        let instance_id = node
            .software_system_instances
            .iter()
            .filter(|i| &i.software_system_id == system)
            .count()
            .saturating_add(1)
            .try_into()
            .unwrap_or(i32::MAX);
        let environment = node.environment.clone();
        node.software_system_instances.push(SoftwareSystemInstance {
            item,
            group: None,
            environment,
            software_system_id: system.clone(),
            instance_id,
            deployment_groups: clean_groups(deployment_groups),
            health_checks: Vec::new(),
            relationships: Vec::new(),
        });
        self.replicate_element_relationships(&id);
        Ok(id)
    }

    /// `containerInstance <containerId> …` under `node`.
    pub fn add_container_instance(
        &mut self,
        node: &Id,
        container: &Id,
        tags_csv: Option<&str>,
        deployment_groups: Vec<String>,
    ) -> Result<Id, CoreError> {
        if !self.is_container(container) {
            return Err(CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            });
        }
        let name = self.item(container).expect("container exists").name.clone();
        let mut item = self.new_item(&name, None, tags_csv);
        item.tags.add(crate::tags::tag::CONTAINER_INSTANCE);
        let id = item.id.clone();
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        // Same per-node per-container 1-based count as `Model.addContainerInstance`.
        let instance_id = node
            .container_instances
            .iter()
            .filter(|i| &i.container_id == container)
            .count()
            .saturating_add(1)
            .try_into()
            .unwrap_or(i32::MAX);
        let environment = node.environment.clone();
        node.container_instances.push(ContainerInstance {
            item,
            group: None,
            environment,
            container_id: container.clone(),
            instance_id,
            deployment_groups: clean_groups(deployment_groups),
            health_checks: Vec::new(),
            relationships: Vec::new(),
        });
        self.replicate_element_relationships(&id);
        Ok(id)
    }

    // ------------------------------------------------------------------
    // relationships
    // ------------------------------------------------------------------

    /// `[id = ]a -> b ["Description"] ["Technology"] ["Tags"]` (or the typed
    /// `a.uses(b, …)` DSL forms, all funnel here).
    ///
    /// Mirrors `Model.addRelationship`: parent/child pairs are rejected,
    /// an efferent source→destination relationship with the same
    /// (normalized) description is a duplicate (`Element.has`), and the
    /// configured [`ImpliedRelationshipsStrategy`] runs for static↔static
    /// pairs exactly like the reference.
    pub fn add_relationship(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        if !self.has_element(source) {
            return Err(CoreError::ElementNotFound { id: source.clone() });
        }
        if !self.has_element(destination) {
            return Err(CoreError::ElementNotFound {
                id: destination.clone(),
            });
        }
        if self.is_child_of(source, destination) || self.is_child_of(destination, source) {
            return Err(CoreError::ParentChildRelationship);
        }
        // `Element.has(Relationship)`: same destination AND same description
        // (technology is NOT part of the check).
        if self.has_efferent(source, destination, description.as_deref().unwrap_or("")) {
            return Err(CoreError::DuplicateRelationship {
                source: self.canonical_name(source),
                destination: self.canonical_name(destination),
            });
        }
        let mut tags = Tags::new();
        if let Some(csv) = tags_csv {
            // reference DSL: `addTags(tags.split(","))` — TRIMMED adds (unlike
            // JSON's setTags).
            for t in csv.split(',') {
                tags.add(t);
            }
        }
        let id = self.insert_relationship(source, destination, description, technology, tags, None);
        // `Model.addRelationship`: the strategy runs only when BOTH ends are
        // static-structure-ish (custom/person/system/container/component).
        if self.implied_strategy != ImpliedRelationshipsStrategy::Default
            && is_implied_kind(self.element_kind(source))
            && is_implied_kind(self.element_kind(destination))
        {
            self.create_implied_relationships(&id);
        }
        Ok(id)
    }

    /// Raw relationship insert (shared id counter, `source.relationships`
    /// index) — the `Model.addRelationship(relationship)` internals without
    /// the guards; used by the implied and replication paths, which pre-check
    /// duplicates exactly like the reference does.
    pub(crate) fn insert_relationship(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
        tags: Tags,
        linked_relationship_id: Option<&Id>,
    ) -> Id {
        let id = self.alloc_id();
        self.workspace.model.relationships.push(Relationship {
            id: id.clone(),
            source: source.clone(),
            destination: destination.clone(),
            description,
            technology,
            tags,
            url: None,
            properties: crate::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: linked_relationship_id.cloned(),
        });
        if let Some(rels) = self.source_relationships_mut(source) {
            rels.push(id.clone());
        }
        id
    }

    /// `AbstractImpliedRelationshipsStrategy` ancestor walk: for every
    /// (source-ancestor, destination-ancestor) pair allowed by
    /// `impliedRelationshipIsAllowed`, create the implied copy unless the
    /// strategy's efferent check says otherwise. Order matches the reference
    /// loops (outer: source chain; inner: destination chain reset per step).
    fn create_implied_relationships(&mut self, base_id: &Id) {
        let base = self
            .workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == base_id)
            .expect("just added");
        let (src, dst, desc, tech) = (
            base.source.clone(),
            base.destination.clone(),
            base.description.clone(),
            base.technology.clone(),
        );
        let unless_same =
            self.implied_strategy == ImpliedRelationshipsStrategy::UnlessSameRelationshipExists;
        let mut source = Some(src);
        while let Some(s) = source {
            let mut destination = Some(dst.clone());
            while let Some(d) = destination {
                if self.implied_relationship_is_allowed(&s, &d) {
                    let create = if unless_same {
                        !self.has_efferent(&s, &d, desc.as_deref().unwrap_or(""))
                    } else {
                        !self.has_any_efferent(&s, &d)
                    };
                    if create {
                        let base_id = base_id.clone();
                        self.insert_relationship(
                            &s,
                            &d,
                            desc.clone(),
                            tech.clone(),
                            Tags::new(),
                            Some(&base_id),
                        );
                    }
                }
                destination = self.element_parent(&d);
            }
            source = self.element_parent(&s);
        }
    }

    /// `AbstractImpliedRelationshipsStrategy.impliedRelationshipIsAllowed`:
    /// not equal, and no ancestor/descendant pair (persons exempt).
    fn implied_relationship_is_allowed(&self, source: &Id, destination: &Id) -> bool {
        source != destination
            && !self.is_child_of(source, destination)
            && !self.is_child_of(destination, source)
    }

    /// Reference `Model.replicateElementRelationships`: when a
    /// software-system/container instance joins the model, replicate matching
    /// static-model relationships between every instance in the SAME
    /// environment and deployment group. Replicated relationships carry EMPTY
    /// tags and a `linkedRelationshipId`.
    pub(crate) fn replicate_element_relationships(&mut self, instance_id: &Id) {
        let Some((my_static, my_env, my_groups)) = self.instance_profile(instance_id) else {
            return;
        };
        // Java iterates a TreeSet ordered by numeric id == insertion order.
        let others: Vec<(Id, Id, String, Vec<String>)> = self
            .all_instances()
            .into_iter()
            .filter(|(_, _, env, _)| *env == my_env)
            .filter(|(_, _, _, groups)| in_same_deployment_group(&my_groups, groups))
            .collect();
        for (other_id, other_static, _, _) in others {
            // element (new instance's static element) -> other static element
            let fwd_ids: Vec<Id> = self
                .workspace
                .model
                .relationships
                .iter()
                .filter(|r| r.source == my_static && r.destination == other_static)
                .map(|r| r.id.clone())
                .collect();
            for base_id in fwd_ids {
                let (desc, tech) = {
                    let base = self
                        .workspace
                        .model
                        .relationships
                        .iter()
                        .find(|r| r.id == base_id)
                        .expect("model relationship");
                    (base.description.clone(), base.technology.clone())
                };
                if self.has_efferent(instance_id, &other_id, desc.as_deref().unwrap_or("")) {
                    continue; // Java: addRelationship returns null → skipped
                }
                self.insert_relationship(
                    instance_id,
                    &other_id,
                    desc,
                    tech,
                    Tags::new(),
                    Some(&base_id),
                );
            }
            // other static element -> element
            let rev_ids: Vec<Id> = self
                .workspace
                .model
                .relationships
                .iter()
                .filter(|r| r.source == other_static && r.destination == my_static)
                .map(|r| r.id.clone())
                .collect();
            for base_id in rev_ids {
                let (desc, tech) = {
                    let base = self
                        .workspace
                        .model
                        .relationships
                        .iter()
                        .find(|r| r.id == base_id)
                        .expect("model relationship");
                    (base.description.clone(), base.technology.clone())
                };
                if self.has_efferent(&other_id, instance_id, desc.as_deref().unwrap_or("")) {
                    continue;
                }
                self.insert_relationship(
                    &other_id,
                    instance_id,
                    desc,
                    tech,
                    Tags::new(),
                    Some(&base_id),
                );
            }
        }
    }

    /// `(static element id, environment, all deployment groups)` of an
    /// instance id, or `None` when it is not an instance.
    fn instance_profile(&self, id: &Id) -> Option<(Id, String, Vec<String>)> {
        for (node, inherited) in flatten_nodes(&self.workspace.model.deployment_nodes) {
            for s in &node.software_system_instances {
                if &s.item.id == id {
                    return Some((
                        s.software_system_id.clone(),
                        s.environment.clone(),
                        all_deployment_groups(&inherited, &s.deployment_groups),
                    ));
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return Some((
                        c.container_id.clone(),
                        c.environment.clone(),
                        all_deployment_groups(&inherited, &c.deployment_groups),
                    ));
                }
            }
        }
        None
    }

    /// Every software-system and container instance as
    /// `(instance id, static element id, environment, groups)` in id order.
    fn all_instances(&self) -> Vec<(Id, Id, String, Vec<String>)> {
        let mut out = Vec::new();
        for (node, inherited) in flatten_nodes(&self.workspace.model.deployment_nodes) {
            for s in &node.software_system_instances {
                out.push((
                    s.item.id.clone(),
                    s.software_system_id.clone(),
                    s.environment.clone(),
                    all_deployment_groups(&inherited, &s.deployment_groups),
                ));
            }
            for c in &node.container_instances {
                out.push((
                    c.item.id.clone(),
                    c.container_id.clone(),
                    c.environment.clone(),
                    all_deployment_groups(&inherited, &c.deployment_groups),
                ));
            }
        }
        out
    }

    /// Reference `Element.getEfferentRelationshipWith(destination, "")`
    /// existence check (`description` normalized to `""` on both sides).
    pub(crate) fn has_efferent(&self, source: &Id, destination: &Id, description: &str) -> bool {
        self.workspace.model.relationships.iter().any(|r| {
            &r.source == source
                && &r.destination == destination
                && r.description.as_deref().unwrap_or("") == description
        })
    }

    /// Reference `Element.hasEfferentRelationshipWith(destination)` (any
    /// description).
    pub(crate) fn has_any_efferent(&self, source: &Id, destination: &Id) -> bool {
        self.workspace
            .model
            .relationships
            .iter()
            .any(|r| &r.source == source && &r.destination == destination)
    }

    /// Reference `Model.isChildOf`: is `e1` a (strict) descendant of `e2`?
    /// Persons have no lineage.
    pub(crate) fn is_child_of(&self, e1: &Id, e2: &Id) -> bool {
        if self.element_kind(e1) == Some(ElementKind::Person)
            || self.element_kind(e2) == Some(ElementKind::Person)
        {
            return false;
        }
        let mut parent = self.element_parent(e2);
        while let Some(p) = parent {
            if &p == e1 {
                return true;
            }
            parent = self.element_parent(&p);
        }
        false
    }

    /// The structural parent of an element (`Element.getParent()`):
    /// container→system, component→container, deployment elements→their node
    /// or parent node; people/systems/custom elements have none.
    #[must_use]
    pub fn element_parent(&self, id: &Id) -> Option<Id> {
        let model = &self.workspace.model;
        for s in &model.software_systems {
            if s.containers.iter().any(|c| &c.item.id == id) {
                return Some(s.item.id.clone());
            }
            for c in &s.containers {
                if c.components.iter().any(|k| &k.item.id == id) {
                    return Some(c.item.id.clone());
                }
            }
        }
        node_parent(&model.deployment_nodes, id)
    }

    /// The reference `Element.getCanonicalName()` for the common kinds
    /// (deployment-element canonical names are irrelevant for the errors that
    /// carry these).
    #[must_use]
    pub fn canonical_name(&self, id: &Id) -> String {
        let name = || self.item(id).map(|i| i.name.clone()).unwrap_or_default();
        match self.element_kind(id) {
            Some(ElementKind::Container) => {
                let system = self
                    .workspace
                    .model
                    .software_systems
                    .iter()
                    .find(|s| s.containers.iter().any(|c| &c.item.id == id))
                    .map(|s| s.item.name.clone())
                    .unwrap_or_default();
                crate::canonical::container(&system, &name())
            }
            Some(ElementKind::Component) => {
                let mut names = (String::new(), String::new());
                for s in &self.workspace.model.software_systems {
                    for c in &s.containers {
                        if c.components.iter().any(|k| &k.item.id == id) {
                            names = (s.item.name.clone(), c.item.name.clone());
                        }
                    }
                }
                crate::canonical::component(&names.0, &names.1, &name())
            }
            _ => name(),
        }
    }

    /// `element.getRelationships()` — efferent relationship ids in model
    /// (insertion) order.
    #[must_use]
    pub fn efferent_relationships(&self, id: &Id) -> Vec<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .filter(|r| &r.source == id)
            .map(|r| r.id.clone())
            .collect()
    }

    /// Attach a perspective to an element or relationship by id.
    pub fn add_perspective(&mut self, id: &Id, perspective: Perspective) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.perspectives.push(perspective);
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.perspectives.push(perspective);
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// Set the optional URL of an element or relationship.
    pub fn set_url(&mut self, id: &Id, url: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.url = Some(url.to_string());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.url = Some(url.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// Add a property to an element, relationship, or the model.
    pub fn add_item_property(
        &mut self,
        id: &Id,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.properties.insert(name.into(), value.into());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.properties.insert(name.into(), value.into());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `group "Name"` for any groupable element.
    pub fn set_group(&mut self, id: &Id, group: &str) -> Result<(), CoreError> {
        let mut element = self
            .element_mut(id)
            .ok_or_else(|| CoreError::ElementNotFound { id: id.clone() })?;
        element.set_group(Some(group.to_string()));
        Ok(())
    }

    /// `technology "…"` for containers/components/nodes/infrastructure nodes.
    pub fn set_technology(&mut self, id: &Id, technology: &str) -> Result<(), CoreError> {
        let mut element = self
            .element_mut(id)
            .ok_or_else(|| CoreError::ElementNotFound { id: id.clone() })?;
        element.set_technology(Some(technology.to_string()));
        Ok(())
    }

    /// `tags "a,b"` — ADD tags (reference `addTags`, trimmed, first position).
    pub fn add_tags(&mut self, id: &Id, csv: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            for t in csv.split(',') {
                item.tags.add(t);
            }
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            for t in csv.split(',') {
                rel.tags.add(t);
            }
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `description "…"` for an element or relationship.
    pub fn set_description(&mut self, id: &Id, description: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.description = Some(description.to_string());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.description = Some(description.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// Mutable access to the model (styles-of-validation escape hatch for M4
    /// loaders, e.g. direct `!identifiers` imports).
    pub fn model_mut(&mut self) -> &mut Model {
        &mut self.workspace.model
    }

    // ------------------------------------------------------------------
    // views
    // ------------------------------------------------------------------
    //
    // Content operations for these live in [`crate::viewops`] (the
    // `ModelView`/`StaticView`/`DynamicView`/`DeploymentView` port).

    /// Create a system landscape view.
    pub fn create_landscape_view(&mut self, key: &str) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "SystemLandscape")?;
        self.workspace
            .views
            .system_landscape_views
            .push(SystemLandscapeView {
                base,
                content: ViewContent::default(),
                // Java field initializer: `enterpriseBoundaryVisible = true`.
                enterprise_boundary_visible: true,
                animations: Vec::new(),
            });
        Ok(ViewRef::Landscape(
            self.workspace.views.system_landscape_views.len() - 1,
        ))
    }

    /// Create a system context view scoped to `system`. Like the reference
    /// `SystemContextView` constructor, the scoped system is added to the
    /// view immediately (with its relationships).
    pub fn create_context_view(&mut self, system: &Id, key: &str) -> Result<ViewRef, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        let base = self.new_view_base(key, "SystemContext")?;
        self.workspace
            .views
            .system_context_views
            .push(SystemContextView {
                base,
                content: ViewContent::default(),
                software_system_id: system.clone(),
                // Java field initializer: `enterpriseBoundaryVisible = true`.
                enterprise_boundary_visible: true,
                animations: Vec::new(),
            });
        let view = ViewRef::Context(self.workspace.views.system_context_views.len() - 1);
        // `SystemContextView()` ctor: `addElement(softwareSystem, true)`.
        self.view_add_element(view, system, true)?;
        Ok(view)
    }

    /// Create a container view scoped to `system`.
    pub fn create_container_view(&mut self, system: &Id, key: &str) -> Result<ViewRef, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        let base = self.new_view_base(key, "Container")?;
        self.workspace.views.container_views.push(ContainerView {
            base,
            content: ViewContent::default(),
            software_system_id: system.clone(),
            external_software_system_boundaries_visible: false,
            animations: Vec::new(),
        });
        Ok(ViewRef::Container(
            self.workspace.views.container_views.len() - 1,
        ))
    }

    /// Create a component view scoped to `container`.
    pub fn create_component_view(
        &mut self,
        container: &Id,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        if !self.is_container(container) {
            return Err(CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            });
        }
        let base = self.new_view_base(key, "Component")?;
        self.workspace
            .views
            .component_views
            .push(crate::views::ComponentView {
                base,
                content: ViewContent::default(),
                container_id: container.clone(),
                external_container_boundaries_visible: false,
                animations: Vec::new(),
            });
        Ok(ViewRef::Component(
            self.workspace.views.component_views.len() - 1,
        ))
    }

    /// Create a dynamic view (element-scoped or workspace-wide).
    pub fn create_dynamic_view(
        &mut self,
        element: Option<Id>,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        if let Some(id) = &element
            && !self.has_element(id)
        {
            return Err(CoreError::ElementNotFound { id: id.clone() });
        }
        let base = self.new_view_base(key, "Dynamic")?;
        self.workspace.views.dynamic_views.push(DynamicView {
            base,
            content: ViewContent::default(),
            element_id: element,
            deployment_environment: None,
            external_boundaries_visible: false,
        });
        Ok(ViewRef::Dynamic(
            self.workspace.views.dynamic_views.len() - 1,
        ))
    }

    /// Create a deployment view for `environment`.
    pub fn create_deployment_view(
        &mut self,
        environment: &str,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Deployment")?;
        self.workspace.views.deployment_views.push(DeploymentView {
            base,
            content: ViewContent::default(),
            environment: environment.to_string(),
            software_system_id: None,
            animations: Vec::new(),
        });
        Ok(ViewRef::Deployment(
            self.workspace.views.deployment_views.len() - 1,
        ))
    }

    /// Create a filtered view over `base_key` (existence checked by
    /// [`crate::validate`], as in the reference hydrate step).
    pub fn create_filtered_view(
        &mut self,
        base_key: &str,
        key: &str,
        mode: crate::views::FilterMode,
        tags: Vec<String>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Filtered")?;
        self.workspace.views.filtered_views.push(FilteredView {
            base,
            base_view_key: base_key.to_string(),
            mode: Some(mode),
            tags,
        });
        Ok(ViewRef::Filtered(
            self.workspace.views.filtered_views.len() - 1,
        ))
    }

    /// Create an image view (DSL `image` / `plantuml`-style content views).
    pub fn create_image_view(
        &mut self,
        element: Option<Id>,
        key: &str,
        content: Option<String>,
        content_type: Option<String>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Image")?;
        self.workspace.views.image_views.push(ImageView {
            base,
            element_id: element,
            content,
            content_light: None,
            content_dark: None,
            content_type,
        });
        Ok(ViewRef::Image(self.workspace.views.image_views.len() - 1))
    }

    /// Create a custom view (`custom <name> [description] [key]`).
    pub fn create_custom_view(
        &mut self,
        key: &str,
        software_system: Option<Id>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Custom")?;
        self.workspace.views.custom_views.push(CustomView {
            base,
            content: ViewContent::default(),
            animations: Vec::new(),
            software_system_id: software_system,
        });
        Ok(ViewRef::Custom(self.workspace.views.custom_views.len() - 1))
    }

    /// Build a view base applying the reference `ViewSet` rules: an empty key
    /// triggers `generateViewKey(prefix)` = `<Prefix>-<counter:000>` scanning
    /// from 1 (marks `generatedKey`); a given key must be unique across ALL
    /// views; `order` = max existing order + 1 across the whole view set.
    fn new_view_base(&mut self, key: &str, prefix: &str) -> Result<ViewBase, CoreError> {
        let (key, generated) = if key.is_empty() {
            (self.generate_view_key(prefix), true)
        } else {
            if self
                .workspace
                .views
                .all_views()
                .iter()
                .any(|v| v.key() == key)
            {
                return Err(CoreError::DuplicateViewKey {
                    key: key.to_string(),
                });
            }
            (key.to_string(), false)
        };
        let order = self
            .workspace
            .views
            .all_views()
            .iter()
            .map(|v| v.order())
            .max()
            .unwrap_or(0)
            + 1;
        let mut base = ViewBase::new(key, order);
        base.generated_key = generated;
        Ok(base)
    }

    /// `ViewSet.generateViewKey`: `DecimalFormat("000")` scan from 1.
    fn generate_view_key(&self, prefix: &str) -> String {
        let mut n = 1_u32;
        loop {
            let candidate = format!("{prefix}-{n:03}");
            if !self
                .workspace
                .views
                .all_views()
                .iter()
                .any(|v| v.key() == candidate)
            {
                return candidate;
            }
            n += 1;
        }
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
            for t in csv.split(',') {
                item.tags.add(t);
            }
        }
        item
    }

    fn check_top_level_name(&self, name: &str) -> Result<(), CoreError> {
        let taken = self
            .workspace
            .model
            .people
            .iter()
            .map(|p| p.item.name.as_str())
            .chain(
                self.workspace
                    .model
                    .software_systems
                    .iter()
                    .map(|s| s.item.name.as_str()),
            )
            .chain(
                self.workspace
                    .model
                    .custom_elements
                    .iter()
                    .map(|c| c.item.name.as_str()),
            )
            .any(|n| n == name);
        if taken {
            return Err(CoreError::DuplicateTopLevelName {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// Whether an id names an element of any kind.
    #[must_use]
    pub fn has_element(&self, id: &Id) -> bool {
        crate::validate::flattened(&self.workspace.model)
            .iter()
            .any(|(eid, _, _)| eid == id)
    }

    /// The kind label of an element id.
    #[must_use]
    pub fn element_kind(&self, id: &Id) -> Option<crate::validate::ElementKind> {
        crate::validate::flattened(&self.workspace.model)
            .into_iter()
            .find(|(eid, _, _)| eid == id)
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
    pub fn item_mut(&mut self, id: &Id) -> Option<&mut ModelItem> {
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
    pub fn system_mut(&mut self, id: &Id) -> Option<&mut SoftwareSystem> {
        self.workspace
            .model
            .software_systems
            .iter_mut()
            .find(|s| &s.item.id == id)
    }

    /// Read access to a software system.
    #[must_use]
    pub fn system_ref(&self, id: &Id) -> Option<&SoftwareSystem> {
        self.workspace
            .model
            .software_systems
            .iter()
            .find(|s| &s.item.id == id)
    }

    /// Read access to a container (any system).
    #[must_use]
    pub fn container_ref(&self, id: &Id) -> Option<&Container> {
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
    pub fn container_mut(&mut self, id: &Id) -> Option<&mut Container> {
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

    /// Mutable view set (content operations — the M4 view statements).
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
    /// Set the group — every element kind is groupable (`GroupableElement` is
    /// in all inheritance chains, deployment elements included).
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
}

/// DSL convention: the reference passes `""` for omitted positional args, and
/// empty descriptions are stored as null (`NON_EMPTY` on write).
fn filter_default(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Kinds eligible for implied relationships (`Model.addRelationship`'s
/// `instanceof` guard).
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
/// top-level node is `None`, like `Element.getParent()`).
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

fn new_node(item: &ModelItem, environment: &str, technology: Option<String>) -> DeploymentNode {
    DeploymentNode {
        item: item.clone(),
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
    fn top_level_names_are_one_namespace() {
        let mut b = WorkspaceBuilder::new();
        b.add_software_system("Dup", None, None).unwrap();
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
}
