//! The unified element view the diagram engine iterates over (the official
//! engine walks live element graphs; the Rust model stores typed
//! vectors, so every view iteration resolves ids through this adapter).
//!
//! Each variant borrows the element AND — where the official tooling engine uses
//! parent identity — its parent, so parent checks need no
//! model lookup. [`ElemRef::target`] resolves `StaticStructureElementInstance`
//! legs (the official target lookup of instances).

use oxidizr_core::Id;
use oxidizr_core::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, InfrastructureNode,
    Model, ModelItem, Person, SoftwareSystem, SoftwareSystemInstance,
};
use oxidizr_core::tags::Tags;
use oxidizr_core::validate::ElementKind;

/// One model element of any kind, with its structural parent (the official tooling
/// parent link).
#[derive(Debug, Clone, Copy)]
pub enum ElemRef<'a> {
    /// `model.CustomElement` (top level — no parent).
    Custom(&'a CustomElement),
    /// `model.Person` (top level).
    Person(&'a Person),
    /// `model.SoftwareSystem` (top level).
    System(&'a SoftwareSystem),
    /// A container with its owning system (the parent link).
    Container {
        /// The container.
        elem: &'a Container,
        /// Owning software system.
        system: &'a SoftwareSystem,
    },
    /// `model.Component` with its owning container.
    Component {
        /// The component.
        elem: &'a Component,
        /// Owning container.
        container: &'a Container,
    },
    /// `model.DeploymentNode` with its parent node (none at top level).
    DeploymentNode {
        /// The node.
        elem: &'a DeploymentNode,
        /// Parent node, if nested.
        parent: Option<&'a DeploymentNode>,
    },
    /// `model.InfrastructureNode` with its node.
    InfrastructureNode {
        /// The infrastructure node.
        elem: &'a InfrastructureNode,
        /// Attached node.
        node: &'a DeploymentNode,
    },
    /// `model.SoftwareSystemInstance` with its node and static system.
    SystemInstance {
        /// The instance.
        elem: &'a SoftwareSystemInstance,
        /// Containing node.
        node: &'a DeploymentNode,
    },
    /// `model.ContainerInstance` with its node and static container.
    ContainerInstance {
        /// The instance.
        elem: &'a ContainerInstance,
        /// Containing node.
        node: &'a DeploymentNode,
    },
}

impl<'a> ElemRef<'a> {
    /// The shared item fields.
    #[must_use]
    pub const fn item(self) -> &'a ModelItem {
        match self {
            ElemRef::Custom(e) => &e.item,
            ElemRef::Person(e) => &e.item,
            ElemRef::System(e) => &e.item,
            ElemRef::Container { elem, .. } => &elem.item,
            ElemRef::Component { elem, .. } => &elem.item,
            ElemRef::DeploymentNode { elem, .. } => &elem.item,
            ElemRef::InfrastructureNode { elem, .. } => &elem.item,
            ElemRef::SystemInstance { elem, .. } => &elem.item,
            ElemRef::ContainerInstance { elem, .. } => &elem.item,
        }
    }

    /// The model id.
    #[must_use]
    pub fn id(self) -> &'a Id {
        &self.item().id
    }

    /// The name.
    #[must_use]
    pub fn name(self) -> &'a str {
        &self.item().name
    }

    /// The optional description.
    #[must_use]
    pub fn description(self) -> Option<&'a str> {
        self.item().description.as_deref()
    }

    /// The optional URL (`ModelItem.url`).
    #[must_use]
    pub fn url(self) -> Option<&'a str> {
        self.item().url.as_deref()
    }

    /// The element kind.
    #[must_use]
    pub const fn kind(self) -> ElementKind {
        match self {
            ElemRef::Custom(_) => ElementKind::CustomElement,
            ElemRef::Person(_) => ElementKind::Person,
            ElemRef::System(_) => ElementKind::SoftwareSystem,
            ElemRef::Container { .. } => ElementKind::Container,
            ElemRef::Component { .. } => ElementKind::Component,
            ElemRef::DeploymentNode { .. } => ElementKind::DeploymentNode,
            ElemRef::InfrastructureNode { .. } => ElementKind::InfrastructureNode,
            ElemRef::SystemInstance { .. } => ElementKind::SoftwareSystemInstance,
            ElemRef::ContainerInstance { .. } => ElementKind::ContainerInstance,
        }
    }

    /// The stored tags (style resolution goes through [`ElemRef::tags_csv`]).
    #[must_use]
    pub fn tags(self) -> &'a Tags {
        &self.item().tags
    }

    /// The FULL tag CSV (defaults + stored) used for style resolution —
    /// The tags as the official model exposes them.
    #[must_use]
    pub fn tags_csv(self) -> String {
        self.tags()
            .serialized_for(oxidizr_core::tags::TagKind::from_element_kind(self.kind()))
    }

    /// The group (`GroupableElement.group`; every kind is groupable at the
    /// identity-based, so every variant answers).
    #[must_use]
    pub fn group(self) -> Option<&'a str> {
        match self {
            ElemRef::Custom(e) => e.group.as_deref(),
            ElemRef::Person(e) => e.group.as_deref(),
            ElemRef::System(e) => e.group.as_deref(),
            ElemRef::Container { elem, .. } => elem.group.as_deref(),
            ElemRef::Component { elem, .. } => elem.group.as_deref(),
            ElemRef::DeploymentNode { elem, .. } => elem.group.as_deref(),
            ElemRef::InfrastructureNode { elem, .. } => elem.group.as_deref(),
            ElemRef::SystemInstance { elem, .. } => elem.group.as_deref(),
            ElemRef::ContainerInstance { elem, .. } => elem.group.as_deref(),
        }
    }

    /// The technology — only `Container`/`Component`/`DeploymentNode`/
    /// `InfrastructureNode` carry one (`typeOf`'s `: <tech>` suffix).
    #[must_use]
    pub fn technology(self) -> Option<&'a str> {
        match self {
            ElemRef::Container { elem, .. } => elem.technology.as_deref(),
            ElemRef::Component { elem, .. } => elem.technology.as_deref(),
            ElemRef::DeploymentNode { elem, .. } => elem.technology.as_deref(),
            ElemRef::InfrastructureNode { elem, .. } => elem.technology.as_deref(),
            _ => None,
        }
    }

    /// The custom element's `metadata` (its `typeOf` value).
    #[must_use]
    pub fn metadata(self) -> Option<&'a str> {
        match self {
            ElemRef::Custom(e) => e.metadata.as_deref(),
            _ => None,
        }
    }

    /// The environment of a deployment element (`DeploymentElement`).
    #[must_use]
    pub fn environment(self) -> Option<&'a str> {
        match self {
            ElemRef::DeploymentNode { elem, .. } => Some(&elem.environment),
            ElemRef::InfrastructureNode { elem, .. } => Some(&elem.environment),
            ElemRef::SystemInstance { elem, .. } => Some(&elem.environment),
            ElemRef::ContainerInstance { elem, .. } => Some(&elem.environment),
            _ => None,
        }
    }

    /// The deployment node’s instance-count string (the instance count,
    /// always non-null; `"1"` default).
    #[must_use]
    pub fn instances(self) -> Option<&'a str> {
        match self {
            ElemRef::DeploymentNode { elem, .. } => Some(&elem.instances),
            _ => None,
        }
    }

    /// The instance's own `instanceId` (the `_n` `PlantUML` id suffix).
    #[must_use]
    pub fn instance_id(self) -> Option<i32> {
        match self {
            ElemRef::SystemInstance { elem, .. } => Some(elem.instance_id),
            ElemRef::ContainerInstance { elem, .. } => Some(elem.instance_id),
            _ => None,
        }
    }

    /// The parent as an id (identity in the engine
    /// becomes id equality — ids are unique workspace-wide).
    #[must_use]
    pub fn parent_id(self) -> Option<&'a Id> {
        match self {
            ElemRef::Container { system, .. } => Some(&system.item.id),
            ElemRef::Component { container, .. } => Some(&container.item.id),
            ElemRef::DeploymentNode { parent, .. } => parent.map(|p| &p.item.id),
            ElemRef::InfrastructureNode { node, .. }
            | ElemRef::SystemInstance { node, .. }
            | ElemRef::ContainerInstance { node, .. } => Some(&node.item.id),
            ElemRef::Custom(_) | ElemRef::Person(_) | ElemRef::System(_) => None,
        }
    }

    /// The `StaticStructureElementInstance.getElement()` target (the static
    /// element an instance represents).
    #[must_use]
    pub fn target(self, model: &'a Model) -> Option<ElemRef<'a>> {
        match self {
            ElemRef::SystemInstance { elem, .. } => element_by_id(model, &elem.software_system_id),
            ElemRef::ContainerInstance { elem, .. } => element_by_id(model, &elem.container_id),
            _ => None,
        }
    }

    /// The owning system of a container element (boundary grouping).
    #[must_use]
    pub fn system(self) -> Option<&'a SoftwareSystem> {
        match self {
            ElemRef::Container { system, .. } => Some(system),
            _ => None,
        }
    }

    /// The owning system’s id for a container (the official
    /// parent-identity test); for every other
    /// kind, the element's own id (the comparison never matches a system
    /// id for those anyway — ids are unique workspace-wide).
    #[must_use]
    pub fn owner_system_id(self) -> &'a Id {
        match self {
            ElemRef::Container { system, .. } => &system.item.id,
            _ => self.id(),
        }
    }

    /// The target element for instances, the element itself otherwise
    /// (`name`/`description`/`type`/`url` resolution in the element writer).
    #[must_use]
    pub fn referent(self, model: &'a Model) -> ElemRef<'a> {
        self.target(model).unwrap_or(self)
    }
}

