//! Invertible editing commands + undo/redo stacks:
//! every geometry edit is a [`Command`] mutating only the
//! [`LayoutOverlay`] — deltas over the stored geometry — while every
//! MODEL edit is a [`ModelCommand`] mutating the workspace itself
//! (relationship create/delete/reconnect/relabel). The DSL text is
//! NEVER rewritten: the shell gates model editing to `.json` sources.
//! Commands carry their own inverse (`from`/`to` pairs, or exact
//! captured positions for model rows), so undo is a symmetric re-apply —
//! no closures on the stack, and a saved session never needs to
//! serialise one.
//!
//! Factories (`move_by`, `align`, `distribute`, `centre_view`,
//! `bring_back`, `remove_all_vertices`, `set_waypoints`, ...)
//! compute the delta from the CURRENT effective geometry: the overlay
//! first, the scene (already built with the overlay) as the stored
//! fallback — `from` is always exactly what the user sees. Every
//! factory returns `None` when nothing would move, so the shell never
//! pushes a no-op onto `History`. The auto-layout engine lands through
//! the same door: the shell turns one run into `MoveElements` +
//! `SetWaypoints` and it becomes undoable like any edit.
//!
//! Layout semantics follow the official tooling's layout editor:
//! `removeAllVertices` before every auto-layout run,
//! Bring-elements-back and Remove-all-vertices as dialog-level commands —
//! but NOT its per-diagram undo stacks: the session carries ONE history,
//! because a model edit is global and undoable per-diagram is undoable
//! incompletely. Grid snap (10 px) is OURS (visual parity is
//! out of contract); the official tooling snaps nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxidizr_core::Id;
use oxidizr_core::IdGenerator;
use oxidizr_core::Workspace;
use oxidizr_core::model::{DeploymentNode, Model, Relationship};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::Tags;
use oxidizr_core::views::{Dimensions, RelationshipView, Vertex, ViewContent, ViewSet};

use crate::scene::{Overrides, Rect, Scene};

/// Grid-snap pitch in world px (10, toggle in the shell).
pub(crate) const GRID_PX: f64 = 10.0;
/// Undo-stack cap: 200 — the oldest command drops off first.
pub(crate) const HISTORY_CAP: usize = 200;

/// One element move: (id, previous top-left, new top-left) — the
/// [`Command::MoveElements`] payload.
pub(crate) type ElementMove = (Id, (f64, f64), (f64, f64));
/// One waypoint replacement: (id, previous interiors, new interiors) —
/// the [`Command::SetWaypoints`] payload.
pub(crate) type WaypointChange = (Id, Vec<(f64, f64)>, Vec<(f64, f64)>);

/// Snap one world coordinate to the [`GRID_PX`] grid (half-way values
/// round away from zero, like every other round in this crate).
#[must_use]
pub(crate) fn snapped(value: f64) -> f64 {
    (value / GRID_PX).round() * GRID_PX
}

/// The session geometry of ONE view: deltas over the stored
/// [`Workspace`], keyed by model ids — exactly
/// the payload [`Overrides`] carries, plus the view-level canvas
/// `dimensions` (the `SetCanvasSize` target; [`crate::scene::build`]
/// ignores it — export sizing reads it). NEVER persisted directly: SAVE
/// maps it onto a cloned workspace.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutOverlay {
    /// The view this overlay belongs to (the shell keys its map, but
    /// [`Command::ClearView`] snapshots need the identity for tests
    /// and the save guard).
    pub view_key: String,
    /// Element id → box TOP-LEFT (world px), replacing the stored `x/y`
    /// — same contract as [`Overrides::elements`].
    pub elements: HashMap<Id, (f64, f64)>,
    /// Relationship id → INTERIOR waypoints, replacing the stored
    /// vertices wholesale — same contract as
    /// [`Overrides::relationships`] (an empty vec straightens — the
    /// "remove vertices" edit lands here).
    pub relationships: HashMap<Id, Vec<(f64, f64)>>,
    /// View canvas size (`dimensions`), `None` = stored/automatic.
    pub dimensions: Option<Dimensions>,
}

impl LayoutOverlay {
    /// An empty overlay for `view_key`.
    #[must_use]
    pub fn for_view(view_key: &str) -> Self {
        Self {
            view_key: view_key.to_string(),
            ..Self::default()
        }
    }

    /// No geometry at all (nothing to save, nothing to reset).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty() && self.relationships.is_empty() && self.dimensions.is_none()
    }

    /// The [`crate::scene::build`] overlay parameter for this session state.
    #[must_use]
    pub fn as_overrides(&self) -> Overrides {
        Overrides {
            elements: self.elements.clone(),
            relationships: self.relationships.clone(),
        }
    }

    /// The TOP-LEFT the element is RENDERED at: the overlay delta, else
    /// the scene box (built from stored geometry) — `None` for ids the
    /// scene does not render.
    #[must_use]
    pub fn position(&self, scene: &Scene, id: &Id) -> Option<(f64, f64)> {
        self.elements.get(id).copied().or_else(|| {
            scene
                .items
                .iter()
                .find(|item| item.id == *id)
                .map(|item| (item.rect.x, item.rect.y))
        })
    }

    /// The box the element is rendered at: overlay position, scene
    /// SIZE (style-resolved — item sizes are not editable).
    #[must_use]
    pub fn rect(&self, scene: &Scene, id: &Id) -> Option<Rect> {
        let size = scene
            .items
            .iter()
            .find(|item| item.id == *id)
            .map(|item| (item.rect.width, item.rect.height))?;
        self.position(scene, id).map(|(x, y)| Rect {
            x,
            y,
            width: size.0,
            height: size.1,
        })
    }

    /// The INTERIOR waypoints the edge is rendered with: the overlay
    /// delta, else the scene polyline's interior (its endpoints are the
    /// box centres, not waypoints).
    #[must_use]
    pub fn interior(&self, scene: &Scene, id: &Id) -> Vec<(f64, f64)> {
        if let Some(points) = self.relationships.get(id) {
            return points.clone();
        }
        scene
            .edges
            .iter()
            .find(|edge| edge.id == *id)
            .map_or_else(Vec::new, |edge| {
                if edge.points.len() >= 2 {
                    edge.points[1..edge.points.len() - 1].to_vec()
                } else {
                    Vec::new()
                }
            })
    }
}

/// One invertible edit. `from`/`to` are the geometry BEFORE/AFTER:
/// [`Command::apply`] writes `to`, [`Command::undo`] re-writes `from` —
/// symmetric, order-stable, and every command survives a round trip by
/// construction (the unit tests pin it for each variant).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Group move: each id's TOP-LEFT `from` → `to` (one command per
    /// DRAG, not per element — multi-selection undoes as one).
    MoveElements {
        /// Moves in selection order.
        moves: Vec<ElementMove>,
    },
    /// Replace the interior waypoints of relationships (one change per
    /// edge; an empty `to` straightens — the "remove vertices" edit).
    SetWaypoints {
        /// Changes in edge order.
        changes: Vec<WaypointChange>,
    },
    /// Canvas size (`view.dimensions`): `None` = stored/automatic.
    SetCanvasSize {
        /// Previous dimensions.
        from: Option<Dimensions>,
        /// New dimensions.
        to: Option<Dimensions>,
    },
    /// "Clear layout": the whole overlay resets (the pre-clear state is
    /// the embedded snapshot, restored on undo).
    ClearView {
        /// The overlay as it was before the clear.
        snapshot: Box<LayoutOverlay>,
    },
    /// Compound edit: the steps land and undo as ONE unit (one
    /// auto-layout run produces positions AND routes — a single Ctrl+Z
    /// reverts both).
    Group {
        /// Steps in apply order.
        steps: Vec<Command>,
    },
}

impl Command {
    /// Write this command's geometry into `overlay` (forward = `to`,
    /// backward = `from`).
    pub fn apply(&self, overlay: &mut LayoutOverlay, forward: bool) {
        match self {
            Self::MoveElements { moves } => {
                for (id, from, to) in moves {
                    overlay
                        .elements
                        .insert(id.clone(), if forward { *to } else { *from });
                }
            }
            Self::SetWaypoints { changes } => {
                for (id, from, to) in changes {
                    overlay
                        .relationships
                        .insert(id.clone(), if forward { to.clone() } else { from.clone() });
                }
            }
            Self::SetCanvasSize { from, to } => {
                overlay.dimensions = if forward { *to } else { *from };
            }
            Self::ClearView { snapshot } => {
                if forward {
                    overlay.elements.clear();
                    overlay.relationships.clear();
                    overlay.dimensions = None;
                } else {
                    *overlay = (**snapshot).clone();
                }
            }
            Self::Group { steps } => {
                if forward {
                    for step in steps {
                        step.apply(overlay, true);
                    }
                } else {
                    for step in steps.iter().rev() {
                        step.apply(overlay, false);
                    }
                }
            }
        }
    }

    /// Apply the inverse geometry.
    pub fn undo(&self, overlay: &mut LayoutOverlay) {
        self.apply(overlay, false);
    }
}

/// One step on the session history: a geometry command tagged with the
/// view it edits, or a model command over the whole workspace. Undo
/// walks ONE stack across views, so a global model edit can never be
/// undone in one view and left standing in another.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Edit {
    /// `command` on the overlay of `view_key`.
    Geometry {
        /// The view whose overlay this step edits.
        view_key: String,
        /// The geometry command.
        command: Command,
    },
    /// A model command — it mutates the workspace (and may move overlay
    /// routes, e.g. a delete), so undo/redo receive both. Boxed: the
    /// delete snapshot (row + per-view rows + routes) outweighs a
    /// geometry step, and the stack stores thousands of steps.
    //: the canvas model ops construct this next; the tests drive it meanwhile.
    #[allow(dead_code)]
    Model(Box<ModelCommand>),
}

/// What one undo/redo step MOVED: the named view's geometry, or the
/// model itself (every view can repaint; the workspace needs a save).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Stepped {
    /// Geometry of this view changed.
    Geometry(String),
    /// The workspace's model changed.
    Model,
}

/// The session's undo/redo stacks over the WHOLE open workspace: `Vec`
/// stacks, cap [`HISTORY_CAP`] (oldest drops), a fresh [`History::run_geometry`]
/// clears the redo branch. ONE instance lives on the shell: view
/// switching no longer swaps stacks (geometry steps carry their view, so
/// Ctrl+Z walks through views); opening a workspace resets.
#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Edit>,
    redo: Vec<Edit>,
}

impl History {
    /// Apply `command` to the overlay of `view_key` (created when the
    /// view has no geometry yet) and remember the step.
    pub fn run_geometry(
        &mut self,
        overlays: &mut HashMap<String, LayoutOverlay>,
        view_key: &str,
        command: Command,
    ) {
        let overlay = overlays
            .entry(view_key.to_string())
            .or_insert_with(|| LayoutOverlay::for_view(view_key));
        command.apply(overlay, true);
        if self.undo.len() == HISTORY_CAP {
            self.undo.remove(0);
        }
        self.undo.push(Edit::Geometry {
            view_key: view_key.to_string(),
            command,
        });
        self.redo.clear();
    }

    /// Apply a MODEL command to the open workspace (and, where the edit
    /// moves waypoints, to the overlays) and remember the step — the
    /// model twin of [`History::run_geometry`] on the SAME session
    /// stack: one Ctrl+Z walks geometry and model edits together.
    //: consumed by the canvas model-op wiring; until it lands the
    // tests drive this (the undo/redo arms keep the shape live).
    #[allow(dead_code)]
    pub fn run_model(
        &mut self,
        workspace: &mut Workspace,
        overlays: &mut HashMap<String, LayoutOverlay>,
        command: ModelCommand,
    ) {
        command.apply(workspace, overlays);
        if self.undo.len() == HISTORY_CAP {
            self.undo.remove(0);
        }
        self.undo.push(Edit::Model(Box::new(command)));
        self.redo.clear();
    }

