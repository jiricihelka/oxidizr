//! View content operations — the `com.structurizr.view` add/remove port that
//! backs the DSL `include`/`exclude`/`animation`/dynamic-step statements.
//!
//! Every rule here mirrors the pinned reference (`ad34036`):
//! `ModelView.addElement/addRelationships/removeElement`, the per-subclass
//! `checkElementCanBeAdded` gates and their `ElementNotPermittedInViewException`
//! texts, `DeploymentView`'s instance-cascading add, `StaticView.addAnimation`,
//! and `DynamicView.addRelationshipViaElements` (including the reverse lookup
//! that produces `response: true`). The DSL content parsers CATCH the
//! [`CoreError::ElementNotPermittedInView`] variant, exactly like the Java
//! `catch (ElementNotPermittedInViewException e)` blocks do.

// Same convention as `builder.rs`: failure modes are enumerated by the shared
// [`CoreError`] enum rather than repeated per method.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::collections::HashMap;

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;
use crate::validate::ElementKind;
use crate::views::{Animation, AutomaticLayout, ElementView, RankDirection, RelationshipView};

/// A view slot, handed out by the `create_*_view` builders and used by every
/// content operation. The payload is the index inside the matching
/// [`crate::views::ViewSet`] collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewRef {
    /// System landscape view.
    Landscape(usize),
    /// System context view.
    Context(usize),
    /// Container view.
    Container(usize),
    /// Component view.
    Component(usize),
    /// Dynamic view.
    Dynamic(usize),
    /// Deployment view.
    Deployment(usize),
    /// Filtered view (no content operations — key identity only).
    Filtered(usize),
    /// Image view (no content operations — key identity only).
    Image(usize),
    /// Custom view.
    Custom(usize),
}

