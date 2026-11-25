//! Read-side queries of [`WorkspaceBuilder`]: canonical names, element and
//! relationship accessors, the DSL-support legs (`dsl_uses`, efferent
//! helpers, relationship removal), the instance-find legs and the
//! deployment canonical-name context. The item/relationship borrows the
//! builders share stay in the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::model::{Container, DeploymentNode, SoftwareSystem};
use crate::tags::Tags;
use crate::validate::ElementKind;

use super::{
    ImpliedRelationshipsStrategy, WorkspaceBuilder, id_sort_key, is_implied_kind, node_parent,
};

impl WorkspaceBuilder {
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
}
