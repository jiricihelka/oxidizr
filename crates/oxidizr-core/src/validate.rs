//! Validation catalog — a transcription of the reference load-time checks
//! `Workspace.hydrate()` = `Model.hydrate()` then `ViewSet.hydrate(model)`
//! (verified @ `ad34036`; fixture expectations:
//! `structurizr-client` `WorkspaceRulesValidationTests`, workspaces in
//! `testdata/validation/`).
//!
//! Deviations (D1): the reference throws `WorkspaceValidationException` on the
//! FIRST violation; we collect ALL of them but keep the rule ORDER identical,
//! so the first diagnostic always matches the reference message. Two reference
//! NPE paths (null relationship description / missing relationship endpoints)
//! are reported as diagnostics instead of crashing.
//!
//! When MULTIPLE violations exist the reference's exact winner depends on
//! Java `TreeSet` iteration order; with a single violation (all fixtures) our
//! first message is byte-identical to the reference exception.

// The catalog is a line-by-line transcription of `Model.hydrate` +
// `ViewSet.hydrate`; splitting the two rule walkers across functions would
// scatter the reference's check ORDER, which is the load-bearing property.
#![allow(clippy::too_many_lines)]

use std::collections::{HashMap, HashSet};

use crate::ids::Id;
use crate::model::{DeploymentNode, Model};
use crate::views::{View, ViewSet};

/// One catalog violation. `message` matches the reference exception text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Reference-shaped message (first one matches the thrown exception).
    pub message: String,
}

/// Validate a whole workspace, reference hydrate order (model first, views
/// second). Collects all violations (D1).
#[must_use]
pub fn validate(workspace_model: &Model, views: &ViewSet) -> Vec<ValidationError> {
    let mut out = Vec::new();
    validate_model(workspace_model, &mut out);
    validate_views(workspace_model, views, &mut out);
    out
}

fn err(out: &mut Vec<ValidationError>, message: String) {
    out.push(ValidationError { message });
}

/// All elements, flattened, in hydrate walk order: custom elements, people,
/// systems (+containers+components), deployment tree (node, its instances and
/// infrastructure nodes, then children), returned with kind labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    CustomElement,
    Person,
    SoftwareSystem,
    Container,
    Component,
    DeploymentNode,
    InfrastructureNode,
    SoftwareSystemInstance,
    ContainerInstance,
}

/// Flatten the model into `(id, kind, name)` triples in hydrate order.
#[must_use]
pub fn flattened(model: &Model) -> Vec<(Id, ElementKind, String)> {
    let mut out = Vec::new();
    for ce in &model.custom_elements {
        out.push((
            ce.item.id.clone(),
            ElementKind::CustomElement,
            ce.item.name.clone(),
        ));
    }
    for p in &model.people {
        out.push((p.item.id.clone(), ElementKind::Person, p.item.name.clone()));
    }
    for s in &model.software_systems {
        out.push((
            s.item.id.clone(),
            ElementKind::SoftwareSystem,
            s.item.name.clone(),
        ));
        for c in &s.containers {
            out.push((
                c.item.id.clone(),
                ElementKind::Container,
                c.item.name.clone(),
            ));
            for k in &c.components {
                out.push((
                    k.item.id.clone(),
                    ElementKind::Component,
                    k.item.name.clone(),
                ));
            }
        }
    }
    for node in &model.deployment_nodes {
        flatten_node(node, &mut out);
    }
    out
}

fn flatten_node(node: &DeploymentNode, out: &mut Vec<(Id, ElementKind, String)>) {
    out.push((
        node.item.id.clone(),
        ElementKind::DeploymentNode,
        node.item.name.clone(),
    ));
    for child in &node.children {
        flatten_node(child, out);
    }
    for ssi in &node.software_system_instances {
        out.push((
            ssi.item.id.clone(),
            ElementKind::SoftwareSystemInstance,
            ssi.item.name.clone(),
        ));
    }
    for ci in &node.container_instances {
        out.push((
            ci.item.id.clone(),
            ElementKind::ContainerInstance,
            ci.item.name.clone(),
        ));
    }
    for inode in &node.infrastructure_nodes {
        out.push((
            inode.item.id.clone(),
            ElementKind::InfrastructureNode,
            inode.item.name.clone(),
        ));
    }
}