    /// Undo the most recent step; `Some(Stepped)` names what moved —
    /// the view whose geometry changed (dirty + repaint follow it), or
    /// the model itself. `None` = nothing left.
    pub fn undo(
        &mut self,
        workspace: &mut Workspace,
        overlays: &mut HashMap<String, LayoutOverlay>,
    ) -> Option<Stepped> {
        let edit = self.undo.pop()?;
        let moved = match &edit {
            Edit::Geometry { view_key, command } => {
                if let Some(overlay) = overlays.get_mut(view_key) {
                    command.undo(overlay);
                }
                Stepped::Geometry(view_key.clone())
            }
            Edit::Model(command) => {
                command.undo(workspace, overlays);
                Stepped::Model
            }
        };
        self.redo.push(edit);
        Some(moved)
    }

    /// Re-apply the most recently undone step; same contract as
    /// [`History::undo`].
    pub fn redo(
        &mut self,
        workspace: &mut Workspace,
        overlays: &mut HashMap<String, LayoutOverlay>,
    ) -> Option<Stepped> {
        let edit = self.redo.pop()?;
        let moved = match &edit {
            Edit::Geometry { view_key, command } => {
                if let Some(overlay) = overlays.get_mut(view_key) {
                    command.apply(overlay, true);
                }
                Stepped::Geometry(view_key.clone())
            }
            Edit::Model(command) => {
                command.apply(workspace, overlays);
                Stepped::Model
            }
        };
        self.undo.push(edit);
        Some(moved)
    }

    /// Whether [`History::undo`] would do anything.
    #[must_use]
    // test-only: the shipped shell probes by the Option (driven by unit tests)
    #[cfg(test)]
    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`History::redo`] would do anything.
    #[must_use]
    // test-only: the shipped shell probes by the Option (driven by unit tests)
    #[cfg(test)]
    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Drop both stacks (workspace open/reload — history does not
    /// survive a new workspace; view SWITCHING deliberately does).
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

// ---- model commands ----------------------------------------------------------

/// Which ENDPOINT of a relationship [`ModelCommand::ReconnectRelationship`]
/// moves. `Source` also MOVES the relationship id between the two
/// elements' efferent `relationships` lists (the JSON nests them there);
/// `Destination` edits the model row alone.
#[allow(dead_code)] //: the canvas model ops construct these next; the tests drive them meanwhile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Endpoint {
    /// The `sourceId` end.
    Source,
    /// The `destinationId` end.
    Destination,
}

/// Which free-text slot of a relationship
/// [`ModelCommand::SetRelationshipText`] relabels.
#[allow(dead_code)] //: the canvas model ops construct these next; the tests drive them meanwhile
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextField {
    /// The `description`.
    Description,
    /// The `technology`.
    Technology,
}

/// One MODEL edit over the open workspace. Deletes snapshot the EXACT
/// positions the inverse has to restore (model index, efferent-list
/// slot, per-view row index, overlay routes) so undo is byte-identical,
/// not merely equivalent — the JSON keeps list order.
#[allow(dead_code)] //: the canvas model ops construct these next; the tests drive them meanwhile
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ModelCommand {
    /// Append a brand-new relationship: the model row, its id in the
    /// source element's efferent list, and a fresh `RelationshipView`
    /// in every STATIC view (landscape/context/container/component)
    /// that already shows both endpoints.
    CreateRelationship {
        /// The row to insert (fresh id from the shared counter).
        relationship: Relationship,
    },
    /// Remove a relationship everywhere it is stored: model row,
    /// efferent-list slot, every content-bearing view's
    /// `RelationshipViews`, and every overlay route keyed by its id.
    DeleteRelationship {
        /// The removed row.
        row: Relationship,
        /// Its index in `model.relationships`.
        model_index: usize,
        /// Its slot in the source element's efferent list (`None` for a
        /// ghost relationship the source never listed).
        source_slot: Option<usize>,
        /// `(view key, index, row)` per stored `RelationshipView`.
        view_rows: Vec<(String, usize, RelationshipView)>,
        /// `(view key, route)` per overlay that carried waypoints.
        overlay_rows: Vec<(String, Vec<(f64, f64)>)>,
    },
    /// Move one endpoint; `Source` re-homes the id in the efferent
    /// lists at apply time (membership is recomputed, never assumed).
    ReconnectRelationship {
        /// The relationship to move.
        id: Id,
        /// Which end moves.
        endpoint: Endpoint,
        /// The endpoint's current id.
        from: Id,
        /// The endpoint's new id.
        to: Id,
        /// Slot `id` held in `from`'s efferent list — the undo restore
        /// point (`None` for `Destination`, which touches no list).
        from_slot: Option<usize>,
    },
    /// Relabel `description`/`technology` on the model row.
    SetRelationshipText {
        /// The relationship to relabel.
        id: Id,
        /// Which slot changes.
        field: TextField,
        /// The previous value (undo restores it exactly).
        from: Option<String>,
        /// The new value.
        to: Option<String>,
    },
}

impl ModelCommand {
    /// Apply the edit forward to the workspace (+ overlay routes).
    pub(crate) fn apply(
        &self,
        workspace: &mut Workspace,
        overlays: &mut HashMap<String, LayoutOverlay>,
    ) {
        match self {
            Self::CreateRelationship { relationship } => {
                workspace.model.relationships.push(relationship.clone());
                if let Some(list) =
                    relationship_list_mut(&mut workspace.model, &relationship.source)
                    && !list.contains(&relationship.id)
                {
                    list.push(relationship.id.clone());
                }
                for (_, content) in static_contents_mut(&mut workspace.views) {
                    inject_relationship_view(content, relationship);
                }
            }
            Self::DeleteRelationship {
                row, overlay_rows, ..
            } => {
                workspace.model.relationships.retain(|r| r.id != row.id);
                if let Some(list) = relationship_list_mut(&mut workspace.model, &row.source) {
                    list.retain(|r| r != &row.id);
                }
                for (_, content) in all_contents_mut(&mut workspace.views) {
                    content.relationships.retain(|r| r.id != row.id);
                }
                for (key, _) in overlay_rows {
                    if let Some(overlay) = overlays.get_mut(key) {
                        overlay.relationships.remove(&row.id);
                    }
                }
            }
            Self::ReconnectRelationship {
                id,
                endpoint,
                from,
                to,
                ..
            } => {
                if let Some(row) = workspace
                    .model
                    .relationships
                    .iter_mut()
                    .find(|r| r.id == *id)
                {
                    match endpoint {
                        Endpoint::Source => row.source = to.clone(),
                        Endpoint::Destination => row.destination = to.clone(),
                    }
                }
                if matches!(endpoint, Endpoint::Source) {
                    if let Some(list) = relationship_list_mut(&mut workspace.model, from) {
                        list.retain(|r| r != id);
                    }
                    if let Some(list) = relationship_list_mut(&mut workspace.model, to)
                        && !list.contains(id)
                    {
                        list.push(id.clone());
                    }
                }
            }
            Self::SetRelationshipText { id, field, to, .. } => {
                set_text(workspace, id, *field, to.clone());
            }
        }
    }

    /// Apply the inverse: every position the forward step moved is put
    /// back at its captured index, so an undone step leaves no trace in
    /// the written JSON.
    pub(crate) fn undo(
        &self,
        workspace: &mut Workspace,
        overlays: &mut HashMap<String, LayoutOverlay>,
    ) {
        match self {
            Self::CreateRelationship { relationship } => {
                workspace
                    .model
                    .relationships
                    .retain(|r| r.id != relationship.id);
                if let Some(list) =
                    relationship_list_mut(&mut workspace.model, &relationship.source)
                {
                    list.retain(|r| r != &relationship.id);
                }
                // The id is brand-new, so removing it EVERYWHERE is the
                // exact inverse of the injection (LIFO: later edits —
                // e.g. a reconnect — undo first).
                for (_, content) in all_contents_mut(&mut workspace.views) {
                    content.relationships.retain(|r| r.id != relationship.id);
                }
            }
            Self::DeleteRelationship {
                row,
                model_index,
                source_slot,
                view_rows,
                overlay_rows,
            } => {
                insert_at(
                    &mut workspace.model.relationships,
                    *model_index,
                    row.clone(),
                );
                if let Some(slot) = source_slot
                    && let Some(list) = relationship_list_mut(&mut workspace.model, &row.source)
                {
                    insert_at(list, *slot, row.id.clone());
                }
                for (key, index, view_row) in view_rows {
                    if let Some(content) = content_of_key(workspace, key) {
                        insert_at(&mut content.relationships, *index, view_row.clone());
                    }
                }
                for (key, route) in overlay_rows {
                    overlays
                        .entry(key.clone())
                        .or_insert_with(|| LayoutOverlay::for_view(key))
                        .relationships
                        .insert(row.id.clone(), route.clone());
                }
            }
            Self::ReconnectRelationship {
                id,
                endpoint,
                from,
                to,
                from_slot,
            } => {
                if let Some(row) = workspace
                    .model
                    .relationships
                    .iter_mut()
                    .find(|r| r.id == *id)
                {
                    match endpoint {
                        Endpoint::Source => row.source = from.clone(),
                        Endpoint::Destination => row.destination = from.clone(),
                    }
                }
                if matches!(endpoint, Endpoint::Source) {
                    if let Some(list) = relationship_list_mut(&mut workspace.model, to) {
                        list.retain(|r| r != id);
                    }
                    if let Some(list) = relationship_list_mut(&mut workspace.model, from) {
                        let at = from_slot.unwrap_or(list.len());
                        insert_at(list, at, id.clone());
                    }
                }
            }
            Self::SetRelationshipText {
                id, field, from, ..
            } => {
                set_text(workspace, id, *field, from.clone());
            }
        }
    }
}

/// Write one text slot of one relationship row (no-op when the id is
/// gone — the row's absence is never recreated by an undo).
fn set_text(workspace: &mut Workspace, id: &Id, field: TextField, value: Option<String>) {
    if let Some(row) = workspace
        .model
        .relationships
        .iter_mut()
        .find(|r| r.id == *id)
    {
        match field {
            TextField::Description => row.description = value,
            TextField::Technology => row.technology = value,
        }
    }
}

/// Insert `value` at `index`, clamped to the list end (the clamped
/// fallback only fires on a model that changed shape since the capture
/// — history LIFO normally guarantees the exact slot is free).
fn insert_at<T>(list: &mut Vec<T>, index: usize, value: T) {
    list.insert(index.min(list.len()), value);
}

/// The fresh `RelationshipView` the view-add semantics store for a
/// relationship no view has seen — id only, every other slot empty (the
/// same shape the builder's add-relationship writes).
fn inject_relationship_view(content: &mut ViewContent, relationship: &Relationship) {
    let shown = |id: &Id| content.elements.iter().any(|e| &e.id == id);
    if shown(&relationship.source)
        && shown(&relationship.destination)
        && !content
            .relationships
            .iter()
            .any(|r| r.id == relationship.id)
    {
        content.relationships.push(RelationshipView {
            id: relationship.id.clone(),
            description: None,
            url: None,
            order: None,
            response: None,
            properties: Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
    }
}

/// Mutable efferent-list access for ONE element, mirroring the core
/// builder's private traversal: people, systems → containers →
/// components, custom elements, deployment nodes recursively. Every
/// element kind has such a list, so `Some` doubles as the
/// element-exists probe.
fn relationship_list_mut<'a>(model: &'a mut Model, id: &Id) -> Option<&'a mut Vec<Id>> {
    if let Some(p) = model.people.iter_mut().find(|p| &p.item.id == id) {
        return Some(&mut p.relationships);
    }
    for s in &mut model.software_systems {
        if &s.item.id == id {
            return Some(&mut s.relationships);
        }
        for c in &mut s.containers {
            if &c.item.id == id {
                return Some(&mut c.relationships);
            }
            for k in &mut c.components {
                if &k.item.id == id {
                    return Some(&mut k.relationships);
                }
            }
        }
    }
    if let Some(c) = model.custom_elements.iter_mut().find(|c| &c.item.id == id) {
        return Some(&mut c.relationships);
    }
    node_list_mut(&mut model.deployment_nodes, id)
}

#[allow(clippy::needless_pass_by_value)] // recursion needs the slice reborrow explicit
fn node_list_mut<'a>(nodes: &'a mut [DeploymentNode], id: &Id) -> Option<&'a mut Vec<Id>> {
    for n in nodes {
        if &n.item.id == id {
            return Some(&mut n.relationships);
        }
        for i in &mut n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(&mut i.relationships);
            }
        }
        for s in &mut n.software_system_instances {
            if &s.item.id == id {
                return Some(&mut s.relationships);
            }
        }
        for c in &mut n.container_instances {
            if &c.item.id == id {
                return Some(&mut c.relationships);
            }
        }
        if let Some(found) = node_list_mut(&mut n.children, id) {
            return Some(found);
        }
    }
    None
}

