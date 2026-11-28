//! Scene-local element resolution and boundary accumulation: the
//! `Resolved` model view, the identifier walkers, the `Placement` enum and
//! the group/deployment-node scope shapes the boundary pass folds. The
//! official UI text helpers live in the parent's label section; the public
//! types, the `build` entry and the tests stay in the parent module.
//!
//! The items here are `pub(super)` as a set: `build` in the parent drives
//! the resolution loop and folds the boundary shapes.

use std::collections::HashMap;

use oxidizr_core::Id;
use oxidizr_core::model::{DeploymentNode, Model, ModelItem};
use oxidizr_core::styles::instance_tag_chain;
use oxidizr_core::tags::TagKind;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{ColorScheme, LineStyle, View};

use super::{
    BOUNDARY_PADDING, Boundary, BoundaryKind, DEFAULT_STROKE_WIDTH, Item, Rect, union_all,
};

// ---- scene-local element resolution ---------------------------------------

/// A model element resolved for rendering: identity + the fields the
/// the official UI draws (the `filter` module's own resolve is tag-only).
pub(super) struct Resolved<'m> {
    pub(super) kind: ElementKind,
    pub(super) item: &'m ModelItem,
    pub(super) technology: Option<&'m str>,
    pub(super) group: Option<&'m str>,
    pub(super) custom_metadata: Option<&'m str>,
    pub(super) parent: Option<&'m Id>,
    pub(super) environment: Option<&'m str>,
    /// Instance → static element (tag-chain prepending, the official UI
    /// `findElementStyle`).
    pub(super) static_element: Option<(&'m ModelItem, ElementKind)>,
}

/// Resolve one id across the whole model (hydrate-order walk).
pub(super) fn resolve<'m>(model: &'m Model, id: &Id) -> Option<Resolved<'m>> {
    for e in &model.custom_elements {
        if e.item.id == *id {
            return Some(Resolved {
                kind: ElementKind::CustomElement,
                item: &e.item,
                technology: None,
                group: e.group.as_deref(),
                custom_metadata: e.metadata.as_deref().filter(|m| !m.is_empty()),
                parent: None,
                environment: None,
                static_element: None,
            });
        }
    }
    for p in &model.people {
        if p.item.id == *id {
            return Some(Resolved {
                kind: ElementKind::Person,
                item: &p.item,
                technology: None,
                group: p.group.as_deref(),
                custom_metadata: None,
                parent: None,
                environment: None,
                static_element: None,
            });
        }
    }
    for ss in &model.software_systems {
        if ss.item.id == *id {
            return Some(Resolved {
                kind: ElementKind::SoftwareSystem,
                item: &ss.item,
                technology: None,
                group: ss.group.as_deref(),
                custom_metadata: None,
                parent: None,
                environment: None,
                static_element: None,
            });
        }
        for c in &ss.containers {
            if c.item.id == *id {
                return Some(Resolved {
                    kind: ElementKind::Container,
                    item: &c.item,
                    technology: c.technology.as_deref(),
                    group: c.group.as_deref(),
                    custom_metadata: None,
                    parent: Some(&ss.item.id),
                    environment: None,
                    static_element: None,
                });
            }
            for component in &c.components {
                if component.item.id == *id {
                    return Some(Resolved {
                        kind: ElementKind::Component,
                        item: &component.item,
                        technology: component.technology.as_deref(),
                        group: component.group.as_deref(),
                        custom_metadata: None,
                        parent: Some(&c.item.id),
                        environment: None,
                        static_element: None,
                    });
                }
            }
        }
    }
    resolve_node(model, &model.deployment_nodes, id, None)
}

