//! The static + deployment model (`com.structurizr.model`). Plain data; field
//! names mirror the Java model classes so the JSON layer maps 1:1 (P1-D4).
//!
//! Structure mirrors OWNSHIP, not the reference's flat id maps: containers live
//! in their system, components in their container, deployment children in their
//! node. Relationships live in a flat arena on [`Model`] (referenced by id from
//! their source element) because they are shared storage with a counter that is
//! ALSO shared with elements (Locked).

use std::collections::BTreeMap;

use crate::ids::Id;
use crate::tags::{InteractionStyle, Tags};

/// A perspective (`model/Perspective`): `{name, description?, value?, url?}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Perspective {
    /// Perspective name (required).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional value.
    pub value: Option<String>,
    /// Optional URL.
    pub url: Option<String>,
}

/// An HTTP health check (`model/HttpHealthCheck`) attached to software-system /
/// container instances. Java primitives `interval` (seconds, default 60) and
/// `timeout` (millis, default 0) are NEVER "empty" under `NON_EMPTY` — both
/// are ALWAYS written (WRITE-CATALOG §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpHealthCheck {
    /// Display name (the reference defaults it to the URL).
    pub name: String,
    /// URL to check.
    pub url: String,
    /// Extra request headers.
    pub headers: BTreeMap<String, String>,
    /// Check interval, seconds.
    pub interval: i32,
    /// Timeout, milliseconds.
    pub timeout: i64,
}

impl Default for HttpHealthCheck {
    fn default() -> Self {
        HttpHealthCheck {
            name: String::new(),
            url: String::new(),
            headers: BTreeMap::new(),
            interval: 60,
            timeout: 0,
        }
    }
}

/// Common fields of every identified model item (`model/ModelItem`): elements,
/// deployment elements and relationships all carry these (relationships just
/// have no NAME — their description plays that role).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelItem {
    /// Shared-counter id (string-typed; see [`crate::ids`]).
    pub id: Id,
    /// Item name (reference: non-null, non-empty).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Stored (custom) tags; kind defaults are derived at render time
    /// ([`crate::tags::TagKind`]).
    pub tags: Tags,
    /// Optional URL.
    pub url: Option<String>,
    /// Free-form properties.
    pub properties: crate::properties::Properties,
    /// Perspectives, in definition order.
    pub perspectives: Vec<Perspective>,
}

impl ModelItem {
    /// A new item with only a required name (empty optionals).
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> ModelItem {
        ModelItem {
            id,
            name: name.into(),
            description: None,
            tags: Tags::new(),
            url: None,
            properties: crate::properties::Properties::new(),
            perspectives: Vec::new(),
        }
    }
}

/// A relationship (`model/Relationship`). Source/destination are ids into the
/// [`Model`] element space; the reference hydrates object links, we resolve on
/// demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    /// Shared-counter id.
    pub id: Id,
    /// Source element id (JSON `sourceId`).
    pub source: Id,
    /// Destination element id (JSON `destinationId`).
    pub destination: Id,
    /// Optional description.
    pub description: Option<String>,
    /// Optional technology.
    pub technology: Option<String>,
    /// Stored tags (a normal relationship ALSO derives `Relationship` +style
    /// tags — [`crate::tags::relationship_defaults`]).
    pub tags: Tags,
    /// Optional URL.
    pub url: Option<String>,
    /// Free-form properties.
    pub properties: crate::properties::Properties,
    /// Perspectives.
    pub perspectives: Vec<Perspective>,
    /// Interaction style; NOT settable from the DSL (JSON round-trip only).
    pub interaction_style: Option<InteractionStyle>,
    /// Set on implied / instance-replicated relationships.
    pub linked_relationship_id: Option<Id>,
}

