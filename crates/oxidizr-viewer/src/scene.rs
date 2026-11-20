//! The retained scene (P3-D3): `Workspace` + resolved styles →
//! [`Scene`] layers/items/edges, rebuilt only on model change, layout edit
//! or zoom-band transition — never per frame.
//!
//! [`build`] is the headless heart of the viewer: it turns one view of a
//! workspace into presentation-ready data — no eframe/egui types anywhere
//! (the boundary guard in [`crate`] keeps it that way).
//!
//! Decisions baked in here (§P3-M1, pinned 2026-11-19):
//!
//! - **Geometry** is the stored `x/y` (top-left), except where a session
//!   [`Overrides`] (P3-M2-5 auto-layout) replaces it — build-time only,
//!   the stored workspace is never touched (editor rule, P3-D0).
//!   **Size** is the resolved style's `width`/`height`,
//!   else the [`DEFAULT_ITEM_WIDTH`]×[`DEFAULT_ITEM_HEIGHT`] box: the
//!   reference auto-sizes boxes by measuring text on a canvas
//!   (`structurizr-diagram.js` `calculateWidth`/`calculateHeight`), which
//!   is visual parity — out of contract per P3-D0.
//! - **Text is kept as plain runs** (name, description and metadata
//!   strings plus the font size). The decomposition sketch allowed
//!   `epaint` layout jobs; deferring them to the M1-4 paint step keeps
//!   this module (and the whole lib core) free of the egui stack —
//!   recorded on the §P3-M1 M1-2 row.
//! - **Styles resolve through [`core`](oxidizr_core::styles) only**
//!   (P2-D2): `element_style`/`relationship_style` guarantee
//!   background/stroke/color after the scheme defaults.
//! - **Metadata lines and terminology mirror the reference UI**:
//!   `structurizr.ui.getMetadataForElement` (`structurizr-ui.js:638`, with
//!   the technology suffix) / `getMetadataForRelationship` /
//!   `Workspace.getTerminologyFor` (`structurizr-workspace.js:1161` — the
//!   instance kinds share their static kind's term) and the
//!   `metadataSymbols` wrapping (default `SquareBrackets`,
//!   `structurizr-workspace.js:735`).
//! - **`DeploymentNode` elements are boundaries, not items** — the
//!   reference skips them in the element loop (`structurizr-diagram.js:558`)
//!   and embeds their children instead. Group boundaries follow the same
//!   scoping as `includeGroup` (`structurizr-diagram.js:663`): scope is the
//!   model parent, else the environment (top-level deployment nodes), else
//!   the root scope. (The reference's `Internal`/`External` scope split and
//!   the enterprise boundary ride the `location`/`enterprise` fields, which
//!   our byte contract does not carry — a single root scope is used and no
//!   enterprise boundary is rendered.)
//! - **Filtered views resolve inside [`build`]**: the base view's content
//!   is evaluated with [`oxidizr_core::filter::eval`] (P3-V5). The `mask`
//!   parameter is the session tag filter (P3-V7: session-only, the model
//!   file is never touched) pre-evaluated by the caller; it INTERSECTS the
//!   view's own filter.
//! - **Determinism**: items and edges follow content order; boundaries are
//!   ordered by their first member's content position (ties by id). Same
//!   input → identical [`Scene`] (it derives [`PartialEq`]).
//! - **Stale ids never fail the build**: unresolvable elements/
//!   relationships are skipped (nothing to render or route).

use std::collections::HashMap;

use oxidizr_core::filter::Included;
use oxidizr_core::model::{DeploymentNode, Model, ModelItem};
use oxidizr_core::styles::{
    DEFAULT_BACKGROUND_DARK, DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_DARK, DEFAULT_COLOR_LIGHT,
    instance_tag_chain,
};
use oxidizr_core::tags::TagKind;
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{
    ColorScheme, ElementView, IconPosition, LineStyle, MetadataSymbols, Shape, Terminology, View,
};
use oxidizr_core::{Id, Workspace};

/// Reference `LIGHT_MODE_DEFAULTS.strokeWidth` / `DARK_MODE_DEFAULTS`
/// (`structurizr-ui.js:15`): the element/boundary stroke width when the
/// resolved style leaves `strokeWidth` unset.
pub const DEFAULT_STROKE_WIDTH: i32 = 2;
/// Box width when the resolved style sets no `width` (see module docs).
pub const DEFAULT_ITEM_WIDTH: f64 = 350.0;
/// Box height when the resolved style sets no `height` (see module docs).
pub const DEFAULT_ITEM_HEIGHT: f64 = 150.0;
/// Padding a boundary's member union grows by (chrome of ours — the
/// reference computes boundary insets from its own layout passes).
pub const BOUNDARY_PADDING: f64 = 20.0;
/// Margin around the [`Scene::bounds`] union.
pub const SCENE_MARGIN: f64 = 20.0;

/// An axis-aligned rectangle in world (stored-layout) coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width (≥ 0 by construction).
    pub width: f64,
    /// Height (≥ 0 by construction).
    pub height: f64,
}

