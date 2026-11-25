//! View content operations — the add/remove behaviour that backs the DSL
//! `include`/`exclude`/`animation`/dynamic-step statements.
//!
//! Every rule here mirrors the official tooling: the container-view element
//! and relationship gates and their "is not permitted in this view" texts,
//! the deployment view's instance-cascading add, the static view's animation
//! append, and the dynamic view's relationship step (including the reverse
//! lookup that produces `response: true`). The DSL content parsers CATCH the
//! [`CoreError::ElementNotPermittedInView`] variant, exactly like the
//! official tooling's matching catch blocks do.

// Same convention as `builder.rs`: failure modes are enumerated by the shared
// [`CoreError`] enum rather than repeated per method.
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
mod content;
mod defaults;
mod gates;

use std::collections::HashMap;

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;
use crate::views::{Animation, RelationshipView};

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

/// Kind-set filters for nearest-neighbour adds.
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
    /// they hold no view content).
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

    /// Mutable model-view content as an error value: [`CoreError::NoViewContent`]
    /// when the slot carries none (image/filtered view, or — theoretically —
    /// a stale slot), replacing the old `expect_content` panic helper
    /// (charter: no panics in library code).
    fn model_content_mut(
        &mut self,
        view: ViewRef,
    ) -> Result<&mut crate::views::ViewContent, CoreError> {
        if let Some(content) = self.content_mut(view) {
            return Ok(content);
        }
        Err(CoreError::NoViewContent {
            key: self.view_key(view),
        })
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
    /// (a scoped system on the model-view kinds, plus `CustomView`'s optional scope).
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

    /// Whether a dynamic view is deployment-scoped (it has an environment
    /// ⇒ its element gate is a no-op, so anything can be added).
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

    /// Whether the view already includes this element.
    #[must_use]
    pub(crate) fn view_contains_element(&self, view: ViewRef, element: &Id) -> bool {
        self.content_ref(view)
            .is_some_and(|c| c.elements.iter().any(|e| &e.id == element))
    }

    /// Whether the view already includes this relationship.
    #[must_use]
    pub fn view_contains_relationship(&self, view: ViewRef, relationship: &Id) -> bool {
        self.content_ref(view)
            .is_some_and(|c| c.relationships.iter().any(|r| &r.id == relationship))
    }

    /// The ids of the elements currently in the view.
    #[must_use]
    pub(crate) fn view_element_ids(&self, view: ViewRef) -> Vec<Id> {
        self.content_ref(view)
            .map(|c| c.elements.iter().map(|e| e.id.clone()).collect())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // animation steps
    // ------------------------------------------------------------------

    /// The animation-step add rule: elements must be in the view and not
    /// already revealed by an earlier step; a relationship joins the step
    /// when exactly one of its ends is revealed now (the other having been
    /// revealed earlier).
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
                        // Deployment-view animation steps reveal the
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

    /// The relationship ids revealed by one animation step: a view
    /// relationship-view joins when exactly one of its model endpoints is in
    /// this step and the other was revealed by an earlier one (same id
    /// spellings as the stored step; the add-side rule is cited on
    /// [`Self::view_add_animation_step`]). Empty for a view without content.
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
                        let model = self.relationship(&rv.id);
                        let Some(model) = model else {
                            return false;
                        };
                        let src_now = this_step.contains(&model.source);
                        let dst_now = this_step.contains(&model.destination);
                        // membership by `&str` — `previous` holds the id
                        // spellings, so no per-comparison String is needed.
                        let src_prev = previous.iter().any(|p| p == model.source.as_str());
                        let dst_prev = previous.iter().any(|p| p == model.destination.as_str());
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

    /// The `<relationshipIdentifier> [description]` step form. The step
    /// number (`order`) comes from the DSL engine's sequence counter.
    ///
    /// Every successful add INSERTS: the step set is keyed by
    /// `id + "/" + order`, so a re-add with a fresh sequence order is a
    /// NEW entry — duplicate
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
            .relationship(relationship)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .ok_or_else(|| CoreError::RelationshipNotFound {
                id: relationship.clone(),
            })?;
        self.check_element_can_be_added(view, &source)?;
        self.check_element_can_be_added(view, &destination)?;
        self.view_add_element(view, &source, false)?;
        self.view_add_element(view, &destination, false)?;
        let content = self.model_content_mut(view)?;
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

    /// The
    /// `a -> b [description] [technology]` step form. The model relationship
    /// must exist: with no technology the lookup prefers the relationship
    /// whose (normalized) description equals the given one and falls back to
    /// any source→destination relationship; with a technology it's an exact
    /// technology match. A reverse relationship ALREADY IN THE VIEW is added
    /// as `response: true`; otherwise [`CoreError::DynamicRelationshipMissing`].
    /// Like [`Self::dynamic_add_relationship`] every successful add INSERTS
    /// (the step set is keyed by id AND order), and the resolved
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
        // All lookups complete before the first mutation below, so they run
        // on a shared borrow of the relationship arena — no need to project
        // every relationship (incl. description/technology Strings) into a
        // snapshot Vec first.
        let (rel_id, response) = {
            let rels = &self.workspace().model.relationships;
            // Forward lookup: no technology →
            // the description-matched efferent lookup — an EXACT
            // normalized-description match ("" matches ""!) — falling back
            // to any efferent source→destination relationship; with a
            // technology the loop runs WITHOUT break, so the LAST
            // tech match wins.
            let forward = if technology.is_empty() {
                find_rel(rels, source, destination, Some(description), "")
                    .or_else(|| find_rel(rels, source, destination, None, ""))
            } else {
                find_rel_last(rels, source, destination, technology)
            };
            if let Some(id) = forward {
                (id, false)
            } else {
                // Reverse lookup; usable only when that relationship is
                // ALREADY in this view (`response: true`).
                let reverse = if technology.is_empty() {
                    find_rel(rels, destination, source, None, "")
                } else {
                    find_rel_last(rels, destination, source, technology)
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
            }
        };
        self.view_add_element(view, source, false)?;
        self.view_add_element(view, destination, false)?;
        let content = self.model_content_mut(view)?;
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

    /// Whether an image view has content: any of the three content fields is
    /// non-blank (the blank check TRIMS, so `" "` is empty).
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
    rels: &[crate::model::Relationship],
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

/// Technology-filtered lookup with a no-break loop: the LAST
/// source→destination relationship with exactly this technology wins.
fn find_rel_last(
    rels: &[crate::model::Relationship],
    from: &Id,
    to: &Id,
    technology: &str,
) -> Option<Id> {
    rels.iter()
        .rev()
        .find(|r| {
            &r.source == from
                && &r.destination == to
                && r.technology.as_deref().unwrap_or("") == technology
        })
        .map(|r| r.id.clone())
}

/// Relationship insertion without the in-view check (callers checked);
/// dedupes by relationship id.
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

/// A context view refuses to remove every element except its scoped system.
fn view_scope_is_context_system(builder: &WorkspaceBuilder, view: ViewRef, element: &Id) -> bool {
    matches!(view, ViewRef::Context(i)
        if builder
            .workspace()
            .views
            .system_context_views
            .get(i)
            .is_some_and(|v| v.software_system_id == *element))
}

/// Node removal cascade order: instances,
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

/// Name comparison in UTF-16 code-unit order —
/// differs from Rust's UTF-8 byte order where supplementary characters
/// compare against `U+E000..U+FFFF` (same precedent as `views::style_cmp`).
fn java_cmp_str(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Default-views helper: maps each container instance to its owning
/// software system, keeping first-seen order (the list is name-sorted
/// afterwards anyway; stable-sort ties follow this traversal).
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

/// The default-views infrastructure probe (any infrastructure node with a
/// matching environment) over one
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
    // dense element fixtures use one-letter names on purpose
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
        // SAME description, different technology -> duplicate.
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
        // Implied-relationship order: (c1,s2) created before (s1,c2), then (s1,s2).
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
        // every successful add INSERTS (the step set is keyed by id AND
        // order, so the response step becomes its own entry).
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
        // the scope's own container IS allowed (official behavior)
        b.add_relationship(&c, &s2, Some("Uses".into()), None, None)
            .unwrap();
        b.dynamic_add_element_pair(v, &c, "", "", &s2, "1").unwrap();
        assert!(b.view_contains_element(v, &c));
    }

    #[test]
    fn create_default_views_mirrors_reference_legs() {
        // Default-view creation: name-sorted systems drive
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
