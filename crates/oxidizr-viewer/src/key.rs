//! The key/legend + session tag filter (P3-M1-4b, headless per P3-D1).
//!
//! Three pure computations over a [`Scene`] / [`Workspace`], each
//! Category-A testable with no windowing:
//!
//! - [`legend`] folds a scene into its distinct appearances (element
//!   swatches by kind+shape+colors, edge swatches by color+line style,
//!   boundary entries by kind+border) in deterministic first-appearance
//!   order — the data the shell renders as a key panel.
//! - [`tags`] inventories the model's stored (custom) tags with hit
//!   counts — the chip list of the session tag filter.
//! - [`session_mask`] evaluates the chip selection with
//!   [`oxidizr_core::filter::eval`] against the view's renderable
//!   content, producing the [`Included`] mask `scene::build` consumes.
//!   The selection is SESSION state (P3-V7): the workspace model and
//!   any view definition are never touched, and the mask INTERSECTS the
//!   view's own filter downstream in `scene::build` (P3-V5).

use std::collections::{BTreeMap, BTreeSet};

use oxidizr_core::Workspace;
use oxidizr_core::filter::{self, Included};
use oxidizr_core::model::{DeploymentNode, Model, ModelItem, Relationship};
use oxidizr_core::tags::Tags;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{FilterMode, LineStyle, Shape, View};

use crate::scene::{BoundaryKind, Scene};

/// One element legend row: deduplicated by this exact quadruple.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementKey {
    /// Model kind the swatch stands for.
    pub kind: ElementKind,
    /// Resolved glyph.
    pub shape: Shape,
    /// Swatch fill `#rrggbb`.
    pub background: String,
    /// Swatch outline `#rrggbb`.
    pub stroke: String,
}

/// One edge legend row: deduplicated by color + line style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeKey {
    /// Line color `#rrggbb`.
    pub color: String,
    /// Line style (Solid/Dashed/Dotted).
    pub line_style: LineStyle,
}

/// One boundary legend row: deduplicated by kind + border style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryKey {
    /// What the boundary box represents.
    pub kind: BoundaryKind,
    /// Border style of the box.
    pub border: LineStyle,
}

/// The distinct appearances of one scene (the key panel's model).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Key {
    /// Element swatches, first-appearance order.
    pub elements: Vec<ElementKey>,
    /// Edge swatches, first-appearance order.
    pub edges: Vec<EdgeKey>,
    /// Boundary entries, first-appearance order.
    pub boundaries: Vec<BoundaryKey>,
}

/// Fold `scene` into its [`Key`] (order: items, then edges, then
/// boundaries; duplicates dropped, first occurrence kept).
#[must_use]
pub fn legend(scene: &Scene) -> Key {
    let mut key = Key::default();
    for item in &scene.items {
        push_unique(
            &mut key.elements,
            ElementKey {
                kind: item.kind,
                shape: item.shape,
                background: item.background.clone(),
                stroke: item.stroke.clone(),
            },
        );
    }
    for edge in &scene.edges {
        push_unique(
            &mut key.edges,
            EdgeKey {
                color: edge.color.clone(),
                line_style: edge.line_style,
            },
        );
    }
    for boundary in &scene.boundaries {
        push_unique(
            &mut key.boundaries,
            BoundaryKey {
                kind: boundary.kind,
                border: boundary.border,
            },
        );
    }
    key
}

fn push_unique<T: PartialEq>(rows: &mut Vec<T>, row: T) {
    if !rows.contains(&row) {
        rows.push(row);
    }
}

/// Panel label for a boundary kind (our wording, the reference draws no
/// boundary key at all).
#[must_use]
pub const fn boundary_label(kind: BoundaryKind) -> &'static str {
    match kind {
        BoundaryKind::Group => "Group",
        BoundaryKind::DeploymentNode => "Deployment node",
        BoundaryKind::SoftwareSystem => "Software system",
        BoundaryKind::Container => "Container",
    }
}

/// One tag chip: the tag plus how many model items store it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    /// Stored (custom) tag text, trimmed.
    pub tag: String,
    /// Elements (full tree incl. deployment children and instances)
    /// storing the tag.
    pub elements: usize,
    /// Relationships storing the tag.
    pub relationships: usize,
}