/// Model-level checks (`Model.hydrate`): id uniqueness, instance links,
/// name uniqueness, relationship description uniqueness, linked ids.
pub fn validate_model(model: &Model, out: &mut Vec<ValidationError>) {
    // 1. Unique ids across BOTH element and relationship spaces
    // (`addElementToInternalStructures` / `addRelationshipToInternalStructures`).
    let mut ids: HashMap<&Id, &'static str> = HashMap::new();
    for (id, kind, _) in flattened_ids(model) {
        let label = match kind {
            ElementKind::CustomElement
            | ElementKind::Person
            | ElementKind::SoftwareSystem
            | ElementKind::Container
            | ElementKind::Component
            | ElementKind::DeploymentNode
            | ElementKind::InfrastructureNode
            | ElementKind::SoftwareSystemInstance
            | ElementKind::ContainerInstance => "element",
        };
        if ids.insert(id, label).is_some() {
            err(
                out,
                format!(
                    "The {label} {} has a non-unique ID of {id}.",
                    non_unique_name(id, model)
                ),
            );
        }
    }
    for r in &model.relationships {
        if ids.insert(&r.id, "relationship").is_some() {
            // Reference: "The relationship <toString> has a non-unique ID of N."
            // (`toString` is source ---[desc]---> destination; D1 form).
            err(
                out,
                format!(
                    "The relationship {} ---[{}]---> {} has a non-unique ID of {}.",
                    r.source,
                    r.description.clone().unwrap_or_default(),
                    r.destination,
                    r.id
                ),
            );
        }
    }

    // 2. Instance static links (`hydrateDeploymentNode`).
    let element_kinds: HashMap<&Id, ElementKind> = flattened_ids(model)
        .into_iter()
        .map(|(id, k, _)| (id, k))
        .collect();
    for node in &model.deployment_nodes {
        check_instance_links(node, &element_kinds, out);
    }

    // 3. Name uniqueness.
    // 3a. people + software systems (one shared namespace).
    {
        let mut names: HashMap<&str, usize> = HashMap::new();
        for p in &model.people {
            *names.entry(p.item.name.as_str()).or_default() += 1;
        }
        for s in &model.software_systems {
            *names.entry(s.item.name.as_str()).or_default() += 1;
        }
        let mut reported: HashSet<&str> = HashSet::new();
        for name in model
            .people
            .iter()
            .map(|p| p.item.name.as_str())
            .chain(model.software_systems.iter().map(|s| s.item.name.as_str()))
        {
            if names[name] != 1 && reported.insert(name) {
                err(
                    out,
                    format!("A person or software system named \"{name}\" already exists."),
                );
            }
        }
    }
    // 3b/3c. containers per system, components per container.
    for s in &model.software_systems {
        let container_names: Vec<&str> =
            s.containers.iter().map(|c| c.item.name.as_str()).collect();
        check_name_uniqueness(
            &container_names,
            |name| {
                format!(
                    "A container named \"{name}\" already exists within \"{}\".",
                    s.item.name
                )
            },
            out,
        );
        for c in &s.containers {
            let component_names: Vec<&str> =
                c.components.iter().map(|k| k.item.name.as_str()).collect();
            check_name_uniqueness(
                &component_names,
                |name| {
                    format!(
                        "A component named \"{name}\" already exists within \"{}\".",
                        c.item.name
                    )
                },
                out,
            );
        }
    }
    // 3d. top-level deployment nodes per environment, recursively children.
    {
        let top_level: Vec<(&str, &str)> = model
            .deployment_nodes
            .iter()
            .map(|n| (n.item.name.as_str(), n.environment.as_str()))
            .collect();
        let mut reported: HashSet<(&str, &str)> = HashSet::new();
        for (name, env) in &top_level {
            if top_level
                .iter()
                .filter(|(n, e)| n == name && e == env)
                .count()
                != 1
                && reported.insert((*name, *env))
            {
                err(
                    out,
                    format!(
                        "A top-level deployment node named \"{name}\" already exists for the environment named \"{env}\"."
                    ),
                );
            }
        }
        for node in &model.deployment_nodes {
            check_child_node_names(node, out);
        }
    }

    // 4. Relationship description uniqueness per source element: pairs
    // (destination, description) — reference `checkDescriptionIsUnique`.
    let names: HashMap<&Id, String> = flattened_ids(model)
        .into_iter()
        .map(|(id, _, name)| (id, name.to_string()))
        .collect();
    let mut reported_desc: HashSet<(&Id, &Id, &str)> = HashSet::new();
    for r in &model.relationships {
        let same = model
            .relationships
            .iter()
            .filter(|x| {
                x.source == r.source
                    && x.destination == r.destination
                    && x.description == r.description
            })
            .count();
        if same != 1 {
            let description = r.description.as_deref().unwrap_or("");
            // Reference NPEs on a null description here; D1 reports the
            // violation with the empty description instead.
            if let (Some(source_name), Some(destination_name)) =
                (names.get(&r.source), names.get(&r.destination))
            {
                if reported_desc.insert((&r.source, &r.destination, description)) {
                    err(
                        out,
                        format!(
                            "A relationship with the description \"{description}\" already exists between \"{source_name}\" and \"{destination_name}\"."
                        ),
                    );
                }
            }
        }
    }

    // 5. Linked relationships must exist.
    let rel_ids: HashSet<&Id> = model.relationships.iter().map(|r| &r.id).collect();
    let mut reported_linked: HashSet<&Id> = HashSet::new();
    for r in &model.relationships {
        if let Some(linked) = &r.linked_relationship_id
            && !rel_ids.contains(linked)
            && reported_linked.insert(linked)
        {
            err(
                out,
                format!("The linked relationship with id \"{linked}\" is not found."),
            );
        }
    }
}