impl Rect {
    /// Smallest rect covering both (both are well-formed).
    #[must_use]
    pub fn union(self, other: Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Rect {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }

    /// Grown by `pad` on every side.
    #[must_use]
    pub fn expand(self, pad: f64) -> Rect {
        Rect {
            x: self.x - pad,
            y: self.y - pad,
            width: self.width + 2.0 * pad,
            height: self.height + 2.0 * pad,
        }
    }

    /// Geometric centre (the edge anchor point, like the reference).
    #[must_use]
    pub fn center(self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Inclusive containment of a point (hit-test semantics: edges of the
    /// box count, matching the reference's `includePoints` boxes).
    #[must_use]
    pub fn contains(self, point: (f64, f64)) -> bool {
        point.0 >= self.x
            && point.0 <= self.x + self.width
            && point.1 >= self.y
            && point.1 <= self.y + self.height
    }
}

/// Fold an iterator of rects; empty in, `None` out.
fn union_all(rects: impl Iterator<Item = Rect>) -> Option<Rect> {
    rects.reduce(Rect::union)
}

/// One rendered model element (reference: the `structurizr.box` cell).
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Model element id.
    pub id: Id,
    /// Model kind (paint-shape selection + tooltips).
    pub kind: ElementKind,
    /// Element name (the bold box title).
    pub name: String,
    /// Description line — `None` when absent or the style hides it
    /// (`style.description == Some(false)`).
    pub description: Option<String>,
    /// Metadata line, e.g. `[Person]` or `[Container: Spring Boot]` —
    /// `None` when empty or the style hides it (`style.metadata`).
    pub metadata: Option<String>,
    /// Group membership (boundary association).
    pub group: Option<String>,
    /// Click-through URL (`None` = no link).
    pub url: Option<String>,
    /// Box geometry (stored `x/y` + style or default size).
    pub rect: Rect,
    /// Resolved shape (default `Box`).
    pub shape: Shape,
    /// Resolved background `#rrggbb` (opacity shading stays a paint
    /// concern — `shadeColor` equivalent lives in M1-4).
    pub background: String,
    /// Resolved stroke `#rrggbb` (auto = darker background per P2-D2).
    pub stroke: String,
    /// Stroke width px (reference default 2).
    pub stroke_width: i32,
    /// Border style (Solid/Dashed/Dotted).
    pub border: LineStyle,
    /// Text color `#rrggbb`.
    pub color: String,
    /// Font size px (reference default 24).
    pub font_size: i32,
    /// Opacity percent (0-100).
    pub opacity: i32,
    /// Icon reference from the style (resolved by the M1-5 atlas).
    pub icon: Option<String>,
    /// Icon slot within the box.
    pub icon_position: IconPosition,
}

/// What a [`Boundary`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryKind {
    /// Group box (`element.group`).
    Group,
    /// Deployment node (never an item — see module docs).
    DeploymentNode,
    /// Parent software system around containers (container/dynamic views).
    SoftwareSystem,
    /// Parent container around components (component/dynamic views).
    Container,
}

/// A boundary box behind the items (reference `createBoundary` /
/// `createDeploymentNode` / group embedding).
#[derive(Debug, Clone, PartialEq)]
pub struct Boundary {
    /// What this boundary is.
    pub kind: BoundaryKind,
    /// Stable identity: `group:<scope>/<name>`, `deployment:<id>`,
    /// `software-system:<id>`, `container:<id>`.
    pub id: String,
    /// Box label (group name or element name).
    pub label: String,
    /// Metadata line under the label — `None` for groups (the reference
    /// `createBoundaryForGroup` passes no metadata).
    pub metadata: Option<String>,
    /// Union of the member rects + [`BOUNDARY_PADDING`].
    pub rect: Rect,
    /// Resolved background `#rrggbb` (Boundary tag chain, theme-stylable).
    pub background: String,
    /// Resolved stroke `#rrggbb`.
    pub stroke: String,
    /// Stroke width px.
    pub stroke_width: i32,
    /// Border style.
    pub border: LineStyle,
    /// Label text color `#rrggbb`.
    pub color: String,
    /// Label font size px.
    pub font_size: i32,
    /// Opacity percent.
    pub opacity: i32,
}

/// One rendered relationship (reference: the `structurizr.arrow` cell).
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Model relationship id.
    pub id: Id,
    /// Source element id.
    pub source: Id,
    /// Destination element id.
    pub destination: Id,
    /// Polyline: source box centre → stored vertices → destination centre.
    pub points: Vec<(f64, f64)>,
    /// Label (the relationship description; the relationship view's
    /// `description` overrides the model's). `None` when absent or the
    /// style hides descriptions.
    pub label: Option<String>,
    /// Technology metadata, e.g. `[HTTPS]` (`getMetadataForRelationship`).
    pub metadata: Option<String>,
    /// Dynamic-step number (the shell draws the small circle).
    pub order: Option<String>,
    /// Line color `#rrggbb`.
    pub color: String,
    /// Line thickness px (reference default 2).
    pub thickness: i32,
    /// Line style (Solid/Dashed/Dotted).
    pub line_style: LineStyle,
    /// Label font size px.
    pub font_size: i32,
    /// Label wrap width px (resolved style `width`, default 200).
    pub label_width: i32,
    /// Label position percent along the line (default 50).
    pub label_position: i32,
    /// Opacity percent.
    pub opacity: i32,
    /// Click-through URL (view override, then model).
    pub url: Option<String>,
}

/// The retained scene graph of one view (P3-D3).
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Key of the rendered view (a filtered view's own key when one was
    /// resolved).
    pub view_key: String,
    /// View title (`getTitleForView` mirror: explicit title, else the
    /// kind-derived default name).
    pub title: String,
    /// Canvas background for the scheme (the reference
    /// `LIGHT/DARK_MODE_DEFAULTS.background` — same values as the core
    /// style defaults).
    pub background: String,
    /// Boundary boxes, bottom layer first (first-member content order).
    pub boundaries: Vec<Boundary>,
    /// Element boxes in content order.
    pub items: Vec<Item>,
    /// Edges in content order.
    pub edges: Vec<Edge>,
    /// Union of boundaries, items and edge points + [`SCENE_MARGIN`]
    /// (the zero rect when the scene is empty).
    pub bounds: Rect,
}

/// Session auto-layout geometry (P3-M2-5): what the [`layout`] module
/// produced for the current view, overlaid on the stored values at
/// [`build`] time. NEVER persisted (the editor rule, P3-D0: session
/// geometry only until M3 owns saving) and per-view (the shell clears
/// it on every view change).
///
/// [`layout`]: crate::layout
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Overrides {
    /// Element id → box TOP-LEFT (world px) replacing the stored
    /// `x/y`. Deployment-node ids never appear: those elements are
    /// boundaries whose rects follow their members.
    pub elements: HashMap<Id, (f64, f64)>,
    /// Relationship id → INTERIOR waypoints. The scene edge polyline is
    /// `[source centre, …waypoints…, destination centre]`, so the stored
    /// relationship-view vertices are REPLACED wholesale — an empty vec
    /// straightens the edge.
    pub relationships: HashMap<Id, Vec<(f64, f64)>>,
}

/// Why a scene could not be built. Bad model ids are never an error here
/// (they are skipped) — only un-renderable views are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneError {
    /// No view with this key.
    #[error("no view with key `{key}` in the workspace")]
    ViewNotFound {
        /// The requested key.
        key: String,
    },
    /// The view has no model content (`ImageView`).
    #[error("view `{key}` ({kind}) has no renderable content")]
    NotRenderable {
        /// The requested key.
        key: String,
        /// Human view kind.
        kind: String,
    },
    /// Filtered view whose `baseViewKey` resolves to nothing.
    #[error("filtered view `{key}` references unknown base view `{base}`")]
    BaseViewNotFound {
        /// The filtered view key.
        key: String,
        /// The dangling base key.
        base: String,
    },
    /// Filtered view whose base is an `ImageView`.
    #[error("base view `{base}` of filtered view `{key}` ({kind}) has no renderable content")]
    BaseViewNotRenderable {
        /// The filtered view key.
        key: String,
        /// The base view key.
        base: String,
        /// Human view kind.
        kind: String,
    },
}