/// Inventory of all stored tags in the model: case-insensitively sorted,
/// ties by exact text — deterministic chip order. Derived tags (kind /
/// `Relationship` defaults) are excluded, matching the reference filter
/// dialog's custom-tag focus.
#[must_use]
pub fn tags(workspace: &Workspace) -> Vec<TagCount> {
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    model_walk(&workspace.model, &mut |item| bump(&mut counts, &item.tags));
    for rel in &workspace.model.relationships {
        bump_rels(&mut counts, rel);
    }
    let mut rows: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, (elements, relationships))| TagCount {
            tag,
            elements,
            relationships,
        })
        .collect();
    rows.sort_by(|a, b| {
        a.tag
            .to_lowercase()
            .cmp(&b.tag.to_lowercase())
            .then_with(|| a.tag.cmp(&b.tag))
    });
    rows
}

/// Count the distinct trimmed tags of one item/relationship (element
/// side of the tuple is bumped by the model walk, relationship side by
/// the caller via [`bump_rels`] — same fold, different counter).
fn bump(counts: &mut BTreeMap<String, (usize, usize)>, tags: &Tags) {
    for tag in distinct_tags(tags) {
        counts.entry(tag).or_default().0 += 1;
    }
}

/// Relationship-side fold of [`bump`].
fn bump_rels(counts: &mut BTreeMap<String, (usize, usize)>, rel: &Relationship) {
    for tag in distinct_tags(&rel.tags) {
        counts.entry(tag).or_default().1 += 1;
    }
}

/// Deduplicated, trimmed, non-empty tag texts.
fn distinct_tags(tags: &Tags) -> Vec<String> {
    let set: BTreeSet<String> = tags
        .stored()
        .iter()
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect();
    set.into_iter().collect()
}

/// Visit EVERY element's [`ModelItem`] in model order: people, systems
/// (+ containers + components), the deployment tree (nodes, children,
/// infrastructure, instances) and custom elements.
fn model_walk(model: &Model, visit: &mut impl FnMut(&ModelItem)) {
    for person in &model.people {
        visit(&person.item);
    }
    for system in &model.software_systems {
        visit(&system.item);
        for container in &system.containers {
            visit(&container.item);
            for component in &container.components {
                visit(&component.item);
            }
        }
    }
    walk_nodes(&model.deployment_nodes, visit);
    for custom in &model.custom_elements {
        visit(&custom.item);
    }
}

fn walk_nodes(nodes: &[DeploymentNode], visit: &mut impl FnMut(&ModelItem)) {
    for node in nodes {
        visit(&node.item);
        for infra in &node.infrastructure_nodes {
            visit(&infra.item);
        }
        for instance in &node.software_system_instances {
            visit(&instance.item);
        }
        for instance in &node.container_instances {
            visit(&instance.item);
        }
        walk_nodes(&node.children, visit);
    }
}