pub(super) fn resolve_node<'m>(
    model: &'m Model,
    nodes: &'m [DeploymentNode],
    id: &Id,
    parent: Option<&'m Id>,
) -> Option<Resolved<'m>> {
    for node in nodes {
        if node.item.id == *id {
            return Some(Resolved {
                kind: ElementKind::DeploymentNode,
                item: &node.item,
                technology: node.technology.as_deref(),
                group: node.group.as_deref(),
                custom_metadata: None,
                parent,
                environment: Some(&node.environment),
                static_element: None,
            });
        }
        for instance in &node.software_system_instances {
            if instance.item.id == *id {
                let static_ss = model
                    .software_systems
                    .iter()
                    .find(|s| s.item.id == instance.software_system_id);
                return Some(Resolved {
                    kind: ElementKind::SoftwareSystemInstance,
                    item: &instance.item,
                    technology: None,
                    group: instance.group.as_deref(),
                    custom_metadata: None,
                    parent: Some(&node.item.id),
                    environment: Some(&node.environment),
                    static_element: static_ss.map(|s| (&s.item, ElementKind::SoftwareSystem)),
                });
            }
        }
        for instance in &node.container_instances {
            if instance.item.id == *id {
                let static_container = model
                    .software_systems
                    .iter()
                    .flat_map(|s| &s.containers)
                    .find(|c| c.item.id == instance.container_id);
                return Some(Resolved {
                    kind: ElementKind::ContainerInstance,
                    item: &instance.item,
                    technology: None,
                    group: instance.group.as_deref(),
                    custom_metadata: None,
                    parent: Some(&node.item.id),
                    environment: Some(&node.environment),
                    static_element: static_container.map(|c| (&c.item, ElementKind::Container)),
                });
            }
        }
        for infra in &node.infrastructure_nodes {
            if infra.item.id == *id {
                return Some(Resolved {
                    kind: ElementKind::InfrastructureNode,
                    item: &infra.item,
                    technology: infra.technology.as_deref(),
                    group: infra.group.as_deref(),
                    custom_metadata: None,
                    parent: Some(&node.item.id),
                    environment: Some(&node.environment),
                    static_element: None,
                });
            }
        }
        if let Some(found) = resolve_node(model, &node.children, id, Some(&node.item.id)) {
            return Some(found);
        }
    }
    None
}

/// The element's full style tag chain as CSV — the official UI
/// `getTags` (`Tags::serialized_for`), with the instance prepending
/// (`instance_tag_chain`, the official UI `Styles.findElementStyle`).
pub(super) fn tags_csv(el: &Resolved<'_>) -> String {
    let own = el
        .item
        .tags
        .serialized_for(TagKind::from_element_kind(el.kind));
    match el.static_element {
        Some((item, kind)) => {
            let parent = item.tags.serialized_for(TagKind::from_element_kind(kind));
            instance_tag_chain(&parent, &own)
        }
        None => own,
    }
}

// ---- boundary accumulation ------------------------------------------------

/// What one content entry rendered as (indices into `items` / `dns`).
#[derive(Debug, Clone, Copy)]
pub(super) enum Placement {
    Item(usize),
    Dn(usize),
}

pub(super) struct DnSpec {
    pub(super) id: Id,
    pub(super) label: String,
    pub(super) metadata: Option<String>,
    pub(super) tags: String,
    pub(super) order: usize,
}

pub(super) struct GroupSpec {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) tags: String,
    pub(super) order: usize,
    pub(super) items: Vec<usize>,
    pub(super) dns: Vec<usize>,
}

pub(super) struct ScopeSpec {
    pub(super) id: String,
    pub(super) kind: BoundaryKind,
    pub(super) label: String,
    pub(super) metadata: Option<String>,
    pub(super) tags: String,
    pub(super) order: usize,
    pub(super) items: Vec<usize>,
}

/// The style/scheme bundle every boundary is painted with (the argument
/// bundle keeps [`boundary_from`] within the pedantic budget).
pub(super) struct Paint<'a> {
    pub(super) styles: &'a oxidizr_core::views::Styles,
    pub(super) color_scheme: ColorScheme,
    pub(super) default_background: &'a str,
    pub(super) default_color: &'a str,
}

/// The identity fields every boundary accumulator carries (kind, stable
/// id, label, metadata line, tag chain) — the argument bundle keeps
/// [`boundary_from`] within the pedantic budget.
pub(super) struct Identity<'a> {
    pub(super) kind: BoundaryKind,
    pub(super) id: String,
    pub(super) label: String,
    pub(super) metadata: Option<String>,
    pub(super) tags: &'a str,
}

/// One boundary: member union + padding, style resolved through the
/// Boundary tag chain, carrying the explicit member ids the layout graph
/// is built from (`crate::layout::graph_for`).
pub(super) fn boundary_from(
    paint: &Paint<'_>,
    identity: Identity<'_>,
    rect: Rect,
    members: Vec<Id>,
) -> Boundary {
    let style = paint
        .styles
        .element_style(identity.tags, paint.color_scheme);
    Boundary {
        kind: identity.kind,
        id: identity.id,
        label: identity.label,
        metadata: identity.metadata,
        rect: rect.expand(BOUNDARY_PADDING),
        members,
        background: style
            .background
            .unwrap_or_else(|| paint.default_background.to_string()),
        stroke: style
            .stroke
            .unwrap_or_else(|| paint.default_color.to_string()),
        stroke_width: style.stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH),
        border: style.border.unwrap_or(LineStyle::Dashed),
        color: style
            .color
            .unwrap_or_else(|| paint.default_color.to_string()),
        font_size: style.font_size.unwrap_or(24).max(1),
        opacity: style.opacity.unwrap_or(100).clamp(0, 100),
    }
}