/// Read-only twin of [`relationship_list_mut`] (factories probe on
/// `&Workspace`).
#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn element_relationships<'a>(model: &'a Model, id: &Id) -> Option<&'a Vec<Id>> {
    if let Some(p) = model.people.iter().find(|p| &p.item.id == id) {
        return Some(&p.relationships);
    }
    for s in &model.software_systems {
        if &s.item.id == id {
            return Some(&s.relationships);
        }
        for c in &s.containers {
            if &c.item.id == id {
                return Some(&c.relationships);
            }
            for k in &c.components {
                if &k.item.id == id {
                    return Some(&k.relationships);
                }
            }
        }
    }
    if let Some(c) = model.custom_elements.iter().find(|c| &c.item.id == id) {
        return Some(&c.relationships);
    }
    node_relationships(&model.deployment_nodes, id)
}

#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn node_relationships<'a>(nodes: &'a [DeploymentNode], id: &Id) -> Option<&'a Vec<Id>> {
    for n in nodes {
        if &n.item.id == id {
            return Some(&n.relationships);
        }
        for i in &n.infrastructure_nodes {
            if &i.item.id == id {
                return Some(&i.relationships);
            }
        }
        for s in &n.software_system_instances {
            if &s.item.id == id {
                return Some(&s.relationships);
            }
        }
        for c in &n.container_instances {
            if &c.item.id == id {
                return Some(&c.relationships);
            }
        }
        if let Some(found) = node_relationships(&n.children, id) {
            return Some(found);
        }
    }
    None
}

/// Visit EVERY element id in the model (the shared-counter registration
/// walk of [`next_relationship_id`]).
#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn for_each_element_id(model: &Model, visit: &mut impl FnMut(&Id)) {
    for p in &model.people {
        visit(&p.item.id);
    }
    for s in &model.software_systems {
        visit(&s.item.id);
        for c in &s.containers {
            visit(&c.item.id);
            for k in &c.components {
                visit(&k.item.id);
            }
        }
    }
    for c in &model.custom_elements {
        visit(&c.item.id);
    }
    for_each_node_id(&model.deployment_nodes, visit);
}

#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn for_each_node_id(nodes: &[DeploymentNode], visit: &mut impl FnMut(&Id)) {
    for n in nodes {
        visit(&n.item.id);
        for i in &n.infrastructure_nodes {
            visit(&i.item.id);
        }
        for s in &n.software_system_instances {
            visit(&s.item.id);
        }
        for c in &n.container_instances {
            visit(&c.item.id);
        }
        for_each_node_id(&n.children, visit);
    }
}

/// The next relationship id from the SHARED counter: register every
/// element and relationship id, then allocate — exactly how the core
/// builder numbers a new relationship (the counter never collides).
#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn next_relationship_id(model: &Model) -> Id {
    let mut counter = IdGenerator::new();
    for_each_element_id(model, &mut |id| counter.found(id));
    for row in &model.relationships {
        counter.found(&row.id);
    }
    Id::from_num(counter.generate())
}

/// `(key, content)` for every CONTENT-BEARING view (delete's sweep —
/// filtered and image views carry no rows of their own).
#[allow(dead_code)] //: consumed by the model factories; the tests drive this meanwhile
fn all_contents(views: &ViewSet) -> Vec<(&str, &ViewContent)> {
    let mut out: Vec<(&str, &ViewContent)> = Vec::new();
    macro_rules! collect {
        ($field:ident) => {
            out.extend(
                views
                    .$field
                    .iter()
                    .map(|view| (view.base.key.as_str(), &view.content)),
            );
        };
    }
    collect!(custom_views);
    collect!(system_landscape_views);
    collect!(system_context_views);
    collect!(container_views);
    collect!(component_views);
    collect!(dynamic_views);
    collect!(deployment_views);
    out
}

/// Mutable [`all_contents`].
fn all_contents_mut(views: &mut ViewSet) -> Vec<(&str, &mut ViewContent)> {
    let mut out: Vec<(&str, &mut ViewContent)> = Vec::new();
    macro_rules! collect {
        ($field:ident) => {
            out.extend(
                views
                    .$field
                    .iter_mut()
                    .map(|view| (view.base.key.as_str(), &mut view.content)),
            );
        };
    }
    collect!(custom_views);
    collect!(system_landscape_views);
    collect!(system_context_views);
    collect!(container_views);
    collect!(component_views);
    collect!(dynamic_views);
    collect!(deployment_views);
    out
}

/// The four STATIC model views (create's injection sweep: relationship
/// rows land only where the endpoint boxes are already laid out).
fn static_contents_mut(views: &mut ViewSet) -> Vec<(&str, &mut ViewContent)> {
    let mut out: Vec<(&str, &mut ViewContent)> = Vec::new();
    macro_rules! collect {
        ($field:ident) => {
            out.extend(
                views
                    .$field
                    .iter_mut()
                    .map(|view| (view.base.key.as_str(), &mut view.content)),
            );
        };
    }
    collect!(system_landscape_views);
    collect!(system_context_views);
    collect!(container_views);
    collect!(component_views);
    out
}

/// Create a `source -> destination` relationship as the tooling stores
/// one: NO `description` key, EMPTY stored tags (the `Relationship` tag
/// is DERIVED at serialization), id from the shared counter. `None`
/// when an end is unknown or the ends coincide, or when an efferent
/// `source -> destination` relationship with an already-empty
/// description exists — the official duplicate check would reject the
/// second one, and saved output must load there.
#[allow(dead_code)] //: consumed by the canvas model ops; the tests drive this meanwhile
#[must_use]
pub(crate) fn create_relationship(
    workspace: &Workspace,
    source: &Id,
    destination: &Id,
) -> Option<ModelCommand> {
    if source == destination {
        return None;
    }
    if element_relationships(&workspace.model, source).is_none()
        || element_relationships(&workspace.model, destination).is_none()
    {
        return None;
    }
    let duplicate = workspace.model.relationships.iter().any(|r| {
        &r.source == source
            && &r.destination == destination
            && r.description.as_deref().unwrap_or("").is_empty()
    });
    if duplicate {
        return None;
    }
    Some(ModelCommand::CreateRelationship {
        relationship: Relationship {
            id: next_relationship_id(&workspace.model),
            source: source.clone(),
            destination: destination.clone(),
            description: None,
            technology: None,
            tags: Tags::new(),
            url: None,
            properties: Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        },
    })
}

/// Delete relationship `id`, snapshotting every position undo has to
/// restore (model row index, efferent slot, per-view rows, overlay
/// routes). `None` when the id names no model row.
#[allow(dead_code)] //: consumed by the canvas model ops; the tests drive this meanwhile
#[must_use]
pub(crate) fn delete_relationship(
    workspace: &Workspace,
    overlays: &HashMap<String, LayoutOverlay>,
    id: &Id,
) -> Option<ModelCommand> {
    let model_index = workspace
        .model
        .relationships
        .iter()
        .position(|r| r.id == *id)?;
    let row = workspace.model.relationships[model_index].clone();
    let source_slot = element_relationships(&workspace.model, &row.source)
        .and_then(|list| list.iter().position(|r| r == id));
    let view_rows: Vec<(String, usize, RelationshipView)> = all_contents(&workspace.views)
        .into_iter()
        .flat_map(|(key, content)| {
            content
                .relationships
                .iter()
                .enumerate()
                .filter(|(_, view_row)| view_row.id == *id)
                .map(move |(index, view_row)| (key.to_string(), index, view_row.clone()))
        })
        .collect();
    let overlay_rows: Vec<(String, Vec<(f64, f64)>)> = overlays
        .iter()
        .filter_map(|(key, overlay)| {
            overlay
                .relationships
                .get(id)
                .cloned()
                .map(|route| (key.clone(), route))
        })
        .collect();
    Some(ModelCommand::DeleteRelationship {
        row,
        model_index,
        source_slot,
        view_rows,
        overlay_rows,
    })
}

/// Reconnect one endpoint of relationship `id` to element `to`. `None`
/// when the relationship or the new element is unknown, the endpoint
/// already points there (a no-op), or the move would point the
/// relationship at its own other end (the self-loop the tooling
/// rejects).
#[allow(dead_code)] //: consumed by the canvas model ops; the tests drive this meanwhile
#[must_use]
pub(crate) fn reconnect_relationship(
    workspace: &Workspace,
    id: &Id,
    endpoint: Endpoint,
    to: &Id,
) -> Option<ModelCommand> {
    let row = workspace.model.relationships.iter().find(|r| r.id == *id)?;
    let (from, other) = match endpoint {
        Endpoint::Source => (&row.source, &row.destination),
        Endpoint::Destination => (&row.destination, &row.source),
    };
    if from == to || to == other || element_relationships(&workspace.model, to).is_none() {
        return None;
    }
    let from_slot = match endpoint {
        Endpoint::Source => element_relationships(&workspace.model, from)
            .and_then(|list| list.iter().position(|r| r == id)),
        Endpoint::Destination => None,
    };
    Some(ModelCommand::ReconnectRelationship {
        id: id.clone(),
        endpoint,
        from: from.clone(),
        to: to.clone(),
        from_slot,
    })
}

/// Relabel `description`/`technology` on relationship `id`; an empty
/// string clears the slot (the JSON omits absent slots). `None` when
/// the id is unknown or the value would not change.
#[allow(dead_code)] //: consumed by the canvas model ops; the tests drive this meanwhile
#[must_use]
pub(crate) fn set_relationship_text(
    workspace: &Workspace,
    id: &Id,
    field: TextField,
    to: Option<String>,
) -> Option<ModelCommand> {
    let to = to.filter(|value| !value.is_empty());
    let row = workspace.model.relationships.iter().find(|r| r.id == *id)?;
    let from = match field {
        TextField::Description => row.description.clone(),
        TextField::Technology => row.technology.clone(),
    };
    if from == to {
        return None;
    }
    Some(ModelCommand::SetRelationshipText {
        id: id.clone(),
        field,
        from,
        to,
    })
}

// ---- command factories -------------------------------------------------------

/// Group-drag `ids` by `delta` (single-select drags pass one id; the
/// multi-selection keeps its relative layout, so the command is
/// UNDOABLE as one unit). `snap` grid-snaps each NEW top-left. `None`
/// when nothing moves.
#[must_use]
// test-only: the shipped shell never calls this (driven by unit tests)
#[cfg(test)]
pub(crate) fn move_by(
    scene: &Scene,
    overlay: &LayoutOverlay,
    ids: &[Id],
    delta: (f64, f64),
    snap: bool,
) -> Option<Command> {
    let moves = resolve_moves(scene, overlay, ids, |rect| {
        let to = (rect.x + delta.0, rect.y + delta.1);
        if snap {
            (snapped(to.0), snapped(to.1))
        } else {
            to
        }
    });
    non_empty_moves(moves)
}

/// Align the selection along one edge/axis (`m` = the middle axis
/// line). Targets: the selection's bounding box, not the canvas. Needs
/// at least two elements; `None` when nothing moves.
#[must_use]
pub(crate) fn align(
    scene: &Scene,
    overlay: &LayoutOverlay,
    ids: &[Id],
    edge: AlignEdge,
) -> Option<Command> {
    if ids.len() < 2 {
        return None;
    }
    let rects: Vec<Rect> = ids
        .iter()
        .filter_map(|id| overlay.rect(scene, id))
        .collect();
    if rects.len() < 2 {
        return None;
    }
    let left = rects.iter().fold(f64::INFINITY, |m, r| m.min(r.x));
    let right = rects
        .iter()
        .fold(f64::NEG_INFINITY, |m, r| m.max(r.x + r.width));
    let top = rects.iter().fold(f64::INFINITY, |m, r| m.min(r.y));
    let bottom = rects
        .iter()
        .fold(f64::NEG_INFINITY, |m, r| m.max(r.y + r.height));
    let moves = resolve_moves(scene, overlay, ids, |rect| match edge {
        AlignEdge::Left => (left, rect.y),
        AlignEdge::CenterX => (f64::midpoint(left, right) - rect.width / 2.0, rect.y),
        AlignEdge::Right => (right - rect.width, rect.y),
        AlignEdge::Top => (rect.x, top),
        AlignEdge::CenterY => (rect.x, f64::midpoint(top, bottom) - rect.height / 2.0),
        AlignEdge::Bottom => (rect.x, bottom - rect.height),
    });
    non_empty_moves(moves)
}

