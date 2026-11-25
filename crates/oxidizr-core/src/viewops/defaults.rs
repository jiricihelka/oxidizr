//! Default content generation for [`ViewRef`] views: the per-view-kind
//! default-elements rule and the `create_default_views` load-path fallback
//! (landscape, per-system and deployment legs). The plan aliases and the
//! shared free helpers stay in the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::collections::HashMap;

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;
use crate::validate::ElementKind;
use crate::views::{AutomaticLayout, RankDirection};

use super::{
    EnvironmentPlan, KindFilter, SystemPlan, SystemRef, ViewRef, collect_instance_systems,
    has_infrastructure_in_subtree, java_cmp_str,
};

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // default elements
    // ------------------------------------------------------------------

    /// Add-the-default-elements per view type. `*` maps to
    /// greedy; `*?` to non-greedy (which prunes relationships back to the
    /// scoped elements).
    #[allow(clippy::too_many_lines)] // one arm per view kind's default-elements rule
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

    /// Add `element`, then everything it is
    /// directly coupled to (either direction) whose kind passes the filter;
    /// not-permitted additions are silently ignored (the official tooling
    /// prints a warning to stdout).
    fn add_nearest_neighbours(&mut self, view: ViewRef, element: &Id, filter: KindFilter) {
        if self.view_add_element(view, element, true).is_err() {
            return;
        }
        // Snapshot only the ids that pass the filter (in relationship order)
        // rather than every endpoint pair — `view_add_element` never touches
        // `model.relationships`, so collect-then-add is behaviour-identical
        // to a full endpoint scan.
        let to_add: Vec<Id> = self
            .workspace()
            .model
            .relationships
            .iter()
            .filter_map(|r| {
                if &r.source == element && self.filter_matches(filter, &r.destination) {
                    Some(r.destination.clone())
                } else if &r.destination == element && self.filter_matches(filter, &r.source) {
                    Some(r.source.clone())
                } else {
                    None
                }
            })
            .collect();
        for id in to_add {
            let _ = self.view_add_element(view, &id, true);
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

    /// All deployment nodes of the environment, each with its
    /// subtree cascade.
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

    /// The node subtree cascade:
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
            // An element this view kind rejects is skipped.
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
            // Infrastructure nodes are added UNGUARDED — a
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

    /// The public node add: the subtree cascade, then the
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

    /// The deployment-view add rules the DSL content parser picks for the
    /// element's kind: deployment nodes cascade their subtree, then walk up
    /// the ancestor chain; infrastructure nodes and static-element instances
    /// add themselves, then walk the same chain; custom elements just add.
    /// Other kinds fall through to `view_add_element`, whose element gate
    /// rejects them with the same messages the official tooling uses.
    pub fn view_add_deployment_element(
        &mut self,
        view: ViewRef,
        id: &Id,
        add_relationships: bool,
    ) -> Result<(), CoreError> {
        match self.element_kind(id) {
            Some(ElementKind::DeploymentNode) => self.deployment_view_add_node_top(view, id),
            Some(
                ElementKind::InfrastructureNode
                | ElementKind::SoftwareSystemInstance
                | ElementKind::ContainerInstance,
            ) => {
                self.view_add_element(view, id, add_relationships)?;
                let mut parent = self.element_parent(id);
                while let Some(p) = parent {
                    self.view_add_element(view, &p, add_relationships)?;
                    parent = self.element_parent(&p);
                }
                Ok(())
            }
            _ => self.view_add_element(view, id, add_relationships),
        }
    }

    // ------------------------------------------------------------------
    // default views
    // ------------------------------------------------------------------

    /// The default-view generation the official load path runs when the model
    /// is non-empty but the view set is empty, applied at the same point of
    /// oxidizr's load pipeline. Mirrors the official tooling exactly: one
    /// landscape view; per software system (stable sort by UTF-16 name order)
    /// a context view, plus a container view with automatic layout
    /// (`TopBottom`) when the system has containers, plus a component view
    /// per name-sorted container that has components; then per deployment
    /// environment (deduped first-seen from the top-level nodes, then sorted)
    /// one scoped deployment view per software system holding container
    /// instances in the environment (name-sorted), or one unscoped view when
    /// the environment holds only infrastructure nodes. The `Err` arms are
    /// the official tooling's unguarded failures (duplicate keys /
    /// not-permitted adds) — unreachable on the empty view set this factory
    /// is called with, but propagated so a surprise fails the load.
    pub fn create_default_views(&mut self) -> Result<(), CoreError> {
        self.create_default_landscape_view()?;
        self.create_default_system_views()?;
        self.create_default_deployment_views()?;
        Ok(())
    }

    /// The landscape leg of [`Self::create_default_views`]: a single System
    /// Landscape view with all people and software systems.
    fn create_default_landscape_view(&mut self) -> Result<(), CoreError> {
        let landscape = self.create_landscape_view("")?;
        self.view_add_default_elements(landscape, true)
    }

    /// The system leg of [`Self::create_default_views`]: per software system
    /// (UTF-16 name order) a context view, plus a container view and
    /// component views.
    fn create_default_system_views(&mut self) -> Result<(), CoreError> {
        // per software system (UTF-16 name order): a context view, plus a
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
        Ok(())
    }

    /// The deployment leg of [`Self::create_default_views`]: one scoped
    /// deployment view per environment (deduped first-seen from the top-level
    /// nodes, then sorted) and software system holding container instances in
    /// the environment (name-sorted), or one unscoped view when the
    /// environment holds only infrastructure nodes.
    fn create_default_deployment_views(&mut self) -> Result<(), CoreError> {
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
                        // The scoped deployment-view creation: the
                        // scope must be set BEFORE the default-elements add (the
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
}