/// A person (`model/Person`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group (`GroupableElement`).
    pub group: Option<String>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A software system (`model/SoftwareSystem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareSystem {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Owned containers.
    pub containers: Vec<Container>,
    /// Nested documentation (`Documentable`; the lazy bean ALWAYS serializes,
    /// at least as `{ }` — WRITE-CATALOG §2).
    pub documentation: crate::docs::Documentation,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A container (`model/Container`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Optional technology.
    pub technology: Option<String>,
    /// Owned components.
    pub components: Vec<Component>,
    /// Nested documentation (`Documentable`; the lazy bean ALWAYS serializes).
    pub documentation: crate::docs::Documentation,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A component (`model/Component`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Optional technology.
    pub technology: Option<String>,
    /// Nested documentation (`Documentable`; the lazy bean ALWAYS serializes).
    pub documentation: crate::docs::Documentation,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A custom element (`model/CustomElement`) — the kind lives in its custom tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomElement {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Free-form `metadata` string (Java-only; the DSL never sets it).
    pub metadata: Option<String>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A deployment node (`model/DeploymentNode`). Environment is a plain stored
/// field (`DeploymentElement.environment`, reference default `"Default"`) —
/// there is NO read-time inheritance; the creation API propagates the
/// environment from parent to child, so JSON produced by the tooling always
/// carries it on every node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentNode {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Environment (empty until a loader sets it; the reference default is
    /// `"Default"`).
    pub environment: String,
    /// Optional technology.
    pub technology: Option<String>,
    /// Instance count (JSON `instances` — a Java STRING, default `"1"`; the
    /// reader must also accept numbers, the writer emits the string).
    pub instances: String,
    /// Deployment groups (reference `TreeSet` — writer emits SORTED).
    pub deployment_groups: Vec<String>,
    /// Child nodes.
    pub children: Vec<DeploymentNode>,
    /// Attached infrastructure nodes.
    pub infrastructure_nodes: Vec<InfrastructureNode>,
    /// Software-system instances.
    pub software_system_instances: Vec<SoftwareSystemInstance>,
    /// Container instances.
    pub container_instances: Vec<ContainerInstance>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// An infrastructure node (`model/InfrastructureNode`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfrastructureNode {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group (deployment elements are groupable too).
    pub group: Option<String>,
    /// Environment (propagated from the parent node at creation).
    pub environment: String,
    /// Optional technology.
    pub technology: Option<String>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A software-system instance (`model/SoftwareSystemInstance`). `instance_id`
/// is the instance's OWN integer (canonical-name `[n]`, JSON `instanceId`); the
/// static link is `software_system_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareSystemInstance {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Environment (propagated from the parent node at creation).
    pub environment: String,
    /// Static software system id (JSON `softwareSystemId`).
    pub software_system_id: Id,
    /// Own integer id (JSON `instanceId` — per-node per-system 1-based count).
    pub instance_id: i32,
    /// Deployment groups (reference `TreeSet` — writer emits SORTED).
    pub deployment_groups: Vec<String>,
    /// Health checks.
    pub health_checks: Vec<HttpHealthCheck>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// A container instance (`model/ContainerInstance`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInstance {
    /// Shared fields.
    pub item: ModelItem,
    /// Optional group.
    pub group: Option<String>,
    /// Environment (propagated from the parent node at creation).
    pub environment: String,
    /// Static container id (JSON `containerId`).
    pub container_id: Id,
    /// Own integer id (JSON `instanceId` — per-node per-container 1-based count).
    pub instance_id: i32,
    /// Deployment groups (reference `TreeSet` — writer emits SORTED).
    pub deployment_groups: Vec<String>,
    /// Health checks.
    pub health_checks: Vec<HttpHealthCheck>,
    /// Outgoing relationship ids, in definition order.
    pub relationships: Vec<Id>,
}

/// The architecture model (`model/Model`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Model {
    /// Custom `element` definitions (JSON `customElements`).
    pub custom_elements: Vec<CustomElement>,
    /// People.
    pub people: Vec<Person>,
    /// Software systems with their containers/components.
    pub software_systems: Vec<SoftwareSystem>,
    /// Top-level deployment nodes per environment.
    pub deployment_nodes: Vec<DeploymentNode>,
    /// ALL relationships (each element's `relationships` lists reference these).
    pub relationships: Vec<Relationship>,
    /// Model-level properties.
    pub properties: crate::properties::Properties,
}

impl Default for DeploymentNode {
    /// Matches the reference field initializer: environment defaults to
    /// [`crate::DEFAULT_ENVIRONMENT`].
    fn default() -> Self {
        DeploymentNode {
            item: ModelItem::new(Id::new(""), String::new()),
            group: None,
            environment: crate::DEFAULT_ENVIRONMENT.to_string(),
            technology: None,
            instances: crate::DEFAULT_INSTANCES.to_string(),
            deployment_groups: Vec::new(),
            children: Vec::new(),
            infrastructure_nodes: Vec::new(),
            software_system_instances: Vec::new(),
            container_instances: Vec::new(),
            relationships: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_defaults_to_default() {
        assert_eq!(
            DeploymentNode::default().environment,
            crate::DEFAULT_ENVIRONMENT
        );
    }
}
