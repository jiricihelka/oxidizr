//! Invertible editing commands + undo/redo stacks (P3-D4, P3-M3-2):
//! every edit is a [`Command`] mutating only the [`LayoutOverlay`] —
//! geometry deltas over the read-only workspace; the model and DSL text
//! are immutable (editor rule, P3-D0). Commands carry their own inverse
//! (`from`/`to` pairs), so undo is a symmetric re-apply — no closures
//! on the stack, and a saved session never needs to serialise one.
//!
//! Factories ([`move_by`], [`align`], [`distribute`], [`centre_view`],
//! [`bring_back`], [`remove_all_vertices`], [`set_waypoints`], ...)
//! compute the delta from the CURRENT effective geometry: the overlay
//! first, the scene (already built with the overlay) as the stored
//! fallback — `from` is always exactly what the user sees. Every
//! factory returns `None` when nothing would move, so the shell never
//! pushes a no-op onto [`History`]. The auto-layout engine lands through
//! the same door: the shell turns one run into `MoveElements` +
//! `SetWaypoints` and it becomes undoable like any edit (P3-M3 row).
//!
//! Reference parity (§P3-D10 V4, monorepo `ad34036`): the layout
//! semantics follow the web tool — `removeAllVertices` before every
//! auto-layout run, Bring-elements-back and Remove-all-vertices as
//! dialog-level commands, per-diagram undo stacks. Grid snap (10 px) is
//! OURS (visual parity out of contract, P3-D0); the reference snaps
//! nothing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use oxidizr_core::Id;
use oxidizr_core::Workspace;
use oxidizr_core::views::{Dimensions, Vertex, ViewContent};

use crate::scene::{Overrides, Rect, Scene};

/// Grid-snap pitch in world px (§P3-D4: 10 px, toggle in the shell).
pub const GRID_PX: f64 = 10.0;
/// Undo-stack cap (§P3-D4: 200 — the oldest command drops off first).
pub const HISTORY_CAP: usize = 200;

/// One element move: (id, previous top-left, new top-left) — the
/// [`Command::MoveElements`] payload.
pub type ElementMove = (Id, (f64, f64), (f64, f64));
/// One waypoint replacement: (id, previous interiors, new interiors) —
/// the [`Command::SetWaypoints`] payload.
pub type WaypointChange = (Id, Vec<(f64, f64)>, Vec<(f64, f64)>);

/// Snap one world coordinate to the [`GRID_PX`] grid (half-way values
/// round away from zero, like every other round in this crate).
#[must_use]
pub fn snapped(value: f64) -> f64 {
    (value / GRID_PX).round() * GRID_PX
}

/// The session geometry of ONE view (P3-D4): deltas over the stored
/// [`Workspace`](oxidizr_core::Workspace), keyed by model ids — exactly
/// the payload [`Overrides`] carries, plus the view-level canvas
/// `dimensions` (the `SetCanvasSize` target; [`scene::build`] ignores
/// it until M4's export sizing). NEVER persisted directly: SAVE maps it
/// onto a cloned workspace (M3-5).
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
    /// [`Overrides::relationships`] (an empty vec straightens: the
    /// reference's "remove vertices" lands here).
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

    /// The [`scene::build`] overlay parameter for this session state.
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
    /// SIZE (style-resolved — item sizes are not editable in P3).
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
    /// Compound edit: the steps land and undo as ONE unit (P3-M3: one
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