/// Evaluate the session chip selection ([`FilterMode`] + tag list)
/// against the view's renderable content (a filtered view's BASE content
/// — the same resolution `scene::build` performs, so chips and the
/// view's own filter compose the way P3-V5/V7 pinned).
///
/// `None` when the view has no renderable content (unknown key, image
/// view, dangling filtered base): the scene build fails the same way,
/// so no mask is meaningful. An empty tag list is the caller's problem
/// (pass no mask at all then).
#[must_use]
pub fn session_mask(
    workspace: &Workspace,
    view_key: &str,
    mode: FilterMode,
    tags: &[String],
) -> Option<Included> {
    let views = workspace.views.all_views();
    let view = views.iter().find(|v| v.key() == view_key).copied()?;
    // `base_view` is declared here so its borrow outlives `content`
    // (the scene::build pattern); the fallback borrows from `view`,
    // which lives for the whole function.
    let base_view: View;
    let content = match view {
        View::Filtered(f) => {
            base_view = views.iter().find(|v| v.key() == f.base_view_key).copied()?;
            base_view.content()?
        }
        _ => view.content()?,
    };
    Some(filter::eval(workspace, content, Some(mode), tags))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use oxidizr_core::Id;
    use oxidizr_core::model::{ModelItem, Person, Relationship, SoftwareSystem};
    use oxidizr_core::tags::Tags;
    use oxidizr_core::views::{ElementView, SystemLandscapeView, ViewBase, ViewContent};

    fn id(n: u32) -> Id {
        Id::new(n.to_string())
    }

    fn person(n: u32, name: &str, group: Option<&str>, tags: &[&str]) -> Person {
        let mut item = ModelItem::new(id(n), name);
        item.tags = tags_of(tags);
        Person {
            item,
            group: group.map(str::to_owned),
            relationships: Vec::new(),
        }
    }

    fn tags_of(tags: &[&str]) -> Tags {
        let mut tags_ = Tags::new();
        for tag in tags {
            tags_.add(tag);
        }
        tags_
    }

    fn rel(n: u32, from: u32, to: u32, tags: &[&str]) -> Relationship {
        Relationship {
            id: id(n),
            source: id(from),
            destination: id(to),
            description: Some("Uses".to_owned()),
            technology: Some("HTTPS".to_owned()),
            tags: tags_of(tags),
            url: None,
            properties: oxidizr_core::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        }
    }

    /// alice(1) [Live, Team?] grouped, bob(2) untagged, bank(4) [Live];
    /// rels 1→4 [Async] and 2→4 []; landscape L1 with all three.
    fn workspace() -> Workspace {
        let mut ws = Workspace::default();
        ws.model
            .people
            .push(person(1, "Alice", Some("Team"), &["Live"]));
        ws.model.people.push(person(2, "Bob", None, &[]));
        let mut bank = SoftwareSystem {
            item: ModelItem::new(id(4), "Bank"),
            group: None,
            containers: Vec::new(),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        };
        bank.item.tags = tags_of(&["Live"]);
        ws.model.software_systems.push(bank);
        ws.model
            .relationships
            .push(rel(10, 1, 4, &["Live", "Async"]));
        ws.model.relationships.push(rel(11, 2, 4, &[]));
        ws.views.system_landscape_views.push(SystemLandscapeView {
            base: ViewBase::new("L1", 1),
            content: ViewContent {
                elements: vec![
                    ElementView {
                        id: id(1),
                        x: 0,
                        y: 0,
                    },
                    ElementView {
                        id: id(2),
                        x: 400,
                        y: 0,
                    },
                    ElementView {
                        id: id(4),
                        x: 0,
                        y: 400,
                    },
                ],
                relationships: vec![oxidizr_core::views::RelationshipView {
                    id: id(10),
                    description: None,
                    url: None,
                    order: None,
                    response: None,
                    properties: oxidizr_core::properties::Properties::new(),
                    vertices: Vec::new(),
                    routing: None,
                    jump: None,
                    position: None,
                }],
                ..ViewContent::default()
            },
            enterprise_boundary_visible: false,
            animations: Vec::new(),
        });
        ws
    }

    fn scene() -> Scene {
        crate::scene::build(
            &workspace(),
            "L1",
            oxidizr_core::views::ColorScheme::Light,
            None,
        )
        .unwrap()
    }

    #[test]
    fn legend_dedupes_by_appearance_order() {
        let key = legend(&scene());
        // Alice and Bob share one swatch row (same kind + default style);
        // the bank is a second row (different kind).
        assert_eq!(key.elements.len(), 2, "person row + system row");
        assert_eq!(key.elements[0].kind, ElementKind::Person);
        assert_eq!(key.elements[1].kind, ElementKind::SoftwareSystem);
        assert_eq!(key.edges.len(), 1, "one edge in content");
        // alice's group makes one boundary row.
        assert_eq!(key.boundaries.len(), 1);
        assert_eq!(key.boundaries[0].kind, BoundaryKind::Group);
        assert_eq!(boundary_label(BoundaryKind::Group), "Group");
    }

    #[test]
    fn tags_inventory_counts_and_sorts() {
        let rows = tags(&workspace());
        // "Async" (1 relationship), "Live" (2 elements + 1 relationship).
        // Case-insensitive sort puts Async first.
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].tag, "Async");
        assert_eq!((rows[0].elements, rows[0].relationships), (0, 1));
        assert_eq!(rows[1].tag, "Live");
        assert_eq!((rows[1].elements, rows[1].relationships), (2, 1));
    }

    #[test]
    fn session_mask_excludes_and_includes() {
        let ws = workspace();
        let live = vec!["Live".to_owned()];
        let excluded = session_mask(&ws, "L1", FilterMode::Exclude, &live).unwrap();
        // Only Bob survives; BOTH relationships touch the Live-tagged
        // bank, so both die with it (reference keeps only edges whose
        // BOTH ends survive).
        assert_eq!(excluded.element_ids, vec![id(2)]);
        assert_eq!(excluded.relationship_ids, [] as [Id; 0]);
        let included = session_mask(&ws, "L1", FilterMode::Include, &live).unwrap();
        assert_eq!(included.element_ids, vec![id(1), id(4)]);
        assert_eq!(included.relationship_ids, vec![id(10)]);
    }

    #[test]
    fn session_mask_none_for_unrenderable_views() {
        let ws = workspace();
        assert!(session_mask(&ws, "Nope", FilterMode::Exclude, &[]).is_none());
    }

    #[test]
    fn legend_is_deterministic() {
        assert_eq!(legend(&scene()), legend(&scene()));
    }
}