/// Build the scene of `view_key` in `color_scheme`.
///
/// `mask` is the optional session tag-filter inclusion ([`Included`],
/// pre-evaluated by the caller with [`oxidizr_core::filter::eval`]); it
/// INTERSECTS the view's own filter (a filtered view is resolved against
/// its base here, P3-V5). `overrides` is the optional session auto-layout
/// geometry ([`Overrides`], P3-M2-5): element top-lefts and relationship
/// interior waypoints OVERLAID on the stored values — boundaries follow
/// their members automatically. Geometry resolution rules: module docs.
///
/// # Errors
///
/// [`SceneError::ViewNotFound`] for an unknown key;
/// [`SceneError::NotRenderable`] for views without model content
/// (`ImageView`); [`SceneError::BaseViewNotFound`] /
/// [`SceneError::BaseViewNotRenderable`] for filtered views with a dangling
/// or un-renderable `baseViewKey`.
// The whole pipeline lives in one read: content pass → embedding → boundary
// emission → edges → bounds; the stage order IS the contract (module docs).
#[allow(clippy::too_many_lines)]
pub fn build(
    workspace: &Workspace,
    view_key: &str,
    color_scheme: ColorScheme,
    mask: Option<&Included>,
    overrides: Option<&Overrides>,
) -> Result<Scene, SceneError> {
    let views = workspace.views.all_views();
    let Some(view) = views.iter().find(|v| v.key() == view_key).copied() else {
        return Err(SceneError::ViewNotFound {
            key: view_key.to_string(),
        });
    };

    // Filtered view: resolve the base (reference `findViewByKey`); its
    // content is the render target and its filter evaluates first (P3-V5).
    // `base_view` is declared here so its borrow outlives `content`.
    let base_view: View;
    let (rendered, content, filtered, title_base) = match view {
        View::Filtered(f) => {
            let Some(found) = views.iter().find(|v| v.key() == f.base_view_key).copied() else {
                return Err(SceneError::BaseViewNotFound {
                    key: view_key.to_string(),
                    base: f.base_view_key.clone(),
                });
            };
            base_view = found;
            let Some(content) = base_view.content() else {
                return Err(SceneError::BaseViewNotRenderable {
                    key: view_key.to_string(),
                    base: f.base_view_key.clone(),
                    kind: base_view.kind_label().to_string(),
                });
            };
            (found, content, Some(f), Some(base_view))
        }
        other => {
            let Some(content) = view.content() else {
                return Err(SceneError::NotRenderable {
                    key: view_key.to_string(),
                    kind: view.kind_label().to_string(),
                });
            };
            (other, content, None, None)
        }
    };

    let own_filter =
        filtered.map(|f| oxidizr_core::filter::eval(workspace, content, f.mode, &f.tags));
    let keep_element = |id: &Id| {
        own_filter
            .as_ref()
            .is_none_or(|i| i.element_ids.contains(id))
            && mask.is_none_or(|m| m.element_ids.contains(id))
    };
    let keep_relationship = |id: &Id| {
        own_filter
            .as_ref()
            .is_none_or(|i| i.relationship_ids.contains(id))
            && mask.is_none_or(|m| m.relationship_ids.contains(id))
    };

    let model = &workspace.model;
    let styles = &workspace.views.configuration.styles;
    let terminology = &workspace.views.configuration.terminology;
    let symbols = workspace
        .views
        .configuration
        .metadata_symbols
        .unwrap_or(MetadataSymbols::SquareBrackets);
    let default_background = if color_scheme == ColorScheme::Dark {
        DEFAULT_BACKGROUND_DARK
    } else {
        DEFAULT_BACKGROUND_LIGHT
    };
    let default_color = if color_scheme == ColorScheme::Dark {
        DEFAULT_COLOR_DARK
    } else {
        DEFAULT_COLOR_LIGHT
    };

    // Container-view legality (`removeIllegalElements`,
    // `structurizr-diagram.js:3401`): a foreign container outranks its
    // parent software system — the parent leaves the diagram.
    let mut removed: Vec<&Id> = Vec::new();
    if let View::Container(v) = rendered {
        for element in &content.elements {
            let Some(el) = resolve(model, &element.id) else {
                continue;
            };
            if el.kind == ElementKind::Container
                && let Some(parent) = el.parent
                && parent != &v.software_system_id
            {
                removed.push(parent);
            }
        }
    }

    // Boundary accumulators; `order` = first member's content index.
    let mut dns: Vec<DnSpec> = Vec::new();
    let mut dn_pos: HashMap<&Id, usize> = HashMap::new();
    let mut groups: Vec<GroupSpec> = Vec::new();
    let mut group_pos: HashMap<(String, String), usize> = HashMap::new();
    let mut scopes: Vec<ScopeSpec> = Vec::new();
    let mut scope_pos: HashMap<String, usize> = HashMap::new();

    let mut items: Vec<Item> = Vec::new();
    // Parallel to `content.elements`: what each entry rendered as.
    let mut placements: Vec<Option<Placement>> = Vec::with_capacity(content.elements.len());

    for (idx, element) in content.elements.iter().enumerate() {
        let id = &element.id;
        if removed.contains(&id) || !keep_element(id) {
            placements.push(None);
            continue;
        }
        let Some(el) = resolve(model, id) else {
            placements.push(None); // stale id: nothing to render
            continue;
        };
        let tags = tags_csv(&el);
        let style = styles.element_style(&tags, color_scheme);
        let placement = if el.kind == ElementKind::DeploymentNode {
            // Reference: `continue` in the element loop
            // (`structurizr-diagram.js:558`) — the node becomes a boundary
            // via its embedded children instead.
            let spec = DnSpec {
                id: id.clone(),
                label: el.item.name.clone(),
                metadata: metadata_line(el.kind, None, None, false, terminology, symbols),
                tags,
                order: idx,
            };
            let dn_idx = dns.len();
            dn_pos.insert(id, dn_idx);
            dns.push(spec);
            Placement::Dn(dn_idx)
        } else {
            // Session auto-layout wins over the stored top-left (P3-M2-5);
            // sizes stay style-resolved either way.
            let (x, y) = overrides
                .and_then(|o| o.elements.get(id))
                .map_or((f64::from(element.x), f64::from(element.y)), |p| *p);
            let rect = Rect {
                x,
                y,
                width: style
                    .width
                    .map_or(DEFAULT_ITEM_WIDTH, |w| f64::from(w.max(1))),
                height: style
                    .height
                    .map_or(DEFAULT_ITEM_HEIGHT, |h| f64::from(h.max(1))),
            };
            let item_idx = items.len();
            items.push(Item {
                id: id.clone(),
                kind: el.kind,
                name: el.item.name.clone(),
                description: if style.description.unwrap_or(true) {
                    el.item
                        .description
                        .as_deref()
                        .filter(|d| !d.is_empty())
                        .map(str::to_string)
                } else {
                    None
                },
                metadata: if style.metadata.unwrap_or(true) {
                    metadata_line(
                        el.kind,
                        el.technology,
                        el.custom_metadata,
                        true,
                        terminology,
                        symbols,
                    )
                } else {
                    None
                },
                group: el.group.map(str::to_string),
                url: el.item.url.clone(),
                rect,
                shape: style.shape.unwrap_or(Shape::Box),
                background: style
                    .background
                    .unwrap_or_else(|| default_background.to_string()),
                stroke: style.stroke.unwrap_or_else(|| default_color.to_string()),
                stroke_width: style.stroke_width.unwrap_or(DEFAULT_STROKE_WIDTH),
                border: style.border.unwrap_or(LineStyle::Solid),
                color: style.color.unwrap_or_else(|| default_color.to_string()),
                font_size: style.font_size.unwrap_or(24).max(1),
                opacity: style.opacity.unwrap_or(100).clamp(0, 100),
                icon: style.icon,
                icon_position: style.icon_position.unwrap_or(IconPosition::Top),
            });
            Placement::Item(item_idx)
        };
        placements.push(Some(placement));

        // Group membership (`includeGroup`, reference `findOrCreateGroup`).
        if let Some(group) = el.group {
            let scope = group_scope(&el);
            let key = (scope.clone(), group.to_string());
            let pos = if let Some(pos) = group_pos.get(&key) {
                *pos
            } else {
                groups.push(GroupSpec {
                    id: format!("group:{scope}/{group}"),
                    label: group.to_string(),
                    tags: format!(
                        "Element,Boundary,Boundary:Enterprise,Boundary:SoftwareSystem,\
                             Boundary:Container,Group,Group:{group}"
                    ),
                    order: idx,
                    items: Vec::new(),
                    dns: Vec::new(),
                });
                group_pos.insert(key, groups.len() - 1);
                groups.len() - 1
            };
            match placement {
                Placement::Item(i) => groups[pos].items.push(i),
                Placement::Dn(d) => groups[pos].dns.push(d),
            }
        }

        // Parent-scope boundaries (the reference's view-type-guarded
        // `addElementToBoundary` calls, `structurizr-diagram.js:710-740`).
        for (kind, boundary_id) in scope_targets(rendered, el.kind, el.parent, model) {
            let key = format!("{}:{boundary_id}", boundary_tag(kind));
            let pos = if let Some(pos) = scope_pos.get(&key) {
                *pos
            } else {
                let Some(owner) = resolve(model, boundary_id) else {
                    continue; // unresolvable parent: no boundary
                };
                scopes.push(ScopeSpec {
                    id: key.clone(),
                    kind,
                    label: owner.item.name.clone(),
                    metadata: metadata_line(owner.kind, None, None, false, terminology, symbols),
                    tags: format!(
                        "{},Boundary,Boundary:Enterprise,Boundary:SoftwareSystem,\
                             Boundary:Container",
                        tags_csv(&owner)
                    ),
                    order: idx,
                    items: Vec::new(),
                });
                scope_pos.insert(key, scopes.len() - 1);
                scopes.len() - 1
            };
            if let Placement::Item(i) = placement {
                scopes[pos].items.push(i);
            }
        }
    }

    // Deployment-node embedding: direct model children on the diagram
    // (reference `createDeploymentNode` + the second embed pass).
    let mut dn_child_items: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut dn_child_dns: HashMap<usize, Vec<usize>> = HashMap::new();
    for (element_idx, placement) in placements.iter().enumerate() {
        let Some(placement) = placement else {
            continue;
        };
        let Some(el) = resolve(model, &content.elements[element_idx].id) else {
            continue;
        };
        let Some(parent) = el.parent else {
            continue;
        };
        let Some(&parent_idx) = dn_pos.get(parent) else {
            continue;
        };
        match placement {
            Placement::Item(i) => dn_child_items.entry(parent_idx).or_default().push(*i),
            Placement::Dn(d) => dn_child_dns.entry(parent_idx).or_default().push(*d),
        }
    }

    // Bottom-up deployment-node rects (nesting is acyclic: children live
    // inside their parent's struct in the model).
    let mut dn_rects: Vec<Option<Rect>> = vec![None; dns.len()];
    let mut dn_done: Vec<bool> = vec![false; dns.len()];
    for idx in 0..dns.len() {
        dn_rect(
            idx,
            &dn_child_items,
            &dn_child_dns,
            &items,
            &mut dn_rects,
            &mut dn_done,
        );
    }

    // Boundaries, ordered by first member then id; `placement_rect`-free
    // — every rect was accumulated from member indices.
    let paint = Paint {
        styles,
        color_scheme,
        default_background,
        default_color,
    };
    let mut pending: Vec<(usize, Boundary)> = Vec::new();
    for (dn_idx, dn) in dns.iter().enumerate() {
        let Some(rect) = dn_rects[dn_idx] else {
            continue; // memberless node: nothing to draw around
        };
        pending.push((
            dn.order,
            boundary_from(
                &paint,
                BoundaryKind::DeploymentNode,
                format!("deployment:{}", dn.id),
                dn.label.clone(),
                dn.metadata.clone(),
                &dn.tags,
                rect,
            ),
        ));
    }
    for group in &groups {
        let members = group
            .items
            .iter()
            .map(|i| items[*i].rect)
            .chain(group.dns.iter().filter_map(|d| dn_rects[*d]));
        let Some(rect) = union_all(members) else {
            continue;
        };
        pending.push((
            group.order,
            boundary_from(
                &paint,
                BoundaryKind::Group,
                group.id.clone(),
                group.label.clone(),
                None,
                &group.tags,
                rect,
            ),
        ));
    }
    for scope in &scopes {
        let Some(rect) = union_all(scope.items.iter().map(|i| items[*i].rect)) else {
            continue;
        };
        pending.push((
            scope.order,
            boundary_from(
                &paint,
                scope.kind,
                scope.id.clone(),
                scope.label.clone(),
                scope.metadata.clone(),
                &scope.tags,
                rect,
            ),
        ));
    }
    pending.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let boundaries: Vec<Boundary> = pending.into_iter().map(|(_, b)| b).collect();

    // Edges: source centre → stored vertices → destination centre.
    let mut edges: Vec<Edge> = Vec::new();
    for rv in &content.relationships {
        if !keep_relationship(&rv.id) {
            continue;
        }
        let Some(rel) = model.relationships.iter().find(|r| r.id == rv.id) else {
            continue; // stale id: nothing to route
        };
        let source = placement_rect(&placements, &content.elements, &items, &rel.source);
        let destination = placement_rect(&placements, &content.elements, &items, &rel.destination);
        let (Some(source), Some(destination)) = (source, destination) else {
            continue; // endpoint not an item (stale, filtered, deployment node)
        };
        let style = styles.relationship_style(rel, model, color_scheme);
        let mut points = vec![source.center()];
        // Session auto-layout replaces the stored vertices wholesale
        // (P3-M2-5) — an empty override vec straightens the edge.
        match overrides.and_then(|o| o.relationships.get(&rv.id)) {
            Some(waypoints) => points.extend(waypoints.iter().copied()),
            None => points.extend(rv.vertices.iter().map(|v| (f64::from(v.x), f64::from(v.y)))),
        }
        points.push(destination.center());
        edges.push(Edge {
            id: rv.id.clone(),
            source: rel.source.clone(),
            destination: rel.destination.clone(),
            points,
            label: if style.description.unwrap_or(true) {
                rv.description
                    .as_deref()
                    .filter(|d| !d.trim().is_empty())
                    .or(rel.description.as_deref().filter(|d| !d.trim().is_empty()))
                    .map(str::to_string)
            } else {
                None
            },
            metadata: if style.metadata.unwrap_or(true) {
                wrap(rel.technology.as_deref(), symbols)
            } else {
                None
            },
            order: rv.order.clone(),
            color: style.color.unwrap_or_else(|| default_color.to_string()),
            thickness: style.thickness.unwrap_or(2).max(1),
            line_style: style.style.unwrap_or(LineStyle::Dashed),
            font_size: style.font_size.unwrap_or(24).max(1),
            label_width: style.width.unwrap_or(200).max(1),
            label_position: style.position.unwrap_or(50),
            opacity: style.opacity.unwrap_or(100).clamp(0, 100),
            url: rv.url.clone().or_else(|| rel.url.clone()),
        });
    }

    let bounds = union_all(
        items
            .iter()
            .map(|i| i.rect)
            .chain(boundaries.iter().map(|b| b.rect))
            .chain(edges.iter().flat_map(|e| {
                e.points.iter().map(|p| Rect {
                    x: p.0,
                    y: p.1,
                    width: 0.0,
                    height: 0.0,
                })
            })),
    )
    .map_or(
        Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        },
        |r| r.expand(SCENE_MARGIN),
    );

    Ok(Scene {
        view_key: view_key.to_string(),
        title: scene_title(workspace, view, title_base),
        background: default_background.to_string(),
        boundaries,
        items,
        edges,
        bounds,
    })
}