/// Undo/redo stacks over one view's [`LayoutOverlay`] (P3-D4): `Vec`
/// stacks, cap [`HISTORY_CAP`] (oldest drops), a fresh [`History::run`]
/// clears the redo branch — the reference's per-diagram `undoStack`,
/// one instance per view on the shell side.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    /// Apply `command` forward and remember it for [`History::undo`].
    pub fn run(&mut self, overlay: &mut LayoutOverlay, command: Command) {
        command.apply(overlay, true);
        if self.undo.len() == HISTORY_CAP {
            self.undo.remove(0);
        }
        self.undo.push(command);
        self.redo.clear();
    }

    /// Undo the most recent command; `false` = nothing to undo.
    pub fn undo(&mut self, overlay: &mut LayoutOverlay) -> bool {
        match self.undo.pop() {
            Some(command) => {
                command.undo(overlay);
                self.redo.push(command);
                true
            }
            None => false,
        }
    }

    /// Re-apply the most recently undone command; `false` = nothing to
    /// redo.
    pub fn redo(&mut self, overlay: &mut LayoutOverlay) -> bool {
        match self.redo.pop() {
            Some(command) => {
                command.apply(overlay, true);
                self.undo.push(command);
                true
            }
            None => false,
        }
    }

    /// Whether [`History::undo`] would do anything.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`History::redo`] would do anything.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Drop both stacks (view switch / workspace reload — the reference
    /// resets its undo buffer when the diagram re-renders from a new
    /// view).
    pub fn reset(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

// ---- command factories -------------------------------------------------------

/// Group-drag `ids` by `delta` (single-select drags pass one id; the
/// multi-selection keeps its relative layout, so the command is
/// UNDOABLE as one unit). `snap` grid-snaps each NEW top-left. `None`
/// when nothing moves.
#[must_use]
pub fn move_by(
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

/// Align the selection along one edge/axis (P3-D4 parity: l/c/r/t/m/b
/// — `m` = the middle axis line). Targets: the selection's bounding
/// box (reference: the lasso's bounding box, not the canvas). Needs at
/// least two elements; `None` when nothing moves.
#[must_use]
pub fn align(
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
/// reference distributes the same way and visual overlap is the user's
/// cue, P3-D0). Needs at least three elements.
#[must_use]
#[allow(clippy::cast_precision_loss)] // selection count: exact in f64 (≤ item count)
pub fn distribute(
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

/// Centre the selection on the scene bounds (P3-D4
/// `CenterViewOrSelection`; the shell passes ALL ids for "centre the
/// view"). `None` when nothing moves.
#[must_use]
pub fn centre_view(scene: &Scene, overlay: &LayoutOverlay, ids: &[Id]) -> Option<Command> {
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
/// (P3-D4 `BringOffCanvasBack`; the shell passes the dimensions rect,
/// else the visible viewport rect — the reference's magnet button uses
/// the current canvas page). Elements are shifted by the minimal
/// delta; the loop order is content order (deterministic).
#[must_use]
pub fn bring_back(scene: &Scene, overlay: &LayoutOverlay, page: Rect) -> Option<Command> {
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

/// Straighten EVERY edge that renders waypoints (P3-D4
/// `RemoveAllEdgeWaypoints`, the reference's dialog-footer button):
/// one `SetWaypoints` change per rendered relationship, emptying the
/// interiors. `None` when all edges are already straight.
#[must_use]
pub fn remove_all_vertices(scene: &Scene, overlay: &LayoutOverlay) -> Option<Command> {
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

/// Replace one edge's interiors (waypoint drag/insert/delete — M3-4
/// computes the new list with the `hit` helpers, this makes it a
/// command). `None` when the list is unchanged.
#[must_use]
pub fn set_waypoints(
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
pub fn set_canvas_size(overlay: &LayoutOverlay, to: Option<Dimensions>) -> Option<Command> {
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
pub fn clear_view(overlay: &LayoutOverlay) -> Command {
    Command::ClearView {
        snapshot: Box::new(overlay.clone()),
    }
}

/// The dragged target of a top-left: `origin` moved by `delta`, grid-
/// snapped when `snap`. Shared by [`drag_move`] and the shell's live
/// preview so preview frames EQUAL the committed geometry.
#[must_use]
pub fn drag_target(origin: (f64, f64), delta: (f64, f64), snap: bool) -> (f64, f64) {
    let to = (origin.0 + delta.0, origin.1 + delta.1);
    if snap {
        (snapped(to.0), snapped(to.1))
    } else {
        to
    }
}

/// Commit a live group drag (P3-M3): `from` are the effective top-lefts
/// captured when the drag BEGAN (the overlay may already carry the live
/// preview positions, so the delta is not re-read from it). `snap` grid-
/// snaps every TARGET top-left exactly like [`move_by`]; the whole group
/// shares `delta`, so relative layout survives. `None` when nothing
/// moves. Re-applying the result is idempotent — the preview and the
/// committed command write the same absolute positions.
#[must_use]
pub fn drag_move(from: &[(Id, (f64, f64))], delta: (f64, f64), snap: bool) -> Option<Command> {
    let moves: Vec<ElementMove> = from
        .iter()
        .filter_map(|(id, (x, y))| {
            let to = drag_target((*x, *y), delta, snap);
            (to != (*x, *y)).then(|| (id.clone(), (*x, *y), to))
        })
        .collect();
    non_empty_moves(moves)
}

/// Drag INTERIOR waypoint `index` of one edge: `from` is the INTERIOR
/// captured at press (the same preview/commit contract as [`drag_move`]
/// — the live preview writes the target first, the commit reads `from`,
/// so re-applying is idempotent). `None` when nothing moves or the
/// index leaves the route (P3-M4).
#[must_use]
pub fn drag_waypoint(
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

/// Land one auto-layout run as a single undoable [`Command`] (P3-M3:
/// the dialog Apply and the adopt-time pass are COMMANDS, not silent
/// state — one Ctrl+Z reverts positions and routes together). `overrides`
/// are the engine's absolute placements/routes ([`layout::apply`]'s
/// output, but headless here); ids already rendering at their placement
/// are skipped, scene iteration order keeps the command deterministic
/// (the [`Overrides`] maps are hash maps). `None` when the run changes
/// nothing.
#[must_use]
pub fn apply_layout(
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

/// Align target for [`align`] (the P3-D4 l/c/r/t/m/b set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignEdge {
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
pub enum Axis {
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

/// A save failure (§P3-M5) — errors as values, presented verbatim.
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

/// Round world px onto the reference's integer grid — `elementViews`,
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
/// (§P3-M5: saving them is blocked, the base view holds the layout).
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
/// view `dimensions` (§P3-M5). The write direction is the read path
/// inverted and NARROWER: only these three geometry slots change —
/// the model, styles and the permanent `automaticLayout` block
/// round-trip untouched (V4: save writes GEOMETRY, never the block).
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

/// The save target for a displayed input (§P3-M5): the opened `.json`
/// in place, a `<stem>.json` SIDE CAR next to a `.dsl` source (the DSL
/// text is never rewritten, P3-D0), `None` for stdin (`"-"`).
#[must_use]
pub fn save_target(display: &str) -> Option<PathBuf> {
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
/// serializer — byte contract, COMPAT.md) to a temp file next to the
/// target, then rename it into place: a crash or a full disk never
/// leaves a truncated workspace behind (P3-D6 posture, shared with the
/// M4 reload writer).
///
/// # Errors
///
/// [`SaveError::Json`] when the writer refuses the workspace,
/// [`SaveError::Io`] when the temp write or rename fails.
pub fn write_workspace(path: &Path, workspace: &Workspace) -> Result<(), SaveError> {
    let bytes = oxidizr_json::to_vec(workspace)?;
    write_atomic(path, &bytes)
}

/// Atomic byte write (the M3-5 pattern, shared by save and M4-5
/// export): `.{pid}.tmp` beside the target, then rename. The temp
/// suffix never matches the reload watch's exact-path filter, so
/// exports landing in a watched directory cannot trigger a reload.
///
/// # Errors
///
/// [`SaveError::Io`] when the temp write or the rename fails (the
/// error carries `path`).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), SaveError> {
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
#[allow(clippy::float_cmp)] // exact geometry: equality IS the contract (§P3-D3)
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
        let mut o = overlay();
        let mut h = History::default();
        let one = move_by(&scene, &o, &[id(1)], (10.0, 0.0), false).unwrap();
        h.run(&mut o, one);
        assert!(h.can_undo() && !h.can_redo());
        assert!(h.undo(&mut o));
        assert!(!h.can_undo() && h.can_redo());
        assert_eq!(o.position(&scene, &id(1)), Some((0.0, 0.0)));
        assert!(h.redo(&mut o));
        assert_eq!(o.position(&scene, &id(1)), Some((10.0, 0.0)));
        // A fresh run kills the redo branch.
        h.undo(&mut o);
        let two = move_by(&scene, &o, &[id(2)], (0.0, 20.0), false).unwrap();
        h.run(&mut o, two);
        assert!(!h.can_redo());
        assert!(h.undo(&mut o));
        assert!(
            !h.undo(&mut o),
            "the fresh run dropped the redo branch — exhausted, no panic"
        );
        // Cap: 205 runs keep the newest 200.
        h.reset();
        for _ in 0..(HISTORY_CAP + 5) {
            let cmd = move_by(&scene, &o, &[id(3)], (10.0, 0.0), false).unwrap();
            h.run(&mut o, cmd);
        }
        let mut undos = 0;
        while h.undo(&mut o) {
            undos += 1;
        }
        assert_eq!(undos, HISTORY_CAP);
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
    fn drag_waypoint_moves_one_vertex_and_survives_preview() {
        let scene = scene();
        let mut o = overlay();
        // The live preview already wrote the target into the overlay;
        // the commit reads the press-captured interior, not the
        // mutated state (P3-M4 handle drag).
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
        // before (M3-2 semantics — the entries keep the stored values,
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
        // hydrate-equivalent validation, so the fixture must be a real
        // workspace the same way the golden_paint tests are (Category A).
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
        // JSON READ applies the reference bean normalizations (an empty
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
}