/// Even spacing along one axis: the first/last element (by centre)
/// stay, the ones between spread so consecutive GAPS are equal
/// (negative gaps — overlapping selections — are kept as computed: the
/// overlap is the user's cue). Needs at least three elements.
#[must_use]
#[allow(clippy::cast_precision_loss)] // selection count: exact in f64 (≤ item count)
pub(crate) fn distribute(
    scene: &Scene,
    overlay: &LayoutOverlay,
    ids: &[Id],
    axis: Axis,
) -> Option<Command> {
    if ids.len() < 3 {
        return None;
    }
    let mut order: Vec<(Id, Rect)> = ids
        .iter()
        .filter_map(|id| overlay.rect(scene, id).map(|rect| (id.clone(), rect)))
        .collect();
    if order.len() < 3 {
        return None;
    }
    order.sort_by(|(id_a, a), (id_b, b)| {
        let (ka, kb) = match axis {
            Axis::Horizontal => (a.x + a.width / 2.0, b.x + b.width / 2.0),
            Axis::Vertical => (a.y + a.height / 2.0, b.y + b.height / 2.0),
        };
        ka.total_cmp(&kb)
            .then_with(|| id_a.as_str().cmp(id_b.as_str()))
    });
    let (first, last) = (&order[0], order[order.len() - 1].clone());
    let (start, sizes): (f64, f64) = match axis {
        Axis::Horizontal => (
            first.1.x,
            order.iter().fold(0.0, |sum, (_, r)| sum + r.width),
        ),
        Axis::Vertical => (
            first.1.y,
            order.iter().fold(0.0, |sum, (_, r)| sum + r.height),
        ),
    };
    let span = match axis {
        Axis::Horizontal => last.1.x + last.1.width - first.1.x,
        Axis::Vertical => last.1.y + last.1.height - first.1.y,
    };
    let gap = (span - sizes) / (order.len() - 1) as f64;
    let mut cursor = start;
    let mut moves: Vec<ElementMove> = Vec::new();
    for (id, rect) in &order {
        let to = match axis {
            Axis::Horizontal => (cursor, rect.y),
            Axis::Vertical => (rect.x, cursor),
        };
        if to != (rect.x, rect.y) {
            moves.push((id.clone(), (rect.x, rect.y), to));
        }
        cursor += match axis {
            Axis::Horizontal => rect.width + gap,
            Axis::Vertical => rect.height + gap,
        };
    }
    non_empty_moves(moves)
}

/// Centre the selection on the scene bounds (the
/// shell passes ALL ids for "centre the view"). `None` when nothing
/// moves.
#[must_use]
pub(crate) fn centre_view(scene: &Scene, overlay: &LayoutOverlay, ids: &[Id]) -> Option<Command> {
    let rects: Vec<Rect> = ids
        .iter()
        .filter_map(|id| overlay.rect(scene, id))
        .collect();
    if rects.is_empty() {
        return None;
    }
    let left = rects.iter().fold(f64::INFINITY, |m, r| m.min(r.x));
    let right = rects
        .iter()
        .fold(f64::NEG_INFINITY, |m, r| m.max(r.x + r.width));
    let top = rects.iter().fold(f64::INFINITY, |m, r| m.min(r.y));
    let bottom = rects
        .iter()
        .fold(f64::NEG_INFINITY, |m, r| m.max(r.y + r.height));
    let (cx, cy) = scene.bounds.center();
    let delta = (
        cx - f64::midpoint(left, right),
        cy - f64::midpoint(top, bottom),
    );
    let moves = resolve_moves(scene, overlay, ids, |rect| {
        (rect.x + delta.0, rect.y + delta.1)
    });
    non_empty_moves(moves)
}

/// Bring every element that is not fully on the `page` back onto it
/// (the shell passes the dimensions rect, else the visible viewport
/// rect — the page the user sees).
/// Elements are shifted by the minimal
/// delta; the loop order is content order (deterministic).
#[must_use]
pub(crate) fn bring_back(scene: &Scene, overlay: &LayoutOverlay, page: Rect) -> Option<Command> {
    let mut moves: Vec<ElementMove> = Vec::new();
    for item in &scene.items {
        let Some(rect) = overlay.rect(scene, &item.id) else {
            continue;
        };
        let dx = if rect.x < page.x {
            page.x - rect.x
        } else if rect.x + rect.width > page.x + page.width {
            (page.x + page.width) - (rect.x + rect.width)
        } else {
            0.0
        };
        let dy = if rect.y < page.y {
            page.y - rect.y
        } else if rect.y + rect.height > page.y + page.height {
            (page.y + page.height) - (rect.y + rect.height)
        } else {
            0.0
        };
        if dx.abs() + dy.abs() > 0.0 {
            moves.push((
                item.id.clone(),
                (rect.x, rect.y),
                (rect.x + dx, rect.y + dy),
            ));
        }
    }
    non_empty_moves(moves)
}

/// Straighten EVERY edge that renders waypoints (the dialog's
/// remove-vertices button):
/// one `SetWaypoints` change per rendered relationship, emptying the
/// interiors. `None` when all edges are already straight.
#[must_use]
pub(crate) fn remove_all_vertices(scene: &Scene, overlay: &LayoutOverlay) -> Option<Command> {
    let changes: Vec<WaypointChange> = scene
        .edges
        .iter()
        .filter_map(|edge| {
            let from = overlay.interior(scene, &edge.id);
            if from.is_empty() {
                None
            } else {
                Some((edge.id.clone(), from, Vec::new()))
            }
        })
        .collect();
    if changes.is_empty() {
        return None;
    }
    Some(Command::SetWaypoints { changes })
}

/// Replace one edge's interiors (waypoint drag/insert/delete — the
/// caller computes the new list with the `hit` helpers, this makes it a
/// command). `None` when the list is unchanged.
#[must_use]
pub(crate) fn set_waypoints(
    scene: &Scene,
    overlay: &LayoutOverlay,
    id: &Id,
    to: Vec<(f64, f64)>,
) -> Option<Command> {
    let from = overlay.interior(scene, id);
    if from == to {
        return None;
    }
    Some(Command::SetWaypoints {
        changes: vec![(id.clone(), from, to)],
    })
}

/// Set the canvas size (`None` clears back to stored/automatic).
/// `None` when already the same.
#[must_use]
// test-only: the shipped shell never calls this (driven by unit tests)
#[cfg(test)]
pub(crate) fn set_canvas_size(overlay: &LayoutOverlay, to: Option<Dimensions>) -> Option<Command> {
    if overlay.dimensions == to {
        return None;
    }
    Some(Command::SetCanvasSize {
        from: overlay.dimensions,
        to,
    })
}

/// "Clear layout" for the view: reset the overlay (undo restores the
/// snapshot — always a command, the shell does not reset silently).
#[must_use]
pub(crate) fn clear_view(overlay: &LayoutOverlay) -> Command {
    Command::ClearView {
        snapshot: Box::new(overlay.clone()),
    }
}

/// The dragged target of a top-left: `origin` moved by `delta`, grid-
/// snapped when `snap`. Shared by [`drag_move`] and the shell's live
/// preview so preview frames EQUAL the committed geometry.
#[must_use]
pub(crate) fn drag_target(origin: (f64, f64), delta: (f64, f64), snap: bool) -> (f64, f64) {
    let to = (origin.0 + delta.0, origin.1 + delta.1);
    if snap {
        (snapped(to.0), snapped(to.1))
    } else {
        to
    }
}

/// Commit a live group drag: `from` are the effective top-lefts
/// captured when the drag BEGAN (the overlay may already carry the live
/// preview positions, so the delta is not re-read from it). `snap` grid-
/// snaps every TARGET top-left exactly like [`move_by`]; the whole group
/// shares `delta`, so relative layout survives. `None` when nothing
/// moves. Re-applying the result is idempotent — the preview and the
/// committed command write the same absolute positions.
#[must_use]
// test-only: the shipped shell never calls this (driven by unit tests)
#[cfg(test)]
pub(crate) fn drag_move(
    from: &[(Id, (f64, f64))],
    delta: (f64, f64),
    snap: bool,
) -> Option<Command> {
    let moves: Vec<ElementMove> = from
        .iter()
        .filter_map(|(id, (x, y))| {
            let to = drag_target((*x, *y), delta, snap);
            (to != (*x, *y)).then(|| (id.clone(), (*x, *y), to))
        })
        .collect();
    non_empty_moves(moves)
}

/// The interior routes a group drag CARRIES: every
/// edge whose BOTH ends are inside `ids` and that has non-empty
/// effective interior waypoints. Endpoints re-anchor off the boxes on
/// every rebuild, so only free-floating handles need translating; an
/// edge leaving the group keeps its stored route (rerouting it would
/// fight the user's stored geometry).
#[must_use]
pub(crate) fn carried_interiors(
    scene: &Scene,
    overlay: &LayoutOverlay,
    ids: &[Id],
) -> Vec<(Id, Vec<(f64, f64)>)> {
    scene
        .edges
        .iter()
        .filter(|edge| ids.contains(&edge.source) && ids.contains(&edge.destination))
        .map(|edge| (edge.id.clone(), overlay.interior(scene, &edge.id)))
        .filter(|(_, interior)| !interior.is_empty())
        .collect()
}

/// Commit a live group drag that also moves the arrow
/// handles: `MoveElements` plus, unless `with_waypoints` is false
/// (Ctrl+drag), a `SetWaypoints` translating every `carries` route by
/// the delta ACTUALLY applied to the first origin after snapping — the
/// routes land exactly where the boxes did. `carries` are press-time
/// [`carried_interiors`] results: the overlay already carries the live
/// preview, so reading routes back from it would capture the wrong
/// `from`. One [`Command::Group`] → one Ctrl+Z; re-applying the result
/// is idempotent exactly like [`drag_move`].
#[must_use]
pub(crate) fn drag_group(
    from: &[(Id, (f64, f64))],
    carries: &[(Id, Vec<(f64, f64)>)],
    delta: (f64, f64),
    snap: bool,
    with_waypoints: bool,
) -> Option<Command> {
    let moves: Vec<ElementMove> = from
        .iter()
        .filter_map(|(id, (x, y))| {
            let to = drag_target((*x, *y), delta, snap);
            (to != (*x, *y)).then(|| (id.clone(), (*x, *y), to))
        })
        .collect();
    if moves.is_empty() {
        return None;
    }
    let mut steps = vec![Command::MoveElements { moves }];
    if with_waypoints {
        // The applied delta of the FIRST origin — the same element the
        // shell's preview measures the carry shift from, so preview
        // frames equal the committed route.
        let (_, (fx, fy)) = from.first()?;
        let (tx, ty) = drag_target((*fx, *fy), delta, snap);
        let applied = (tx - fx, ty - fy);
        if applied.0.abs() + applied.1.abs() > f64::EPSILON && !carries.is_empty() {
            let changes: Vec<WaypointChange> = carries
                .iter()
                .map(|(id, wps)| {
                    let to: Vec<(f64, f64)> = wps
                        .iter()
                        .map(|(x, y)| (x + applied.0, y + applied.1))
                        .collect();
                    (id.clone(), wps.clone(), to)
                })
                .collect();
            steps.push(Command::SetWaypoints { changes });
        }
    }
    if steps.len() == 1 {
        steps.pop()
    } else {
        Some(Command::Group { steps })
    }
}

/// Drag INTERIOR waypoint `index` of one edge: `from` is the INTERIOR
/// captured at press (the same preview/commit contract as [`drag_move`]
/// — the live preview writes the target first, the commit reads `from`,
/// so re-applying is idempotent). `None` when nothing moves or the
/// index leaves the route.
#[must_use]
pub(crate) fn drag_waypoint(
    id: &Id,
    from: &[(f64, f64)],
    index: usize,
    delta: (f64, f64),
    snap: bool,
) -> Option<Command> {
    let (x, y) = *from.get(index)?;
    let to = drag_target((x, y), delta, snap);
    if to == (x, y) {
        return None;
    }
    let mut points = from.to_vec();
    points[index] = to;
    Some(Command::SetWaypoints {
        changes: vec![(id.clone(), from.to_vec(), points)],
    })
}