// ---- scene-local element resolution ---------------------------------------

/// A model element resolved for rendering: identity + the fields the
/// reference draws (the `filter` module's own resolve is tag-only).
struct Resolved<'m> {
    kind: ElementKind,
    item: &'m ModelItem,
    technology: Option<&'m str>,
    group: Option<&'m str>,
    custom_metadata: Option<&'m str>,
    parent: Option<&'m Id>,
    environment: Option<&'m str>,
    /// Instance → static element (tag-chain prepending, reference
    /// `findElementStyle`).
    static_element: Option<(&'m ModelItem, ElementKind)>,
}

/// Resolve one id across the whole model (hydrate-order walk).
fn resolve<'m>(model: &'m Model, id: &Id) -> Option<Resolved<'m>> {
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

fn resolve_node<'m>(
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

/// The element's full style tag chain as CSV — the reference
/// `getTags()` (`Tags::serialized_for`), with the instance prepending
/// (`instance_tag_chain`, reference `Styles.findElementStyle`).
fn tags_csv(el: &Resolved<'_>) -> String {
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
enum Placement {
    Item(usize),
    Dn(usize),
}

struct DnSpec {
    id: Id,
    label: String,
    metadata: Option<String>,
    tags: String,
    order: usize,
}

struct GroupSpec {
    id: String,
    label: String,
    tags: String,
    order: usize,
    items: Vec<usize>,
    dns: Vec<usize>,
}

struct ScopeSpec {
    id: String,
    kind: BoundaryKind,
    label: String,
    metadata: Option<String>,
    tags: String,
    order: usize,
    items: Vec<usize>,
}

/// The style/scheme bundle every boundary is painted with (the argument
/// bundle keeps [`boundary_from`] within the pedantic budget).
struct Paint<'a> {
    styles: &'a oxidizr_core::views::Styles,
    color_scheme: ColorScheme,
    default_background: &'a str,
    default_color: &'a str,
}