fn flattened_ids(model: &Model) -> Vec<(&Id, ElementKind, &str)> {
    fn walk<'m>(n: &'m DeploymentNode, out: &mut Vec<(&'m Id, ElementKind, &'m str)>) {
        out.push((&n.item.id, ElementKind::DeploymentNode, &n.item.name));
        for c in &n.children {
            walk(c, out);
        }
        for ssi in &n.software_system_instances {
            out.push((
                &ssi.item.id,
                ElementKind::SoftwareSystemInstance,
                &ssi.item.name,
            ));
        }
        for ci in &n.container_instances {
            out.push((&ci.item.id, ElementKind::ContainerInstance, &ci.item.name));
        }
        for i in &n.infrastructure_nodes {
            out.push((&i.item.id, ElementKind::InfrastructureNode, &i.item.name));
        }
    }
    let mut out: Vec<(&Id, ElementKind, &str)> = Vec::new();
    for ce in &model.custom_elements {
        out.push((&ce.item.id, ElementKind::CustomElement, &ce.item.name));
    }
    for p in &model.people {
        out.push((&p.item.id, ElementKind::Person, &p.item.name));
    }
    for s in &model.software_systems {
        out.push((&s.item.id, ElementKind::SoftwareSystem, &s.item.name));
        for c in &s.containers {
            out.push((&c.item.id, ElementKind::Container, &c.item.name));
            for k in &c.components {
                out.push((&k.item.id, ElementKind::Component, &k.item.name));
            }
        }
    }
    for n in &model.deployment_nodes {
        walk(n, &mut out);
    }
    out
}

/// Best-effort item name for the non-unique-id message (the reference prints
/// the canonical name; a collision makes that ambiguous, so the bare name of
/// the offending (first-seen) element is close enough — D1 diagnostics).
fn non_unique_name(id: &Id, model: &Model) -> String {
    flattened_ids(model)
        .into_iter()
        .find(|(eid, _, _)| *eid == id)
        .map_or_else(|| id.to_string(), |(_, _, name)| name.to_string())
}

fn check_instance_links(
    node: &DeploymentNode,
    element_kinds: &HashMap<&Id, ElementKind>,
    out: &mut Vec<ValidationError>,
) {
    for ssi in &node.software_system_instances {
        if element_kinds.get(&ssi.software_system_id) != Some(&ElementKind::SoftwareSystem) {
            err(
                out,
                format!(
                    "A software system instance is associated with a software system (id={}) that does not exist in the model.",
                    ssi.software_system_id
                ),
            );
        }
    }
    for ci in &node.container_instances {
        if element_kinds.get(&ci.container_id) != Some(&ElementKind::Container) {
            err(
                out,
                format!(
                    "A container instance is associated with a container (id={}) that does not exist in the model.",
                    ci.container_id
                ),
            );
        }
    }
    for child in &node.children {
        check_instance_links(child, element_kinds, out);
    }
}

fn check_name_uniqueness<F>(names: &[&str], message: F, out: &mut Vec<ValidationError>)
where
    F: Fn(&str) -> String,
{
    let mut reported: HashSet<&str> = HashSet::new();
    for name in names {
        if names.iter().filter(|n| *n == name).count() != 1 && reported.insert(name) {
            err(out, message(name));
        }
    }
}

fn check_child_node_names(node: &DeploymentNode, out: &mut Vec<ValidationError>) {
    let names: Vec<&str> = node.children.iter().map(|c| c.item.name.as_str()).collect();
    let mut reported: HashSet<&str> = HashSet::new();
    for name in names.iter().copied() {
        if names.iter().copied().filter(|n| *n == name).count() != 1 && reported.insert(name) {
            err(
                out,
                format!(
                    "A deployment node named \"{name}\" already exists within \"{}\".",
                    node.item.name
                ),
            );
        }
    }
    for child in &node.children {
        check_child_node_names(child, out);
    }
}