/// Insert-and-route as ONE command: the commit of a press-drag on a
/// segment-midpoint insert dot of the selected edge. `from` is the
/// INTERIOR route captured at press (same preview/commit contract as
/// [`drag_waypoint`]); `segment` counts the edge's POLYLINE segments
/// (dot `s` sits between points `s` and `s + 1`), which maps to an
/// insert at INTERIOR index `segment` (appending at the last dot);
/// `at` is the already-snapped release point. Undo removes exactly the
/// spliced vertex — always a command, the press on a dot is never a
/// no-op.
#[must_use]
pub(crate) fn insert_waypoint(
    id: &Id,
    from: &[(f64, f64)],
    segment: usize,
    at: (f64, f64),
) -> Command {
    let mut to = from.to_vec();
    to.insert(segment.min(to.len()), at);
    Command::SetWaypoints {
        changes: vec![(id.clone(), from.to_vec(), to)],
    }
}

/// Land one auto-layout run as a single undoable [`Command`]:
/// the dialog Apply and the adopt-time pass are COMMANDS, not silent
/// state — one Ctrl+Z reverts positions and routes together). `overrides`
/// are the engine's absolute placements/routes ([`crate::layout::apply`]'s
/// output, but headless here); ids already rendering at their placement
/// are skipped, scene iteration order keeps the command deterministic
/// (the [`Overrides`] maps are hash maps). `None` when the run changes
/// nothing.
#[must_use]
pub(crate) fn apply_layout(
    scene: &Scene,
    overlay: &LayoutOverlay,
    overrides: &Overrides,
) -> Option<Command> {
    let moves: Vec<ElementMove> = scene
        .items
        .iter()
        .filter_map(|item| {
            let to = overrides.elements.get(&item.id)?;
            let from = overlay.position(scene, &item.id)?;
            (from != *to).then(|| (item.id.clone(), from, *to))
        })
        .collect();
    let changes: Vec<WaypointChange> = scene
        .edges
        .iter()
        .filter_map(|edge| {
            let to = overrides.relationships.get(&edge.id)?;
            let from = overlay.interior(scene, &edge.id);
            (from != *to).then(|| (edge.id.clone(), from, to.clone()))
        })
        .collect();
    let mut steps = Vec::new();
    if !moves.is_empty() {
        steps.push(Command::MoveElements { moves });
    }
    if !changes.is_empty() {
        steps.push(Command::SetWaypoints { changes });
    }
    match steps.len() {
        0 => None,
        1 => steps.pop(),
        _ => Some(Command::Group { steps }),
    }
}

/// Align target for [`align`] (edges and centre lines).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlignEdge {
    /// Common left edge.
    Left,
    /// Common vertical centre line.
    CenterX,
    /// Common right edge.
    Right,
    /// Common top edge.
    Top,
    /// Common horizontal centre line.
    CenterY,
    /// Common bottom edge.
    Bottom,
}

/// Distribute axis for [`distribute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Axis {
    /// Even horizontal spacing.
    Horizontal,
    /// Even vertical spacing.
    Vertical,
}

/// Resolve each id's effective rect through `target`, collecting the
/// positions that actually change (a factory-level no-op filter).
fn resolve_moves(
    scene: &Scene,
    overlay: &LayoutOverlay,
    ids: &[Id],
    target: impl Fn(Rect) -> (f64, f64),
) -> Vec<ElementMove> {
    ids.iter()
        .filter_map(|id| {
            let rect = overlay.rect(scene, id)?;
            let to = target(rect);
            if to == (rect.x, rect.y) {
                None
            } else {
                Some((id.clone(), (rect.x, rect.y), to))
            }
        })
        .collect()
}

fn non_empty_moves(moves: Vec<ElementMove>) -> Option<Command> {
    if moves.is_empty() {
        None
    } else {
        Some(Command::MoveElements { moves })
    }
}

/// A save failure — errors as values, presented verbatim.
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    /// The shared `oxidizr_json` writer refused the workspace.
    #[error("serialize: {0}")]
    Json(#[from] oxidizr_json::JsonError),
    /// The temp write or the rename onto the target failed.
    #[error("write {}: {source}", .path.display())]
    Io {
        /// The intended target (not the temp name).
        path: PathBuf,
        /// The OS-level reason.
        source: std::io::Error,
    },
}

/// Round world px onto the stored integer grid — `elementViews`,
/// `vertices` and `dimensions` all store `Integer`s and overlay values
/// come from grid-snapped or pixel drags.
#[allow(clippy::cast_possible_truncation)] // clamped into i32::MIN..=MAX first
fn to_px(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// The stored content of one MODEL view by key — `None` for filtered
/// and image views, which carry no geometry of their own to save
/// (saving them is blocked: the base view holds the layout).
fn content_of_key<'a>(workspace: &'a mut Workspace, key: &str) -> Option<&'a mut ViewContent> {
    let views = &mut workspace.views;
    macro_rules! find {
        ($field:ident) => {
            if let Some(view) = views.$field.iter_mut().find(|view| view.base.key == key) {
                return Some(&mut view.content);
            }
        };
    }
    find!(custom_views);
    find!(system_landscape_views);
    find!(system_context_views);
    find!(container_views);
    find!(component_views);
    find!(dynamic_views);
    find!(deployment_views);
    None
}

/// Clone `workspace` and write `overlay`'s deltas into the stored
/// geometry of `view_key`: element-view `x`/`y`, relationship-view
/// `vertices` (replaced wholesale — an empty vec straightens) and the
/// view `dimensions`. The write direction is the read path
/// inverted and NARROWER: only these three geometry slots change —
/// the model, styles and the permanent `automaticLayout` block
/// round-trip untouched (save writes GEOMETRY, never the block).
/// A key without a content slot changes nothing.
#[must_use]
pub fn apply_overlay(workspace: &Workspace, view_key: &str, overlay: &LayoutOverlay) -> Workspace {
    let mut workspace = workspace.clone();
    if let Some(content) = content_of_key(&mut workspace, view_key) {
        for (id, (x, y)) in &overlay.elements {
            if let Some(view) = content.elements.iter_mut().find(|view| view.id == *id) {
                view.x = to_px(*x);
                view.y = to_px(*y);
            }
        }
        for (id, route) in &overlay.relationships {
            if let Some(view) = content.relationships.iter_mut().find(|view| view.id == *id) {
                view.vertices = route
                    .iter()
                    .map(|(x, y)| Vertex {
                        x: to_px(*x),
                        y: to_px(*y),
                    })
                    .collect();
            }
        }
        if overlay.dimensions.is_some() {
            content.dimensions = overlay.dimensions;
        }
    }
    workspace
}

/// The save target for a displayed input: the opened `.json`
/// in place, a `<stem>.json` SIDE CAR next to a `.dsl` source (the DSL
/// text is never rewritten), `None` for stdin (`"-"`).
#[must_use]
pub(crate) fn save_target(display: &str) -> Option<PathBuf> {
    if display == "-" {
        return None;
    }
    let path = PathBuf::from(display);
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("dsl"))
    {
        Some(path.with_extension("json"))
    } else {
        Some(path)
    }
}

/// Serialize with the SHARED JSON writer (the save path adds no second
/// serializer — one byte contract) to a temp file next to the
/// target, then rename it into place: a crash or a full disk never
/// leaves a truncated workspace behind (the same atomic posture the
/// reload writer keeps).
///
/// # Errors
///
/// [`SaveError::Json`] when the writer refuses the workspace,
/// [`SaveError::Io`] when the temp write or rename fails.
pub fn write_workspace(path: &Path, workspace: &Workspace) -> Result<(), SaveError> {
    let bytes = oxidizr_json::to_vec(workspace)?;
    write_atomic(path, &bytes)
}