/// One boundary: member union + padding, style resolved through the
/// Boundary tag chain.
fn boundary_from(
    paint: &Paint<'_>,
    kind: BoundaryKind,
    id: String,
    label: String,
    metadata: Option<String>,
    tags: &str,
    rect: Rect,
) -> Boundary {
    let style = paint.styles.element_style(tags, paint.color_scheme);
    Boundary {
        kind,
        id,
        label,
        metadata,
        rect: rect.expand(BOUNDARY_PADDING),
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

/// Memoized bottom-up deployment-node rect (reference: the deployment-node
/// cells embed child cells, then the second pass embeds nodes into their
/// parent cell).
fn dn_rect(
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

/// Reference `includeGroup` scoping (`structurizr-diagram.js:663`): the
/// model parent, else the environment (top-level deployment nodes), else
/// the single root scope (the `location` split is not in the contract).
fn group_scope(el: &Resolved<'_>) -> String {
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
fn scope_targets<'m>(
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

fn boundary_tag(kind: BoundaryKind) -> &'static str {
    match kind {
        BoundaryKind::SoftwareSystem => "software-system",
        BoundaryKind::Container => "container",
        BoundaryKind::Group | BoundaryKind::DeploymentNode => "boundary",
    }
}

// ---- reference text helpers ------------------------------------------------

/// The element metadata line: `getMetadataForElement(element,
/// includeTechnology)` — `symbol + term [+ ": " + technology] + symbol`;
/// custom elements render their free-form `metadata` string instead.
/// `None` for the empty result.
fn metadata_line(
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
fn wrap(text: Option<&str>, symbols: MetadataSymbols) -> Option<String> {
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
fn scene_title(workspace: &Workspace, view: View<'_>, base: Option<View<'_>>) -> String {
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
fn placement_rect(
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use oxidizr_core::model::{Component, Container, Person, Relationship, SoftwareSystem};
    use oxidizr_core::views::{
        ComponentView, ContainerView, DeploymentView, DynamicView, FilteredView, ImageView,
        RelationshipView, SystemLandscapeView, Vertex, ViewBase, ViewContent,
    };
    fn id(n: u32) -> Id {
        Id::new(n.to_string())
    }

    fn element(n: u32, x: i32, y: i32) -> ElementView {
        ElementView { id: id(n), x, y }
    }

    fn rel_view(n: u32) -> RelationshipView {
        RelationshipView {
            id: id(n),
            description: None,
            url: None,
            order: None,
            response: None,
            properties: oxidizr_core::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        }
    }

    fn rel(n: u32, from: u32, to: u32, description: &str, technology: &str) -> Relationship {
        Relationship {
            id: id(n),
            source: id(from),
            destination: id(to),
            description: Some(description.to_string()),
            technology: Some(technology.to_string()),
            tags: oxidizr_core::tags::Tags::new(),
            url: None,
            properties: oxidizr_core::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        }
    }

    fn person(n: u32, name: &str, group: Option<&str>) -> Person {
        Person {
            item: ModelItem::new(id(n), name),
            group: group.map(str::to_string),
            relationships: Vec::new(),
        }
    }

    fn component(n: u32, name: &str, technology: &str) -> Component {
        Component {
            item: ModelItem::new(id(n), name),
            group: None,
            technology: Some(technology.to_string()),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn container(n: u32, name: &str, technology: &str, group: Option<&str>) -> Container {
        Container {
            item: ModelItem::new(id(n), name),
            group: group.map(str::to_string),
            technology: Some(technology.to_string()),
            components: Vec::new(),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn landscape(content: ViewContent) -> SystemLandscapeView {
        SystemLandscapeView {
            base: ViewBase::new("L1", 1),
            content,
            enterprise_boundary_visible: false,
            animations: Vec::new(),
        }
    }

    /// Fixture model: `alice(1)`, `bob(2)` + `carol(3)` [group Team],
    /// `bank(4)` / `web(5, Spring Boot)` / `home(6, Java)`,
    /// `mail(7)` / `smtp(8)`; deployment `aws(9) > eks(10) > lb(11) +
    /// ci(12)`; rels `1→4 (Uses/HTTPS)`, `4→5 (Includes/HTTP)`,
    /// `11→12 (routes/TCP)`.
    fn workspace() -> Workspace {
        let mut ws = Workspace::default();
        let home = component(6, "Homepage", "Java");
        let mut web = container(5, "Web Application", "Spring Boot", None);
        web.components.push(home);
        let bank = SoftwareSystem {
            item: ModelItem::new(id(4), "Internet Banking"),
            group: None,
            containers: vec![web],
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        };
        let smtp = container(8, "SMTP", "Postfix", None);
        let mail = SoftwareSystem {
            item: ModelItem::new(id(7), "E-mail"),
            group: None,
            containers: vec![smtp],
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        };
        let mut ci = oxidizr_core::model::ContainerInstance {
            item: ModelItem::new(id(12), "Web Application Instance"),
            group: None,
            environment: "Default".to_string(),
            container_id: id(5),
            instance_id: 1,
            deployment_groups: Vec::new(),
            health_checks: Vec::new(),
            relationships: Vec::new(),
        };
        ci.item.description = None;
        let lb = oxidizr_core::model::InfrastructureNode {
            item: ModelItem::new(id(11), "Load Balancer"),
            group: None,
            environment: "Default".to_string(),
            technology: Some("nginx".to_string()),
            relationships: Vec::new(),
        };
        let eks = DeploymentNode {
            item: ModelItem::new(id(10), "EKS"),
            group: None,
            environment: "Default".to_string(),
            technology: Some("Kubernetes".to_string()),
            instances: "1".to_string(),
            deployment_groups: Vec::new(),
            children: Vec::new(),
            infrastructure_nodes: vec![lb],
            software_system_instances: Vec::new(),
            container_instances: vec![ci],
            relationships: Vec::new(),
        };
        let aws = DeploymentNode {
            item: ModelItem::new(id(9), "AWS"),
            group: None,
            environment: "Default".to_string(),
            technology: None,
            instances: "1".to_string(),
            deployment_groups: Vec::new(),
            children: vec![eks],
            infrastructure_nodes: Vec::new(),
            software_system_instances: Vec::new(),
            container_instances: Vec::new(),
            relationships: Vec::new(),
        };
        ws.model = Model {
            custom_elements: Vec::new(),
            people: vec![
                person(1, "Alice", None),
                person(2, "Bob", Some("Team")),
                person(3, "Carol", Some("Team")),
            ],
            software_systems: vec![bank, mail],
            deployment_nodes: vec![aws],
            relationships: vec![
                rel(20, 1, 4, "Uses", "HTTPS"),
                rel(21, 4, 5, "Includes", "HTTP"),
                rel(22, 11, 12, "routes", "TCP"),
            ],
            properties: oxidizr_core::properties::Properties::new(),
        };
        ws
    }

    fn with_landscape(mut ws: Workspace, content: ViewContent) -> Workspace {
        ws.views.system_landscape_views.push(landscape(content));
        ws
    }

    #[test]
    fn landscape_items_edges_and_defaults() {
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0)],
                relationships: vec![rel_view(20)],
                ..ViewContent::default()
            },
        );
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(scene.view_key, "L1");
        assert_eq!(scene.title, "System Landscape View");
        assert_eq!(scene.background, "#ffffff");
        assert_eq!(scene.items.len(), 2);
        assert_eq!(scene.items[0].name, "Alice");
        assert_eq!(scene.items[0].metadata.as_deref(), Some("[Person]"));
        // Default box 350×150 at the stored top-left.
        assert_eq!(scene.items[0].rect.width, DEFAULT_ITEM_WIDTH);
        assert_eq!(scene.items[1].rect.x, 400.0);
        assert_eq!(
            scene.items[1].metadata.as_deref(),
            Some("[Software System]")
        );
        assert_eq!(scene.boundaries, [] as [Boundary; 0]);
        // Edge = source centre → destination centre.
        assert_eq!(scene.edges.len(), 1);
        assert_eq!(scene.edges[0].points, vec![(175.0, 75.0), (575.0, 75.0)]);
        assert_eq!(scene.edges[0].label.as_deref(), Some("Uses"));
        assert_eq!(scene.edges[0].metadata.as_deref(), Some("[HTTPS]"));
        assert_eq!(scene.edges[0].line_style, LineStyle::Dashed);
        // Bounds = item union + margins.
        assert_eq!(scene.bounds.x, -SCENE_MARGIN);
        assert_eq!(scene.bounds.width, 750.0 + 2.0 * SCENE_MARGIN);
    }

    #[test]
    fn dark_scheme_background_and_colors() {
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
        );
        let scene = build(&ws, "L1", ColorScheme::Dark, None, None).unwrap();
        assert_eq!(scene.background, "#111111");
        assert_eq!(scene.items[0].color, "#cccccc");
    }

    #[test]
    fn technology_metadata_and_terminology_override() {
        let mut ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(5, 0, 0)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
        );
        // getMetadataForElement includes the technology suffix (diagram.js:3305).
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(
            scene.items[0].metadata.as_deref(),
            Some("[Container: Spring Boot]")
        );
        // views.configuration.terminology overrides the term.
        ws.views.configuration.terminology.container = Some("Service".to_string());
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(
            scene.items[0].metadata.as_deref(),
            Some("[Service: Spring Boot]")
        );
    }

    #[test]
    fn deployment_nodes_are_boundaries_not_items() {
        let mut ws = workspace();
        // A second infra node directly on AWS (not inside EKS).
        ws.model.deployment_nodes[0].infrastructure_nodes.push(
            oxidizr_core::model::InfrastructureNode {
                item: ModelItem::new(id(13), "Gateway"),
                group: None,
                environment: "Default".to_string(),
                technology: None,
                relationships: Vec::new(),
            },
        );
        ws.views.deployment_views.push(DeploymentView {
            base: ViewBase::new("D1", 1),
            content: ViewContent {
                elements: vec![
                    element(9, 0, 0),
                    element(10, 100, 100),
                    element(11, 150, 150),
                    element(12, 150, 350),
                    element(13, -100, 500),
                ],
                relationships: vec![rel_view(22)],
                ..ViewContent::default()
            },
            environment: "Default".to_string(),
            software_system_id: None,
            animations: Vec::new(),
        });
        let scene = build(&ws, "D1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(scene.title, "Deployment View: Default");
        // Items: infra nodes + container instance only (diagram.js:558).
        let ids: Vec<&str> = scene.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["11", "12", "13"]);
        assert_eq!(
            scene.items[0].metadata.as_deref(),
            Some("[Infrastructure Node: nginx]")
        );
        // Nested deployment-node boundaries, first member order.
        let bs: Vec<(&str, &str)> = scene
            .boundaries
            .iter()
            .map(|b| (b.id.as_str(), b.label.as_str()))
            .collect();
        assert_eq!(bs, vec![("deployment:9", "AWS"), ("deployment:10", "EKS")]);
        assert_eq!(scene.boundaries[0].kind, BoundaryKind::DeploymentNode);
        // AWS wraps EKS plus its own direct child, so it is strictly bigger.
        assert!(scene.boundaries[0].rect.width > scene.boundaries[1].rect.width);
        assert!(scene.boundaries[0].rect.x < scene.boundaries[1].rect.x);
        assert_eq!(
            scene.boundaries[1].metadata.as_deref(),
            Some("[Deployment Node]")
        );
        // Instance edge still routes between the two item boxes.
        assert_eq!(scene.edges.len(), 1);
        assert_eq!(scene.edges[0].metadata.as_deref(), Some("[TCP]"));
    }

    #[test]
    fn group_boundaries_scope_by_parent() {
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(2, 400, 0), element(3, 800, 0)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
        );
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        // Only bob+carol carry the "Team" group; alice is single.
        assert_eq!(scene.boundaries.len(), 1);
        let group = &scene.boundaries[0];
        assert_eq!(group.kind, BoundaryKind::Group);
        assert_eq!(group.label, "Team");
        assert_eq!(group.id, "group:root/Team");
        // Union of bob+carol + padding: alice at x=0 stays outside.
        assert_eq!(group.rect.x, 400.0 - BOUNDARY_PADDING);
        assert_eq!(group.rect.width, 350.0 + 400.0 + 2.0 * BOUNDARY_PADDING);
        // The box covers bob+carol only (alice at x=0 stays outside).
        assert!(group.rect.x <= 400.0 && group.rect.x + group.rect.width >= 1150.0);
    }

    #[test]
    fn container_view_legality_and_parent_boundaries() {
        let mut ws = workspace();
        ws.views.container_views.push(ContainerView {
            base: ViewBase::new("C1", 1),
            content: ViewContent {
                elements: vec![element(5, 0, 0), element(8, 400, 0), element(7, 800, 0)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
            software_system_id: id(4),
            external_software_system_boundaries_visible: true,
            animations: Vec::new(),
        });
        let scene = build(&ws, "C1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(scene.title, "Container View: Internet Banking");
        // removeIllegalElements: foreign parent `mail(7)` drops out.
        let ids: Vec<&str> = scene.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["5", "8"]);
        // Parent-SS boundaries for both containers, first-member order.
        let bs: Vec<&str> = scene.boundaries.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(bs, ["software-system:4", "software-system:7"]);
        assert_eq!(
            scene.boundaries[0].metadata.as_deref(),
            Some("[Software System]")
        );
    }

    #[test]
    fn component_view_nested_boundaries() {
        let mut ws = workspace();
        ws.views.component_views.push(ComponentView {
            base: ViewBase::new("P1", 1),
            content: ViewContent {
                elements: vec![element(6, 0, 0), element(5, 400, 0)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
            container_id: id(5),
            external_container_boundaries_visible: true,
            animations: Vec::new(),
        });
        let scene = build(&ws, "P1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(
            scene.title,
            "Component View: Internet Banking - Web Application"
        );
        // Component home(6): container boundary web(5) + SS boundary bank(4);
        // container 5 on a component view: SS boundary bank(4) again.
        let bs: Vec<&str> = scene.boundaries.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(bs, ["container:5", "software-system:4"]);
    }

    #[test]
    fn filtered_view_resolves_base_and_intersects_mask() {
        let mut ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0)],
                relationships: vec![rel_view(20)],
                ..ViewContent::default()
            },
        );
        // Alice carries the stored tag "Customer"; the include keeps her only.
        ws.model.people[0].item.tags.add("Customer");
        ws.views.filtered_views.push(FilteredView {
            base: ViewBase::new("F1", 2),
            base_view_key: "L1".to_string(),
            mode: Some(oxidizr_core::views::FilterMode::Include),
            tags: vec!["Customer".to_string()],
        });
        let scene = build(&ws, "F1", ColorScheme::Light, None, None).unwrap();
        // The filtered view's own key, the base view's title.
        assert_eq!(scene.view_key, "F1");
        assert_eq!(scene.title, "System Landscape View");
        let ids: Vec<&str> = scene.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["1"]);
        // The endpoint got filtered away: edge dropped.
        assert_eq!(scene.edges, [] as [Edge; 0]);

        // A session mask on the BASE view composes the same way (V7).
        let mask = Included {
            element_ids: vec![id(1)],
            relationship_ids: Vec::new(),
        };
        let masked = build(&ws, "L1", ColorScheme::Light, Some(&mask), None).unwrap();
        assert_eq!(masked.items.len(), 1);
        assert_eq!(masked.edges, [] as [Edge; 0]);
    }

    #[test]
    fn unknown_image_and_dangling_base_are_errors() {
        let mut ws = with_landscape(workspace(), ViewContent::default());
        let err = build(&ws, "nope", ColorScheme::Light, None, None).unwrap_err();
        assert!(matches!(err, SceneError::ViewNotFound { key } if key == "nope"));

        ws.views.image_views.push(ImageView {
            base: ViewBase::new("I1", 3),
            element_id: None,
            content: Some("plantuml text".to_string()),
            content_light: None,
            content_dark: None,
            content_type: None,
        });
        let err = build(&ws, "I1", ColorScheme::Light, None, None).unwrap_err();
        assert!(matches!(err, SceneError::NotRenderable { .. }));

        ws.views.filtered_views.push(FilteredView {
            base: ViewBase::new("F2", 4),
            base_view_key: "gone".to_string(),
            mode: None,
            tags: Vec::new(),
        });
        let err = build(&ws, "F2", ColorScheme::Light, None, None).unwrap_err();
        assert!(matches!(err, SceneError::BaseViewNotFound { .. }));
    }

    #[test]
    fn explicit_title_wins_and_stale_ids_skip() {
        let mut ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(999, 100, 100)],
                relationships: vec![rel_view(999)],
                ..ViewContent::default()
            },
        );
        ws.views.system_landscape_views[0].base.title = Some("  My Landscape  ".to_string());
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(scene.title, "  My Landscape  ");
        // Stale element + relationship: rendered content stays valid.
        assert_eq!(scene.items.len(), 1);
        assert_eq!(scene.edges, [] as [Edge; 0]);
    }

    #[test]
    fn build_is_deterministic() {
        let mut ws = workspace();
        ws.views.deployment_views.push(DeploymentView {
            base: ViewBase::new("D1", 1),
            content: ViewContent {
                elements: vec![
                    element(12, 150, 350),
                    element(9, 0, 0),
                    element(11, 150, 150),
                    element(10, 100, 100),
                ],
                relationships: vec![rel_view(22)],
                ..ViewContent::default()
            },
            environment: "Default".to_string(),
            software_system_id: None,
            animations: Vec::new(),
        });
        ws.views.system_landscape_views.push(landscape(ViewContent {
            elements: vec![element(1, 0, 0), element(2, 400, 0), element(4, 800, 0)],
            relationships: vec![rel_view(20)],
            ..ViewContent::default()
        }));
        let a = build(&ws, "D1", ColorScheme::Light, None, None).unwrap();
        let b = build(&ws, "D1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(a, b);
        let l1 = build(&ws, "L1", ColorScheme::Dark, None, None).unwrap();
        let l2 = build(&ws, "L1", ColorScheme::Dark, None, None).unwrap();
        assert_eq!(l1, l2);
    }

    #[test]
    fn relationship_view_overrides_and_vertices() {
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0)],
                relationships: vec![RelationshipView {
                    description: Some("Uses banking".to_string()),
                    order: Some("1".to_string()),
                    vertices: vec![Vertex { x: 300, y: 200 }],
                    url: Some("https://example.test".to_string()),
                    ..rel_view(20)
                }],
                ..ViewContent::default()
            },
        );
        let scene = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        let edge = &scene.edges[0];
        assert_eq!(edge.label.as_deref(), Some("Uses banking"));
        assert_eq!(edge.order.as_deref(), Some("1"));
        assert_eq!(edge.url.as_deref(), Some("https://example.test"));
        assert_eq!(
            edge.points,
            vec![(175.0, 75.0), (300.0, 200.0), (575.0, 75.0)]
        );
    }

    #[test]
    fn dynamic_view_scoped_boundaries_and_title() {
        let mut ws = workspace();
        ws.views.dynamic_views.push(DynamicView {
            base: ViewBase::new("Y1", 1),
            content: ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0), element(5, 800, 0)],
                relationships: vec![rel_view(20), rel_view(21)],
                ..ViewContent::default()
            },
            element_id: Some(id(4)),
            deployment_environment: None,
            external_boundaries_visible: false,
        });
        let scene = build(&ws, "Y1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(scene.title, "Dynamic View: Internet Banking");
        assert_eq!(scene.items.len(), 3);
        // Container 5 on a dynamic view: parent-SS boundary (bank 4).
        let bs: Vec<&str> = scene.boundaries.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(bs, ["software-system:4"]);
    }

    #[test]
    fn overrides_replace_stored_geometry() {
        // P3-M2-5 seam: element top-lefts and relationship INTERIOR
        // waypoints overlay the stored values (the workspace is never
        // touched — session state, like the tag mask).
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0), element(5, 800, 0)],
                relationships: vec![rel_view(20), rel_view(21)],
                ..ViewContent::default()
            },
        );
        let overrides = Overrides {
            elements: HashMap::from([(id(1), (2000.0, 600.0))]),
            relationships: HashMap::from([(id(20), vec![(2100.0, 300.0), (300.0, 300.0)])]),
        };
        let scene = build(&ws, "L1", ColorScheme::Light, None, Some(&overrides)).unwrap();
        // Rel 20 (alice → bank): alice moved, stored geometry kept for
        // the bank; interior replaced wholesale.
        let alice = scene.items.iter().find(|i| i.id == id(1)).unwrap();
        assert_eq!((alice.rect.x, alice.rect.y), (2000.0, 600.0));
        let edge = scene.edges.iter().find(|e| e.id == id(20)).unwrap();
        assert_eq!(
            edge.points,
            vec![
                (2175.0, 675.0),
                (2100.0, 300.0),
                (300.0, 300.0),
                (575.0, 75.0),
            ]
        );
        // Bounds follow the moved item (union + margin recomputed).
        assert!(scene.bounds.x + scene.bounds.width > 2350.0);
        // An absent relationship keeps its stored vertices (rel 21 has
        // none in the fixture): still a straight centre pair.
        let kept = scene.edges.iter().find(|e| e.id == id(21)).unwrap();
        assert_eq!(kept.points.len(), 2);
    }

    #[test]
    fn overrides_group_boundary_follows_moved_member() {
        // Boundaries are member unions recomputed AFTER the overlay
        // (V1): moving both Team members moves the group box.
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(2, 400, 0), element(3, 400, 200)],
                relationships: Vec::new(),
                ..ViewContent::default()
            },
        );
        let overrides = Overrides {
            elements: HashMap::from([(id(2), (0.0, 0.0)), (id(3), (0.0, 200.0))]),
            relationships: HashMap::new(),
        };
        let before = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        let after = build(&ws, "L1", ColorScheme::Light, None, Some(&overrides)).unwrap();
        let group = |scene: &Scene| {
            scene
                .boundaries
                .iter()
                .find(|b| b.id.starts_with("group:"))
                .unwrap()
                .rect
        };
        // Stored bob(2)/carol(3) sit at (400,0)/(400,200) (fixture):
        // moved to x=0, the union starts 400 px earlier.
        assert_eq!(group(&before).x, 400.0 - BOUNDARY_PADDING);
        assert_eq!(group(&after).x, 0.0 - BOUNDARY_PADDING);
    }

    #[test]
    fn empty_waypoint_override_straightens_edge() {
        // `relationships[id] == []` (the engine's 2-point route) drops
        // the stored vertices — the "remove vertices" contract of M3.
        let ws = with_landscape(
            workspace(),
            ViewContent {
                elements: vec![element(1, 0, 0), element(4, 400, 0)],
                relationships: vec![RelationshipView {
                    vertices: vec![oxidizr_core::views::Vertex { x: 300, y: 200 }],
                    ..rel_view(20)
                }],
                ..ViewContent::default()
            },
        );
        let stored = build(&ws, "L1", ColorScheme::Light, None, None).unwrap();
        assert_eq!(stored.edges[0].points.len(), 3);
        let overrides = Overrides {
            relationships: HashMap::from([(id(20), Vec::new())]),
            ..Overrides::default()
        };
        let straight = build(&ws, "L1", ColorScheme::Light, None, Some(&overrides)).unwrap();
        assert_eq!(straight.edges[0].points, vec![(175.0, 75.0), (575.0, 75.0)]);
    }
}
