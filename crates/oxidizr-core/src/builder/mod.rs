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
mod relationships;
mod views;

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

    /// The structural parent of an element:
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

    /// The canonical name for the common kinds
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

    /// Efferent relationship ids in model
    /// (insertion) order.
    // Official-parity surface (the read-side relationship list); consumers
    // use the efferent CHECKS instead — kept deliberately.
    #[allow(dead_code)]
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

    /// Every model element id in official iteration order
    /// (people, software systems with their containers/components, custom
    /// elements, then the deployment tree). Used by the DSL canonical-name
    /// lookups and the `include *` / expression finders.
    #[must_use]
    pub fn all_element_ids(&self) -> Vec<Id> {
        let mut ids: Vec<Id> = crate::validate::flattened_ids(&self.workspace.model)
            .into_iter()
            .map(|(id, _, _)| id.clone())
            .collect();
        ids.sort_by_key(|id| id_sort_key(id.as_str()));
        ids
    }

    /// The element id whose canonical name matches exactly.
    #[must_use]
    pub fn element_with_canonical_name(&self, canonical_name: &str) -> Option<Id> {
        self.all_element_ids()
            .into_iter()
            .find(|id| self.canonical_name_full(id).as_deref() == Some(canonical_name))
    }

    /// The relationship id whose canonical name matches exactly.
    #[must_use]
    pub fn relationship_with_canonical_name(&self, canonical_name: &str) -> Option<Id> {
        self.workspace
            .model
            .relationships
            .iter()
            .find(|r| self.relationship_canonical_name(&r.id).as_deref() == Some(canonical_name))
            .map(|r| r.id.clone())
    }

    /// The full canonical name of any element (deployment
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

    /// The canonical name of a relationship (see [`mod@crate::canonical`]).
    #[must_use]
    pub fn relationship_canonical_name(&self, id: &Id) -> Option<String> {
        let r = self.relationship(id)?;
        let source = self.canonical_name_full(&r.source)?;
        let dest = self.canonical_name_full(&r.destination)?;
        Some(crate::canonical::relationship(
            &source,
            &dest,
            r.description.as_deref(),
        ))
    }

    /// The technology of containers, components, deployment
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
        // The walk already carries a found value in its `Option<T>` return
        // channel — stashing it into a captured `out`
        // holder with stop-signals was wasted machinery.
        self.walk_nodes_search(&mut |node, _ancestors| {
            if &node.item.id == id {
                return Some(node.technology.clone().unwrap_or_default());
            }
            for i in &node.infrastructure_nodes {
                if &i.item.id == id {
                    return Some(i.technology.clone().unwrap_or_default());
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return Some(
                        self.container_ref(&c.container_id)
                            .and_then(|c| c.technology.clone())
                            .unwrap_or_default(),
                    );
                }
            }
            None
        })
        .unwrap_or_default()
    }

    /// `Relationship.getTechnology()` (empty string when unset).
    // Official-parity surface (the relationship technology getter); the
    // exporters read the model field directly — kept deliberately.
    #[allow(dead_code)]
    #[must_use]
    pub fn relationship_technology(&self, id: &Id) -> String {
        self.relationship(id)
            .and_then(|r| r.technology.clone())
            .unwrap_or_default()
    }

    /// `Relationship.getLinkedRelationshipId()`.
    #[must_use]
    pub fn linked_relationship_id(&self, id: &Id) -> Option<Id> {
        self.relationship(id)
            .and_then(|r| r.linked_relationship_id.clone())
    }

    /// The group name for any groupable element.
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

    /// Relationship iteration order: the official model stores
    /// relationships ordered by the item comparator —
    /// ascending numeric id (see `id_sort_key`).
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

    /// Whether `tag` is an EFFECTIVE tag of the item: derived
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
        if let Some(r) = self.relationship(id) {
            let defaults = crate::tags::relationship_defaults(
                r.linked_relationship_id.is_some(),
                r.interaction_style,
            );
            return r.tags.has_against(&defaults, tag);
        }
        false
    }

    /// Whether an element or relationship carries `name=value`.
    #[must_use]
    pub fn item_has_property(&self, id: &Id, name: &str, value: &str) -> bool {
        if let Some(item) = self.item(id) {
            return item.properties.get(name) == Some(value);
        }
        self.relationship(id)
            .is_some_and(|r| r.properties.get(name) == Some(value))
    }

    /// The default tags of an element id's kind;
    /// empty for a relationship or unknown id.
    // Official-parity surface (the derived-default-tags accessor); the JSON
    // reader computes defaults from kind directly — kept deliberately.
    #[allow(dead_code)]
    #[must_use]
    pub fn element_tag_defaults(&self, id: &Id) -> &'static [&'static str] {
        self.element_kind(id).map_or(&[], |k| {
            crate::tags::TagKind::from_element_kind(k).default_tags()
        })
    }

    /// The interaction style of a relationship (never set from the DSL).
    #[must_use]
    pub fn relationship_style(&self, id: &Id) -> Option<crate::tags::InteractionStyle> {
        self.relationship(id).and_then(|r| r.interaction_style)
    }

    /// The DSL relationship
    /// entry point, in EXACT official order: kind dispatch (`uses(...)`
    /// overload selection), then the parent/child
    /// guard, then the null-result duplicate check (destination AND
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
        // The official add returns null only when destination AND
        // description match. The official parser's any-description
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

    /// The permitted relationship-kind dispatch: `CustomElement` on either
    /// side, a static-structure pair, or the deployment chain node→node,
    /// node→infra, infra→deployment-element, instance→infra.
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

    /// The implied-relationships hook, shared by
    /// [`WorkspaceBuilder::add_relationship`] and
    /// [`WorkspaceBuilder::dsl_uses`].
    fn maybe_create_implied(&mut self, id: &Id) {
        // The relationship was inserted moments before every call site; if
        // it were missing there is nothing to hook.
        let Some((source, destination)) = self
            .relationship(id)
            .map(|r| (r.source.clone(), r.destination.clone()))
        else {
            return;
        };
        if self.implied_strategy != ImpliedRelationshipsStrategy::Default
            && is_implied_kind(self.element_kind(&source))
            && is_implied_kind(self.element_kind(&destination))
        {
            self.create_implied_relationships(id);
        }
    }

    /// The first
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
    /// official semantics (trim, first position kept).
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

    /// The first efferent to `destination` with exactly `description`.
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
        let r = self.relationship(id)?;
        Some((
            r.description.clone().unwrap_or_default(),
            r.technology.clone().unwrap_or_default(),
            r.linked_relationship_id.clone(),
        ))
    }

    /// The relationship's own id (existence check).
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

    /// Instance lookup for the deployment relationship arms:
    /// All software-system instances of `system` in `env`, in element order.
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

    /// All container instances of `container` in `env`, in element order.
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

    /// Every instance of a static element in ANY environment (the official
    /// stream matches by type and equality alone,
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

    /// The parent of a static-structure-element instance (its
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

    /// For a node:
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
        // Perspectives, object path — element, relationship and
        // instance equivalents — duplicate-name guard only
        // (the empty-name guard lives in the name-based add, which
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
    /// fan-out arm.
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
    /// the format stores the RAW string (numbers and ranges alike), no
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

    /// `tags "a,b"` — ADD tags (trimmed, first position).
    // Official-parity surface (the add-tags entry); the DSL statement uses
    // `add_tag_parts` (the varargs form) — kept per the parity surface,
    #[allow(dead_code)]
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
