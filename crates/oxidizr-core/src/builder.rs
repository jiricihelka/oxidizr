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
use crate::configuration::{Role, User, Visibility, WorkspaceScope};
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

    /// Adopt a pre-built workspace as the one under construction
    /// (`workspace extends` — `PHASE_1.md` §M5-a, mirrors reference
    /// `WorkspaceParser.parse`, which REPLACES the workspace object; later
    /// statements layer over the base). Every element and relationship id of
    /// the base is replayed through [`Self::found`] — the reference seeds its
    /// id counter inside `Model.add` on every add, JSON loads included
    /// (`Model.java:274/286` @`ad34036`) — so subsequent [`Self::alloc_id`]
    /// continues after the base's highest id. Implied-relationship strategy
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

    /// `scope none` — reference `setScope(null)` (`ConfigurationParser`).
    pub fn clear_scope(&mut self) {
        self.workspace.configuration.scope = None;
    }

    /// `visibility <private|public>` (`ConfigurationParser`).
    pub fn set_visibility(&mut self, visibility: Visibility) {
        self.workspace.configuration.visibility = Some(visibility);
    }

    /// `users { … }` role statements — `WorkspaceConfiguration.addUser`:
    /// the reference `TreeSet<User>` is ordered by username and silently
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

    /// Drop a top-level property (`DslUtils.clearDsl` — `workspace extends`
    /// discards the base's retained `structurizr.dsl`).
    pub fn remove_property(&mut self, name: &str) {
        self.workspace.properties.remove(name);
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
        self.check_top_level_name(name, ElementKind::Person)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.people.push(Person {
            item,
            group: None,
            relationships: Vec::new(),
        });
        Ok(id)
    }

    /// `softwareSystem "Name" ["Desc"] ["Tags"]`.
    pub fn add_software_system(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name, ElementKind::SoftwareSystem)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.software_systems.push(SoftwareSystem {
            item,
            group: None,
            containers: Vec::new(),
            documentation: crate::docs::Documentation::default(),
            relationships: Vec::new(),
        });
        Ok(id)
    }

    /// `element "Name" ["metadata"] ["Desc"] ["Tags"]` — custom element.
    ///
    /// The reference stores fresh DSL custom elements in REVERSE creation
    /// order (2026-11-17 oracle witness: `A`..`F` export as `F`..`A`, and
    /// `Zulu`,`Alpha` as `Alpha`,`Zulu` — reverse insertion, not id sort,
    /// which the JSON-import path proves by re-sorting ascending through
    /// `Model.setCustomElements`' `TreeSet` rebuild). JSON export is stored
    /// order, so insertion goes at the FRONT.
    pub fn add_custom_element(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name, ElementKind::CustomElement)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.custom_elements.insert(
            0,
            CustomElement {
                item,
                group: None,
                metadata: None,
                relationships: Vec::new(),
            },
        );
        Ok(id)
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
        self.system_ref(system)
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
            });
        }
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.system_mut(system)
            .expect("checked above")
            .containers
            .push(Container {
                item,
                group: None,
                technology,
                components: Vec::new(),
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(id)
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
        self.container_ref(container)
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
            });
        }
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.container_mut(container)
            .expect("checked above")
            .components
            .push(Component {
                item,
                group: None,
                technology,
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(id)
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
        let id = item.id.clone();
        self.workspace
            .model
            .deployment_nodes
            .push(new_node(item, environment, technology));
        Ok(id)
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
        let id = item.id.clone();
        self.node_mut(parent)
            .expect("just found")
            .children
            .push(new_node(item, &env, technology));
        Ok(id)
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
        let id = item.id.clone();
        self.node_mut(node)
            .expect("checked above")
            .infrastructure_nodes
            .push(InfrastructureNode {
                item,
                group: None,
                environment,
                technology,
                relationships: Vec::new(),
            });
        Ok(id)
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

    /// `deploymentGroup <deploymentGroups>` inside a node — Java
    /// `DeploymentNode.addDeploymentGroup`: blank names rejected, the name is
    /// added to the node's (deduplicated) set.
    pub fn add_deployment_group(&mut self, node: &Id, name: &str) -> Result<(), CoreError> {
        let node = self
            .node_mut(node)
            .ok_or_else(|| CoreError::DeploymentNodeNotFound { id: node.clone() })?;
        if name.trim().is_empty() {
            // `StringUtils.isNullOrEmpty` trims.
            return Err(CoreError::DeploymentGroupNameRequired);
        }
        if !node.deployment_groups.iter().any(|g| g == name) {
            node.deployment_groups.push(name.to_string());
        }
        Ok(())
    }

    /// `healthCheck <name> <url> [interval] [timeout]` — Java
    /// `StaticStructureElementInstance.addHealthCheck`: validation and its
    /// messages mirror the reference; the check lands on the instance's list
    /// verbatim (`HttpHealthCheck`'s ctor stores name/url untrimmed).
    pub fn add_health_check(
        &mut self,
        instance: &Id,
        name: &str,
        url: &str,
        interval: i32,
        timeout: i64,
    ) -> Result<(), CoreError> {
        if name.trim().is_empty() {
            return Err(CoreError::HealthCheckInvalid(
                "The name must not be null or empty.".to_string(),
            ));
        }
        if url.trim().is_empty() {
            return Err(CoreError::HealthCheckInvalid(
                "The URL must not be null or empty.".to_string(),
            ));
        }
        if !crate::url::is_url(url) {
            return Err(CoreError::HealthCheckInvalid(format!(
                "{url} is not a valid URL."
            )));
        }
        if interval < 0 {
            return Err(CoreError::HealthCheckInvalid(
                "The polling interval must be zero or a positive integer.".to_string(),
            ));
        }
        if timeout < 0 {
            return Err(CoreError::HealthCheckInvalid(
                "The timeout must be zero or a positive integer.".to_string(),
            ));
        }
        let checks = Self::health_checks_mut(&mut self.workspace.model.deployment_nodes, instance)
            .ok_or_else(|| CoreError::InstanceNotFound {
                id: instance.clone(),
            })?;
        checks.push(crate::model::HttpHealthCheck {
            name: name.to_string(),
            url: url.to_string(),
            headers: std::collections::BTreeMap::new(),
            interval,
            timeout,
        });
        Ok(())
    }

    /// Mutable health-check list of a software-system / container instance.
    fn health_checks_mut<'a>(
        nodes: &'a mut [crate::model::DeploymentNode],
        id: &Id,
    ) -> Option<&'a mut Vec<crate::model::HttpHealthCheck>> {
        for n in nodes {
            for i in &mut n.software_system_instances {
                if &i.item.id == id {
                    return Some(&mut i.health_checks);
                }
            }
            for i in &mut n.container_instances {
                if &i.item.id == id {
                    return Some(&mut i.health_checks);
                }
            }
            if let Some(checks) = Self::health_checks_mut(&mut n.children, id) {
                return Some(checks);
            }
        }
        None
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
            for t in java_split_csv(csv) {
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
                        self.insert_relationship(
                            &s,
                            &d,
                            desc.clone(),
                            tech.clone(),
                            Tags::new(),
                            Some(base_id),
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

    /// Every model element id in `Model.getElements()` iteration order
    /// (people, software systems with their containers/components, custom
    /// elements, then the deployment tree). Used by the DSL canonical-name
    /// lookups and the `include *` / expression finders.
    #[must_use]
    pub fn all_element_ids(&self) -> Vec<Id> {
        let mut ids: Vec<Id> = crate::validate::flattened(&self.workspace.model)
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        ids.sort_by_key(|id| id_sort_key(id.as_str()));
        ids
    }

    /// `Model.getElementWithCanonicalName(canonicalName)`.
    #[must_use]
    pub fn element_with_canonical_name(&self, canonical_name: &str) -> Option<Id> {
        self.all_element_ids()
            .into_iter()
            .find(|id| self.canonical_name_full(id).as_deref() == Some(canonical_name))
    }

    /// `Model.getRelationshipWithCanonicalName(canonicalName)`.
    #[must_use]
    pub fn relationship_with_canonical_name(&self, canonical_name: &str) -> Option<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| self.relationship_canonical_name(&r.id).as_deref() == Some(canonical_name))
            .map(|r| r.id.clone())
    }

    /// The full `Element.getCanonicalName()` for any element (deployment
    /// nodes, infrastructure nodes and instances included), via
    /// `CanonicalNameGenerator`. `None` for a non-element id.
    #[must_use]
    pub fn canonical_name_full(&self, id: &Id) -> Option<String> {
        use crate::canonical as cn;

        let kind = self.element_kind(id)?;
        let name = || self.item(id).map(|i| i.name.clone()).unwrap_or_default();
        match kind {
            ElementKind::CustomElement => Some(cn::custom_element(&name())),
            ElementKind::Person => Some(cn::person(&name())),
            ElementKind::SoftwareSystem => Some(cn::software_system(&name())),
            ElementKind::Container => {
                let system = self.system_of_container(id)?.item.name.clone();
                Some(cn::container(&system, &name()))
            }
            ElementKind::Component => {
                let (sys, container) = self.system_and_container_of_component(id)?;
                Some(cn::component(&sys.item.name, &container.item.name, &name()))
            }
            ElementKind::DeploymentNode => {
                let (env, ancestors, own) = self.node_context(id)?;
                let chain: Vec<&str> = ancestors.iter().map(String::as_str).collect();
                Some(cn::deployment_node(&env, &chain, &own))
            }
            ElementKind::InfrastructureNode => {
                let (node_chain, own) = self.infra_parent_chain(id)?;
                Some(cn::infrastructure_node(&node_chain, &own))
            }
            ElementKind::SoftwareSystemInstance => {
                let (node_chain, system_name, instance_id) = self.ssi_context(id)?;
                Some(cn::software_system_instance(
                    &node_chain,
                    &system_name,
                    instance_id,
                ))
            }
            ElementKind::ContainerInstance => {
                let (node_chain, system_name, container_name, instance_id) = self.ci_context(id)?;
                Some(cn::container_instance(
                    &node_chain,
                    &system_name,
                    &container_name,
                    instance_id,
                ))
            }
        }
    }

    /// `Relationship.getCanonicalName()` (`CanonicalNameGenerator`).
    #[must_use]
    pub fn relationship_canonical_name(&self, id: &Id) -> Option<String> {
        let r = self
            .workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)?;
        let source = self.canonical_name_full(&r.source)?;
        let dest = self.canonical_name_full(&r.destination)?;
        Some(crate::canonical::relationship(
            &source,
            &dest,
            r.description.as_deref(),
        ))
    }

    /// `Element.getTechnology()` for containers, components, deployment
    /// nodes, infrastructure nodes and container instances (which report
    /// their STATIC container's technology); `""` when unset or the kind has
    /// no technology.
    #[must_use]
    pub fn element_technology(&self, id: &Id) -> String {
        let model = &self.workspace.model;
        for s in &model.software_systems {
            for c in &s.containers {
                if &c.item.id == id {
                    return c.technology.clone().unwrap_or_default();
                }
                for k in &c.components {
                    if &k.item.id == id {
                        return k.technology.clone().unwrap_or_default();
                    }
                }
            }
        }
        let mut out: Option<String> = None;
        self.walk_nodes_search(&mut |node, _ancestors| {
            if &node.item.id == id {
                out = Some(node.technology.clone().unwrap_or_default());
                return Some(());
            }
            for i in &node.infrastructure_nodes {
                if &i.item.id == id {
                    out = Some(i.technology.clone().unwrap_or_default());
                    return Some(());
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    out = Some(
                        self.container_ref(&c.container_id)
                            .and_then(|c| c.technology.clone())
                            .unwrap_or_default(),
                    );
                    return Some(());
                }
            }
            None
        });
        out.unwrap_or_default()
    }

    /// `Relationship.getTechnology()` (empty string when unset).
    #[must_use]
    pub fn relationship_technology(&self, id: &Id) -> String {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .and_then(|r| r.technology.clone())
            .unwrap_or_default()
    }

    /// `Relationship.getLinkedRelationshipId()`.
    #[must_use]
    pub fn linked_relationship_id(&self, id: &Id) -> Option<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .and_then(|r| r.linked_relationship_id.clone())
    }

    /// `Element.getGroup()` — the group name for any groupable element.
    #[must_use]
    pub fn element_group(&self, id: &Id) -> Option<String> {
        let model = &self.workspace.model;
        for p in &model.people {
            if &p.item.id == id {
                return p.group.clone();
            }
        }
        for c in &model.custom_elements {
            if &c.item.id == id {
                return c.group.clone();
            }
        }
        for s in &model.software_systems {
            if &s.item.id == id {
                return s.group.clone();
            }
            for c in &s.containers {
                if &c.item.id == id {
                    return c.group.clone();
                }
                for k in &c.components {
                    if &k.item.id == id {
                        return k.group.clone();
                    }
                }
            }
        }
        Self::group_in_tree(&model.deployment_nodes, id)
    }

    /// The software system an id "belongs" to for `getElement(id, SoftwareSystem.class)`;
    /// only a real software system qualifies (exact kind — see `checkElementType`).
    #[must_use]
    pub fn as_software_system(&self, id: &Id) -> Option<Id> {
        (self.element_kind(id) == Some(ElementKind::SoftwareSystem)).then(|| id.clone())
    }

    /// The container an id names (exact kind).
    #[must_use]
    pub fn as_container(&self, id: &Id) -> Option<Id> {
        (self.element_kind(id) == Some(ElementKind::Container)).then(|| id.clone())
    }

    /// `Model.getRelationships()` iteration order: the reference stores
    /// relationships in a `TreeSet` ordered by `ModelItem.compareTo` —
    /// ascending numeric id (see [`id_sort_key`]).
    #[must_use]
    pub fn all_relationship_ids(&self) -> Vec<Id> {
        let mut ids: Vec<Id> = self
            .workspace
            .model
            .relationships
            .iter()
            .map(|r| r.id.clone())
            .collect();
        ids.sort_by_key(|id| id_sort_key(id.as_str()));
        ids
    }

    /// `ModelItem.hasTag(tag)` — `getTagsAsSet().contains(tag)`: derived
    /// defaults for the item's kind, then the stored tags.
    #[must_use]
    pub fn item_has_tag(&self, id: &Id, tag: &str) -> bool {
        if let Some(item) = self.item(id) {
            let kind = self
                .element_kind(id)
                .map_or(crate::tags::TagKind::CustomElement, |k| {
                    crate::tags::TagKind::from_element_kind(k)
                });
            return item.tags.has(kind, tag);
        }
        if let Some(r) = self
            .workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
        {
            let defaults = crate::tags::relationship_defaults(
                r.linked_relationship_id.is_some(),
                r.interaction_style,
            );
            return r.tags.has_against(&defaults, tag);
        }
        false
    }

    /// `ModelItem.hasProperty(name, value)` for an element or relationship.
    #[must_use]
    pub fn item_has_property(&self, id: &Id, name: &str, value: &str) -> bool {
        if let Some(item) = self.item(id) {
            return item.properties.get(name) == Some(value);
        }
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .is_some_and(|r| r.properties.get(name) == Some(value))
    }

    /// The default tags of an element id's kind (`ModelItem.getDefaultTags()`);
    /// empty for a relationship or unknown id.
    #[must_use]
    pub fn element_tag_defaults(&self, id: &Id) -> &'static [&'static str] {
        self.element_kind(id).map_or(&[], |k| {
            crate::tags::TagKind::from_element_kind(k).default_tags()
        })
    }

    /// `Relationship.getInteractionStyle()` (never set from the DSL).
    #[must_use]
    pub fn relationship_style(&self, id: &Id) -> Option<crate::tags::InteractionStyle> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .and_then(|r| r.interaction_style)
    }

    /// `AbstractRelationshipParser.createRelationship` — the DSL relationship
    /// entry point, in EXACT reference order: kind dispatch (`uses(...)`
    /// overload selection), then the `Model.addRelationship` parent/child
    /// guard, then the null-result duplicate check (`Element.has`, i.e.
    /// destination + description).  The returned id carries
    /// `linkedRelationshipId = None` — explicit DSL relationships are never
    /// linked (only implied / replicated ones are).
    pub fn dsl_uses(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
    ) -> Result<Id, CoreError> {
        if !self.permitted_dsl_pair(source, destination) {
            return Err(CoreError::RelationshipPairNotPermitted {
                source: self.canonical_name_full(source).unwrap_or_default(),
                destination: self.canonical_name_full(destination).unwrap_or_default(),
            });
        }
        if self.is_child_of(source, destination) || self.is_child_of(destination, source) {
            return Err(CoreError::ParentChildRelationship);
        }
        // Core `Element.has`: the add returns null only when destination AND
        // description match. The reference parser's any-description
        // `hasEfferentRelationshipWith` check only adorns the message inside
        // the `relationship == null` branch — same pair, different
        // description ADDS fine.
        if self.has_efferent(source, destination, description.as_deref().unwrap_or("")) {
            return Err(CoreError::DuplicateRelationship {
                source: self.canonical_name_full(source).unwrap_or_default(),
                destination: self.canonical_name_full(destination).unwrap_or_default(),
            });
        }
        let id = self.insert_relationship(
            source,
            destination,
            description,
            technology,
            Tags::new(),
            None,
        );
        self.maybe_create_implied(&id);
        Ok(id)
    }

    /// The `createRelationship` instanceof dispatch (class hierarchy @
    /// `ad34036`): `CustomElement` on either side, a static-structure pair,
    /// or the deployment chain node→node, node→infra, infra→deployment-
    /// element, instance→infra.
    #[must_use]
    fn permitted_dsl_pair(&self, source: &Id, destination: &Id) -> bool {
        use ElementKind as K;

        let (Some(s), Some(d)) = (self.element_kind(source), self.element_kind(destination)) else {
            return false;
        };
        let static_struct = |k: K| {
            matches!(
                k,
                K::Person | K::SoftwareSystem | K::Container | K::Component
            )
        };
        if s == K::CustomElement || d == K::CustomElement {
            return true;
        }
        if static_struct(s) && static_struct(d) {
            return true;
        }
        matches!(
            (s, d),
            (K::DeploymentNode, K::DeploymentNode | K::InfrastructureNode)
                | (K::InfrastructureNode, _)
                | (
                    K::SoftwareSystemInstance | K::ContainerInstance,
                    K::InfrastructureNode
                )
        )
    }

    /// The implied-relationships hook (`Model.addRelationship`), shared by
    /// [`WorkspaceBuilder::add_relationship`] and
    /// [`WorkspaceBuilder::dsl_uses`].
    fn maybe_create_implied(&mut self, id: &Id) {
        let (source, destination) = self
            .workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .expect("just inserted");
        if self.implied_strategy != ImpliedRelationshipsStrategy::Default
            && is_implied_kind(self.element_kind(&source))
            && is_implied_kind(self.element_kind(&destination))
        {
            self.create_implied_relationships(id);
        }
    }

    /// `Element.getEfferentRelationshipWith(destination)` — the first
    /// outgoing relationship to `destination` (any description).
    #[must_use]
    pub fn efferent_to(&self, source: &Id, destination: &Id) -> Option<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.source == source && &r.destination == destination)
            .map(|r| r.id.clone())
    }

    /// Add already-split tag parts to an element or relationship with the
    /// reference `addTags(String[])` semantics (trim, first position kept).
    pub fn add_tag_parts(&mut self, id: &Id, parts: &[String]) {
        if let Some(item) = self.item_mut(id) {
            for t in parts {
                item.tags.add(t);
            }
            return;
        }
        if let Some(rel) = self.relationship_mut(id) {
            for t in parts {
                rel.tags.add(t);
            }
        }
    }

    /// `Element.getEfferentRelationshipWith(destination, description)`.
    #[must_use]
    pub fn efferent_to_with_description(
        &self,
        source: &Id,
        destination: &Id,
        description: &str,
    ) -> Option<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| {
                &r.source == source
                    && &r.destination == destination
                    && r.description.as_deref().unwrap_or("") == description
            })
            .map(|r| r.id.clone())
    }

    /// The `(description, technology, linkedRelationshipId)` of a
    /// relationship (for `-/>` and the `NoRelationship` context hand-off).
    #[must_use]
    pub fn relationship_parts(&self, id: &Id) -> Option<(String, String, Option<Id>)> {
        let r = self
            .workspace
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)?;
        Some((
            r.description.clone().unwrap_or_default(),
            r.technology.clone().unwrap_or_default(),
            r.linked_relationship_id.clone(),
        ))
    }

    /// `Model.getRelationship(id)` — the relationship's own id (existence check).
    #[must_use]
    pub fn has_relationship(&self, id: &Id) -> bool {
        self.workspace
            .model
            .relationships
            .iter()
            .any(|r| &r.id == id)
    }

    /// `Workspace.remove(Relationship)`: drop the relationship from the model
    /// and from its source's efferent list.
    pub fn remove_relationship(&mut self, id: &Id) {
        if let Some(pos) = self
            .workspace
            .model
            .relationships
            .iter()
            .position(|r| &r.id == id)
        {
            let rel = self.workspace.model.relationships.remove(pos);
            if let Some(rels) = self.source_relationships_mut(&rel.source) {
                rels.retain(|r| r != id);
            }
        }
    }

    /// `AbstractRelationshipParser.findSoftwareSystemInstances(system, env)`:
    /// all software-system instances of `system` in `env`, in element order.
    #[must_use]
    pub fn find_software_system_instances(&self, system: &Id, env: &str) -> Vec<Id> {
        let mut out = Vec::new();
        self.walk_instances(&mut |id, static_id, instance_env, kind| {
            if kind == ElementKind::SoftwareSystemInstance
                && static_id == system
                && instance_env == env
            {
                out.push(id.clone());
            }
        });
        out.sort_by_key(|id| id_sort_key(id.as_str()));
        out
    }

    /// `AbstractRelationshipParser.findContainerInstances(container, env)`.
    #[must_use]
    pub fn find_container_instances(&self, container: &Id, env: &str) -> Vec<Id> {
        let mut out = Vec::new();
        self.walk_instances(&mut |id, static_id, instance_env, kind| {
            if kind == ElementKind::ContainerInstance
                && static_id == container
                && instance_env == env
            {
                out.push(id.clone());
            }
        });
        out.sort_by_key(|id| id_sort_key(id.as_str()));
        out
    }

    /// `DeploymentEnvironmentDslContext.findInstancesOf(element)`: instances of
    /// a static structure element in `env` (either instance kind), element order.
    #[must_use]
    pub fn find_instances_of(&self, element: &Id, env: &str) -> Vec<Id> {
        let mut out = Vec::new();
        self.walk_instances(&mut |id, static_id, instance_env, kind| {
            if static_id == element && instance_env == env && kind != ElementKind::Person {
                out.push(id.clone());
            }
        });
        out.sort_by_key(|id| id_sort_key(id.as_str()));
        out
    }

    /// Every instance of a static element in ANY environment (reference
    /// `DeploymentViewExpressionParser.getElements`, which streams ALL
    /// model elements through `instanceof` + `equals` without consulting
    /// the deployment environment).
    #[must_use]
    pub fn all_instances_of(&self, element: &Id) -> Vec<Id> {
        let mut out = Vec::new();
        self.walk_instances(&mut |id, static_id, _env, kind| {
            if static_id == element && kind != ElementKind::Person {
                out.push(id.clone());
            }
        });
        out.sort_by_key(|id| id_sort_key(id.as_str()));
        out
    }

    /// `Element.getParent()` for a static-structure-element instance (its
    /// deployment node). Used by `!element`'s group assignment (which only
    /// fires for `StaticStructureElement`, so never for instances).
    #[must_use]
    pub fn instance_static(&self, id: &Id) -> Option<(Id, String, Vec<String>)> {
        self.instance_profile(id)
    }

    // --- deployment canonical-name context helpers -----------------------

    /// `(environment, ancestor node names root-first, own name)` of a
    /// deployment node id.
    fn node_context(&self, id: &Id) -> Option<(String, Vec<String>, String)> {
        self.walk_nodes_search(&mut |node, ancestors| {
            (&node.item.id == id).then(|| {
                (
                    node.environment.clone(),
                    ancestors.to_vec(),
                    node.item.name.clone(),
                )
            })
        })
    }

    /// `(node chain WITHOUT scheme, own name)` for an infrastructure node —
    /// the chain is the parent deployment node's full canonical chain
    /// (env + ancestor node names + parent node name).
    fn infra_parent_chain(&self, id: &Id) -> Option<(String, String)> {
        self.walk_nodes_search(&mut |node, ancestors| {
            node.infrastructure_nodes
                .iter()
                .find(|i| &i.item.id == id)
                .map(|infra| {
                    (
                        Self::node_chain_of(node, ancestors),
                        infra.item.name.clone(),
                    )
                })
        })
    }

    /// `(node chain WITHOUT scheme, system name, instanceId)`.
    fn ssi_context(&self, id: &Id) -> Option<(String, String, i32)> {
        self.walk_nodes_search(&mut |node, ancestors| {
            node.software_system_instances
                .iter()
                .find(|i| &i.item.id == id)
                .map(|inst| {
                    let system = self
                        .system_ref(&inst.software_system_id)
                        .map(|s| s.item.name.clone())
                        .unwrap_or_default();
                    (
                        Self::node_chain_of(node, ancestors),
                        system,
                        inst.instance_id,
                    )
                })
        })
    }

    /// `(node chain WITHOUT scheme, system name, container name, instanceId)`.
    fn ci_context(&self, id: &Id) -> Option<(String, String, String, i32)> {
        self.walk_nodes_search(&mut |node, ancestors| {
            node.container_instances
                .iter()
                .find(|i| &i.item.id == id)
                .map(|inst| {
                    let container = self
                        .container_ref(&inst.container_id)
                        .map(|c| c.item.name.clone())
                        .unwrap_or_default();
                    let system = self
                        .system_of_container(&inst.container_id)
                        .map(|s| s.item.name.clone())
                        .unwrap_or_default();
                    (
                        Self::node_chain_of(node, ancestors),
                        system,
                        container,
                        inst.instance_id,
                    )
                })
        })
    }

    /// The reference `generate(DeploymentNode).substring("DeploymentNode://")`
    /// for a node: `ENV/A/…/nodeName` (env + ancestors + own name).
    fn node_chain_of(node: &DeploymentNode, ancestors: &[String]) -> String {
        let chain: Vec<&str> = ancestors
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(node.item.name.as_str()))
            .collect();
        crate::canonical::deployment_node_chain(&node.environment, &chain)
    }

    // --- private tree walkers --------------------------------------------

    /// Find the software system containing a container id.
    fn system_of_container(&self, id: &Id) -> Option<&SoftwareSystem> {
        self.workspace
            .model
            .software_systems
            .iter()
            .find(|s| s.containers.iter().any(|c| &c.item.id == id))
    }

    /// `(system, container)` for a component id.
    fn system_and_container_of_component(&self, id: &Id) -> Option<(&SoftwareSystem, &Container)> {
        for s in &self.workspace.model.software_systems {
            for c in &s.containers {
                if c.components.iter().any(|k| &k.item.id == id) {
                    return Some((s, c));
                }
            }
        }
        None
    }

    /// The `group` of a deployment-tree element (node, infra, or instance).
    fn group_in_tree(nodes: &[DeploymentNode], id: &Id) -> Option<String> {
        for node in nodes {
            if &node.item.id == id {
                return node.group.clone();
            }
            for i in &node.infrastructure_nodes {
                if &i.item.id == id {
                    return i.group.clone();
                }
            }
            for s in &node.software_system_instances {
                if &s.item.id == id {
                    return s.group.clone();
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return c.group.clone();
                }
            }
            if let Some(g) = Self::group_in_tree(&node.children, id) {
                return Some(g);
            }
        }
        None
    }

    /// Walk every deployment node in DFS (model) order, passing the node and
    /// its ancestor names (root-first) to `f`; return the first `Some`.
    fn walk_nodes_search<T>(
        &self,
        f: &mut dyn FnMut(&DeploymentNode, &[String]) -> Option<T>,
    ) -> Option<T> {
        fn go<T>(
            nodes: &[DeploymentNode],
            ancestors: &mut Vec<String>,
            f: &mut dyn FnMut(&DeploymentNode, &[String]) -> Option<T>,
        ) -> Option<T> {
            for node in nodes {
                if let Some(r) = f(node, ancestors) {
                    return Some(r);
                }
                ancestors.push(node.item.name.clone());
                let r = go(&node.children, ancestors, f);
                ancestors.pop();
                if r.is_some() {
                    return r;
                }
            }
            None
        }
        go(&self.workspace.model.deployment_nodes, &mut Vec::new(), f)
    }

    /// Visit every instance in element order (SSIs then CIs per node,
    /// depth-first) with `(id, static element id, environment, kind)`.
    fn walk_instances(&self, f: &mut dyn FnMut(&Id, &Id, &str, ElementKind)) {
        fn go(nodes: &[DeploymentNode], f: &mut dyn FnMut(&Id, &Id, &str, ElementKind)) {
            for node in nodes {
                for s in &node.software_system_instances {
                    f(
                        &s.item.id,
                        &s.software_system_id,
                        &s.environment,
                        ElementKind::SoftwareSystemInstance,
                    );
                }
                for c in &node.container_instances {
                    f(
                        &c.item.id,
                        &c.container_id,
                        &c.environment,
                        ElementKind::ContainerInstance,
                    );
                }
                go(&node.children, f);
            }
        }
        go(&self.workspace.model.deployment_nodes, f);
    }

    /// Attach a perspective to an element or relationship by id.
    pub fn add_perspective(&mut self, id: &Id, perspective: Perspective) -> Result<(), CoreError> {
        // `ModelItem.addPerspective(Perspective)` / the relationship and
        // instance equivalents — the OBJECT path: duplicate-name guard only
        // (the empty-name guard lives in the 3-arg `addPerspective`, which
        // the DSL inline/archetype-application paths run before calling
        // this).
        let duplicate = if let Some(item) = self.item_mut(id) {
            let dup = item.perspectives.iter().any(|p| p.name == perspective.name);
            if !dup {
                item.perspectives.push(perspective.clone());
            }
            dup
        } else if let Some(rel) = self.relationship_mut(id) {
            let dup = rel.perspectives.iter().any(|p| p.name == perspective.name);
            if !dup {
                rel.perspectives.push(perspective.clone());
            }
            dup
        } else {
            return Err(CoreError::ElementNotFound { id: id.clone() });
        };
        if duplicate {
            return Err(CoreError::DuplicatePerspective {
                name: perspective.name,
            });
        }
        Ok(())
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

    /// `technology <technology>` for relationships — the
    /// `RelationshipsParser` fan-out arm (reference `Relationship.setTechnology`).
    pub fn set_relationship_technology(
        &mut self,
        id: &Id,
        technology: &str,
    ) -> Result<(), CoreError> {
        if let Some(rel) = self.relationship_mut(id) {
            rel.technology = Some(technology.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `instances <number|range>` — `DeploymentNode.setInstances`: the
    /// reference stores the RAW string (numbers and ranges alike), no
    /// validation.
    pub fn set_instances(&mut self, id: &Id, instances: &str) -> Result<(), CoreError> {
        match self.element_mut(id) {
            Some(ElementMut::Node(n)) => {
                n.instances = instances.to_string();
                Ok(())
            }
            _ => Err(CoreError::ElementNotFound { id: id.clone() }),
        }
    }

    /// `tags "a,b"` — ADD tags (reference `addTags`, trimmed, first position).
    pub fn add_tags(&mut self, id: &Id, csv: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            for t in java_split_csv(csv) {
                item.tags.add(t);
            }
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            for t in java_split_csv(csv) {
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
            for t in java_split_csv(csv) {
                item.tags.add(t);
            }
        }
        item
    }

    /// `Model.addPerson`/`addSoftwareSystem`/`addCustomElement` duplicate
    /// guard — verified @ `ad34036`: the lookup (`getPersonWithName`, …)
    /// covers ONLY the same kind, so a person and a software system MAY
    /// share a name (all three methods throw the same text).
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

    /// `GroupableElement.getGroup()` — the stored group name.
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

/// DSL convention: the reference passes `""` for omitted positional args, and
/// empty descriptions are stored as null (`NON_EMPTY` on write).
fn filter_default(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Java `String.split(",")` (limit 0): an EMPTY input yields `[""]`;
/// otherwise all TRAILING empty parts are dropped (leading/middle empties
/// survive, and `","` splits to the empty array). Tag parts are then added
/// via [`crate::tags::Tags::add`] which trims — so `tags "a, ,"` stores
/// `["a", ""]` exactly like the reference.
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

/// Model id ordering matching Java's `Model.getElements()` /
/// `getRelationships()` iteration: ids come from one sequential counter, so
/// creation order is ascending numeric id order. Non-numeric ids (JSON
/// loads) sort last, lexicographically.
pub(crate) fn id_sort_key(s: &str) -> (u64, String) {
    match s.parse::<u64>() {
        Ok(n) => (n, String::new()),
        Err(_) => (u64::MAX, s.to_string()),
    }
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
        // `AbstractRelationshipParser.createRelationship` + `Element.has`:
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
        // Verified @ ad34036: `Model.addPerson`/`addSoftwareSystem`/
        // `addCustomElement` each guard ONLY against a same-kind collision
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
        // M5-a (`workspace extends`): the base becomes the workspace under
        // construction and its ids seed the counter (reference `Model.add`
        // calls found() on every add, JSON loads included).
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
