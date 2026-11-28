//! The retained scene: `Workspace` + resolved styles →
//! [`Scene`] layers/items/edges, rebuilt only on model change, layout edit
//! or zoom-band transition — never per frame.
//!
//! [`build`] is the headless heart of the viewer: it turns one view of a
//! workspace into presentation-ready data — no eframe/egui types anywhere
//! (the boundary guard in [`crate`] keeps it that way).
//!
//! Decisions baked in here:
//!
//! - **Geometry** is the stored `x/y` (top-left), except where a session
//!   [`Overrides`] (auto-layout) replaces it — build-time only,
//!   the stored workspace is never touched (editor rule).
//!   **Size** is the resolved style's `width`/`height`,
//!   else the `DEFAULT_ITEM_WIDTH`×`DEFAULT_ITEM_HEIGHT` box: the
//!   the official UI auto-sizes boxes by measuring text on a canvas
//!   (`structurizr-diagram.js` `calculateWidth`/`calculateHeight`), which
//!   is visual parity — out of contract per .
//! - **Text is kept as plain runs** (name, description and metadata
//!   strings plus the font size). The decomposition sketch allowed
//!   `epaint` layout jobs; deferring them to the paint step keeps
//!   this module (and the whole lib core) free of the egui stack —
//!   recorded on the row.
//! - **Styles resolve through [`core`](oxidizr_core::styles) only**
//!   : `element_style`/`relationship_style` guarantee
//!   background/stroke/color after the scheme defaults.
//! - **Metadata lines and terminology mirror the official UI UI**:
//!   `structurizr.ui.getMetadataForElement` (`structurizr-ui.js:638`, with
//!   the technology suffix) / `getMetadataForRelationship` /
//!   `Workspace.getTerminologyFor` (`structurizr-workspace.js:1161` — the
//!   instance kinds share their static kind's term) and the
//!   `metadataSymbols` wrapping (default `SquareBrackets`,
//!   `structurizr-workspace.js:735`).
//! - **`DeploymentNode` elements are boundaries, not items** — the
//!   the official UI skips them in the element loop
//!   and embeds their children instead. Group boundaries follow the same
//!   scoping as `includeGroup`: scope is the
//!   model parent, else the environment (top-level deployment nodes), else
//!   the root scope. (The the official UI's `Internal`/`External` scope split and
//!   the enterprise boundary ride the `location`/`enterprise` fields, which
//!   our byte contract does not carry — a single root scope is used and no
//!   enterprise boundary is rendered.)
//! - **Filtered views resolve inside [`build`]**: the base view's content
//!   is evaluated with [`oxidizr_core::filter::eval`] . The `mask`
//!   parameter is the session tag filter (: session-only, the model
//!   file is never touched) pre-evaluated by the caller; it INTERSECTS the
//!   view's own filter.
//! - **Determinism**: items and edges follow content order; boundaries are
//!   ordered by their first member's content position (ties by id). Same
//!   input → identical [`Scene`] (it derives [`PartialEq`]).
//! - **Stale ids never fail the build**: unresolvable elements/
//!   relationships are skipped (nothing to render or route).

use std::collections::HashMap;

use oxidizr_core::filter::Included;
use oxidizr_core::styles::{
    DEFAULT_BACKGROUND_DARK, DEFAULT_BACKGROUND_LIGHT, DEFAULT_COLOR_DARK, DEFAULT_COLOR_LIGHT,
};
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{ColorScheme, IconPosition, LineStyle, MetadataSymbols, Shape, View};
use oxidizr_core::{Id, Workspace};
mod label;

use label::{border_anchor, metadata_line, placement_rect, scene_title, wrap};
mod resolve;

use resolve::{
    DnSpec, GroupSpec, Identity, Paint, Placement, ScopeSpec, boundary_from, boundary_tag,
    dn_member_items, dn_rect, group_scope, member_ids, resolve, scope_targets, tags_csv,
};