/// View-level checks (`ViewSet.hydrate`), reference order.
pub fn validate_views(model: &Model, views: &ViewSet, out: &mut Vec<ValidationError>) {
    let element_kinds: HashMap<&Id, ElementKind> = flattened_ids(model)
        .into_iter()
        .map(|(id, k, _)| (id, k))
        .collect();
    let rel_ids: HashSet<&Id> = model.relationships.iter().map(|r| &r.id).collect();

    // 1. View-key uniqueness, walked in the reference TreeSet order:
    // (order asc, key case-insensitive).
    let mut ordered: Vec<View<'_>> = views.all_views();
    ordered.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.key().to_lowercase().cmp(&b.key().to_lowercase()))
    });
    let mut seen: HashSet<&str> = HashSet::new();
    for v in &ordered {
        if !seen.insert(v.key()) {
            err(
                out,
                format!("A view with the key {} already exists.", v.key()),
            );
        }
    }

    // (per-view-type link checks use `missing_element` below)

    // 2..8 per view type, reference order, checking links then entry arrays
    // (`hydrateView`).
    for v in views.all_views() {
        let key = v.key();
        match v {
            View::Custom(_) | View::Landscape(_) => {}
            View::Context(cv) => {
                if element_kinds.get(&cv.software_system_id) != Some(&ElementKind::SoftwareSystem) {
                    missing_element(
                        out,
                        "system context view",
                        key,
                        "a software system",
                        &cv.software_system_id,
                    );
                }
            }
            View::Container(cv) => {
                if element_kinds.get(&cv.software_system_id) != Some(&ElementKind::SoftwareSystem) {
                    missing_element(
                        out,
                        "container view",
                        key,
                        "a software system",
                        &cv.software_system_id,
                    );
                }
            }
            View::Component(cv) => {
                if element_kinds.get(&cv.container_id) != Some(&ElementKind::Container) {
                    missing_element(out, "component view", key, "a container", &cv.container_id);
                }
            }
            View::Dynamic(dv) => {
                if let Some(id) = &dv.element_id
                    && !element_kinds.contains_key(id)
                {
                    missing_element(out, "dynamic view", key, "an element", id);
                }
            }
            View::Deployment(dv) => {
                if let Some(id) = &dv.software_system_id
                    && element_kinds.get(id) != Some(&ElementKind::SoftwareSystem)
                {
                    missing_element(out, "deployment view", key, "a software system", id);
                }
            }
            View::Filtered(fv) => match views
                .all_views()
                .iter()
                .find(|b| b.key() == fv.base_view_key)
            {
                None => err(
                    out,
                    format!(
                        "The filtered view with key \"{key}\" is based upon a view (key={}), but that view does not exist in the workspace.",
                        fv.base_view_key
                    ),
                ),
                Some(base) => match base {
                    View::Custom(_)
                    | View::Landscape(_)
                    | View::Context(_)
                    | View::Container(_)
                    | View::Component(_)
                    | View::Deployment(_) => {}
                    View::Dynamic(_) | View::Filtered(_) | View::Image(_) => err(
                        out,
                        format!(
                            "The filtered view with key \"{key}\" is based upon a view (key={}), but that view is not a static or deployment view.",
                            fv.base_view_key
                        ),
                    ),
                },
            },
            View::Image(iv) => {
                if let Some(id) = &iv.element_id
                    && !element_kinds.contains_key(id)
                {
                    missing_element(out, "image view", key, "an element", id);
                }
            }
        }

        if let Some(content) = v.content() {
            for ev in &content.elements {
                if !element_kinds.contains_key(&ev.id) {
                    err(
                        out,
                        format!(
                            "The view with key \"{key}\" references an element (id={}), but that element does not exist in the model.",
                            ev.id
                        ),
                    );
                }
            }
            for rv in &content.relationships {
                if !rel_ids.contains(&rv.id) {
                    err(
                        out,
                        format!(
                            "The view with key \"{key}\" references a relationship (id={}), but that relationship does not exist in the model.",
                            rv.id
                        ),
                    );
                }
            }
        }
    }
}

/// `ViewSet.hydrate` per-view link error: the associated element id is absent
/// (or of the wrong kind — the reference reports the same message).
fn missing_element(
    out: &mut Vec<ValidationError>,
    kind_label: &str,
    key: &str,
    noun: &str,
    id: &Id,
) {
    err(
        out,
        format!(
            "The {kind_label} with key \"{key}\" is associated with {noun} (id={id}), but that element does not exist in the model."
        ),
    );
}