/// Item ids behind sorted item indices — a boundary's member list in
/// scene order.
pub(super) fn member_ids(indices: &[usize], items: &[Item]) -> Vec<Id> {
    indices.iter().map(|i| items[*i].id.clone()).collect()
}

/// Memoized transitive member items of a deployment node: direct child
/// items plus the members of child nodes — the same closure [`dn_rect`]
/// unions. Returns sorted item indices (scene order); the memo doubles as
/// the cycle guard [`dn_rect`] uses (a hypothetical revisit sees the
/// placeholder `Vec::new()` instead of recursing forever).
pub(super) fn dn_member_items(
    idx: usize,
    child_items: &HashMap<usize, Vec<usize>>,
    child_dns: &HashMap<usize, Vec<usize>>,
    cache: &mut Vec<Option<Vec<usize>>>,
) -> Vec<usize> {
    if let Some(members) = &cache[idx] {
        return members.clone();
    }
    cache[idx] = Some(Vec::new());
    let mut out: Vec<usize> = child_items.get(&idx).cloned().unwrap_or_default();
    if let Some(kids) = child_dns.get(&idx) {
        for kid in kids.clone() {
            out.extend(dn_member_items(kid, child_items, child_dns, cache));
        }
    }
    out.sort_unstable();
    out.dedup();
    cache[idx] = Some(out.clone());
    out
}

/// Memoized bottom-up deployment-node rect (the official UI: the deployment-node
/// cells embed child cells, then the second pass embeds nodes into their
/// parent cell).
pub(super) fn dn_rect(
    idx: usize,
    child_items: &HashMap<usize, Vec<usize>>,
    child_dns: &HashMap<usize, Vec<usize>>,
    items: &[Item],
    rects: &mut Vec<Option<Rect>>,
    done: &mut Vec<bool>,
) -> Option<Rect> {
    if done[idx] {
        return rects[idx];
    }
    // Mark done FIRST: nesting is acyclic by model construction, and a
    // (hypothetical) revisit then sees `None` instead of recursing forever.
    done[idx] = true;
    let mut members: Vec<Rect> = child_items.get(&idx).map_or_else(Vec::new, |kids| {
        kids.iter().map(|i| items[*i].rect).collect()
    });
    if let Some(kids) = child_dns.get(&idx) {
        for kid in kids.clone() {
            members.extend(dn_rect(kid, child_items, child_dns, items, rects, done));
        }
    }
    let rect = union_all(members.into_iter());
    rects[idx] = rect;
    rect
}

/// Group-membership scoping: the
/// model parent, else the environment (top-level deployment nodes), else
/// the single root scope (the `location` split is not in the contract).
pub(super) fn group_scope(el: &Resolved<'_>) -> String {
    if let Some(parent) = el.parent {
        parent.to_string()
    } else if let Some(environment) = el.environment {
        environment.to_string()
    } else {
        "root".to_string()
    }
}

/// View-guarded parent boundaries of one element
/// (`structurizr-diagram.js:710-740`): container on a container/component/
/// dynamic view → the parent software system; component on a component/
/// dynamic view → the parent container and its software system.
pub(super) fn scope_targets<'m>(
    view: View<'_>,
    kind: ElementKind,
    parent: Option<&'m Id>,
    model: &'m Model,
) -> Vec<(BoundaryKind, &'m Id)> {
    let mut out = Vec::new();
    let Some(parent) = parent else {
        return out;
    };
    match kind {
        ElementKind::Container
            if matches!(
                view,
                View::Container(_) | View::Component(_) | View::Dynamic(_)
            ) =>
        {
            out.push((BoundaryKind::SoftwareSystem, parent));
        }
        ElementKind::Component if matches!(view, View::Component(_) | View::Dynamic(_)) => {
            out.push((BoundaryKind::Container, parent));
            if let Some(container) = resolve(model, parent)
                && let Some(ss) = container.parent
            {
                out.push((BoundaryKind::SoftwareSystem, ss));
            }
        }
        _ => {}
    }
    out
}

pub(super) fn boundary_tag(kind: BoundaryKind) -> &'static str {
    match kind {
        BoundaryKind::SoftwareSystem => "software-system",
        BoundaryKind::Container => "container",
        BoundaryKind::Group | BoundaryKind::DeploymentNode => "boundary",
    }
}