/// Atomic byte write (shared by save and export): `.{pid}.tmp` beside
/// the target, then rename. The temp
/// suffix never matches the reload watch's exact-path filter, so
/// exports landing in a watched directory cannot trigger a reload.
///
/// # Errors
///
/// [`SaveError::Io`] when the temp write or the rename fails (the
/// error carries `path`).
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
    let name = path.file_name().map_or_else(
        || "workspace.json".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let temp = path.with_file_name(format!("{name}.{}.tmp", std::process::id()));
    let io = |source: std::io::Error| SaveError::Io {
        path: path.to_path_buf(),
        source,
    };
    std::fs::write(&temp, bytes).map_err(io)?;
    std::fs::rename(&temp, path).map_err(io)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // test fixtures
#[allow(clippy::float_cmp)] // exact geometry: equality IS the contract
mod tests {
    use super::*;
    use crate::scene::{Edge, Item};
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{IconPosition, LineStyle, Shape};

    fn id(n: u32) -> Id {
        Id::new(n.to_string())
    }

    fn item(n: u32, x: f64, y: f64) -> Item {
        Item {
            id: id(n),
            kind: ElementKind::SoftwareSystem,
            name: String::new(),
            description: None,
            metadata: None,
            group: None,
            url: None,
            rect: Rect {
                x,
                y,
                width: 100.0,
                height: 50.0,
            },
            shape: Shape::Box,
            background: "#ffffff".to_string(),
            stroke: "#000000".to_string(),
            stroke_width: 2,
            border: LineStyle::Solid,
            color: "#000000".to_string(),
            font_size: 24,
            opacity: 100,
            icon: None,
            icon_position: IconPosition::Top,
        }
    }

    fn edge(n: u32, points: Vec<(f64, f64)>) -> Edge {
        Edge {
            id: id(n),
            source: id(1),
            destination: id(2),
            points,
            label: None,
            metadata: None,
            order: None,
            color: "#000000".to_string(),
            thickness: 2,
            line_style: LineStyle::Solid,
            font_size: 24,
            label_width: 200,
            label_position: 50,
            opacity: 100,
            url: None,
        }
    }

    /// Three boxes (100×50) at (0,0)/(220,60)/(450,0) + a two-vertex
    /// edge (10) and a straight one (11); bounds the 0..550 × 0..110
    /// page plus margin.
    fn scene() -> Scene {
        Scene {
            view_key: "L1".to_string(),
            title: String::new(),
            background: "#ffffff".to_string(),
            boundaries: Vec::new(),
            items: vec![item(1, 0.0, 0.0), item(2, 220.0, 60.0), item(3, 450.0, 0.0)],
            edges: vec![
                edge(
                    10,
                    vec![(50.0, 25.0), (120.0, 40.0), (200.0, 80.0), (270.0, 85.0)],
                ),
                edge(11, vec![(270.0, 85.0), (500.0, 25.0)]),
            ],
            bounds: Rect {
                x: -20.0,
                y: -20.0,
                width: 590.0,
                height: 150.0,
            },
        }
    }

    fn overlay() -> LayoutOverlay {
        LayoutOverlay::for_view("L1")
    }

    fn system(n: u32) -> oxidizr_core::model::SoftwareSystem {
        oxidizr_core::model::SoftwareSystem {
            item: oxidizr_core::model::ModelItem::new(id(n), format!("S{n}")),
            group: None,
            containers: Vec::new(),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        }
    }

    fn rel(n: u32, source: u32, destination: u32) -> Relationship {
        Relationship {
            id: id(n),
            source: id(source),
            destination: id(destination),
            description: None,
            technology: None,
            tags: Tags::new(),
            url: None,
            properties: Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        }
    }

    fn element_view(n: u32) -> oxidizr_core::views::ElementView {
        oxidizr_core::views::ElementView {
            id: id(n),
            x: 0,
            y: 0,
        }
    }

    fn relationship_view(n: u32) -> RelationshipView {
        RelationshipView {
            id: id(n),
            description: None,
            url: None,
            order: None,
            response: None,
            properties: Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        }
    }

    /// Systems 1–3, `1 -[3]-> 2` listed in 1's efferent list; landscape
    /// view `L1` shows boxes 1/2/3 and carries row 3; dynamic view `D1`
    /// also carries row 3 (delete sweeps it, create never injects it);
    /// `Other` shows only box 1 (create never injects there).
    fn workspace() -> Workspace {
        let mut s1 = system(1);
        s1.relationships.push(id(3));
        let landscape = oxidizr_core::views::SystemLandscapeView {
            base: oxidizr_core::views::ViewBase::new("L1", 1),
            content: ViewContent {
                elements: vec![element_view(1), element_view(2), element_view(3)],
                relationships: vec![relationship_view(3)],
                paper_size: None,
                dimensions: None,
                automatic_layout: None,
            },
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        };
        let other = oxidizr_core::views::SystemLandscapeView {
            base: oxidizr_core::views::ViewBase::new("Other", 2),
            content: ViewContent {
                elements: vec![element_view(1)],
                relationships: Vec::new(),
                paper_size: None,
                dimensions: None,
                automatic_layout: None,
            },
            enterprise_boundary_visible: true,
            animations: Vec::new(),
        };
        let dynamic = oxidizr_core::views::DynamicView {
            base: oxidizr_core::views::ViewBase::new("D1", 3),
            content: ViewContent {
                elements: vec![element_view(1), element_view(2)],
                relationships: vec![relationship_view(3)],
                paper_size: None,
                dimensions: None,
                automatic_layout: None,
            },
            element_id: None,
            deployment_environment: None,
            external_boundaries_visible: false,
        };
        Workspace {
            model: Model {
                people: Vec::new(),
                custom_elements: Vec::new(),
                software_systems: vec![s1, system(2), system(3)],
                deployment_nodes: Vec::new(),
                relationships: vec![rel(3, 1, 2)],
                properties: Properties::new(),
            },
            views: ViewSet {
                custom_views: Vec::new(),
                system_landscape_views: vec![landscape, other],
                system_context_views: Vec::new(),
                container_views: Vec::new(),
                component_views: Vec::new(),
                dynamic_views: vec![dynamic],
                deployment_views: Vec::new(),
                filtered_views: Vec::new(),
                image_views: Vec::new(),
                configuration: oxidizr_core::views::ViewConfiguration::default(),
            },
            ..Workspace::default()
        }
    }

    #[test]
    fn move_by_round_trips_and_snaps() {
        let scene = scene();
        let mut o = overlay();
        let cmd = move_by(&scene, &o, &[id(1)], (15.0, 5.0), false).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![(id(1), (0.0, 0.0), (15.0, 5.0))]
            }
        );
        cmd.apply(&mut o, true);
        assert_eq!(o.position(&scene, &id(1)), Some((15.0, 5.0)));
        // The overlay wins even where the scene box still says stored.
        cmd.undo(&mut o);
        assert_eq!(o.position(&scene, &id(1)), Some((0.0, 0.0)));
        // Snap ON moves (0,0) by (15,5) to the (10,10)/(20,10) grid.
        let cmd = move_by(&scene, &o, &[id(1)], (14.0, 4.0), true).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![(id(1), (0.0, 0.0), (10.0, 0.0))]
            }
        );
        // Zero delta = no command at all.
        assert!(move_by(&scene, &o, &[id(1)], (0.0, 0.0), true).is_none());
    }

    #[test]
    fn align_targets_the_selection_bounding_box() {
        let scene = scene();
        let o = overlay();
        let all = [id(1), id(2), id(3)];
        let cmd = align(&scene, &o, &all, AlignEdge::Left).unwrap();
        // Everything not at x = 0 comes to the left edge: box 2 (220)
        // and box 3 (450) move; box 1 is already there.
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![
                    (id(2), (220.0, 60.0), (0.0, 60.0)),
                    (id(3), (450.0, 0.0), (0.0, 0.0)),
                ]
            }
        );
        // Right: the right edge is box 3's 550 → every box ends at
        // x = 450; box 3 is already there.
        let cmd = align(&scene, &o, &all, AlignEdge::Right).unwrap();
        let moved: Vec<(String, (f64, f64))> = cmd_moves(&cmd);
        assert!(moved.contains(&("1".to_string(), (450.0, 0.0))));
        assert!(moved.contains(&("2".to_string(), (450.0, 60.0))));
        // Centre-Y: the shared centre line is y = 55 → every 50-px box
        // at its TOP y = 30; box 2 slides 60 → 30, boxes 1/3 lift 30.
        let cmd = align(&scene, &o, &all, AlignEdge::CenterY).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![
                    (id(1), (0.0, 0.0), (0.0, 30.0)),
                    (id(2), (220.0, 60.0), (220.0, 30.0)),
                    (id(3), (450.0, 0.0), (450.0, 30.0)),
                ]
            }
        );
        // Under two elements: nothing to align.
        assert!(align(&scene, &o, &[id(1)], AlignEdge::Left).is_none());
    }

    #[test]
    fn distribute_holds_the_extremes_and_evencs_the_gaps() {
        let scene = scene();
        let o = overlay();
        let all = [id(1), id(2), id(3)];
        let cmd = distribute(&scene, &o, &all, Axis::Horizontal).unwrap();
        // Span 0..550, sizes 300, gap 125: boxes land at 0/225/450 —
        // box 2 moves 220 → 225, the extremes stay.
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![(id(2), (220.0, 60.0), (225.0, 60.0))]
            }
        );
        assert!(distribute(&scene, &o, &[id(1), id(2)], Axis::Vertical).is_none());
    }

    #[test]
    fn centre_view_and_bring_back_move_whats_off() {
        let scene = scene();
        let o = overlay();
        // Centre the selection (1 + 3: bbox 0..550 × 0..50, centre
        // 275/25) on the bounds centre (275/55): pure +30 in y.
        let cmd = centre_view(&scene, &o, &[id(1), id(3)]).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![
                    (id(1), (0.0, 0.0), (0.0, 30.0)),
                    (id(3), (450.0, 0.0), (450.0, 30.0)),
                ]
            }
        );
        // Bring back onto a 400×400 page: box 3 (right edge 550) slides
        // back to 300; the others are fully on and never move.
        let page = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 400.0,
        };
        let cmd = bring_back(&scene, &o, page).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![(id(3), (450.0, 0.0), (300.0, 0.0))]
            }
        );
        // The full page contains everything: no command.
        let page = Rect {
            x: -20.0,
            y: -20.0,
            width: 590.0,
            height: 150.0,
        };
        assert!(bring_back(&scene, &o, page).is_none());
    }

    #[test]
    fn waypoint_and_canvas_commands_round_trip() {
        let scene = scene();
        let mut o = overlay();
        // Remove-all: only edge 10 renders vertices; edge 11 is skipped.
        let cmd = remove_all_vertices(&scene, &o).unwrap();
        assert_eq!(
            cmd,
            Command::SetWaypoints {
                changes: vec![(id(10), vec![(120.0, 40.0), (200.0, 80.0)], Vec::new())]
            }
        );
        cmd.apply(&mut o, true);
        assert_eq!(o.interior(&scene, &id(10)), Vec::new());
        assert!(remove_all_vertices(&scene, &o).is_none());
        // The override REPLACES: an interior list the scene never had
        // round-trips through set_waypoints too.
        let cmd = set_waypoints(&scene, &o, &id(11), vec![(300.0, 300.0)]).unwrap();
        cmd.apply(&mut o, true);
        assert_eq!(o.interior(&scene, &id(11)), vec![(300.0, 300.0)]);
        assert!(set_waypoints(&scene, &o, &id(11), vec![(300.0, 300.0)]).is_none());
        // Canvas size.
        let dims = Dimensions {
            width: 800,
            height: 600,
        };
        let cmd = set_canvas_size(&o, Some(dims)).unwrap();
        cmd.apply(&mut o, true);
        assert_eq!(o.dimensions, Some(dims));
        cmd.undo(&mut o);
        assert_eq!(o.dimensions, None);
        assert!(set_canvas_size(&o, None).is_none());
    }

    #[test]
    fn clear_view_resets_and_restores_the_snapshot() {
        let scene = scene();
        let mut o = overlay();
        let cmd = move_by(&scene, &o, &[id(1)], (40.0, 40.0), false).unwrap();
        cmd.apply(&mut o, true);
        let cmd = set_canvas_size(
            &o,
            Some(Dimensions {
                width: 5,
                height: 5,
            }),
        )
        .unwrap();
        cmd.apply(&mut o, true);
        let dirty = o.clone();

        let cmd = clear_view(&o);
        cmd.apply(&mut o, true);
        assert!(o.is_empty());
        assert_eq!(o.view_key, "L1", "the identity survives");
        cmd.undo(&mut o);
        assert_eq!(o, dirty);
    }

    #[test]
    fn history_round_trips_clears_redo_and_caps() {
        let scene = scene();
        let mut ws = Workspace::default();
        let mut overlays = HashMap::from([("L1".to_string(), overlay())]);
        let mut h = History::default();
        let one = move_by(&scene, &overlays["L1"], &[id(1)], (10.0, 0.0), false).unwrap();
        h.run_geometry(&mut overlays, "L1", one);
        assert!(h.can_undo() && !h.can_redo());
        assert_eq!(
            h.undo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("L1".to_string()))
        );
        assert!(!h.can_undo() && h.can_redo());
        assert_eq!(overlays["L1"].position(&scene, &id(1)), Some((0.0, 0.0)));
        assert_eq!(
            h.redo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("L1".to_string()))
        );
        assert_eq!(overlays["L1"].position(&scene, &id(1)), Some((10.0, 0.0)));
        // A fresh run kills the redo branch.
        h.undo(&mut ws, &mut overlays);
        let two = move_by(&scene, &overlays["L1"], &[id(2)], (0.0, 20.0), false).unwrap();
        h.run_geometry(&mut overlays, "L1", two);
        assert!(!h.can_redo());
        assert!(h.undo(&mut ws, &mut overlays).is_some());
        assert!(
            h.undo(&mut ws, &mut overlays).is_none(),
            "the fresh run dropped the redo branch — exhausted, no panic"
        );
        // Cap: 205 runs keep the newest 200.
        h.reset();
        for _ in 0..(HISTORY_CAP + 5) {
            let cmd = move_by(&scene, &overlays["L1"], &[id(3)], (10.0, 0.0), false).unwrap();
            h.run_geometry(&mut overlays, "L1", cmd);
        }
        let mut undos = 0;
        while h.undo(&mut ws, &mut overlays).is_some() {
            undos += 1;
        }
        assert_eq!(undos, HISTORY_CAP);
    }

    #[test]
    fn history_walks_through_views() {
        let scene = scene();
        let mut ws = Workspace::default();
        let mut overlays: HashMap<String, LayoutOverlay> = HashMap::new();
        let mut h = History::default();
        // Two edits made while viewing two DIFFERENT views: overlays are
        // created on demand, the steps carry their view.
        let a = move_by(
            &scene,
            &LayoutOverlay::for_view("A"),
            &[id(1)],
            (10.0, 0.0),
            false,
        )
        .unwrap();
        h.run_geometry(&mut overlays, "A", a);
        let b = move_by(
            &scene,
            &LayoutOverlay::for_view("B"),
            &[id(2)],
            (0.0, 20.0),
            false,
        )
        .unwrap();
        h.run_geometry(&mut overlays, "B", b);
        // One stack: undo walks back through the views in LIFO order.
        assert_eq!(
            h.undo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("B".to_string()))
        );
        assert_eq!(
            h.undo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("A".to_string()))
        );
        assert!(h.undo(&mut ws, &mut overlays).is_none());
        assert_eq!(overlays["A"].position(&scene, &id(1)), Some((0.0, 0.0)));
        assert_eq!(overlays["B"].position(&scene, &id(2)), Some((220.0, 60.0)));
        // Redo walks forward through both again.
        assert_eq!(
            h.redo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("A".to_string()))
        );
        assert_eq!(
            h.redo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("B".to_string()))
        );
        assert!(h.redo(&mut ws, &mut overlays).is_none());
        assert_eq!(overlays["A"].position(&scene, &id(1)), Some((10.0, 0.0)));
        assert_eq!(overlays["B"].position(&scene, &id(2)), Some((220.0, 80.0)));
        // reset drops everything (workspace open).
        h.reset();
        assert!(h.undo(&mut ws, &mut overlays).is_none());
    }

    #[test]
    fn overlay_readers_prefer_the_delta() {
        let scene = scene();
        let o = LayoutOverlay {
            view_key: "L1".to_string(),
            elements: HashMap::from([(id(9), (1.0, 2.0))]),
            relationships: HashMap::from([(id(9), Vec::new())]),
            dimensions: None,
        };
        // Unknown ids: None from the readers that need the scene.
        assert_eq!(o.position(&scene, &id(9)), Some((1.0, 2.0)));
        assert_eq!(o.position(&scene, &id(1)), Some((0.0, 0.0)));
        assert_eq!(o.rect(&scene, &id(9)), None);
        assert_eq!(o.rect(&scene, &id(1)).unwrap().width, 100.0);
        // Empty vec IS a delta (straight) — interior returns it, not the
        // scene's stored pair (edge 10 has two vertices stored).
        let o2 = LayoutOverlay {
            relationships: HashMap::from([(id(10), Vec::new())]),
            ..LayoutOverlay::for_view("L1")
        };
        assert_eq!(o2.interior(&scene, &id(10)), Vec::new());
        // Without a delta the stored interior excludes the centres.
        assert_eq!(
            overlay().interior(&scene, &id(10)),
            vec![(120.0, 40.0), (200.0, 80.0)]
        );
        assert_eq!(overlay().interior(&scene, &id(11)), Vec::new());
        // as_overrides carries both maps verbatim.
        let ov = o2.as_overrides();
        assert_eq!(ov.relationships[&id(10)], [] as [(f64, f64); 0]);
        assert!(ov.elements.is_empty());
    }

    #[test]
    fn drag_move_snaps_and_survives_the_live_preview() {
        let scene = scene();
        let mut o = overlay();
        // The shell captures the origins at press; the live preview has
        // already written the target position into the overlay — the
        // commit reads `from`, never the mutated state.
        o.elements.insert(id(1), (15.0, 5.0));
        let from = vec![(id(1), (0.0, 0.0))];
        let cmd = drag_move(&from, (15.0, 5.0), true).unwrap();
        assert_eq!(
            cmd,
            Command::MoveElements {
                moves: vec![(id(1), (0.0, 0.0), (20.0, 10.0))]
            }
        );
        cmd.apply(&mut o, true);
        assert_eq!(o.position(&scene, &id(1)), Some((20.0, 10.0)));
        // Re-applying the absolute targets is idempotent: the preview
        // and the committed command write the SAME geometry.
        cmd.apply(&mut o, true);
        assert_eq!(o.position(&scene, &id(1)), Some((20.0, 10.0)));
        cmd.undo(&mut o);
        assert_eq!(o.position(&scene, &id(1)), Some((0.0, 0.0)));
        // A drag that snaps back onto the origin commits nothing.
        assert!(drag_move(&from, (4.0, 0.0), true).is_none());
    }

    #[test]
    fn carried_interiors_follow_fully_contained_edges_only() {
        let scene = scene();
        let o = overlay();
        // Edge 10 (1→2, two vertices) sits fully inside group [1,2];
        // edge 11 is contained too but straight (no interior to carry).
        let carried = carried_interiors(&scene, &o, &[id(1), id(2)]);
        assert_eq!(carried.len(), 1, "only the bent edge carries");
        assert_eq!(carried[0].0, id(10));
        assert_eq!(carried[0].1.len(), 2);
        // A group holding only ONE endpoint leaves the route alone.
        assert_eq!(
            carried_interiors(&scene, &o, &[id(1)]),
            Vec::new(),
            "one shared endpoint is not containment"
        );
        assert_eq!(carried_interiors(&scene, &o, &[id(3)]), Vec::new());
    }

    #[test]
    fn drag_group_carries_routes_and_detaches_on_demand() {
        let scene = scene();
        let mut o = overlay();
        let from = vec![(id(1), (0.0, 0.0)), (id(2), (220.0, 60.0))];
        let carries = vec![(id(10), vec![(100.0, 20.0), (180.0, 70.0)])];
        let cmd = drag_group(&from, &carries, (10.0, 10.0), false, true).unwrap();
        assert!(
            matches!(&cmd, Command::Group { steps } if steps.len() == 2),
            "boxes + routes land as ONE undo step: {cmd:?}"
        );
        cmd.apply(&mut o, true);
        assert_eq!(o.position(&scene, &id(1)), Some((10.0, 10.0)));
        assert_eq!(o.position(&scene, &id(2)), Some((230.0, 70.0)));
        assert_eq!(
            o.relationships.get(&id(10)).unwrap(),
            &vec![(110.0, 30.0), (190.0, 80.0)],
            "the route translated by exactly the applied delta"
        );
        // One undo walks BOTH steps back to the press values.
        cmd.undo(&mut o);
        assert_eq!(o.position(&scene, &id(1)), Some((0.0, 0.0)));
        assert_eq!(
            o.relationships.get(&id(10)),
            Some(&vec![(100.0, 20.0), (180.0, 70.0)])
        );
        // Ctrl+drag (`with_waypoints` false): plain move, routes stay.
        let cmd = drag_group(&from, &carries, (10.0, 10.0), false, false).unwrap();
        assert!(
            matches!(&cmd, Command::MoveElements { .. }),
            "no carry → no Group: {cmd:?}"
        );
        // Nothing moves → nothing to commit (not even routes).
        assert!(drag_group(&[], &carries, (10.0, 10.0), false, true).is_none());
    }

    #[test]
    fn drag_waypoint_moves_one_vertex_and_survives_preview() {
        let scene = scene();
        let mut o = overlay();
        // The live preview already wrote the target into the overlay;
        // the commit reads the press-captured interior, not the
        // mutated state (a handle drag).
        o.relationships
            .insert(id(10), vec![(120.0, 40.0), (215.0, 85.0)]);
        let from = [(120.0, 40.0), (200.0, 80.0)];
        let cmd = drag_waypoint(&id(10), &from, 1, (15.0, 5.0), true).unwrap();
        assert_eq!(
            cmd,
            Command::SetWaypoints {
                changes: vec![(
                    id(10),
                    vec![(120.0, 40.0), (200.0, 80.0)],
                    vec![(120.0, 40.0), (220.0, 90.0)]
                )]
            }
        );
        cmd.apply(&mut o, true);
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (220.0, 90.0)]
        );
        // Idempotent re-apply: preview and commit write the SAME route.
        cmd.apply(&mut o, true);
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (220.0, 90.0)]
        );
        cmd.undo(&mut o);
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (200.0, 80.0)]
        );
        // Snapped back onto the vertex: nothing to commit; an index
        // outside the route never mutates anything either.
        assert!(drag_waypoint(&id(10), &from, 1, (4.0, 0.0), true).is_none());
        assert!(drag_waypoint(&id(10), &from, 2, (10.0, 0.0), false).is_none());
    }

    #[test]
    fn insert_waypoint_splices_one_vertex_undone_exactly() {
        let scene = scene();
        let mut o = overlay();
        // Press on the midpoint dot of SEGMENT 1 (polyline points 1→2):
        // the commit splices the dragged point at INTERIOR index 1.
        let from = vec![(120.0, 40.0), (200.0, 80.0)];
        let cmd = insert_waypoint(&id(10), &from, 1, (150.0, 60.0));
        cmd.apply(&mut o, true);
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (150.0, 60.0), (200.0, 80.0)]
        );
        // Undo removes exactly the spliced vertex.
        cmd.undo(&mut o);
        assert_eq!(o.interior(&scene, &id(10)), from);
        // The LAST dot (segment == interior count) appends.
        let cmd = insert_waypoint(&id(10), &from, 2, (240.0, 85.0));
        cmd.apply(&mut o, true);
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (200.0, 80.0), (240.0, 85.0)]
        );
        cmd.undo(&mut o);
        assert_eq!(o.interior(&scene, &id(10)), from);
        // A straight edge (no interior route): its single dot inserts
        // the first vertex.
        let cmd = insert_waypoint(&id(11), &[], 0, (385.0, 55.0));
        cmd.apply(&mut o, true);
        assert_eq!(o.interior(&scene, &id(11)), vec![(385.0, 55.0)]);
        cmd.undo(&mut o);
        assert_eq!(o.interior(&scene, &id(11)), Vec::new());
    }

    #[test]
    fn apply_layout_lands_engine_output_as_one_command() {
        let scene = scene();
        let mut o = overlay();
        let overrides = Overrides {
            elements: HashMap::from([(id(1), (0.0, 0.0)), (id(2), (300.0, 60.0))]),
            relationships: HashMap::from([(id(10), Vec::new()), (id(11), vec![(300.0, 50.0)])]),
        };
        // Box 1 already renders at (0,0): skipped. Edge 10 straightens
        // and edge 11 gains a waypoint → moves + changes GROUP into one
        // undoable step.
        let cmd = apply_layout(&scene, &o, &overrides).unwrap();
        let Command::Group { steps } = &cmd else {
            panic!("expected a Group, got {cmd:?}");
        };
        assert_eq!(steps.len(), 2);
        cmd.apply(&mut o, true);
        assert_eq!(o.position(&scene, &id(2)), Some((300.0, 60.0)));
        assert_eq!(o.interior(&scene, &id(10)), Vec::new());
        assert_eq!(o.interior(&scene, &id(11)), vec![(300.0, 50.0)]);
        // Undo writes the `from` geometry back: the overlay RENDERS as
        // before (the entries keep the stored values,
        // they are not erased), and a save would write the identical
        // file.
        cmd.undo(&mut o);
        assert_eq!(o.position(&scene, &id(2)), Some((220.0, 60.0)));
        assert_eq!(
            o.interior(&scene, &id(10)),
            vec![(120.0, 40.0), (200.0, 80.0)]
        );
        assert_eq!(o.interior(&scene, &id(11)), Vec::new());
        // Position-only output is a plain MoveElements (no Group).
        let only_moves = Overrides {
            relationships: HashMap::new(),
            ..overrides
        };
        let cmd = apply_layout(&scene, &o, &only_moves).unwrap();
        assert!(matches!(cmd, Command::MoveElements { .. }));
        // Everything already laid out: nothing to run.
        let laid = LayoutOverlay {
            view_key: "L1".to_string(),
            elements: HashMap::from([
                (id(1), (0.0, 0.0)),
                (id(2), (220.0, 60.0)),
                (id(3), (450.0, 0.0)),
            ]),
            relationships: HashMap::from([
                (id(10), vec![(120.0, 40.0), (200.0, 80.0)]),
                (id(11), Vec::new()),
            ]),
            dimensions: None,
        };
        assert!(apply_layout(&scene, &laid, &overrides_only_matching(&laid)).is_none());
    }

    /// The engine output that reproduces `laid` exactly (identity run).
    fn overrides_only_matching(laid: &LayoutOverlay) -> Overrides {
        Overrides {
            elements: laid.elements.clone(),
            relationships: laid.relationships.clone(),
        }
    }

    fn cmd_moves(cmd: &Command) -> Vec<(String, (f64, f64))> {
        match cmd {
            Command::MoveElements { moves } => moves
                .iter()
                .map(|(id, _, to)| (id.as_str().to_string(), *to))
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn save_target_maps_sources_onto_json() {
        // An opened .json is rewritten in place.
        assert_eq!(
            save_target("/tmp/workspace.json"),
            Some(PathBuf::from("/tmp/workspace.json"))
        );
        // A .dsl never gains new bytes — the sidecar takes the geometry.
        assert_eq!(
            save_target("/tmp/demo.dsl"),
            Some(PathBuf::from("/tmp/demo.json"))
        );
        // Uppercase extension still recognised; stdin has no target.
        assert_eq!(
            save_target("/tmp/Demo.DSL"),
            Some(PathBuf::from("/tmp/Demo.json"))
        );
        assert_eq!(save_target("-"), None);
    }

    #[test]
    fn apply_overlay_then_write_saves_idempotent_json() {
        use oxidizr_api::open;
        use oxidizr_api::pipeline::{ApiOptions, Input};

        // A real open (not a hand-built model): `from_slice` runs the
        // same validation the JSON reader applies, so the fixture must
        // be a real workspace the same way the golden_paint tests are.
        let dir = std::env::temp_dir().join(format!("oxidizr-m35-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dsl = dir.join("demo.dsl");
        std::fs::write(
            &dsl,
            "workspace \"Save Demo\" {\n    model {\n        alice = person \"Alice\"\n        \
             bank = softwaresystem \"Bank\"\n        alice -> bank \"Uses\"\n    }\n}",
        )
        .unwrap();
        let opened = open(&Input::File(dsl), &ApiOptions::default()).unwrap();
        let first = opened.workspace.views.all_views().remove(0);
        let key = first.key().to_owned();
        let content = first.content().unwrap();
        let element = content.elements[0].id.clone();
        let relationship = content.relationships[0].id.clone();

        // The write direction is the read path inverted: top-lefts,
        // interiors, dimensions — rounded to the stored i32 grid.
        let mut overlay = LayoutOverlay::for_view(&key);
        overlay.elements.insert(element.clone(), (222.4, 333.6));
        overlay
            .relationships
            .insert(relationship.clone(), vec![(10.0, 20.0), (30.0, 40.0)]);
        overlay.dimensions = Some(Dimensions {
            width: 1234,
            height: 567,
        });
        let applied = apply_overlay(&opened.workspace, &key, &overlay);
        let views = applied.views.all_views();
        let view = views.first().unwrap();
        let saved = view.content().unwrap();
        let stored = saved.elements.iter().find(|v| v.id == element).unwrap();
        assert_eq!((stored.x, stored.y), (222, 334), "0.5 away rounds");
        let stored = saved
            .relationships
            .iter()
            .find(|v| v.id == relationship)
            .unwrap();
        assert_eq!(
            stored
                .vertices
                .iter()
                .map(|v| (v.x, v.y))
                .collect::<Vec<_>>(),
            vec![(10, 20), (30, 40)]
        );
        assert_eq!(
            saved.dimensions,
            Some(Dimensions {
                width: 1234,
                height: 567
            })
        );
        // An unknown key (a filtered view, stdin) changes nothing.
        assert_eq!(
            apply_overlay(&opened.workspace, "ghost", &overlay),
            opened.workspace
        );

        // Sidecar target, temp+rename write, reader round-trip. The
        // JSON READ applies its own normalizations (an empty
        // `description` string binds as `Some("")`, tags are rebuilt
        // from the serialized form) and the writer sorts the entry
        // arrays by id — so the contract is not struct equality but
        // BYTE stability: re-writing the re-read workspace reproduces
        // the file exactly (the property that makes repeated saves
        // idempotent).
        let target = save_target(&opened.display).unwrap();
        assert_eq!(target, dir.join("demo.json"));
        write_workspace(&target, &applied).unwrap();
        let bytes = std::fs::read(&target).unwrap();
        let parsed = oxidizr_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("Save Demo"), "model untouched");
        assert_eq!(oxidizr_json::to_vec(&parsed).unwrap(), bytes);
    }

    // ---- model commands -----------------------------------------------------

    #[test]
    fn create_relationship_stores_the_canonical_row() {
        let ws = workspace();
        let cmd = create_relationship(&ws, &id(1), &id(3)).unwrap();
        let ModelCommand::CreateRelationship { relationship } = &cmd else {
            panic!("create builds its own variant");
        };
        // The shared counter knows 1/2/3 (elements) + 3 (relationship).
        assert_eq!(relationship.id, id(4), "fresh id from the shared counter");
        assert_eq!(relationship.source, id(1));
        assert_eq!(relationship.destination, id(3));
        assert!(
            relationship.description.is_none(),
            "bare arrows carry no description"
        );
        assert!(
            relationship.tags.stored().is_empty(),
            "the Relationship tag is DERIVED, never stored"
        );
    }

    #[test]
    fn create_and_undo_round_trip_the_workspace() {
        let ws = workspace();
        let cmd = create_relationship(&ws, &id(1), &id(3)).unwrap();
        let mut applied = ws.clone();
        let mut overlays = HashMap::from([("Other".to_string(), LayoutOverlay::for_view("Other"))]);
        cmd.apply(&mut applied, &mut overlays);
        // Model row + efferent list.
        assert!(applied.model.relationships.iter().any(|r| r.id == id(4)));
        let s1 = applied
            .model
            .software_systems
            .iter()
            .find(|s| s.item.id == id(1))
            .unwrap();
        assert_eq!(
            s1.relationships,
            vec![id(3), id(4)],
            "appended to the source's list"
        );
        // Injected where BOTH endpoints show (L1), not into `Other`
        // (only box 1) and never into the dynamic view (not static).
        let row_in = |key: &str| {
            let content = all_contents(&applied.views)
                .into_iter()
                .find(|(k, _)| *k == key)
                .unwrap()
                .1;
            content.relationships.iter().any(|r| r.id == id(4))
        };
        assert!(row_in("L1"), "both boxes show in L1");
        assert!(!row_in("Other"), "box 3 absent there");
        assert!(!row_in("D1"), "dynamic views are not an injection target");
        cmd.undo(&mut applied, &mut overlays);
        assert_eq!(applied, ws, "undo leaves no trace");
    }

    #[test]
    fn create_rejects_unknown_ends_loops_and_duplicates() {
        let mut ws = workspace();
        assert!(
            create_relationship(&ws, &id(1), &id(99)).is_none(),
            "unknown destination"
        );
        assert!(
            create_relationship(&ws, &id(99), &id(1)).is_none(),
            "unknown source"
        );
        assert!(
            create_relationship(&ws, &id(1), &id(1)).is_none(),
            "self-loop"
        );
        assert!(
            create_relationship(&ws, &id(1), &id(2)).is_none(),
            "1 -[3]-> 2 already has an empty description — the duplicate check would reject a second"
        );
        // A described sibling does NOT block: empty-description uniqueness only.
        ws.model.relationships[0].description = Some("uses".to_string());
        assert!(create_relationship(&ws, &id(1), &id(2)).is_some());
    }

    #[test]
    fn delete_and_undo_round_trip_the_workspace() {
        let ws = workspace();
        let mut overlays = HashMap::from([(
            "L1".to_string(),
            LayoutOverlay {
                relationships: HashMap::from([(id(3), vec![(1.0, 2.0), (3.0, 4.0)])]),
                ..LayoutOverlay::for_view("L1")
            },
        )]);
        let cmd = delete_relationship(&ws, &overlays, &id(3)).unwrap();
        let mut applied = ws.clone();
        cmd.apply(&mut applied, &mut overlays);
        assert_eq!(applied.model.relationships, [] as [Relationship; 0]);
        assert_eq!(
            applied.model.software_systems[0].relationships,
            [] as [Id; 0],
            "the efferent slot is gone"
        );
        for (_, content) in all_contents(&applied.views) {
            assert!(
                content.relationships.iter().all(|r| r.id != id(3)),
                "row swept from every content-bearing view"
            );
        }
        assert!(
            !overlays["L1"].relationships.contains_key(&id(3)),
            "the overlay route goes too"
        );
        cmd.undo(&mut applied, &mut overlays);
        assert_eq!(
            applied, ws,
            "row, slot and per-view rows restore at their exact index"
        );
        assert_eq!(
            overlays["L1"].relationships[&id(3)],
            vec![(1.0, 2.0), (3.0, 4.0)]
        );
        assert!(
            delete_relationship(&ws, &overlays, &id(99)).is_none(),
            "unknown id"
        );
    }

    #[test]
    fn reconnect_moves_efferent_membership_and_round_trips() {
        let ws = workspace();
        // Source move: the id re-homes from system 1's list into 3's.
        let cmd = reconnect_relationship(&ws, &id(3), Endpoint::Source, &id(3)).unwrap();
        let mut applied = ws.clone();
        cmd.apply(&mut applied, &mut HashMap::new());
        assert_eq!(applied.model.relationships[0].source, id(3));
        assert_eq!(
            applied.model.software_systems[0].relationships,
            [] as [Id; 0]
        );
        assert_eq!(applied.model.software_systems[2].relationships, vec![id(3)]);
        cmd.undo(&mut applied, &mut HashMap::new());
        assert_eq!(applied, ws, "list slot and endpoint restore");
        // Destination move: model row only, no list is touched.
        let cmd = reconnect_relationship(&ws, &id(3), Endpoint::Destination, &id(3)).unwrap();
        cmd.apply(&mut applied, &mut HashMap::new());
        assert_eq!(applied.model.relationships[0].destination, id(3));
        assert_eq!(
            applied.model.software_systems[1].relationships,
            ws.model.software_systems[1].relationships,
            "the destination owns no list"
        );
        cmd.undo(&mut applied, &mut HashMap::new());
        assert_eq!(applied, ws);
    }

    #[test]
    fn reconnect_rejects_no_ops_own_ends_and_unknowns() {
        let ws = workspace();
        assert!(
            reconnect_relationship(&ws, &id(3), Endpoint::Source, &id(1)).is_none(),
            "already there"
        );
        assert!(
            reconnect_relationship(&ws, &id(3), Endpoint::Source, &id(2)).is_none(),
            "would point at its own destination"
        );
        assert!(
            reconnect_relationship(&ws, &id(3), Endpoint::Destination, &id(99)).is_none(),
            "unknown element"
        );
        assert!(
            reconnect_relationship(&ws, &id(99), Endpoint::Source, &id(3)).is_none(),
            "unknown relationship"
        );
    }

    #[test]
    fn set_text_round_trips_and_clears() {
        let ws = workspace();
        let cmd = set_relationship_text(&ws, &id(3), TextField::Description, Some("uses".into()))
            .unwrap();
        let mut applied = ws.clone();
        cmd.apply(&mut applied, &mut HashMap::new());
        assert_eq!(
            applied.model.relationships[0].description.as_deref(),
            Some("uses")
        );
        cmd.undo(&mut applied, &mut HashMap::new());
        assert_eq!(applied, ws);
        // Empty text CLEARS the slot (the JSON omits absent slots).
        let described = {
            let mut w = ws.clone();
            w.model.relationships[0].description = Some("uses".into());
            w
        };
        let cmd = set_relationship_text(
            &described,
            &id(3),
            TextField::Description,
            Some(String::new()),
        )
        .unwrap();
        let ModelCommand::SetRelationshipText { to, .. } = &cmd else {
            panic!("text edits build their own variant");
        };
        assert_eq!(*to, None, "empty normalizes to absent");
        cmd.apply(&mut applied, &mut HashMap::new());
        assert!(applied.model.relationships[0].description.is_none());
        // Technology rides the same rail; unchanged values are no-ops.
        let cmd =
            set_relationship_text(&ws, &id(3), TextField::Technology, Some("HTTP".into())).unwrap();
        cmd.apply(&mut applied, &mut HashMap::new());
        assert_eq!(
            applied.model.relationships[0].technology.as_deref(),
            Some("HTTP")
        );
        assert!(
            set_relationship_text(&ws, &id(3), TextField::Description, None).is_none(),
            "absent onto absent changes nothing"
        );
        assert!(
            set_relationship_text(&ws, &id(99), TextField::Description, Some("x".into())).is_none(),
            "unknown id"
        );
    }

    #[test]
    fn history_walks_model_and_geometry_together() {
        let scene = scene();
        let mut ws = workspace();
        let mut overlays = HashMap::from([("L1".to_string(), overlay())]);
        let mut h = History::default();
        // A geometry step, then a model step: one stack, LIFO across kinds.
        let geo = move_by(&scene, &overlays["L1"], &[id(1)], (10.0, 0.0), false).unwrap();
        h.run_geometry(&mut overlays, "L1", geo);
        let create = create_relationship(&ws, &id(1), &id(3)).unwrap();
        let expected = ws.clone();
        h.run_model(&mut ws, &mut overlays, create);
        assert_ne!(ws, expected, "the create landed on the workspace");
        assert_eq!(h.undo(&mut ws, &mut overlays), Some(Stepped::Model));
        assert_eq!(ws, expected, "the model step undid");
        assert_eq!(
            h.undo(&mut ws, &mut overlays),
            Some(Stepped::Geometry("L1".to_string()))
        );
        assert!(!h.can_undo());
        // And forward again, model last.
        assert!(h.redo(&mut ws, &mut overlays).is_some());
        assert_eq!(h.redo(&mut ws, &mut overlays), Some(Stepped::Model));
        assert_ne!(ws, expected);
    }
}
