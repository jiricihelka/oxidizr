//! The official UI text helpers: the element metadata line, terminology
//! lookup, symbol wrapping and view-title fallback, plus the geometry
//! legs the edge pass anchors through (border anchor + placement rect).
//! The public types, the `build` entry and the tests stay in the parent
//! module; element resolution and the boundary shapes live in `resolve`.

use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{ElementView, MetadataSymbols, Terminology, View};
use oxidizr_core::{Id, Workspace};

use super::{Item, Placement, Rect, resolve};

// ---- the official UI text helpers ------------------------------------------------

/// The element metadata line: `getMetadataForElement(element,
/// includeTechnology)` — `symbol + term [+ ": " + technology] + symbol`;
/// custom elements render their free-form `metadata` string instead.
/// `None` for the empty result.
pub(super) fn metadata_line(
    kind: ElementKind,
    technology: Option<&str>,
    custom_metadata: Option<&str>,
    include_technology: bool,
    terminology: &Terminology,
    symbols: MetadataSymbols,
) -> Option<String> {
    if kind == ElementKind::CustomElement {
        return wrap(custom_metadata, symbols);
    }
    let term = terminology_for(kind, terminology);
    if term.is_empty() {
        return None;
    }
    let text = match (include_technology, technology) {
        (true, Some(technology)) if !technology.is_empty() => format!("{term}: {technology}"),
        _ => term,
    };
    wrap(Some(&text), symbols)
}

/// `Workspace.getTerminologyFor` (`structurizr-workspace.js:1161`): the
/// instance kinds share their static kind's term; custom elements have
/// none.
fn terminology_for(kind: ElementKind, t: &Terminology) -> String {
    match kind {
        ElementKind::Person => t.person.clone().unwrap_or_else(|| "Person".to_string()),
        ElementKind::SoftwareSystem | ElementKind::SoftwareSystemInstance => t
            .software_system
            .clone()
            .unwrap_or_else(|| "Software System".to_string()),
        ElementKind::Container | ElementKind::ContainerInstance => t
            .container
            .clone()
            .unwrap_or_else(|| "Container".to_string()),
        ElementKind::Component => t
            .component
            .clone()
            .unwrap_or_else(|| "Component".to_string()),
        ElementKind::DeploymentNode => t
            .deployment_node
            .clone()
            .unwrap_or_else(|| "Deployment Node".to_string()),
        ElementKind::InfrastructureNode => t
            .infrastructure_node
            .clone()
            .unwrap_or_else(|| "Infrastructure Node".to_string()),
        ElementKind::CustomElement => String::new(),
    }
}

/// `openingMetadataSymbols` / `closingMetadataSymbols` wrapping
/// (`structurizr-ui.js:620`); `None`-symbols render the bare text.
pub(super) fn wrap(text: Option<&str>, symbols: MetadataSymbols) -> Option<String> {
    let text = text.filter(|t| !t.is_empty())?;
    let (open, close) = match symbols {
        MetadataSymbols::SquareBrackets => ("[", "]"),
        MetadataSymbols::RoundBrackets => ("(", ")"),
        MetadataSymbols::CurlyBrackets => ("{", "}"),
        MetadataSymbols::AngleBrackets => ("<", ">"),
        MetadataSymbols::DoubleAngleBrackets => ("<<", ">>"),
        MetadataSymbols::None => ("", ""),
    };
    Some(format!("{open}{text}{close}"))
}

/// `getTitleForView` (`structurizr-ui.js:545`): explicit title wins (the
/// filtered view's own, else its base's), else the kind default.
pub(super) fn scene_title(workspace: &Workspace, view: View<'_>, base: Option<View<'_>>) -> String {
    if let Some(title) = view.base().title.as_deref()
        && !title.trim().is_empty()
    {
        return title.to_string();
    }
    let source = base.unwrap_or(view);
    if let Some(title) = source.base().title.as_deref()
        && !title.trim().is_empty()
    {
        return title.to_string();
    }
    default_title(workspace, source)
}