/// The official UI default stroke widths (`LIGHT_MODE_DEFAULTS` /
/// `DARK_MODE_DEFAULTS`): the element/boundary stroke width when the
/// resolved style leaves `strokeWidth` unset.
pub const DEFAULT_STROKE_WIDTH: i32 = 2;
/// Box width when the resolved style sets no `width` (see module docs).
pub(crate) const DEFAULT_ITEM_WIDTH: f64 = 350.0;
/// Box height when the resolved style sets no `height` (see module docs).
pub(crate) const DEFAULT_ITEM_HEIGHT: f64 = 150.0;
/// Padding a boundary's member union grows by (chrome of ours — the
/// the official UI computes boundary insets from its own layout passes).
/// Public: the compound-layout tests assert frames against it.
pub const BOUNDARY_PADDING: f64 = 20.0;
/// Margin around the [`Scene::bounds`] union.
pub(crate) const SCENE_MARGIN: f64 = 20.0;

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

    /// Geometric centre (the origin of the edge-anchor ray).
    #[must_use]
    pub fn center(self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Inclusive containment of a point (hit-test semantics: edges of the
    /// box count, matching the official UI’s `includePoints` boxes).
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

/// One rendered model element (the official UI: the `structurizr.box` cell).
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
    /// concern — `shadeColor` equivalent lives in).
    pub background: String,
    /// Resolved stroke `#rrggbb` (auto = darker background per).
    pub stroke: String,
    /// Stroke width px (the official UI default 2).
    pub stroke_width: i32,
    /// Border style (Solid/Dashed/Dotted).
    pub border: LineStyle,
    /// Text color `#rrggbb`.
    pub color: String,
    /// Font size px (the official UI default 24).
    pub font_size: i32,
    /// Opacity percent (0-100).
    pub opacity: i32,
    /// Icon the official UI from the style (resolved by the atlas).
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

/// A boundary box behind the items (the official UI `createBoundary` /
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
    /// Metadata line under the label — `None` for groups (the official UI
    /// `createBoundaryForGroup` passes no metadata).
    pub metadata: Option<String>,
    /// Union of the member rects + `BOUNDARY_PADDING`.
    pub rect: Rect,
    /// The scene items this boundary contains, in scene order. Direct
    /// items plus the items of contained deployment nodes (transitively —
    /// the same closure the rect is drawn from). This is also the
    /// compound-membership input the layout graph is built from
    /// (`crate::layout`).
    pub members: Vec<Id>,
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

/// One rendered relationship (the official UI: the `structurizr.arrow` cell).
#[derive(Debug, Clone, PartialEq)]
pub struct Edge {
    /// Model relationship id.
    pub id: Id,
    /// Source element id.
    pub source: Id,
    /// Destination element id.
    pub destination: Id,
    /// Polyline: source box SIDE → stored vertices → destination side
    /// (endpoints are the centre rays clipped to the
    /// box borders, so arrows never land on the middle).
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
    /// Line thickness px (the official UI default 2).
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

/// The retained scene graph of one view .
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Key of the rendered view (a filtered view's own key when one was
    /// resolved).
    pub view_key: String,
    /// View title (`getTitleForView` mirror: explicit title, else the
    /// kind-derived default name).
    pub title: String,
    /// Canvas background for the scheme (the official UI
    /// `LIGHT/DARK_MODE_DEFAULTS.background` — same values as the core
    /// style defaults).
    pub background: String,
    /// Boundary boxes, bottom layer first (first-member content order).
    pub boundaries: Vec<Boundary>,
    /// Element boxes in content order.
    pub items: Vec<Item>,
    /// Edges in content order.
    pub edges: Vec<Edge>,
    /// Union of boundaries, items and edge points + `SCENE_MARGIN`
    /// (the zero rect when the scene is empty).
    pub bounds: Rect,
}

/// Session auto-layout geometry: what the [`layout`] module
/// produced for the current view, overlaid on the stored values at
/// [`build`] time. NEVER persisted (the editor rule: session geometry
/// only until the editor owns saving) and per-view (the shell clears
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
    /// `[source side, …waypoints…, destination side]`, so the stored
    /// relationship-view vertices are REPLACED wholesale — an empty vec
    /// straightens the edge.
    pub relationships: HashMap<Id, Vec<(f64, f64)>>,
}

impl Overrides {
    /// The shell's display composition: the engine geometry of
    /// an un-laid-out view with the user overlay merged on top PER ID —
    /// a dragged element pins its own box, everything else keeps the
    /// engine's placement. `user` wins, `auto` fills the gaps.
    #[must_use]
    pub fn display(auto: &Overrides, user: Option<&Overrides>) -> Overrides {
        let mut merged = auto.clone();
        if let Some(user) = user {
            merged.elements.extend(user.elements.clone());
            merged.relationships.extend(user.relationships.clone());
        }
        merged
    }
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
/// its base here). `overrides` is the optional session auto-layout
/// geometry ([`Overrides`]): element top-lefts and relationship
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

    // Filtered view: resolve the base (the official UI `findViewByKey`); its
    // content is the render target and its filter evaluates first .
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
    // the official UI script): a foreign container outranks its
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
            // The official UI `continue`s in the element loop
            // — the node becomes a boundary
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
            // Session auto-layout wins over the stored top-left;
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

        // Group membership (`includeGroup`, the official UI `findOrCreateGroup`).
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

        // Parent-scope boundaries (the official UI’s view-type-guarded
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
    // (the official UI `createDeploymentNode` + the second embed pass).
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
    // Shared memo for the transitive dn member walks (deployment-node
    // boundaries here, group member closures below).
    let mut dn_member_cache: Vec<Option<Vec<usize>>> = vec![None; dns.len()];
    for (dn_idx, dn) in dns.iter().enumerate() {
        let Some(rect) = dn_rects[dn_idx] else {
            continue; // memberless node: nothing to draw around
        };
        pending.push((
            dn.order,
            boundary_from(
                &paint,
                Identity {
                    kind: BoundaryKind::DeploymentNode,
                    id: format!("deployment:{}", dn.id),
                    label: dn.label.clone(),
                    metadata: dn.metadata.clone(),
                    tags: &dn.tags,
                },
                rect,
                member_ids(
                    &dn_member_items(dn_idx, &dn_child_items, &dn_child_dns, &mut dn_member_cache),
                    &items,
                ),
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
        let member_items: Vec<usize> = {
            let mut set: std::collections::BTreeSet<usize> = group.items.iter().copied().collect();
            for d in &group.dns {
                set.extend(dn_member_items(
                    *d,
                    &dn_child_items,
                    &dn_child_dns,
                    &mut dn_member_cache,
                ));
            }
            set.into_iter().collect()
        };
        pending.push((
            group.order,
            boundary_from(
                &paint,
                Identity {
                    kind: BoundaryKind::Group,
                    id: group.id.clone(),
                    label: group.label.clone(),
                    metadata: None,
                    tags: &group.tags,
                },
                rect,
                member_ids(&member_items, &items),
            ),
        ));
    }
    for scope in &scopes {
        let Some(rect) = union_all(scope.items.iter().map(|i| items[*i].rect)) else {
            continue;
        };
        let member_items: Vec<usize> = {
            let set: std::collections::BTreeSet<usize> = scope.items.iter().copied().collect();
            set.into_iter().collect()
        };
        pending.push((
            scope.order,
            boundary_from(
                &paint,
                Identity {
                    kind: scope.kind,
                    id: scope.id.clone(),
                    label: scope.label.clone(),
                    metadata: scope.metadata.clone(),
                    tags: &scope.tags,
                },
                rect,
                member_ids(&member_items, &items),
            ),
        ));
    }
    pending.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let boundaries: Vec<Boundary> = pending.into_iter().map(|(_, b)| b).collect();

    // Edges: source side → stored vertices → destination side
    // (see `border_anchor`).
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
        // arrows LAND ON BOX SIDES, not centres — each
        // endpoint is the ray from its box centre toward the next route
        // point, clipped where it exits the box (the dx/dy exit rule
        // matches the engine's `endpoint(None)`, so an engine-routed
        // view enters the same side the engine chose). With no
        // waypoints the pair is the two facing border points.
        let waypoints: Vec<(f64, f64)> = match overrides.and_then(|o| o.relationships.get(&rv.id)) {
            // Session auto-layout replaces the stored vertices
            // wholesale — an empty override vec
            // straightens the edge.
            Some(points) => points.clone(),
            None => rv
                .vertices
                .iter()
                .map(|v| (f64::from(v.x), f64::from(v.y)))
                .collect(),
        };
        let mut points = Vec::with_capacity(waypoints.len() + 2);
        points.push(border_anchor(
            source,
            waypoints
                .first()
                .copied()
                .unwrap_or_else(|| destination.center()),
        ));
        points.extend(waypoints.iter().copied());
        points.push(border_anchor(
            destination,
            waypoints.last().copied().unwrap_or_else(|| source.center()),
        ));
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use oxidizr_core::model::{
        Component, Container, DeploymentNode, Model, ModelItem, Person, Relationship,
        SoftwareSystem,
    };
    use oxidizr_core::views::{
        ComponentView, ContainerView, DeploymentView, DynamicView, ElementView, FilteredView,
        ImageView, RelationshipView, SystemLandscapeView, Vertex, ViewBase, ViewContent,
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
        // Edge = facing box SIDES (not centres).
        assert_eq!(scene.edges.len(), 1);
        assert_eq!(scene.edges[0].points, vec![(350.0, 75.0), (400.0, 75.0)]);
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

        // A session mask on the BASE view composes the same way .
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
            // Ray-picked sides, then the anchor SLIDES to face the
            // waypoint: both legs become axis-parallel
            // — bottom facing x 300, bottom facing x 400.
            vec![(300.0, 150.0), (300.0, 200.0), (400.0, 150.0)]
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
        // seam: element top-lefts and relationship INTERIOR
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
        // Endpoints land on the sides, slid to face the adjacent vertex
        //: alice exits the TOP directly above (2100,300);
        // the bank enters from the BOTTOM at its near corner — toward.x
        // (300) is left of the bank (x starts at 400), so it clamps to
        // the bank's bottom-left corner.
        assert_eq!(
            edge.points,
            vec![
                (2100.0, 600.0),
                (2100.0, 300.0),
                (300.0, 300.0),
                (400.0, 150.0),
            ]
        );
        // Bounds follow the moved item (union + margin recomputed).
        assert!(scene.bounds.x + scene.bounds.width > 2350.0);
        // An absent relationship keeps its stored vertices (rel 21 has
        // none in the fixture): still a straight side pair.
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
        // the stored vertices — the "remove vertices" contract.
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
        assert_eq!(straight.edges[0].points, vec![(350.0, 75.0), (400.0, 75.0)]);
    }
}
