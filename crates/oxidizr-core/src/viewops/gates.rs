//! Per-view-kind element gates: the checks that decide which elements a view
//! of each kind may contain, with the official "is not permitted in this
//! view" texts. The add/remove legs, the default generation and the step
//! legs live in the sibling modules; the shared helpers stay in the parent.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;
use crate::model::DeploymentNode;
use crate::validate::ElementKind;

use super::ViewRef;

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // gates (per-view-kind element checks)
    // ------------------------------------------------------------------

    /// The element gate for a view, dispatched on the view kind: a landscape
    /// view takes people and systems only, a context view takes the scoped
    /// system with its people/system/containers, a container view is scoped
    /// to one system (containers of it, plus components of those), a
    /// component view to one container, and a deployment view follows the
    /// environment/scope rules. One fn per view kind's rules, hence the
    /// length.
    #[allow(clippy::too_many_lines)]
    pub(super) fn check_element_can_be_added(
        &self,
        view: ViewRef,
        element: &Id,
    ) -> Result<(), CoreError> {
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

    /// The dynamic view's element gate.
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
            // Deployment-scoped dynamic views accept everything.
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
        // Pair the scope with its kind: `scope_kind` is derived from `scope`,
        // so a `Some` kind always implies a `Some` scope — the branches used
        // to split the pair and re-join it with `clone().expect("checked")`.
        let scoped = scope_kind.map(|k| (scope.as_ref(), k));
        if let Some((Some(scope), ElementKind::SoftwareSystem)) = scoped {
            if element == scope {
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
        if let Some((Some(scope), ElementKind::Container)) = scoped {
            let scope_parent = self.element_parent(scope);
            if element == scope || Some(element.clone()) == scope_parent {
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

    /// A parent and its children may not both be in one view.
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

    /// The `environment` of a deployment element. Recursive pre-order over
    /// the node tree checking node → infrastructure/ssi/ci → children — the
    /// exact visit order of a full flatten scan, without building (and
    /// cloning) the inherited-group vectors nobody reads.
    #[must_use]
    pub fn element_environment(&self, id: &Id) -> Option<String> {
        fn find<'a>(nodes: &'a [DeploymentNode], id: &Id) -> Option<&'a str> {
            for n in nodes {
                if &n.item.id == id {
                    return Some(&n.environment);
                }
                for i in &n.infrastructure_nodes {
                    if &i.item.id == id {
                        return Some(&i.environment);
                    }
                }
                for s in &n.software_system_instances {
                    if &s.item.id == id {
                        return Some(&s.environment);
                    }
                }
                for c in &n.container_instances {
                    if &c.item.id == id {
                        return Some(&c.environment);
                    }
                }
                if let Some(e) = find(&n.children, id) {
                    return Some(e);
                }
            }
            None
        }
        find(&self.workspace().model.deployment_nodes, id).map(str::to_string)
    }

    /// The static element an instance is based on.
    fn instance_static_element(&self, id: &Id) -> Option<Id> {
        fn find(nodes: &[DeploymentNode], id: &Id) -> Option<Id> {
            for n in nodes {
                for s in &n.software_system_instances {
                    if &s.item.id == id {
                        return Some(s.software_system_id.clone());
                    }
                }
                for c in &n.container_instances {
                    if &c.item.id == id {
                        return Some(c.container_id.clone());
                    }
                }
                if let Some(x) = find(&n.children, id) {
                    return Some(x);
                }
            }
            None
        }
        find(&self.workspace().model.deployment_nodes, id)
    }

    /// For a container instance: the software system its container belongs
    /// to (container → owning system).
    pub(super) fn instance_container_system(&self, id: &Id) -> Option<Id> {
        let container = self.instance_static_element(id)?;
        self.element_parent(&container)
    }
}