/// Kind-set filters used by `addNearestNeighbours(element, Class)`.
#[derive(Clone, Copy)]
enum KindFilter {
    Custom,
    Person,
    SoftwareSystem,
}

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // accessors
    // ------------------------------------------------------------------

    /// The content of any model-view kind (`Filtered`/`Image` have none —
    /// the reference has no `ModelView` object for them).
    #[must_use]
    pub fn content_ref(&self, view: ViewRef) -> Option<&crate::views::ViewContent> {
        let views = &self.workspace().views;
        match view {
            ViewRef::Landscape(i) => views.system_landscape_views.get(i).map(|v| &v.content),
            ViewRef::Context(i) => views.system_context_views.get(i).map(|v| &v.content),
            ViewRef::Container(i) => views.container_views.get(i).map(|v| &v.content),
            ViewRef::Component(i) => views.component_views.get(i).map(|v| &v.content),
            ViewRef::Dynamic(i) => views.dynamic_views.get(i).map(|v| &v.content),
            ViewRef::Deployment(i) => views.deployment_views.get(i).map(|v| &v.content),
            ViewRef::Custom(i) => views.custom_views.get(i).map(|v| &v.content),
            ViewRef::Filtered(_) | ViewRef::Image(_) => None,
        }
    }

    /// Mutable content; public for the DSL engine's relationship-view
    /// property/url statements (everything else goes through the
    /// content-operations below).
    pub fn content_mut(&mut self, view: ViewRef) -> Option<&mut crate::views::ViewContent> {
        let views = &mut self.workspace_mut().views;
        match view {
            ViewRef::Landscape(i) => views
                .system_landscape_views
                .get_mut(i)
                .map(|v| &mut v.content),
            ViewRef::Context(i) => views
                .system_context_views
                .get_mut(i)
                .map(|v| &mut v.content),
            ViewRef::Container(i) => views.container_views.get_mut(i).map(|v| &mut v.content),
            ViewRef::Component(i) => views.component_views.get_mut(i).map(|v| &mut v.content),
            ViewRef::Dynamic(i) => views.dynamic_views.get_mut(i).map(|v| &mut v.content),
            ViewRef::Deployment(i) => views.deployment_views.get_mut(i).map(|v| &mut v.content),
            ViewRef::Custom(i) => views.custom_views.get_mut(i).map(|v| &mut v.content),
            ViewRef::Filtered(_) | ViewRef::Image(_) => None,
        }
    }

    fn animations_mut(&mut self, view: ViewRef) -> Option<&mut Vec<Animation>> {
        let views = &mut self.workspace_mut().views;
        match view {
            ViewRef::Landscape(i) => views
                .system_landscape_views
                .get_mut(i)
                .map(|v| &mut v.animations),
            ViewRef::Context(i) => views
                .system_context_views
                .get_mut(i)
                .map(|v| &mut v.animations),
            ViewRef::Container(i) => views.container_views.get_mut(i).map(|v| &mut v.animations),
            ViewRef::Component(i) => views.component_views.get_mut(i).map(|v| &mut v.animations),
            ViewRef::Deployment(i) => views.deployment_views.get_mut(i).map(|v| &mut v.animations),
            ViewRef::Custom(i) => views.custom_views.get_mut(i).map(|v| &mut v.animations),
            ViewRef::Dynamic(_) | ViewRef::Filtered(_) | ViewRef::Image(_) => None,
        }
    }

    /// The software-system scope of a model view, when it has one
    /// (`ModelView.getSoftwareSystem` / `CustomView`'s optional scope).
    fn view_scope_system(&self, view: ViewRef) -> Option<Id> {
        let views = &self.workspace().views;
        match view {
            ViewRef::Context(i) => views
                .system_context_views
                .get(i)
                .map(|v| v.software_system_id.clone()),
            ViewRef::Container(i) => views
                .container_views
                .get(i)
                .map(|v| v.software_system_id.clone()),
            ViewRef::Component(i) => views
                .component_views
                .get(i)
                .and_then(|v| self.element_parent(&v.container_id)),
            ViewRef::Deployment(i) => views
                .deployment_views
                .get(i)
                .and_then(|v| v.software_system_id.clone()),
            ViewRef::Custom(i) => views
                .custom_views
                .get(i)
                .and_then(|v| v.software_system_id.clone()),
            _ => None,
        }
    }

    /// The container scope of a component view.
    fn view_scope_container(&self, view: ViewRef) -> Option<Id> {
        match view {
            ViewRef::Component(i) => self
                .workspace()
                .views
                .component_views
                .get(i)
                .map(|v| v.container_id.clone()),
            _ => None,
        }
    }

    /// The scope element of a dynamic view.
    fn dynamic_scope(&self, view: ViewRef) -> Option<Id> {
        match view {
            ViewRef::Dynamic(i) => self
                .workspace()
                .views
                .dynamic_views
                .get(i)
                .and_then(|v| v.element_id.clone()),
            _ => None,
        }
    }

    /// Whether a dynamic view is deployment-scoped (`deploymentEnvironment !=
    /// null` ⇒ `checkElementCanBeAdded` is a no-op in the reference).
    fn dynamic_is_deployment(&self, view: ViewRef) -> bool {
        matches!(view, ViewRef::Dynamic(i) if self
            .workspace()
            .views
            .dynamic_views
            .get(i)
            .is_some_and(|v| v.deployment_environment.is_some()))
    }

    /// The environment of a deployment view.
    fn deployment_environment(&self, view: ViewRef) -> Option<String> {
        match view {
            ViewRef::Deployment(i) => self
                .workspace()
                .views
                .deployment_views
                .get(i)
                .map(|v| v.environment.clone()),
            _ => None,
        }
    }

    /// `ModelView.isElementInView`.
    #[must_use]
    pub fn view_contains_element(&self, view: ViewRef, element: &Id) -> bool {
        self.content_ref(view)
            .is_some_and(|c| c.elements.iter().any(|e| &e.id == element))
    }

    /// `ModelView.isRelationshipInView`.
    #[must_use]
    pub fn view_contains_relationship(&self, view: ViewRef, relationship: &Id) -> bool {
        self.content_ref(view)
            .is_some_and(|c| c.relationships.iter().any(|r| &r.id == relationship))
    }

    /// The ids of the elements currently in the view.
    #[must_use]
    pub fn view_element_ids(&self, view: ViewRef) -> Vec<Id> {
        self.content_ref(view)
            .map(|c| c.elements.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // gates (per-subclass `checkElementCanBeAdded`)
    // ------------------------------------------------------------------

    // One fn per reference `checkElementCanBeAdded` override keeps the port
    // 1:1 with the Java subclasses, hence the length.
    #[allow(clippy::too_many_lines)]
    fn check_element_can_be_added(&self, view: ViewRef, element: &Id) -> Result<(), CoreError> {
        let kind = self
            .element_kind(element)
            .ok_or_else(|| CoreError::ElementNotFound {
                id: element.clone(),
            })?;
        let not_permitted = |msg: &str| Err(CoreError::ElementNotPermittedInView(msg.to_string()));
        match view {
            ViewRef::Landscape(_) => {
                if matches!(
                    kind,
                    ElementKind::CustomElement | ElementKind::Person | ElementKind::SoftwareSystem
                ) {
                    Ok(())
                } else {
                    not_permitted(
                        "Only people and software systems can be added to a system landscape view.",
                    )
                }
            }
            ViewRef::Context(_) => {
                if matches!(
                    kind,
                    ElementKind::CustomElement | ElementKind::Person | ElementKind::SoftwareSystem
                ) {
                    Ok(())
                } else {
                    not_permitted(
                        "Only people and software systems can be added to a system context view.",
                    )
                }
            }
            ViewRef::Container(_) => {
                match kind {
                    ElementKind::CustomElement | ElementKind::Person => return Ok(()),
                    ElementKind::SoftwareSystem => {
                        if self.view_scope_system(view).as_ref() == Some(element) {
                            return not_permitted(
                                "The software system in scope cannot be added to a container view.",
                            );
                        }
                        self.check_parent_and_children(view, element)?;
                        return Ok(());
                    }
                    ElementKind::Container => {
                        self.check_parent_and_children(view, element)?;
                        return Ok(());
                    }
                    _ => {}
                }
                not_permitted(
                    "Only people, software systems, and containers can be added to a container view.",
                )
            }
            ViewRef::Component(_) => {
                match kind {
                    ElementKind::CustomElement | ElementKind::Person => return Ok(()),
                    ElementKind::SoftwareSystem => {
                        let scoped = self
                            .view_scope_container(view)
                            .and_then(|c| self.element_parent(&c));
                        if scoped.as_ref() == Some(element) {
                            return not_permitted(
                                "The software system in scope cannot be added to a component view.",
                            );
                        }
                        self.check_parent_and_children(view, element)?;
                        return Ok(());
                    }
                    ElementKind::Container => {
                        if self.view_scope_container(view).as_ref() == Some(element) {
                            return not_permitted(
                                "The container in scope cannot be added to a component view.",
                            );
                        }
                        self.check_parent_and_children(view, element)?;
                        return Ok(());
                    }
                    ElementKind::Component => {
                        self.check_parent_and_children(view, element)?;
                        return Ok(());
                    }
                    _ => {}
                }
                not_permitted(
                    "Only people, software systems, containers, and components can be added to a component view.",
                )
            }
            ViewRef::Dynamic(_) => self.check_dynamic_can_be_added(view, element, kind),
            ViewRef::Deployment(_) => {
                if kind == ElementKind::CustomElement {
                    return Ok(());
                }
                if !matches!(
                    kind,
                    ElementKind::DeploymentNode
                        | ElementKind::InfrastructureNode
                        | ElementKind::SoftwareSystemInstance
                        | ElementKind::ContainerInstance
                ) {
                    return not_permitted(
                        "Only deployment nodes, infrastructure nodes, software system instances, and container instances can be added to deployment views.",
                    );
                }
                let env = self.deployment_environment(view).unwrap_or_default();
                if self.element_environment(element).as_deref() != Some(env.as_str()) {
                    return Err(CoreError::ElementNotPermittedInView(format!(
                        "Only elements in the {env} deployment environment can be added to this view."
                    )));
                }
                if kind == ElementKind::SoftwareSystemInstance {
                    let Some(system) = self.instance_static_element(element) else {
                        return Ok(());
                    };
                    if self.view_scope_system(view).as_ref() == Some(&system) {
                        return not_permitted(
                            "The software system in scope cannot be added to a deployment view.",
                        );
                    }
                    // a child CONTAINER INSTANCE of the same system already in view?
                    let child_in_view = self.view_element_ids(view).into_iter().any(|e| {
                        self.element_kind(&e) == Some(ElementKind::ContainerInstance)
                            && self.instance_container_system(&e).as_ref() == Some(&system)
                    });
                    if child_in_view {
                        return Err(CoreError::ElementNotPermittedInView(format!(
                            "A child of {} is already in this view.",
                            self.item(element)
                                .map(|i| i.name.clone())
                                .unwrap_or_default()
                        )));
                    }
                }
                if kind == ElementKind::ContainerInstance {
                    let Some(system) = self.instance_container_system(element) else {
                        return Ok(());
                    };
                    let parent_in_view = self.view_element_ids(view).into_iter().any(|e| {
                        self.element_kind(&e) == Some(ElementKind::SoftwareSystemInstance)
                            && self.instance_static_element(&e).as_ref() == Some(&system)
                    });
                    if parent_in_view {
                        return Err(CoreError::ElementNotPermittedInView(format!(
                            "A parent of {} is already in this view.",
                            self.item(element)
                                .map(|i| i.name.clone())
                                .unwrap_or_default()
                        )));
                    }
                }
                Ok(())
            }
            ViewRef::Custom(_) => {
                if kind == ElementKind::CustomElement {
                    Ok(())
                } else {
                    not_permitted("Only custom elements can be added to a custom view.")
                }
            }
            ViewRef::Filtered(_) | ViewRef::Image(_) => Ok(()),
        }
    }

    /// `DynamicView.checkElementCanBeAdded`.
    fn check_dynamic_can_be_added(
        &self,
        view: ViewRef,
        element: &Id,
        kind: ElementKind,
    ) -> Result<(), CoreError> {
        let not_permitted = |msg: String| Err(CoreError::ElementNotPermittedInView(msg));
        let name = self
            .item(element)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        if self.dynamic_is_deployment(view) {
            // Reference: `else { // todo }` — deployment-scoped dynamic views
            // accept everything.
            return Ok(());
        }
        if kind == ElementKind::CustomElement {
            return Ok(());
        }
        if !matches!(
            kind,
            ElementKind::Person
                | ElementKind::SoftwareSystem
                | ElementKind::Container
                | ElementKind::Component
        ) {
            return not_permitted(
                "Only people, software systems, containers and components can be added to dynamic views."
                    .to_string(),
            );
        }
        if kind == ElementKind::Person {
            return Ok(());
        }
        let scope = self.dynamic_scope(view);
        let scope_kind = scope.as_ref().and_then(|s| self.element_kind(s));
        if scope_kind == Some(ElementKind::SoftwareSystem) {
            let scope = scope.clone().expect("checked");
            if element == &scope {
                return not_permitted(format!(
                    "{name} is already the scope of this view and cannot be added to it."
                ));
            }
            match kind {
                ElementKind::SoftwareSystem | ElementKind::Container => {
                    self.check_parent_and_children(view, element)?;
                }
                ElementKind::Component => {
                    return not_permitted(
                        "Components can't be added to a dynamic view when the scope is a software system."
                            .to_string(),
                    );
                }
                _ => {}
            }
            return Ok(());
        }
        if scope_kind == Some(ElementKind::Container) {
            let scope = scope.clone().expect("checked");
            let scope_parent = self.element_parent(&scope);
            if element == &scope || Some(element.clone()) == scope_parent {
                return not_permitted(format!(
                    "{name} is already the scope of this view and cannot be added to it."
                ));
            }
            self.check_parent_and_children(view, element)?;
            return Ok(());
        }
        // No scope: only people and software systems.
        if kind != ElementKind::SoftwareSystem {
            return not_permitted(
                "Only people and software systems can be added to this dynamic view.".to_string(),
            );
        }
        Ok(())
    }

    /// `ModelView.checkParentAndChildrenHaveNotAlreadyBeenAdded`.
    fn check_parent_and_children(&self, view: ViewRef, element: &Id) -> Result<(), CoreError> {
        let name = self
            .item(element)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let in_view: Vec<Id> = self.view_element_ids(view);
        let mut parent = self.element_parent(element);
        while let Some(p) = parent {
            if in_view.contains(&p) {
                return Err(CoreError::ElementNotPermittedInView(format!(
                    "A parent of {name} is already in this view."
                )));
            }
            parent = self.element_parent(&p);
        }
        for e in &in_view {
            let mut parent = self.element_parent(e);
            while let Some(p) = parent {
                if &p == element {
                    return Err(CoreError::ElementNotPermittedInView(format!(
                        "A child of {name} is already in this view."
                    )));
                }
                parent = self.element_parent(&p);
            }
        }
        Ok(())
    }

    /// The `environment` of a deployment element.
    #[must_use]
    pub fn element_environment(&self, id: &Id) -> Option<String> {
        let nodes = crate::builder::flatten_nodes(&self.workspace().model.deployment_nodes);
        for (node, _) in &nodes {
            if &node.item.id == id {
                return Some(node.environment.clone());
            }
            for i in &node.infrastructure_nodes {
                if &i.item.id == id {
                    return Some(i.environment.clone());
                }
            }
            for s in &node.software_system_instances {
                if &s.item.id == id {
                    return Some(s.environment.clone());
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return Some(c.environment.clone());
                }
            }
        }
        None
    }

    /// The static element an instance is based on.
    fn instance_static_element(&self, id: &Id) -> Option<Id> {
        let nodes = crate::builder::flatten_nodes(&self.workspace().model.deployment_nodes);
        for (node, _) in &nodes {
            for s in &node.software_system_instances {
                if &s.item.id == id {
                    return Some(s.software_system_id.clone());
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return Some(c.container_id.clone());
                }
            }
        }
        None
    }

    /// For a container instance: the software system its container belongs
    /// to (`ContainerInstance.getContainer().getSoftwareSystem()`).
    fn instance_container_system(&self, id: &Id) -> Option<Id> {
        let container = self.instance_static_element(id)?;
        self.element_parent(&container)
    }

    // ------------------------------------------------------------------
    // add / remove
    // ------------------------------------------------------------------

    /// `ModelView.addElement(element, addRelationships)`.
    pub fn view_add_element(
        &mut self,
        view: ViewRef,
        element: &Id,
        add_relationships: bool,
    ) -> Result<(), CoreError> {
        self.check_element_can_be_added(view, element)?;
        {
            let content = self
                .content_mut(view)
                .ok_or_else(|| CoreError::ElementNotFound {
                    id: element.clone(),
                })?;
            if !content.elements.iter().any(|e| &e.id == element) {
                content.elements.push(ElementView {
                    id: element.clone(),
                    x: 0,
                    y: 0,
                });
            }
        }
        if add_relationships {
            self.view_add_relationships_for(view, element);
        }
        Ok(())
    }

    /// `ModelView.addRelationships(element)`: every relationship whose ends
    /// are BOTH in the view now gets a relationship view.
    fn view_add_relationships_for(&mut self, view: ViewRef, element: &Id) {
        let in_view = self.view_element_ids(view);
        let rel_ids: Vec<Id> = self
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| {
                (&r.source == element || &r.destination == element)
                    && in_view.contains(&r.source)
                    && in_view.contains(&r.destination)
            })
            .map(|r| r.id.clone())
            .collect();
        let content = expect_content(self.content_mut(view));
        for id in rel_ids {
            upsert_relationship_view(content, id);
        }
    }

    /// `View.addRelationship(relationship)`: add only when both ends are in
    /// the view; returns whether the relationship view exists afterwards.
    pub fn view_add_relationship(
        &mut self,
        view: ViewRef,
        relationship: &Id,
    ) -> Result<bool, CoreError> {
        let (source, destination) = self
            .workspace()
            .model
            .relationships
            .iter()
            .find(|r| &r.id == relationship)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .ok_or_else(|| CoreError::RelationshipNotFound {
                id: relationship.clone(),
            })?;
        if !self.view_contains_element(view, &source)
            || !self.view_contains_element(view, &destination)
        {
            return Ok(false);
        }
        let content = expect_content(self.content_mut(view));
        upsert_relationship_view(content, relationship.clone());
        Ok(true)
    }

    /// `ModelView.removeElement` — plus `DeploymentView.remove(DeploymentNode)`'s
    /// cascade through the node subtree.
    pub fn view_remove_element(&mut self, view: ViewRef, element: &Id) -> Result<(), CoreError> {
        if view_scope_is_context_system(self, view, element) {
            return Err(CoreError::ElementCannotBeRemoved {
                name: self
                    .item(element)
                    .map(|i| i.name.clone())
                    .unwrap_or_default(),
            });
        }
        let subtree: Vec<Id> = if matches!(view, ViewRef::Deployment(_))
            && self.element_kind(element) == Some(ElementKind::DeploymentNode)
        {
            let mut ids = Vec::new();
            collect_deployment_subtree(self, element, &mut ids);
            ids
        } else {
            vec![element.clone()]
        };
        for id in subtree {
            let connected: Vec<Id> = self
                .workspace()
                .model
                .relationships
                .iter()
                .filter(|r| r.source == id || r.destination == id)
                .map(|r| r.id.clone())
                .collect();
            if let Some(content) = self.content_mut(view) {
                content.elements.retain(|e| e.id != id);
                for rel in connected {
                    content.relationships.retain(|r| r.id != rel);
                }
            }
        }
        Ok(())
    }

    /// `ModelView.remove(relationship)`.
    pub fn view_remove_relationship(&mut self, view: ViewRef, relationship: &Id) {
        if let Some(content) = self.content_mut(view) {
            content.relationships.retain(|r| &r.id != relationship);
        }
    }

    /// `ModelView.removeRelationshipsNotConnectedToElements`.
    pub fn view_remove_relationships_not_connected(&mut self, view: ViewRef, keep: &[Id]) {
        let connected: Vec<Id> = self
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| keep.contains(&r.source) || keep.contains(&r.destination))
            .map(|r| r.id.clone())
            .collect();
        if let Some(content) = self.content_mut(view) {
            content.relationships.retain(|r| connected.contains(&r.id));
        }
    }

    // ------------------------------------------------------------------
    // default elements
    // ------------------------------------------------------------------

    /// `addDefaultElements(boolean greedy)` per view type. `*` maps to
    /// greedy; `*?` to non-greedy (which prunes relationships back to the
    /// scoped elements).
    #[allow(clippy::too_many_lines)] // one arm per reference `addDefaultElements`
    pub fn view_add_default_elements(
        &mut self,
        view: ViewRef,
        greedy: bool,
    ) -> Result<(), CoreError> {
        match view {
            ViewRef::Landscape(_) => {
                let systems: Vec<Id> = self
                    .workspace()
                    .model
                    .software_systems
                    .iter()
                    .map(|s| s.item.id.clone())
                    .collect();
                for s in systems {
                    let _ = self.view_add_element(view, &s, true);
                }
                let people: Vec<Id> = self
                    .workspace()
                    .model
                    .people
                    .iter()
                    .map(|p| p.item.id.clone())
                    .collect();
                for p in people {
                    let _ = self.view_add_element(view, &p, true);
                }
                for e in self.view_element_ids(view) {
                    self.add_nearest_neighbours(view, &e, KindFilter::Custom);
                }
            }
            ViewRef::Context(_) => {
                let Some(system) = self.view_scope_system(view) else {
                    return Ok(());
                };
                for filter in [
                    KindFilter::Custom,
                    KindFilter::Person,
                    KindFilter::SoftwareSystem,
                ] {
                    self.add_nearest_neighbours(view, &system, filter);
                }
                if !greedy {
                    self.view_remove_relationships_not_connected(view, &[system]);
                }
            }
            ViewRef::Container(_) => {
                let Some(system) = self.view_scope_system(view) else {
                    return Ok(());
                };
                let containers: Vec<Id> = self
                    .system_ref(&system)
                    .map(|s| s.containers.iter().map(|c| c.item.id.clone()).collect())
                    .unwrap_or_default();
                for c in &containers {
                    let _ = self.view_add_element(view, c, true);
                    for filter in [
                        KindFilter::Custom,
                        KindFilter::Person,
                        KindFilter::SoftwareSystem,
                    ] {
                        self.add_nearest_neighbours(view, c, filter);
                    }
                }
                if !greedy {
                    self.view_remove_relationships_not_connected(view, &containers);
                }
            }
            ViewRef::Component(_) => {
                let Some(container) = self.view_scope_container(view) else {
                    return Ok(());
                };
                let components: Vec<Id> = self
                    .container_ref(&container)
                    .map(|c| c.components.iter().map(|k| k.item.id.clone()).collect())
                    .unwrap_or_default();
                let siblings: Vec<Id> = self
                    .element_parent(&container)
                    .and_then(|s| {
                        self.system_ref(&s)
                            .map(|s| s.containers.iter().map(|c| c.item.id.clone()).collect())
                    })
                    .unwrap_or_default();
                for k in &components {
                    let _ = self.view_add_element(view, k, true);
                    for c in &siblings {
                        if self.has_any_efferent(c, k) || self.has_any_efferent(k, c) {
                            let _ = self.view_add_element(view, c, true);
                        }
                    }
                    for filter in [
                        KindFilter::Custom,
                        KindFilter::Person,
                        KindFilter::SoftwareSystem,
                    ] {
                        self.add_nearest_neighbours(view, k, filter);
                    }
                }
                if !greedy {
                    self.view_remove_relationships_not_connected(view, &components);
                }
            }
            ViewRef::Deployment(_) => {
                self.add_all_deployment_nodes(view)?;
                for e in self.view_element_ids(view) {
                    self.add_nearest_neighbours(view, &e, KindFilter::Custom);
                }
            }
            ViewRef::Custom(_) => {
                let customs: Vec<Id> = self
                    .workspace()
                    .model
                    .custom_elements
                    .iter()
                    .map(|c| c.item.id.clone())
                    .collect();
                for c in customs {
                    let _ = self.view_add_element(view, &c, true);
                }
            }
            ViewRef::Dynamic(_) | ViewRef::Filtered(_) | ViewRef::Image(_) => {}
        }
        Ok(())
    }

    /// `ModelView.addNearestNeighbours`: add `element`, then everything it is
    /// directly coupled to (either direction) whose kind passes the filter;
    /// not-permitted additions are silently ignored (Java prints to stdout).
    fn add_nearest_neighbours(&mut self, view: ViewRef, element: &Id, filter: KindFilter) {
        if self.view_add_element(view, element, true).is_err() {
            return;
        }
        let candidates: Vec<(Id, Id)> = self
            .workspace()
            .model
            .relationships
            .iter()
            .map(|r| (r.source.clone(), r.destination.clone()))
            .collect();
        for (source, destination) in candidates {
            if &source == element && self.filter_matches(filter, &destination) {
                let _ = self.view_add_element(view, &destination, true);
            } else if &destination == element && self.filter_matches(filter, &source) {
                let _ = self.view_add_element(view, &source, true);
            }
        }
    }

    fn filter_matches(&self, filter: KindFilter, id: &Id) -> bool {
        let kind = self.element_kind(id);
        match filter {
            KindFilter::Custom => kind == Some(ElementKind::CustomElement),
            KindFilter::Person => kind == Some(ElementKind::Person),
            KindFilter::SoftwareSystem => kind == Some(ElementKind::SoftwareSystem),
        }
    }

    /// `DeploymentView.addAllDeploymentNodes` + the node-cascading
    /// `add(DeploymentNode)`.
    fn add_all_deployment_nodes(&mut self, view: ViewRef) -> Result<(), CoreError> {
        let env = self.deployment_environment(view);
        let tops: Vec<Id> = self
            .workspace()
            .model
            .deployment_nodes
            .iter()
            .filter(|n| env.as_deref().is_none_or(|e| e == n.environment))
            .map(|n| n.item.id.clone())
            .collect();
        for t in tops {
            self.deployment_view_add_node_top(view, &t)?;
        }
        Ok(())
    }

    /// `DeploymentView.addElementInstancesAndDeploymentNodesAndInfrastructureNodes`:
    /// software system instances, then (scope-filtered) container instances,
    /// then infrastructure nodes, then children recursively; the node itself
    /// joins only when the subtree holds anything, and the public
    /// `add(DeploymentNode)` then walks up the ancestor chain.
    fn deployment_view_add_node(&mut self, view: ViewRef, node: &Id) -> Result<bool, CoreError> {
        let (ss_instances, container_instances, infra, children) = {
            let Some(n) = self.node_ref(node) else {
                return Ok(false);
            };
            (
                n.software_system_instances
                    .iter()
                    .map(|i| i.item.id.clone())
                    .collect::<Vec<_>>(),
                n.container_instances
                    .iter()
                    .map(|i| i.item.id.clone())
                    .collect::<Vec<_>>(),
                n.infrastructure_nodes
                    .iter()
                    .map(|i| i.item.id.clone())
                    .collect::<Vec<_>>(),
                n.children
                    .iter()
                    .map(|c| c.item.id.clone())
                    .collect::<Vec<_>>(),
            )
        };
        let scope = self.view_scope_system(view);
        let mut has = false;
        for i in ss_instances {
            // Java: `catch (ElementNotPermittedInViewException)` — skip.
            if self.view_add_element(view, &i, true).is_ok() {
                has = true;
            }
        }
        for i in container_instances {
            if scope
                .as_ref()
                .is_none_or(|s| self.instance_container_system(&i).as_ref() == Some(s))
                && self.view_add_element(view, &i, true).is_ok()
            {
                has = true;
            }
        }
        for i in infra {
            // Reference: infrastructure nodes are added UNGUARDED — a
            // rejected add propagates out of the whole cascade (the DSL
            // content parsers catch it there).
            self.view_add_element(view, &i, true)?;
            has = true;
        }
        for c in &children {
            has |= self.deployment_view_add_node(view, c)?;
        }
        if has {
            self.view_add_element(view, node, true)?;
        }
        Ok(has)
    }

    /// The public `DeploymentView.add(DeploymentNode)`: the cascade, then the
    /// ancestor chain.
    fn deployment_view_add_node_top(&mut self, view: ViewRef, node: &Id) -> Result<(), CoreError> {
        if !self.deployment_view_add_node(view, node)? {
            return Ok(());
        }
        let mut parent = self.element_parent(node);
        while let Some(p) = parent {
            let _ = self.view_add_element(view, &p, true);
            parent = self.element_parent(&p);
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // default views (`ViewSet.createDefaultViews`)
    // ------------------------------------------------------------------

    /// `ViewSet.createDefaultViews` (`ViewSet` 1191-1264, pinned `ad34036`):
    /// the app-layer generation `AbstractCommand.loadWorkspace` runs when the
    /// model is non-empty but the view set is empty — the pipeline applies it
    /// where the reference's shared load tail sits (M1-e-B). Mirrors the
    /// reference exactly: one landscape view; per software system (stable
    /// sort by Java UTF-16 name order) a context view, plus a container view
    /// with `enableAutomaticLayout(TopBottom)` when the system has
    /// containers, plus a component view per name-sorted container that has
    /// components; then per deployment environment (deduped first-seen from
    /// the top-level nodes, then sorted) one scoped deployment view per
    /// software system holding container instances in the environment
    /// (name-sorted), or one unscoped view when the environment holds only
    /// infrastructure nodes. The `Err` arms are the reference's unguarded
    /// throws (duplicate keys / not-permitted adds) — unreachable on the
    /// empty view set this factory is called with, but propagated so a
    /// surprise fails the load like the reference's abort.
    #[allow(clippy::too_many_lines)] // one leg per reference `createDefaultViews`
    pub fn create_default_views(&mut self) -> Result<(), CoreError> {
        // a single System Landscape view with all people and software systems
        let landscape = self.create_landscape_view("")?;
        self.view_add_default_elements(landscape, true)?;

        // per software system (Java name order): a context view, plus a
        // container view and component views
        let mut systems: Vec<SystemPlan> = self
            .workspace()
            .model
            .software_systems
            .iter()
            .map(|s| {
                (
                    s.item.id.clone(),
                    s.item.name.clone(),
                    s.containers
                        .iter()
                        .map(|c| {
                            (
                                c.item.id.clone(),
                                c.item.name.clone(),
                                !c.components.is_empty(),
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        systems.sort_by(|a, b| java_cmp_str(&a.1, &b.1));
        for (system_id, _, mut containers) in systems {
            let context = self.create_context_view(&system_id, "")?;
            self.view_add_default_elements(context, true)?;
            if containers.is_empty() {
                continue;
            }
            containers.sort_by(|a, b| java_cmp_str(&a.1, &b.1));
            let container_view = self.create_container_view(&system_id, "")?;
            self.view_add_default_elements(container_view, true)?;
            // `enableAutomaticLayout(TopBottom)`: the `AutomaticLayout.
            // DEFAULT_*` separation/vertices defaults (100/50/50/true).
            if let Some(content) = self.content_mut(container_view) {
                content.automatic_layout = Some(AutomaticLayout {
                    rank_direction: Some(RankDirection::TopBottom),
                    rank_separation: 100,
                    node_separation: 50,
                    edge_separation: 50,
                    vertices: true,
                });
            }
            for (container_id, _, has_components) in containers {
                if has_components {
                    let component_view = self.create_component_view(&container_id, "")?;
                    self.view_add_default_elements(component_view, true)?;
                }
            }
        }

        // deployment views for each environment and software system pair:
        // plan from the (still unmutated) model, then create
        let mut environments: Vec<String> = Vec::new();
        for node in &self.workspace().model.deployment_nodes {
            if !environments.contains(&node.environment) {
                environments.push(node.environment.clone());
            }
        }
        environments.sort_by(|a, b| java_cmp_str(a, b));
        let owner: HashMap<Id, SystemRef> = self
            .workspace()
            .model
            .software_systems
            .iter()
            .flat_map(|s| {
                s.containers
                    .iter()
                    .map(|c| (c.item.id.clone(), (s.item.id.clone(), s.item.name.clone())))
            })
            .collect();
        let plans: Vec<EnvironmentPlan> = environments
            .into_iter()
            .map(|environment| {
                let mut systems_in_env: Vec<SystemRef> = Vec::new();
                for node in self
                    .workspace()
                    .model
                    .deployment_nodes
                    .iter()
                    .filter(|n| n.environment == environment)
                {
                    collect_instance_systems(node, &owner, &mut systems_in_env);
                }
                let infrastructure = self
                    .workspace()
                    .model
                    .deployment_nodes
                    .iter()
                    .any(|n| has_infrastructure_in_subtree(n, &environment));
                (environment, systems_in_env, infrastructure)
            })
            .collect();
        for (environment, mut systems_in_env, infrastructure) in plans {
            if systems_in_env.is_empty() {
                // no container instances: an unscoped view only when the
                // environment holds infrastructure nodes
                if infrastructure {
                    let view = self.create_deployment_view(&environment, "")?;
                    self.view_add_default_elements(view, true)?;
                }
            } else {
                systems_in_env.sort_by(|a, b| java_cmp_str(&a.1, &b.1));
                for (system_id, _) in systems_in_env {
                    let view = self.create_deployment_view(&environment, "")?;
                    if let ViewRef::Deployment(index) = view {
                        // `createDeploymentView(softwareSystem, ...)`: the
                        // scope must be set BEFORE `addDefaultElements` (the
                        // instance-cascading add filters on it).
                        self.workspace_mut().views.deployment_views[index].software_system_id =
                            Some(system_id);
                    }
                    self.view_add_default_elements(view, true)?;
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // animation steps
    // ------------------------------------------------------------------

    /// `StaticView/CustomView/DeploymentView.addAnimation`: elements must be
    /// in the view and not already revealed by an earlier step; a
    /// relationship joins the step when exactly one of its ends is revealed
    /// now (the other having been revealed earlier).
    pub fn view_add_animation_step(
        &mut self,
        view: ViewRef,
        elements: &[Id],
    ) -> Result<(), CoreError> {
        let mut previous: Vec<String> = Vec::new();
        if let Some(animations) = self.animations_mut(view) {
            for step in animations.iter() {
                previous.extend(step.element_ids.iter().cloned());
            }
        } else {
            return Ok(());
        }
        let mut this_step: Vec<Id> = Vec::new();
        let deployment = matches!(view, ViewRef::Deployment(_));
        for id in elements {
            if self.view_contains_element(view, id) {
                let s = id.as_str().to_string();
                if !previous.contains(&s) {
                    previous.push(s);
                    this_step.push(id.clone());
                    if deployment {
                        // `DeploymentView.addAnimationStep` reveals the
                        // element's deployment-node ancestors too.
                        let mut parent = self.element_parent(id);
                        while let Some(p) = parent {
                            let ps = p.as_str().to_string();
                            if !previous.contains(&ps) {
                                previous.push(ps);
                                this_step.push(p.clone());
                            }
                            parent = self.element_parent(&p);
                        }
                    }
                }
            }
        }
        if this_step.is_empty() {
            return Err(CoreError::NoAnimatableElements);
        }
        let rels = self.animation_step_relationships(view, &this_step, &previous);
        let order = i32::try_from(
            self.animations_mut(view)
                .map_or(0, |a| a.len())
                .saturating_add(1),
        )
        .unwrap_or(i32::MAX);
        if let Some(animations) = self.animations_mut(view) {
            animations.push(Animation {
                order,
                element_ids: this_step.iter().map(|i| i.as_str().to_string()).collect(),
                relationship_ids: rels,
            });
        }
        Ok(())
    }

    fn animation_step_relationships(
        &self,
        view: ViewRef,
        this_step: &[Id],
        previous: &[String],
    ) -> Vec<String> {
        self.content_ref(view)
            .map(|content| {
                content
                    .relationships
                    .iter()
                    .filter(|rv| {
                        let model = self
                            .workspace()
                            .model
                            .relationships
                            .iter()
                            .find(|r| r.id == rv.id);
                        let Some(model) = model else {
                            return false;
                        };
                        let src_now = this_step.contains(&model.source);
                        let dst_now = this_step.contains(&model.destination);
                        let src_prev = previous.contains(&model.source.as_str().to_string());
                        let dst_prev = previous.contains(&model.destination.as_str().to_string());
                        (src_now && dst_prev) || (src_prev && dst_now)
                    })
                    .map(|rv| rv.id.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // dynamic view steps
    // ------------------------------------------------------------------

    /// `DynamicView.add(relationship, description)` — the
    /// `<relationshipIdentifier> [description]` step form. The step number
    /// (`order`) comes from the DSL engine's `SequenceNumber` mirror.
    ///
    /// Every successful add INSERTS: the reference `TreeSet` is keyed by
    /// `getId() + "/" + getOrder()` (`RelationshipView.compareTo`), so a
    /// re-add with a fresh sequence order is a NEW entry — duplicate
    /// relationship ids inside one dynamic view are the whole point of the
    /// step form. The `description` is stored verbatim (the DSL parser
    /// passes `""` when absent; the JSON writer omits empties).
    pub fn dynamic_add_relationship(
        &mut self,
        view: ViewRef,
        relationship: &Id,
        description: &str,
        order: &str,
    ) -> Result<Id, CoreError> {
        let (source, destination) = self
            .workspace()
            .model
            .relationships
            .iter()
            .find(|r| &r.id == relationship)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .ok_or_else(|| CoreError::RelationshipNotFound {
                id: relationship.clone(),
            })?;
        self.check_element_can_be_added(view, &source)?;
        self.check_element_can_be_added(view, &destination)?;
        self.view_add_element(view, &source, false)?;
        self.view_add_element(view, &destination, false)?;
        let content = expect_content(self.content_mut(view));
        content.relationships.push(RelationshipView {
            id: relationship.clone(),
            description: Some(description.to_string()),
            url: None,
            order: Some(order.to_string()),
            response: Some(false),
            properties: crate::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
        Ok(relationship.clone())
    }

    /// `DynamicView.addRelationshipViaElements` — the
    /// `a -> b [description] [technology]` step form. The model relationship
    /// must exist: with no technology the lookup prefers the relationship
    /// whose (normalized) description equals the given one and falls back to
    /// any source→destination relationship; with a technology it's an exact
    /// technology match. A reverse relationship ALREADY IN THE VIEW is added
    /// as `response: true`; otherwise [`CoreError::DynamicRelationshipMissing`].
    /// Like [`Self::dynamic_add_relationship`] every successful add INSERTS
    /// (the reference `TreeSet` is keyed by id AND order), and the resolved
    /// relationship id is returned.
    pub fn dynamic_add_element_pair(
        &mut self,
        view: ViewRef,
        source: &Id,
        description: &str,
        technology: &str,
        destination: &Id,
        order: &str,
    ) -> Result<Id, CoreError> {
        self.check_element_can_be_added(view, source)?;
        self.check_element_can_be_added(view, destination)?;
        let rels: Vec<RelInfo> = self
            .workspace()
            .model
            .relationships
            .iter()
            .map(|r| RelInfo {
                id: r.id.clone(),
                source: r.source.clone(),
                destination: r.destination.clone(),
                description: r.description.clone(),
                technology: r.technology.clone(),
            })
            .collect();
        // Forward lookup (reference order): no technology →
        // `getEfferentRelationshipWith(dest, description)` — an EXACT
        // normalized-description match ("" matches ""!) — falling back to any
        // efferent source→destination relationship; with a technology the
        // reference loops WITHOUT break, so the LAST tech match wins.
        let forward = if technology.is_empty() {
            find_rel(&rels, source, destination, Some(description), "")
                .or_else(|| find_rel(&rels, source, destination, None, ""))
        } else {
            find_rel_last(&rels, source, destination, technology)
        };
        let (rel_id, response) = if let Some(id) = forward {
            (id, false)
        } else {
            // Reverse lookup; usable only when that relationship is
            // ALREADY in this view (`response: true`).
            let reverse = if technology.is_empty() {
                find_rel(&rels, destination, source, None, "")
            } else {
                find_rel_last(&rels, destination, source, technology)
            };
            match reverse {
                Some(id) if self.view_contains_relationship(view, &id) => (id, true),
                _ => {
                    return Err(CoreError::DynamicRelationshipMissing {
                        source: self
                            .item(source)
                            .map(|i| i.name.clone())
                            .unwrap_or_default(),
                        destination: self
                            .item(destination)
                            .map(|i| i.name.clone())
                            .unwrap_or_default(),
                        technology: (!technology.is_empty()).then(|| technology.to_string()),
                    });
                }
            }
        };
        self.view_add_element(view, source, false)?;
        self.view_add_element(view, destination, false)?;
        let content = expect_content(self.content_mut(view));
        content.relationships.push(RelationshipView {
            id: rel_id.clone(),
            description: Some(description.to_string()),
            url: None,
            order: Some(order.to_string()),
            response: Some(response),
            properties: crate::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
        Ok(rel_id)
    }

    /// The key of a view slot (for diagnostics and `filtered` bases).
    #[must_use]
    pub fn view_key(&self, view: ViewRef) -> String {
        let views = &self.workspace().views;
        let base = |i: usize| -> Option<String> {
            let key: Option<&String> = match view {
                ViewRef::Landscape(_) => views.system_landscape_views.get(i).map(|v| &v.base.key),
                ViewRef::Context(_) => views.system_context_views.get(i).map(|v| &v.base.key),
                ViewRef::Container(_) => views.container_views.get(i).map(|v| &v.base.key),
                ViewRef::Component(_) => views.component_views.get(i).map(|v| &v.base.key),
                ViewRef::Dynamic(_) => views.dynamic_views.get(i).map(|v| &v.base.key),
                ViewRef::Deployment(_) => views.deployment_views.get(i).map(|v| &v.base.key),
                ViewRef::Filtered(_) => views.filtered_views.get(i).map(|v| &v.base.key),
                ViewRef::Image(_) => views.image_views.get(i).map(|v| &v.base.key),
                ViewRef::Custom(_) => views.custom_views.get(i).map(|v| &v.base.key),
            };
            key.cloned()
        };
        match view {
            ViewRef::Landscape(i)
            | ViewRef::Context(i)
            | ViewRef::Container(i)
            | ViewRef::Component(i)
            | ViewRef::Dynamic(i)
            | ViewRef::Deployment(i)
            | ViewRef::Filtered(i)
            | ViewRef::Image(i)
            | ViewRef::Custom(i) => base(i).unwrap_or_default(),
        }
    }

    /// `ImageView.hasContent()` — any of the three content fields is
    /// non-blank (`StringUtils.isNullOrEmpty` TRIMS, so `" "` is empty).
    #[must_use]
    pub fn image_has_content(&self, view: ViewRef) -> bool {
        let ViewRef::Image(i) = view else {
            return true;
        };
        let Some(v) = self.workspace().views.image_views.get(i) else {
            return true;
        };
        let filled = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.trim().is_empty());
        filled(&v.content) || filled(&v.content_light) || filled(&v.content_dark)
    }

    /// The mutable [`ViewBase`](crate::views::ViewBase) of a view (title /
    /// description / auto-layout / properties statements).
    pub fn view_base_mut(&mut self, view: ViewRef) -> Option<&mut crate::views::ViewBase> {
        let views = &mut self.workspace_mut().views;
        match view {
            ViewRef::Landscape(i) => views.system_landscape_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Context(i) => views.system_context_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Container(i) => views.container_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Component(i) => views.component_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Dynamic(i) => views.dynamic_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Deployment(i) => views.deployment_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Filtered(i) => views.filtered_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Image(i) => views.image_views.get_mut(i).map(|v| &mut v.base),
            ViewRef::Custom(i) => views.custom_views.get_mut(i).map(|v| &mut v.base),
        }
    }

    /// The borrowed [`ViewRef`] of the view with `key`, if any (for
    /// `filtered <baseKey>` handling).
    #[must_use]
    pub fn view_ref_of_key(&self, key: &str) -> Option<ViewRef> {
        let views = &self.workspace().views;
        if let Some(i) = views
            .system_landscape_views
            .iter()
            .position(|v| v.base.key == key)
        {
            return Some(ViewRef::Landscape(i));
        }
        if let Some(i) = views
            .system_context_views
            .iter()
            .position(|v| v.base.key == key)
        {
            return Some(ViewRef::Context(i));
        }
        if let Some(i) = views.container_views.iter().position(|v| v.base.key == key) {
            return Some(ViewRef::Container(i));
        }
        if let Some(i) = views.component_views.iter().position(|v| v.base.key == key) {
            return Some(ViewRef::Component(i));
        }
        if let Some(i) = views.dynamic_views.iter().position(|v| v.base.key == key) {
            return Some(ViewRef::Dynamic(i));
        }
        if let Some(i) = views
            .deployment_views
            .iter()
            .position(|v| v.base.key == key)
        {
            return Some(ViewRef::Deployment(i));
        }
        if let Some(i) = views.filtered_views.iter().position(|v| v.base.key == key) {
            return Some(ViewRef::Filtered(i));
        }
        if let Some(i) = views.image_views.iter().position(|v| v.base.key == key) {
            return Some(ViewRef::Image(i));
        }
        views
            .custom_views
            .iter()
            .position(|v| v.base.key == key)
            .map(ViewRef::Custom)
    }
}

/// Minimal relationship projection for the dynamic lookups (avoids holding
/// a borrow of the workspace across mutations).
struct RelInfo {
    id: Id,
    source: Id,
    destination: Id,
    description: Option<String>,
    technology: Option<String>,
}

/// `(id, name)` of a software system, as planned by `create_default_views`.
type SystemRef = (Id, String);

/// `(id, name, has_components)` of a container, as planned by
/// `create_default_views` (the component-view leg skips the false entries).
type ContainerPlan = (Id, String, bool);

/// `(id, name, containers)` per software system — `create_default_views`'
/// pre-mutation plan (the model is fully planned before any view is added).
type SystemPlan = (Id, String, Vec<ContainerPlan>);

/// `(environment, software systems with container instances, infrastructure
/// present)` per deployment environment — `create_default_views`'s plan for
/// the deployment leg (scoped views when systems is non-empty, else one
/// unscoped view iff infrastructure).
type EnvironmentPlan = (String, Vec<SystemRef>, bool);

/// One directional relationship lookup: exact normalized description when
/// `Some`, any description when `None`; exact technology when non-empty.
fn find_rel(
    rels: &[RelInfo],
    from: &Id,
    to: &Id,
    description: Option<&str>,
    technology: &str,
) -> Option<Id> {
    rels.iter()
        .find(|r| {
            &r.source == from
                && &r.destination == to
                && description.is_none_or(|d| r.description.as_deref().unwrap_or("") == d)
                && (technology.is_empty() || r.technology.as_deref().unwrap_or("") == technology)
        })
        .map(|r| r.id.clone())
}

/// Technology-filtered lookup with the reference's no-break loop: the LAST
/// source→destination relationship with exactly this technology wins.
fn find_rel_last(rels: &[RelInfo], from: &Id, to: &Id, technology: &str) -> Option<Id> {
    rels.iter()
        .rev()
        .find(|r| {
            &r.source == from
                && &r.destination == to
                && r.technology.as_deref().unwrap_or("") == technology
        })
        .map(|r| r.id.clone())
}

/// `View.addRelationship` insertion without the in-view check (callers
/// checked); dedupes by relationship id like the reference `TreeSet`.
fn upsert_relationship_view(content: &mut crate::views::ViewContent, relationship: Id) {
    if !content.relationships.iter().any(|r| r.id == relationship) {
        content.relationships.push(RelationshipView {
            id: relationship,
            description: None,
            url: None,
            order: None,
            response: None,
            properties: crate::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
    }
}

fn expect_content(
    content: Option<&mut crate::views::ViewContent>,
) -> &mut crate::views::ViewContent {
    content.expect("model views always have content")
}

/// `SystemContextView.canBeRemoved`: the scoped system is the only element
/// the reference refuses to remove.
fn view_scope_is_context_system(builder: &WorkspaceBuilder, view: ViewRef, element: &Id) -> bool {
    matches!(view, ViewRef::Context(i)
        if builder
            .workspace()
            .views
            .system_context_views
            .get(i)
            .is_some_and(|v| v.software_system_id == *element))
}

/// `DeploymentView.remove(DeploymentNode)` cascade order: instances,
/// infrastructure nodes, children (recursive), then the node itself.
fn collect_deployment_subtree(builder: &WorkspaceBuilder, node: &Id, out: &mut Vec<Id>) {
    let Some(n) = builder.node_ref(node) else {
        return;
    };
    for s in &n.software_system_instances {
        out.push(s.item.id.clone());
    }
    for c in &n.container_instances {
        out.push(c.item.id.clone());
    }
    for i in &n.infrastructure_nodes {
        out.push(i.item.id.clone());
    }
    let children: Vec<Id> = n.children.iter().map(|c| c.item.id.clone()).collect();
    for c in children {
        collect_deployment_subtree(builder, &c, out);
    }
    out.push(node.clone());
}

/// Java `String.compareTo`: UTF-16 code-unit order (`Comparator.comparing
/// (Element::getName)` / `String::compareTo` in `createDefaultViews`) —
/// differs from Rust's UTF-8 byte order where supplementary characters
/// compare against `U+E000..U+FFFF` (same precedent as `views::style_cmp`).
fn java_cmp_str(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `ViewSet.createDefaultViews`' `getContainerInstances` recursion, mapping
/// each container instance to its owning software system and keeping the
/// first-seen order (the reference dedupes the system list as it walks; the
/// result is name-sorted afterwards anyway, but stable-sort ties follow the
/// reference's traversal).
fn collect_instance_systems(
    node: &crate::model::DeploymentNode,
    owner: &HashMap<Id, SystemRef>,
    out: &mut Vec<SystemRef>,
) {
    for instance in &node.container_instances {
        if let Some(system) = owner.get(&instance.container_id)
            && !out.iter().any(|(id, _)| id == &system.0)
        {
            out.push(system.clone());
        }
    }
    for child in &node.children {
        collect_instance_systems(child, owner, out);
    }
}

/// The `createDefaultViews` infrastructure probe (`model.getElements().
/// anyMatch(instanceof InfrastructureNode && environment equals)`) over one
/// top-level node's subtree; infrastructure nodes inherit the environment.
fn has_infrastructure_in_subtree(node: &crate::model::DeploymentNode, environment: &str) -> bool {
    node.infrastructure_nodes
        .iter()
        .any(|i| i.environment == environment)
        || node
            .children
            .iter()
            .any(|c| has_infrastructure_in_subtree(c, environment))
}

#[cfg(test)]
mod tests {
    // dense element fixtures mirror the reference's one-letter DSL test names
    #![allow(clippy::many_single_char_names)]

    use super::ViewRef;
    use crate::builder::{ImpliedRelationshipsStrategy, WorkspaceBuilder};

    fn rel<'a>(
        b: &'a WorkspaceBuilder,
        src: &crate::Id,
        dst: &crate::Id,
        desc: &str,
    ) -> Option<&'a crate::model::Relationship> {
        b.workspace().model.relationships.iter().find(|r| {
            &r.source == src
                && &r.destination == dst
                && r.description.as_deref().unwrap_or("") == desc
        })
    }

    #[test]
    fn duplicate_relationship_matches_description_only() {
        let mut b = WorkspaceBuilder::new();
        let a = b.add_software_system("A", None, None).unwrap();
        let z = b.add_software_system("B", None, None).unwrap();
        b.add_relationship(&a, &z, Some("Uses".into()), Some("HTTP".into()), None)
            .unwrap();
        // SAME description, different technology -> duplicate (Element.has).
        assert_eq!(
            b.add_relationship(&a, &z, Some("Uses".into()), Some("FTP".into()), None),
            Err(crate::CoreError::DuplicateRelationship {
                source: "A".into(),
                destination: "B".into()
            })
        );
        // different description -> fine
        b.add_relationship(&a, &z, Some("Sees".into()), None, None)
            .unwrap();
    }

    #[test]
    fn parent_child_relationships_rejected() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let c = b.add_container(&s, "C", None, None, None).unwrap();
        assert_eq!(
            b.add_relationship(&s, &c, None, None, None),
            Err(crate::CoreError::ParentChildRelationship)
        );
        assert_eq!(
            b.add_relationship(&c, &s, None, None, None),
            Err(crate::CoreError::ParentChildRelationship)
        );
        // persons have no lineage
        let p = b.add_person("P", None, None).unwrap();
        let s2 = b.add_software_system("S2", None, None).unwrap();
        b.add_relationship(&p, &s2, None, None, None).unwrap();
    }

    #[test]
    fn unless_any_creates_implied_ancestor_relationships() {
        let mut b = WorkspaceBuilder::new();
        b.set_implied_strategy(ImpliedRelationshipsStrategy::UnlessAnyRelationshipExists);
        let s1 = b.add_software_system("S1", None, None).unwrap();
        let c1 = b.add_container(&s1, "C1", None, None, None).unwrap();
        let s2 = b.add_software_system("S2", None, None).unwrap();
        let c2 = b.add_container(&s2, "C2", None, None, None).unwrap();
        let base = b
            .add_relationship(&c1, &c2, Some("Uses".into()), Some("HTTP".into()), None)
            .unwrap();
        // Reference loop order: (c1,s2) created before (s1,c2), then (s1,s2).
        let implied = b
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| r.linked_relationship_id.as_ref() == Some(&base))
            .collect::<Vec<_>>();
        assert_eq!(implied.len(), 3);
        assert_eq!(
            (implied[0].source.as_str(), implied[0].destination.as_str()),
            (c1.as_str(), s2.as_str())
        );
        assert_eq!(
            (implied[1].source.as_str(), implied[1].destination.as_str()),
            (s1.as_str(), c2.as_str())
        );
        assert_eq!(
            (implied[2].source.as_str(), implied[2].destination.as_str()),
            (s1.as_str(), s2.as_str())
        );
        for r in &implied {
            assert_eq!(r.description.as_deref(), Some("Uses"));
            assert_eq!(r.technology.as_deref(), Some("HTTP"));
            assert!(
                r.tags.stored().is_empty(),
                "implied relationships carry no tags"
            );
        }
    }

    #[test]
    fn unless_any_skips_existing_any_description_unless_same_does_not() {
        let mk = |strategy: ImpliedRelationshipsStrategy| {
            let mut b = WorkspaceBuilder::new();
            b.set_implied_strategy(strategy);
            let s1 = b.add_software_system("S1", None, None).unwrap();
            let c1 = b.add_container(&s1, "C1", None, None, None).unwrap();
            let s2 = b.add_software_system("S2", None, None).unwrap();
            let c2 = b.add_container(&s2, "C2", None, None, None).unwrap();
            b.add_relationship(&s1, &s2, Some("Sends".into()), None, None)
                .unwrap();
            b.add_relationship(&c1, &c2, Some("Uses".into()), None, None)
                .unwrap();
            b
        };
        let any = mk(ImpliedRelationshipsStrategy::UnlessAnyRelationshipExists);
        assert!(
            rel(&any, &s1_of(&any), &s2_of(&any), "Uses").is_none(),
            "UnlessAny skips (S1,S2) because ANY efferent exists"
        );
        let same = mk(ImpliedRelationshipsStrategy::UnlessSameRelationshipExists);
        assert!(
            rel(&same, &s1_of(&same), &s2_of(&same), "Uses").is_some(),
            "UnlessSame only skips same-description matches"
        );
    }

    fn s1_of(b: &WorkspaceBuilder) -> crate::Id {
        b.workspace().model.software_systems[0].item.id.clone()
    }
    fn s2_of(b: &WorkspaceBuilder) -> crate::Id {
        b.workspace().model.software_systems[1].item.id.clone()
    }

    #[test]
    fn default_strategy_creates_nothing() {
        let mut b = WorkspaceBuilder::new();
        let s1 = b.add_software_system("S1", None, None).unwrap();
        let c1 = b.add_container(&s1, "C1", None, None, None).unwrap();
        let s2 = b.add_software_system("S2", None, None).unwrap();
        let c2 = b.add_container(&s2, "C2", None, None, None).unwrap();
        b.add_relationship(&c1, &c2, Some("Uses".into()), None, None)
            .unwrap();
        assert_eq!(b.workspace().model.relationships.len(), 1);
    }

    #[test]
    fn instance_replication_follows_environment_and_groups() {
        let mut b = WorkspaceBuilder::new();
        let s1 = b.add_software_system("A", None, None).unwrap();
        let s2 = b.add_software_system("B", None, None).unwrap();
        b.add_relationship(&s1, &s2, Some("Uses".into()), Some("HTTP".into()), None)
            .unwrap();
        let n1 = b
            .add_deployment_node("Live", "N1", None, None, None)
            .unwrap();
        let n2 = b
            .add_deployment_node("Live", "N2", None, None, None)
            .unwrap();
        let i1 = b
            .add_software_system_instance(&n1, &s1, None, Vec::new())
            .unwrap();
        let i2 = b
            .add_software_system_instance(&n2, &s2, None, Vec::new())
            .unwrap();
        let replicated = b
            .workspace()
            .model
            .relationships
            .iter()
            .find(|r| r.source == i1 && r.destination == i2)
            .expect("same-environment instances replicate the static relationship");
        assert_eq!(replicated.description.as_deref(), Some("Uses"));
        assert_eq!(replicated.tags.stored().len(), 0);
        assert!(replicated.linked_relationship_id.is_some());
        // different environment -> no replication
        let n3 = b
            .add_deployment_node("Test", "N3", None, None, None)
            .unwrap();
        let i3 = b
            .add_software_system_instance(&n3, &s2, None, Vec::new())
            .unwrap();
        assert!(
            b.workspace()
                .model
                .relationships
                .iter()
                .all(|r| r.destination != i3),
            "instances in another environment get no replicated relationships"
        );
        // disjoint deployment groups -> no replication
        let n4 = b
            .add_deployment_node("Live", "N4", None, None, None)
            .unwrap();
        let i4 = b
            .add_software_system_instance(&n4, &s2, None, vec!["canary".into()])
            .unwrap();
        // i1 (no groups) vs i4 (canary): not the same group, so nothing may
        // point at i4 (its only possible incoming rels would be replicated).
        let to_i4 = b
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| r.destination == i4)
            .count();
        assert_eq!(to_i4, 0, "group-disjoint instances do not replicate");
    }

    #[test]
    fn generated_view_keys_and_order() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let l1 = b.create_landscape_view("").unwrap();
        let c1 = b.create_context_view(&s, "").unwrap();
        let l2 = b.create_landscape_view("").unwrap();
        assert_eq!(b.view_key(l1), "SystemLandscape-001");
        assert_eq!(b.view_key(c1), "SystemContext-001");
        assert_eq!(b.view_key(l2), "SystemLandscape-002");
        let mut orders: Vec<i32> = b
            .workspace()
            .views
            .all_views()
            .iter()
            .map(|v| v.order())
            .collect();
        orders.sort_unstable();
        assert_eq!(
            orders,
            vec![1, 2, 3],
            "order = max(existing) + 1 across ALL views"
        );
        {
            let views = &b.workspace().views;
            assert_eq!(views.system_landscape_views[0].base.order, 1);
            assert_eq!(views.system_context_views[0].base.order, 2);
            assert_eq!(views.system_landscape_views[1].base.order, 3);
        }
        assert!(
            b.workspace()
                .views
                .all_views()
                .iter()
                .any(|v| v.key() == "SystemLandscape-001" && v.base().generated_key),
            "generated keys carry the generatedKey flag"
        );
        // explicit keys are unique across ALL view kinds
        assert!(b.create_landscape_view("SystemContext-001").is_err());
        assert!(b.create_context_view(&s, "SystemContext-001").is_err());
    }

    #[test]
    fn context_view_seeds_scoped_system() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let p = b.add_person("P", None, None).unwrap();
        b.add_relationship(&p, &s, Some("Uses".into()), None, None)
            .unwrap();
        let v = b.create_context_view(&s, "").unwrap();
        assert_eq!(b.view_element_ids(v), vec![s.clone()]);
        // adding the person afterwards pulls the relationship into the view
        b.view_add_element(v, &p, true).unwrap();
        assert!(b.view_contains_element(v, &p));
        assert!(b.view_contains_relationship(v, &b.workspace().model.relationships[0].id));
        // the scoped system cannot be removed
        assert_eq!(
            b.view_remove_element(v, &s),
            Err(crate::CoreError::ElementCannotBeRemoved { name: "S".into() })
        );
    }

    #[test]
    fn element_not_permitted_messages_are_verbatim() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let c = b.add_container(&s, "C", None, None, None).unwrap();
        let p = b.add_person("P", None, None).unwrap();

        let land = b.create_landscape_view("land").unwrap();
        assert_eq!(
            b.view_add_element(land, &c, true),
            Err(crate::CoreError::ElementNotPermittedInView(
                "Only people and software systems can be added to a system landscape view.".into()
            ))
        );
        let _ = p;

        let cont = b.create_container_view(&s, "cont").unwrap();
        assert_eq!(
            b.view_add_element(cont, &s, true),
            Err(crate::CoreError::ElementNotPermittedInView(
                "The software system in scope cannot be added to a container view.".into()
            ))
        );
    }

    #[test]
    fn parent_child_view_gates() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let cont = b.create_container_view(&s, "cont").unwrap();
        // the scoped system itself is rejected with the in-scope message...
        assert_eq!(
            b.view_add_element(cont, &s, true),
            Err(crate::CoreError::ElementNotPermittedInView(
                "The software system in scope cannot be added to a container view.".into()
            ))
        );
        // ...but a DIFFERENT system whose container is already in the view
        // trips the child check
        let s2 = b.add_software_system("S2", None, None).unwrap();
        let c2 = b.add_container(&s2, "C2", None, None, None).unwrap();
        b.view_add_element(cont, &c2, true).unwrap();
        assert_eq!(
            b.view_add_element(cont, &s2, true),
            Err(crate::CoreError::ElementNotPermittedInView(
                "A child of S2 is already in this view.".into()
            ))
        );
        // fresh view, parent first -> the parent check trips
        let cont2 = b.create_container_view(&s, "cont2").unwrap();
        b.view_add_element(cont2, &s2, true).unwrap();
        assert_eq!(
            b.view_add_element(cont2, &c2, true),
            Err(crate::CoreError::ElementNotPermittedInView(
                "A parent of C2 is already in this view.".into()
            ))
        );
    }

    #[test]
    fn animation_steps_reveal_relationships_laterally() {
        let mut b = WorkspaceBuilder::new();
        let a = b.add_software_system("A", None, None).unwrap();
        let z = b.add_software_system("B", None, None).unwrap();
        b.add_relationship(&a, &z, Some("Uses".into()), None, None)
            .unwrap();
        let v = b.create_landscape_view("land").unwrap();
        b.view_add_element(v, &a, true).unwrap();
        b.view_add_element(v, &z, true).unwrap();

        b.view_add_animation_step(v, std::slice::from_ref(&a))
            .unwrap();
        {
            let anim = &b.workspace().views.system_landscape_views[0].animations;
            assert_eq!(anim.len(), 1);
            assert_eq!(anim[0].order, 1);
            assert!(anim[0].relationship_ids.is_empty(), "B not revealed yet");
        }
        b.view_add_animation_step(v, std::slice::from_ref(&z))
            .unwrap();
        {
            let anim = &b.workspace().views.system_landscape_views[0].animations;
            assert_eq!(anim[1].order, 2);
            assert_eq!(anim[1].relationship_ids.len(), 1, "A->B joins step 2");
        }
        assert_eq!(
            b.view_add_animation_step(v, &[a, z]),
            Err(crate::CoreError::NoAnimatableElements)
        );
    }

    #[test]
    fn dynamic_steps_match_descriptions_and_answer_back() {
        let mut b = WorkspaceBuilder::new();
        let p = b.add_person("P", None, None).unwrap();
        let s = b.add_software_system("S", None, None).unwrap();
        let uses = b
            .add_relationship(&p, &s, Some("Uses".into()), Some("HTTP".into()), None)
            .unwrap();
        let sees = b
            .add_relationship(&p, &s, Some("Sees".into()), None, None)
            .unwrap();
        let v = b.create_dynamic_view(None, "dyn").unwrap();
        // description-preferring lookup finds the "Sees" relationship
        b.dynamic_add_element_pair(v, &p, "Sees", "", &s, "1")
            .unwrap();
        {
            let rv = &b.workspace().views.dynamic_views[0].content.relationships[0];
            assert_eq!(rv.id, sees);
            assert_eq!(rv.order.as_deref(), Some("1"));
            assert_eq!(rv.response, Some(false));
        }
        // reverse step: the reverse lookup finds the FIRST efferent ("Uses"),
        // which is NOT in the view -> missing-relationship error even though
        // "Sees" (same pair) IS in the view.
        let err = b
            .dynamic_add_element_pair(v, &s, "", "", &p, "2")
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "A relationship between S and P does not exist in model."
        );
        // once "Uses" itself is in the view the reverse step resolves to it;
        // every successful add INSERTS (the reference TreeSet is keyed by
        // id AND order, so the response step becomes its own entry).
        b.dynamic_add_relationship(v, &uses, "", "3").unwrap();
        b.dynamic_add_element_pair(v, &s, "", "", &p, "4").unwrap();
        {
            let rvs = &b.workspace().views.dynamic_views[0].content.relationships;
            assert_eq!(rvs.len(), 3, "Sees + Uses + Uses response");
            let response = rvs.last().unwrap();
            assert_eq!(response.id, uses);
            assert_eq!(response.order.as_deref(), Some("4"));
            assert_eq!(response.response, Some(true));
        }
        // missing relationships (both message variants)
        let s2 = b.add_software_system("S2", None, None).unwrap();
        let err = b
            .dynamic_add_element_pair(v, &p, "", "", &s2, "5")
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "A relationship between P and S2 does not exist in model."
        );
        let err = b
            .dynamic_add_element_pair(v, &p, "", "AMQP", &s2, "6")
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "A relationship between P and S2 with technology AMQP does not exist in model."
        );
    }

    #[test]
    fn dynamic_view_scope_gates() {
        let mut b = WorkspaceBuilder::new();
        let s = b.add_software_system("S", None, None).unwrap();
        let c = b.add_container(&s, "C", None, None, None).unwrap();
        let k = b.add_component(&c, "K", None, None, None).unwrap();
        let s2 = b.add_software_system("S2", None, None).unwrap();
        // system-scoped dynamic view: the scope itself and components are
        // rejected (before any relationship lookup)
        let v = b.create_dynamic_view(Some(s.clone()), "dyn").unwrap();
        assert_eq!(
            b.dynamic_add_element_pair(v, &s, "", "", &s2, "1"),
            Err(crate::CoreError::ElementNotPermittedInView(
                "S is already the scope of this view and cannot be added to it.".into()
            ))
        );
        assert_eq!(
            b.dynamic_add_element_pair(v, &k, "", "", &s2, "1"),
            Err(crate::CoreError::ElementNotPermittedInView(
                "Components can't be added to a dynamic view when the scope is a software system."
                    .into()
            ))
        );
        // the scope's own container IS allowed (reference behavior)
        b.add_relationship(&c, &s2, Some("Uses".into()), None, None)
            .unwrap();
        b.dynamic_add_element_pair(v, &c, "", "", &s2, "1").unwrap();
        assert!(b.view_contains_element(v, &c));
    }

    #[test]
    fn create_default_views_mirrors_reference_legs() {
        // M1-e-B (`ViewSet.createDefaultViews`): name-sorted systems drive
        // context/container/component views; environments (UTF-16 sorted)
        // drive scoped deployment views, infrastructure-only environments
        // an unscoped one.
        let mut b = WorkspaceBuilder::new();
        let a = b.add_software_system("B", None, None).unwrap();
        let p = b.add_person("P", None, None).unwrap();
        let z = b.add_software_system("A", None, None).unwrap();
        b.add_relationship(&p, &z, None, None, None).unwrap();
        // only B has structure: container "c" (definition order before "a"
        // — the sort must reorder) with a component; container "a" none.
        let c = b.add_container(&a, "c", None, None, None).unwrap();
        let ca = b.add_container(&a, "a", None, None, None).unwrap();
        b.add_component(&c, "k", None, None, None).unwrap();
        // environments sort "Default" < "prod" (UTF-16); "prod" holds only
        // infrastructure.
        let n1 = b
            .add_deployment_node("Default", "n", None, None, None)
            .unwrap();
        b.add_container_instance(&n1, &c, None, Vec::new()).unwrap();
        let n2 = b
            .add_deployment_node("prod", "n", None, None, None)
            .unwrap();
        b.add_infrastructure_node(&n2, "lb", None, None, None)
            .unwrap();

        b.create_default_views().unwrap();

        let views = &b.workspace().views;
        let keys: Vec<(String, i32, bool)> = views
            .all_views()
            .iter()
            .map(|v| (v.key().to_string(), v.order(), v.base().generated_key))
            .collect();
        // name order A(z) before B(a); per-system context -> container ->
        // components; "Default" environment before "prod"
        assert_eq!(
            keys,
            vec![
                ("SystemLandscape-001".to_string(), 1, true),
                ("SystemContext-001".to_string(), 2, true), // A
                ("SystemContext-002".to_string(), 3, true), // B
                ("Container-001".to_string(), 4, true),     // B
                ("Component-001".to_string(), 5, true),     // B:c
                ("Deployment-001".to_string(), 6, true),    // Default/B
                ("Deployment-002".to_string(), 7, true),    // prod/unscoped
            ]
        );
        // the landscape view holds all people and software systems
        let landscape = views.system_landscape_views.first().unwrap();
        assert_eq!(landscape.content.elements.len(), 3);
        // the context view is scoped to A (first by name)
        assert_eq!(views.system_context_views[0].software_system_id, z);
        // the container view is scoped to B and carries the TopBottom
        // automatic layout with the `AutomaticLayout.DEFAULT_*` settings
        let container_view = views.container_views.first().unwrap();
        assert_eq!(container_view.software_system_id, a);
        assert_eq!(
            container_view.content.automatic_layout,
            Some(crate::views::AutomaticLayout {
                rank_direction: Some(crate::views::RankDirection::TopBottom),
                rank_separation: 100,
                node_separation: 50,
                edge_separation: 50,
                vertices: true,
            })
        );
        // the component view is scoped to container "c" (the only one with
        // components); "a" gets none
        assert_eq!(views.component_views.first().unwrap().container_id, c);
        assert_eq!(views.component_views.len(), 1);
        // deployment: "Default" scoped to B, "prod" unscoped; each view
        // holds its environment's top-level nodes
        assert_eq!(
            views.deployment_views[0].software_system_id.as_ref(),
            Some(&a)
        );
        assert_eq!(views.deployment_views[0].environment, "Default");
        assert_eq!(views.deployment_views[1].software_system_id, None);
        assert_eq!(views.deployment_views[1].environment, "prod");
        assert!(b.view_contains_element(ViewRef::Deployment(0), &n1));
        assert!(!b.view_contains_element(ViewRef::Deployment(0), &n2));
        assert!(b.view_contains_element(ViewRef::Deployment(1), &n2));
        // the container view holds BOTH of B's containers (name-sorted add,
        // `addAllContainers`) — only "c" got a component view
        assert!(b.view_contains_element(ViewRef::Container(0), &ca));
        assert!(b.view_contains_element(ViewRef::Container(0), &c));
    }
}