/// `getDefaultViewName` (`structurizr-ui.js:565`); the enterprise suffix
/// and the `name` fallback ride fields our contract does not carry.
fn default_title(workspace: &Workspace, view: View<'_>) -> String {
    let model = &workspace.model;
    let element_name = |id: &Id| resolve(model, id).map(|el| el.item.name.clone());
    match view {
        View::Custom(_) => "Custom View: Untitled".to_string(),
        View::Landscape(_) => "System Landscape View".to_string(),
        View::Context(v) => format!(
            "System Context View{}",
            element_name(&v.software_system_id).map_or(String::new(), |n| format!(": {n}"))
        ),
        View::Container(v) => format!(
            "Container View{}",
            element_name(&v.software_system_id).map_or(String::new(), |n| format!(": {n}"))
        ),
        View::Component(v) => match resolve(model, &v.container_id) {
            Some(container) => match container.parent.and_then(element_name) {
                Some(ss) => format!("Component View: {ss} - {}", container.item.name),
                None => format!("Component View: {}", container.item.name),
            },
            None => "Component View".to_string(),
        },
        View::Dynamic(v) => {
            let scoped = v.element_id.as_ref().and_then(|id| resolve(model, id));
            match scoped {
                Some(el) if el.kind == ElementKind::SoftwareSystem => {
                    format!("Dynamic View: {}", el.item.name)
                }
                Some(el) if el.kind == ElementKind::Container => {
                    match el.parent.and_then(element_name) {
                        Some(ss) => format!("Dynamic View: {ss} - {}", el.item.name),
                        None => format!("Dynamic View: {}", el.item.name),
                    }
                }
                _ => "Dynamic View".to_string(),
            }
        }
        View::Deployment(v) => match v
            .software_system_id
            .as_ref()
            .and_then(|id| resolve(model, id))
        {
            Some(ss) => format!("Deployment View: {} - {}", ss.item.name, v.environment),
            None => format!("Deployment View: {}", v.environment),
        },
        View::Filtered(_) | View::Image(_) => String::new(), // resolved before the call
    }
}

/// The rect an id rendered as, when it became an item.
/// Edge endpoint (spread-aware since): the ray
/// from `rect`'s centre toward `toward` CHOOSES the exit side (the
/// exact `endpoint(None)` rule, so engine-routed views enter the side
/// it chose), but the anchor then sits where `toward` FACES that side
/// — its cross coordinate projected onto the side and clamped to the
/// side's extent. Axis-parallel route legs therefore stay
/// axis-parallel after re-anchoring, which is what re-adds the engine
/// port-spread anchors EXACTLY (an engine leg shares its cross
/// coordinate with the port). Pure centre→centre pairs still meet at
/// the facing midpoints; a degenerate ray (both deltas zero) falls
/// back to the centre.
pub(super) fn border_anchor(rect: Rect, toward: (f64, f64)) -> (f64, f64) {
    let mid = rect.center();
    let (dx, dy) = (toward.0 - mid.0, toward.1 - mid.1);
    let sx = (rect.width / 2.0) / dx.abs();
    let sy = (rect.height / 2.0) / dy.abs();
    if !(sx.is_finite() || sy.is_finite()) {
        return mid;
    }
    let (top, bottom) = (rect.y, rect.y + rect.height);
    let (left, right) = (rect.x, rect.x + rect.width);
    if sx <= sy {
        // The ray exits a VERTICAL side: slide along it to face `toward`.
        let x = if dx >= 0.0 { right } else { left };
        (x, toward.1.clamp(top, bottom))
    } else {
        let y = if dy >= 0.0 { bottom } else { top };
        (toward.0.clamp(left, right), y)
    }
}

pub(super) fn placement_rect(
    placements: &[Option<Placement>],
    elements: &[ElementView],
    items: &[Item],
    id: &Id,
) -> Option<Rect> {
    elements
        .iter()
        .position(|e| e.id == *id)
        .and_then(|idx| placements.get(idx))
        .copied()
        .flatten()
        .and_then(|placement| match placement {
            Placement::Item(i) => Some(items[i].rect),
            Placement::Dn(_) => None,
        })
}