/// Resolve any model id to its [`ElemRef`] (the engine's `getElement()`);
/// walks systems' containers/components and deployment trees.
#[must_use]
pub fn element_by_id<'a>(model: &'a Model, id: &Id) -> Option<ElemRef<'a>> {
    for e in &model.custom_elements {
        if &e.item.id == id {
            return Some(ElemRef::Custom(e));
        }
    }
    for p in &model.people {
        if &p.item.id == id {
            return Some(ElemRef::Person(p));
        }
    }
    for s in &model.software_systems {
        if &s.item.id == id {
            return Some(ElemRef::System(s));
        }
        for c in &s.containers {
            if &c.item.id == id {
                return Some(ElemRef::Container { elem: c, system: s });
            }
            for k in &c.components {
                if &k.item.id == id {
                    return Some(ElemRef::Component {
                        elem: k,
                        container: c,
                    });
                }
            }
        }
    }
    for n in &model.deployment_nodes {
        if &n.item.id == id {
            return Some(ElemRef::DeploymentNode {
                elem: n,
                parent: None,
            });
        }
        if let Some(e) = deployment_by_id(n, id) {
            return Some(e);
        }
    }
    None
}

/// Depth-first lookup inside a deployment tree (`node` is the parent for
/// every element found here).
fn deployment_by_id<'a>(node: &'a DeploymentNode, id: &Id) -> Option<ElemRef<'a>> {
    for i in &node.infrastructure_nodes {
        if &i.item.id == id {
            return Some(ElemRef::InfrastructureNode { elem: i, node });
        }
    }
    for si in &node.software_system_instances {
        if &si.item.id == id {
            return Some(ElemRef::SystemInstance { elem: si, node });
        }
    }
    for ci in &node.container_instances {
        if &ci.item.id == id {
            return Some(ElemRef::ContainerInstance { elem: ci, node });
        }
    }
    for child in &node.children {
        if &child.item.id == id {
            return Some(ElemRef::DeploymentNode {
                elem: child,
                parent: Some(node),
            });
        }
        if let Some(e) = deployment_by_id(child, id) {
            return Some(e);
        }
    }
    None
}
