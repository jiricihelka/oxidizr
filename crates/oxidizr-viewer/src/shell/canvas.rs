//! Canvas interaction legs of the eframe shell: the `canvas` paint pass
//! and the pointer gestures it drives (pan/zoom, click-select, waypoint
//! editing, lasso). The structs, the entry, the App wiring and the panels
//! stay in the parent module; the interaction state rides `Viewer` fields.

use crate::{commands, hit, overview, paint, scene, shortcuts};
use eframe::egui::{self, Pos2, Vec2};
use oxidizr_core::Id as ModelId;

use super::widgets::{lasso_drag, tooltip_text, url_at};
use super::{Drag, InsertDotHit, Viewer, WaypointHit};

/// World px → egui screen px for the canvas overlays painted by hand
/// (preview line, port dots): the screen is the display resolution —
/// truncation cannot lose what the pixels can show.
#[allow(clippy::cast_possible_truncation)]
fn to_pos(at: (f64, f64)) -> Pos2 {
    egui::pos2(at.0 as f32, at.1 as f32)
}

impl Viewer {
    #[allow(clippy::too_many_lines)] // the canvas pass: input → fit → paint → tip
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        self.last_rect = Some(rect);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        // input state machine: the drag MODE is decided at press
        // and every frame of that mode acts on it. SWAPPED the
        // button roles (user request): left-drag on empty canvas PANS,
        // right-drag LASSOS — panning is what the free button should do
        // on background; selection/edit (click, box-move, waypoint
        // drag) all stay Left. Middle and Space+Left keep panning.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let minimap_press = response.drag_started_by(egui::PointerButton::Primary)
            && ui
                .input(|i| i.pointer.press_origin())
                .is_some_and(|origin| self.minimap_contains(origin));
        if minimap_press {
            self.drag = Drag::Minimap;
        } else if response.drag_started_by(egui::PointerButton::Secondary) {
            // The lasso never hit-tests: a right-press always drags a
            // marquee (zero motion clears the selection, exactly as the
            // left-lasso did).
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                let shift = ui.input(|i| i.modifiers.shift);
                self.drag = lasso_drag(self.selection.as_ref(), origin, shift);
            }
        } else if response.drag_started_by(egui::PointerButton::Middle)
            || (space && response.drag_started_by(egui::PointerButton::Primary))
        {
            self.drag = Drag::Pan;
        } else if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        {
            let (shift, alt, ctrl) =
                ui.input(|i| (i.modifiers.shift, i.modifiers.alt, i.modifiers.command));
            self.drag = self.press_mode(origin, shift, alt, ctrl);
        }
        let cursor = response
            .interact_pointer_pos()
            .or_else(|| ui.input(|i| i.pointer.hover_pos()));
        // `release` marks the frame the drag ENDED: the state machine
        // decides the mode at press, mutates during, commits here.
        let release = !response.dragged()
            && !matches!(self.drag, Drag::Idle)
            && !ui.input(|i| i.pointer.any_down());
        let mode = std::mem::replace(&mut self.drag, Drag::Idle);
        match mode {
            Drag::Idle => {}
            Drag::Minimap => {
                // Click or drag on the minimap: the pointer's world
                // point IS the new viewport center — no
                // scene edit, no history entry.
                if let Some(pos) = cursor
                    && let Some(world) = self.minimap_world(pos)
                {
                    self.center_world(world);
                }
                if !release {
                    self.drag = Drag::Minimap;
                }
            }
            Drag::Pan => {
                if response.dragged() {
                    let delta = response.drag_delta();
                    self.viewport
                        .pan_by((f64::from(delta.x), f64::from(delta.y)));
                }
                if !release {
                    self.drag = Drag::Pan;
                }
            }
            Drag::Move {
                origins,
                carries,
                carry,
                press,
                delta: last,
            } => {
                // World delta of the live pointer from the press point
                // (`last` carries it through pointer-less frames).
                let world_delta = cursor.map_or(last, |pos| {
                    let world = self
                        .viewport
                        .screen_to_world((f64::from(pos.x), f64::from(pos.y)));
                    (world.0 - press.0, world.1 - press.1)
                });
                if response.dragged() {
                    self.preview_move(&origins, &carries, carry, world_delta);
                    self.drag = Drag::Move {
                        origins,
                        carries,
                        carry,
                        press,
                        delta: world_delta,
                    };
                } else if release {
                    // Released: commit the SAME absolute geometry the
                    // preview wrote (idempotent re-apply through the
                    // history, so one Ctrl+Z reverts the whole drag —
                    // carried routes included).
                    if let Some(command) = commands::drag_group(
                        &origins,
                        &carries,
                        world_delta,
                        self.toggles.snap,
                        carry,
                    ) {
                        self.edit_current_view(command);
                    } else {
                        // Nothing moved (a snap pulled the target back
                        // onto its origin): restore the press geometry
                        // and press routes the live preview had written.
                        {
                            let overlay = self.overlay_for();
                            for (id, origin) in &origins {
                                overlay.elements.insert(id.clone(), *origin);
                            }
                            if carry {
                                for (id, wps) in &carries {
                                    overlay.relationships.insert(id.clone(), wps.clone());
                                }
                            }
                        }
                        self.rebuild_scene();
                    }
                }
            }
            Drag::Lasso { anchor, base } => {
                if response.dragged() {
                    if let Some(pos) = cursor {
                        self.lasso_select(anchor, pos, &base);
                    }
                    if !release {
                        self.drag = Drag::Lasso { anchor, base };
                    }
                }
            }
            Drag::Waypoint {
                id,
                from,
                index,
                press,
                delta: last,
            } => {
                // Same measuring as Move: world delta from the press
                // point, `last` carrying it through pointer-less frames.
                let world_delta = cursor.map_or(last, |pos| {
                    let world = self
                        .viewport
                        .screen_to_world((f64::from(pos.x), f64::from(pos.y)));
                    (world.0 - press.0, world.1 - press.1)
                });
                if response.dragged() {
                    self.preview_waypoint(&id, &from, index, world_delta);
                    self.drag = Drag::Waypoint {
                        id,
                        from,
                        index,
                        press,
                        delta: world_delta,
                    };
                } else if release {
                    // Released: the preview's route IS the commit
                    // (idempotent re-apply through the history).
                    if let Some(command) =
                        commands::drag_waypoint(&id, &from, index, world_delta, self.toggles.snap)
                    {
                        self.grabbed = Some((id.clone(), index));
                        self.edit_current_view(command);
                    } else {
                        // Nothing moved: restore the press route the
                        // preview had written into the overlay.
                        {
                            let overlay = self.overlay_for();
                            overlay.relationships.insert(id, from);
                        }
                        self.rebuild_scene();
                    }
                }
            }
            Drag::Insert {
                id,
                from,
                segment,
                at,
                press,
                delta: last,
            } => {
                // Same measuring as Waypoint: the release commits ONE
                // SetWaypoints splicing the dragged point in, so one
                // Ctrl+Z removes the insert again.
                let world_delta = cursor.map_or(last, |pos| {
                    let world = self
                        .viewport
                        .screen_to_world((f64::from(pos.x), f64::from(pos.y)));
                    (world.0 - press.0, world.1 - press.1)
                });
                if response.dragged() {
                    self.preview_insert(&id, &from, segment, at, world_delta);
                    self.drag = Drag::Insert {
                        id,
                        from,
                        segment,
                        at,
                        press,
                        delta: world_delta,
                    };
                } else if release {
                    // Preview and commit write the SAME route (both
                    // splice the snapped target at `segment`).
                    let target = commands::drag_target(at, world_delta, self.toggles.snap);
                    let index = segment.min(from.len());
                    let command = commands::insert_waypoint(&id, &from, segment, target);
                    self.grabbed = Some((id.clone(), index));
                    self.edit_current_view(command);
                }
            }
            Drag::Create { source, from } => {
                if response.dragged() {
                    // Nothing mutates during the drag — the preview
                    // line paints from the ARMED state below, so the
                    // arm is a pure hold.
                    self.drag = Drag::Create { source, from };
                } else if release {
                    // Released: onto another element the CREATE lands
                    // (the funnel selects the new edge); onto empty
                    // canvas or the source itself it cancels silently —
                    // a refused duplicate says why in the status bar.
                    let target =
                        cursor.and_then(|pos| self.item_at((f64::from(pos.x), f64::from(pos.y))));
                    if let Some(target) = target.filter(|target| *target != source) {
                        let (src, dst) = (source.clone(), target.clone());
                        let done = self.run_model_edit(move |workspace, _| {
                            commands::create_relationship(workspace, &src, &dst)
                        });
                        if !done && self.can_edit_model() {
                            self.model_note = Some(format!(
                                "create blocked: {source} → {target} is already connected \
                                 without a description"
                            ));
                        }
                    }
                }
            }
            Drag::Reconnect { id, endpoint } => {
                if response.dragged() {
                    self.drag = Drag::Reconnect { id, endpoint };
                } else if release {
                    // Released: onto an element the END moves there
                    // (factory-gated: no-ops and own-end drops are
                    // silent); onto empty canvas the arrow stays put.
                    if let Some(target) =
                        cursor.and_then(|pos| self.item_at((f64::from(pos.x), f64::from(pos.y))))
                    {
                        let same =
                            self.edge_ends(&id)
                                .map(|(source, destination)| match endpoint {
                                    commands::Endpoint::Source => source,
                                    commands::Endpoint::Destination => destination,
                                });
                        let (edge, to) = (id.clone(), target.clone());
                        let done = self.run_model_edit(move |workspace, _| {
                            commands::reconnect_relationship(workspace, &edge, endpoint, &to)
                        });
                        if !done && self.can_edit_model() && same.as_ref() != Some(&target) {
                            self.model_note =
                                Some(format!("reconnect blocked: {id} cannot point at {target}"));
                        }
                    }
                }
            }
        }
        // gestures, split by scroll UNIT (winit reports the
        // mouse WHEEL in Line/Page notches and touchpad scroll/pinch in
        // Points): wheel zooms at the pointer as before; two-finger
        // scroll PANS (natural direction — egui's delta already states
        // how the content should move); pinch — a ctrl-scroll synthesis
        // on Linux/Windows, a magnify gesture on macOS — is folded into
        // `zoom_delta` by egui itself, so ctrl-scroll events are
        // SKIPPED here to avoid double-zooming.
        if response.hovered() {
            let (zoom, pan) = ui.input(|i| {
                let mut zoom = i.zoom_delta();
                let mut pan = Vec2::ZERO;
                let mut wheel = 0.0_f32;
                for event in &i.events {
                    if let egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } = event
                    {
                        if modifiers.command {
                            continue; // egui already counted it as zoom
                        }
                        match unit {
                            egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => {
                                wheel += delta.y;
                            }
                            egui::MouseWheelUnit::Point => pan += *delta,
                        }
                    }
                }
                if wheel.abs() > f32::EPSILON {
                    zoom *= (wheel * 0.05).exp();
                }
                (zoom, pan)
            });
            if let Some(pt) = ui.input(|i| i.pointer.hover_pos()) {
                if (f64::from(zoom) - 1.0).abs() > f64::EPSILON {
                    self.viewport
                        .zoom_at(f64::from(zoom), (f64::from(pt.x), f64::from(pt.y)));
                }
                let pan_by = (f64::from(pan.x), f64::from(pan.y));
                if pan.x.abs() + pan.y.abs() > f32::EPSILON {
                    self.viewport.pan_by(pan_by);
                }
            }
        }
        if response.clicked()
            && let Some(pt) = response.interact_pointer_pos()
        {
            // A click inside the minimap recenters; it never picks
            // a box .
            if let Some(world) = self.minimap_world(pt) {
                self.center_world(world);
            } else {
                let (command, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                self.canvas_click(pt, command, shift, &ctx);
            }
        }
        //: a double-click on an edge splices a waypoint into the
        // nearest segment (the second click also selected it).
        if response.double_clicked()
            && let Some(pt) = response.interact_pointer_pos()
            && !self.minimap_contains(pt)
        {
            self.insert_waypoint_at((f64::from(pt.x), f64::from(pt.y)));
        }
        self.hover = match (response.hovered(), ui.input(|i| i.pointer.hover_pos())) {
            (true, Some(pointer)) if !self.minimap_contains(pointer) => {
                self.scene.as_ref().and_then(|scene| {
                    hit::pick(
                        scene,
                        self.viewport,
                        (f64::from(pointer.x), f64::from(pointer.y)),
                    )
                })
            }
            _ => None,
        };
        ctx.set_cursor_icon(if self.hover.is_some() {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        });

        let screen = (f64::from(rect.width()), f64::from(rect.height()));
        // the sidecar memory wins over both the startup
        // zoom restore and the re-fit; the remembered WORLD point is
        // re-centred for whatever canvas size this first frame reports
        // (storing world coords is what makes the memory resolution-
        // independent). Viewport::new clamps a hand-edited zoom.
        if let Some(memory) = self.pending_viewport.take() {
            self.restore_scale = None;
            self.viewport = hit::Viewport::new(
                memory.zoom,
                (
                    screen.0 / 2.0 - memory.centre.0 * memory.zoom,
                    screen.1 / 2.0 - memory.centre.1 * memory.zoom,
                ),
            );
        } else if let Some(scale) = self.restore_scale.take() {
            if let Some(scene) = &self.scene {
                let center = scene.bounds.center();
                self.viewport = hit::Viewport::new(
                    scale,
                    (
                        screen.0 / 2.0 - center.0 * scale,
                        screen.1 / 2.0 - center.1 * scale,
                    ),
                );
            }
        } else if self.latches.needs_fit
            && let Some(scene) = &self.scene
        {
            self.viewport = hit::Viewport::fit(scene.bounds, screen);
            self.latches.needs_fit = false;
        }

        let painter = ui.painter_at(rect);
        if let Some(scene) = &self.scene {
            let hover_edge = self.hovered_edge();
            let options = paint::PaintOptions {
                screen: rect,
                selection: self.selection.as_ref(),
                hover_edge: hover_edge.as_ref(),
                model_editing: self.can_edit_model(),
            };
            let shapes =
                ctx.fonts_mut(|fonts| paint::paint_scene(scene, &self.viewport, fonts, &options));
            for shape in shapes {
                painter.add(shape);
            }
            //: rasterized style icons over the placeholders the
            // paint step drew (an opaque cell covers its box; failed
            // icons keep the dashed stand-in and surface in diagnostics).
            if let Some(texture) = &self.icon_texture {
                for item in &scene.items {
                    if let Some(uv) = item.icon.as_ref().and_then(|icon| self.icon_uv.get(icon)) {
                        let rect = paint::icon_slot(
                            paint::to_screen_rect(item.rect, &self.viewport),
                            item.icon_position,
                        );
                        painter.image(texture.id(), rect, *uv, egui::Color32::WHITE);
                    }
                }
            }
        } else if let Some(error) = &self.scene_error {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                error.clone(),
                egui::FontId::default(),
                egui::Color32::from_rgb(185, 60, 60),
            );
        } else {
            //: the empty-canvas help line is LOOKED UP in the table,
            // never hand-copied (same source as the overlay).
            let help = shortcuts::all()
                .iter()
                .flat_map(|section| section.bindings)
                .filter(|binding| binding.keys.starts_with("F1"))
                .map(|binding| format!("{} — {}", binding.keys, binding.action))
                .next()
                .unwrap_or_default();
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "Open a workspace: enter a path in the toolbar (or launch\n\
                     with `oxidizr-viewer <file>`)\n\n{help}"
                ),
                egui::FontId::default(),
                ui.visuals().weak_text_color(),
            );
        }
        //: lasso rubber band — accent rect from anchor to pointer.
        if let Drag::Lasso { anchor, .. } = &self.drag
            && let Some(pos) = cursor
        {
            let band = egui::Rect::from_two_pos(*anchor, pos);
            painter.rect_stroke(
                band,
                0.0,
                egui::Stroke::new(1.0, paint::SELECTION_COLOR),
                egui::StrokeKind::Inside,
            );
        }
        //: model-op affordances (model-editing sources only): PORT dots
        // on the hovered box — the create-drag grabs — and, while a
        // create/reconnect drag runs, its transient preview line plus an
        // accent rim on the hovered drop target. Drawing only: no edit
        // exists before the release, so the overlay never sees a
        // preview it would have to commit.
        if let Some(scene) = self.scene.as_ref().filter(|_| self.can_edit_model()) {
            let hover_item = self.hover.and_then(|hover| match hover {
                hit::Hit::Item(index) => scene.items.get(index),
                hit::Hit::Edge(_) => None,
            });
            if matches!(self.drag, Drag::Create { .. } | Drag::Reconnect { .. }) {
                if let (Some(from), Some(pos)) = (self.drag_preview_from(), cursor) {
                    let from = to_pos(self.viewport.world_to_screen(from));
                    painter
                        .line_segment([from, pos], egui::Stroke::new(1.5, paint::SELECTION_COLOR));
                    if let Some(item) = hover_item {
                        painter.rect_stroke(
                            paint::to_screen_rect(item.rect, &self.viewport),
                            0.0,
                            egui::Stroke::new(2.0, paint::SELECTION_COLOR),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            } else if let Some(item) = hover_item {
                for port in hit::ports(item.rect) {
                    let at = to_pos(self.viewport.world_to_screen(port));
                    painter.circle_filled(at, paint::INSERT_DOT_PX, egui::Color32::WHITE);
                    painter.circle_stroke(
                        at,
                        paint::INSERT_DOT_PX,
                        egui::Stroke::new(1.0, paint::SELECTION_COLOR),
                    );
                }
            }
        }
        //: minimap (the navigator.jspf analogue) — the whole
        // scene overview + the live viewport rect, bottom-right. The
        // painter is clipped to the box, so a viewport rect bigger than
        // the scene cannot spill onto the canvas.
        if let (Some(scene), Some(mini)) = (&self.scene, Self::minimap_rect(rect))
            && let Some(map) = overview::Overview::new(scene.bounds, mini)
        {
            let tl = self
                .viewport
                .screen_to_world((f64::from(rect.min.x), f64::from(rect.min.y)));
            let br = self
                .viewport
                .screen_to_world((f64::from(rect.max.x), f64::from(rect.max.y)));
            let view_world = scene::Rect {
                x: tl.0.min(br.0),
                y: tl.1.min(br.1),
                width: (br.0 - tl.0).abs(),
                height: (br.1 - tl.1).abs(),
            };
            let bg = ui.visuals().window_fill;
            let edge = ui.visuals().noninteractive().fg_stroke.color;
            let map_painter = ui.painter_at(mini);
            map_painter.rect_filled(mini, 6.0, bg);
            for shape in map.shapes(scene, Some(view_world), self.selection.as_ref()) {
                map_painter.add(shape);
            }
            map_painter.rect_stroke(
                mini,
                6.0,
                egui::Stroke::new(1.0, edge),
                egui::StrokeKind::Inside,
            );
        }
        // Hover tooltip: egui's own tooltip machinery instead
        // of the hand-rolled Area — the old `Id::unique` gave every
        // frame a brand-new tooltip identity (no state to stabilise on),
        // which is exactly what made it flicker away. `on_hover_ui_at_pointer`
        // rides this response's stable id, follows the pointer and keeps
        // the same 12 px gap.
        if let (Some(hit), Some(scene)) = (self.hover, &self.scene) {
            let mut text = tooltip_text(scene, hit);
            //: the editing kit advertises itself — hovering a painted
            // affordance appends its gesture line (wording mirrors the
            // table; the V6 audit keeps them honest).
            if let Some(hint) = self.affordance_hint(cursor) {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(hint);
            }
            if !text.is_empty() {
                response.on_hover_ui_at_pointer(move |ui| {
                    ui.label(text);
                });
            }
        }
    }

    /// The hover hint of an editing affordance under the pointer: the
    /// FIRST hit in press-chain order (waypoint handles already ride the
    /// plain edge tooltip — these are the three NEW grabs: insert dot,
    /// endpoint square, port dot). `None` while any drag runs: the
    /// pointer is busy, not advertised.
    fn affordance_hint(&self, cursor: Option<Pos2>) -> Option<&'static str> {
        if !matches!(self.drag, Drag::Idle) {
            return None;
        }
        let screen = (f64::from(cursor?.x), f64::from(cursor?.y));
        if self.insert_dot_hit(screen).is_some() {
            return Some("drag to insert a waypoint and route it in one move");
        }
        if self.can_edit_model() {
            if self.endpoint_handle_hit(screen).is_some() {
                return Some("drag onto another element to reconnect this end");
            }
            if self.port_hit(screen).is_some() {
                return Some("drag onto another element to create a relationship (Esc cancels)");
            }
        }
        None
    }

    /// Decide what a fresh left-button press starts (/): a
    /// waypoint handle of the SELECTED edge grabs FIRST (Alt+press
    /// deletes that vertex, a plain press drags it), then a segment-
    /// midpoint INSERT dot of the selected edge starts an
    /// insert-and-route drag, then — on model-editing sources — an
    /// ENDPOINT handle of the selected edge (reconnect drag) and a PORT
    /// dot of a box (create drag), then a box under the cursor begins a
    /// group MOVE (the box joins the selection first; Shift keeps the
    /// group, a plain press elsewhere selects just it), anything else
    /// PANS (: the LASSO
    /// moved to the right button and never hit-tests). `ctrl` detaches
    /// the carried arrow handles from the move .
    fn press_mode(&mut self, origin: Pos2, shift: bool, alt: bool, ctrl: bool) -> Drag {
        let screen = (f64::from(origin.x), f64::from(origin.y));
        if let Some((id, mut from, index)) = self.waypoint_hit(screen) {
            if alt {
                self.drop_grab(&id);
                from.remove(index);
                let overlay = self.overlay_for().clone();
                let command = self
                    .scene
                    .as_ref()
                    .and_then(|scene| commands::set_waypoints(scene, &overlay, &id, from));
                if let Some(command) = command {
                    self.edit_current_view(command);
                }
                return Drag::Idle;
            }
            return Drag::Waypoint {
                id,
                from,
                index,
                press: self.viewport.screen_to_world(screen),
                delta: (0.0, 0.0),
            };
        }
        // Then a segment-midpoint INSERT dot of the selected edge:
        // the press-drag splices a waypoint and routes it in one move
        // (checked before boxes: dots sit ON the route, never on a box).
        if let Some((id, from, segment, at)) = self.insert_dot_hit(screen) {
            return Drag::Insert {
                id,
                from,
                segment,
                at,
                press: self.viewport.screen_to_world(screen),
                delta: (0.0, 0.0),
            };
        }
        // Then an ENDPOINT handle of the selected edge: press-drag onto
        // an element RECONNECTS that end (model-editing sources only —
        // blocked sources keep the plain box press).
        if self.can_edit_model()
            && let Some((id, endpoint)) = self.endpoint_handle_hit(screen)
        {
            return Drag::Reconnect { id, endpoint };
        }
        // Then a PORT dot (a box side midpoint): press-drag onto another
        // element CREATES a relationship (the dots paint on the hovered
        // box only, and only for model-editing sources).
        if self.can_edit_model()
            && let Some((source, from)) = self.port_hit(screen)
        {
            return Drag::Create { source, from };
        }
        let picked = self
            .scene
            .as_ref()
            .and_then(|scene| hit::pick(scene, self.viewport, screen));
        let item = match picked {
            Some(hit::Hit::Item(index)) => self
                .scene
                .as_ref()
                .and_then(|scene| scene.items.get(index))
                .map(|item| item.id.clone()),
            _ => None,
        };
        let Some(id) = item else {
            // swapped roles: a left-press on empty canvas PANS
            // (the lasso rides the right button, see the press chain).
            return Drag::Pan;
        };
        let mut ids: Vec<ModelId> = match &self.selection {
            Some(paint::Selection::Items(ids)) => ids.clone(),
            _ => Vec::new(),
        };
        if !ids.contains(&id) {
            if shift {
                ids.push(id);
            } else {
                ids = vec![id];
            }
            self.selection = Some(paint::Selection::Items(ids.clone()));
        }
        let origins = self.press_origins(&ids);
        if origins.is_empty() {
            Drag::Idle
        } else {
            // capture the press-time routes that follow
            // the group (Ctrl+drag carries nothing).
            let carries = {
                let overlay = self.overlay_for().clone();
                self.scene.as_ref().map_or_else(Vec::new, |scene| {
                    commands::carried_interiors(scene, &overlay, &ids)
                })
            };
            Drag::Move {
                origins,
                carries,
                carry: !ctrl,
                press: self.viewport.screen_to_world(screen),
                delta: (0.0, 0.0),
            }
        }
    }

    /// The edge currently under the pointer, if any:
    /// its waypoint handles paint on hover and a handle press hits, so
    /// handles are seen — and deleted — before the edge is selected.
    fn hovered_edge(&self) -> Option<ModelId> {
        let (scene, hit) = (self.scene.as_ref()?, self.hover?);
        match hit {
            hit::Hit::Edge(index) => scene.edges.get(index).map(|edge| edge.id.clone()),
            hit::Hit::Item(_) => None,
        }
    }

    /// Press on a waypoint handle of the SELECTED edge — or
    /// of the HOVERED one: the
    /// tolerance is [`hit::HIT_TOLERANCE_PX`] screen px converted to
    /// world px, and the scene polyline IS the effective route (the
    /// scene is rebuilt with the overlay). Returns the press-time
    /// route plus the route-relative vertex index.
    fn waypoint_hit(&self, screen: (f64, f64)) -> Option<WaypointHit> {
        let id = match &self.selection {
            Some(paint::Selection::Edge(id)) => id.clone(),
            _ => self.hovered_edge()?,
        };
        let scene = self.scene.as_ref()?;
        let edge = scene.edges.iter().find(|edge| edge.id == id)?;
        if edge.points.len() < 3 {
            return None;
        }
        let world = self.viewport.screen_to_world(screen);
        let tol = hit::HIT_TOLERANCE_PX / self.viewport.scale();
        let index = hit::nearest_waypoint(world, &edge.points, tol)?;
        Some((
            id,
            edge.points[1..edge.points.len() - 1].to_vec(),
            index - 1,
        ))
    }

    /// Live waypoint preview frame: write exactly the route the
    /// release will commit ([`commands::drag_waypoint`] shares
    /// [`commands::drag_target`] with the box-move preview), then
    /// repaint — preview frames EQUAL the committed geometry.
    fn preview_waypoint(
        &mut self,
        id: &ModelId,
        from: &[(f64, f64)],
        index: usize,
        delta: (f64, f64),
    ) {
        let mut route = from.to_vec();
        if let Some(at) = route.get_mut(index) {
            *at = commands::drag_target(*at, delta, self.toggles.snap);
        }
        {
            let overlay = self.overlay_for();
            overlay.relationships.insert(id.clone(), route);
        }
        self.rebuild_scene();
    }

    /// Live INSERT preview frame: write exactly the route the release
    /// will commit (the press route with the snapped target spliced in
    /// at `segment`), then repaint — preview frames EQUAL the committed
    /// geometry, so the commit re-applies idempotently.
    fn preview_insert(
        &mut self,
        id: &ModelId,
        from: &[(f64, f64)],
        segment: usize,
        at: (f64, f64),
        delta: (f64, f64),
    ) {
        let mut route = from.to_vec();
        let target = commands::drag_target(at, delta, self.toggles.snap);
        route.insert(segment.min(route.len()), target);
        {
            let overlay = self.overlay_for();
            overlay.relationships.insert(id.clone(), route);
        }
        self.rebuild_scene();
    }

    /// Press on a segment-midpoint INSERT dot of the SELECTED edge (the
    /// dots paint there only): the scene polyline IS the effective
    /// route, dots sit at every segment midpoint within
    /// [`hit::HIT_TOLERANCE_PX`] screen px. Returns the press-time
    /// INTERIOR route, the hit SEGMENT index (= its insert index into
    /// the route, the last dot appends) and the dot's world position
    /// (targets measure from it).
    fn insert_dot_hit(&self, screen: (f64, f64)) -> Option<InsertDotHit> {
        let id = match &self.selection {
            Some(paint::Selection::Edge(id)) => id.clone(),
            _ => return None,
        };
        let scene = self.scene.as_ref()?;
        let edge = scene.edges.iter().find(|edge| edge.id == id)?;
        if edge.points.len() < 2 {
            return None;
        }
        let world = self.viewport.screen_to_world(screen);
        let tol = hit::HIT_TOLERANCE_PX / self.viewport.scale();
        let segment = hit::nearest_midpoint(world, &edge.points, tol)?;
        let (a, b) = (edge.points[segment], edge.points[segment + 1]);
        Some((
            id,
            edge.points[1..edge.points.len() - 1].to_vec(),
            segment,
            (f64::midpoint(a.0, b.0), f64::midpoint(a.1, b.1)),
        ))
    }

    /// Press on an ENDPOINT handle of the SELECTED edge (the squares
    /// the route kit paints there): within [`hit::HIT_TOLERANCE_PX`]
    /// screen px of one polyline end. Maps the hit end onto the model
    /// endpoint it drags.
    fn endpoint_handle_hit(&self, screen: (f64, f64)) -> Option<(ModelId, commands::Endpoint)> {
        let id = match &self.selection {
            Some(paint::Selection::Edge(id)) => id.clone(),
            _ => return None,
        };
        let scene = self.scene.as_ref()?;
        let edge = scene.edges.iter().find(|edge| edge.id == id)?;
        let world = self.viewport.screen_to_world(screen);
        let tol = hit::HIT_TOLERANCE_PX / self.viewport.scale();
        let end = hit::nearest_endpoint(world, &edge.points, tol)?;
        Some((
            id,
            if end == 0 {
                commands::Endpoint::Source
            } else {
                commands::Endpoint::Destination
            },
        ))
    }

    /// Press on a PORT dot (a box side midpoint, [`hit::ports`]) — the
    /// dots paint on the hovered box, and a press near another box's
    /// port is that box's press (nearest port per box, first box in
    /// content order wins). Returns the element plus the pressed port
    /// (world px, where the preview line starts).
    fn port_hit(&self, screen: (f64, f64)) -> Option<(ModelId, (f64, f64))> {
        let scene = self.scene.as_ref()?;
        let world = self.viewport.screen_to_world(screen);
        let tol = hit::HIT_TOLERANCE_PX / self.viewport.scale();
        scene.items.iter().find_map(|item| {
            hit::nearest_port(world, item.rect, tol).map(|at| (item.id.clone(), at))
        })
    }

    /// The element under a screen point — the create/reconnect drop
    /// target (an edge press is not a drop).
    fn item_at(&self, screen: (f64, f64)) -> Option<ModelId> {
        let scene = self.scene.as_ref()?;
        let index = match hit::pick(scene, self.viewport, screen)? {
            hit::Hit::Item(index) => index,
            hit::Hit::Edge(_) => return None,
        };
        scene.items.get(index).map(|item| item.id.clone())
    }

    /// The model ends of a scene edge — the elements an endpoint drag
    /// already points at (a drop there is a silent no-op). `None` when
    /// the edge has left the scene.
    fn edge_ends(&self, id: &ModelId) -> Option<(ModelId, ModelId)> {
        self.scene
            .as_ref()?
            .edges
            .iter()
            .find(|edge| &edge.id == id)
            .map(|edge| (edge.source.clone(), edge.destination.clone()))
    }

    /// The fixed end of the create/reconnect preview line (world px):
    /// the pressed port, or the endpoint the selected edge paints
    /// today. `None` = no model drag armed (or its edge is gone).
    fn drag_preview_from(&self) -> Option<(f64, f64)> {
        match &self.drag {
            Drag::Create { from, .. } => Some(*from),
            Drag::Reconnect { id, endpoint } => {
                let points = self
                    .scene
                    .as_ref()?
                    .edges
                    .iter()
                    .find(|edge| &edge.id == id)?
                    .points
                    .as_slice();
                match endpoint {
                    commands::Endpoint::Source => points.first(),
                    commands::Endpoint::Destination => points.last(),
                }
                .copied()
            }
            _ => None,
        }
    }

    /// Arrow-key nudge: one step (`dx`/`dy` in world px, multiples of
    /// [`commands::GRID_PX`]) moves the GRABBED waypoint of the still-
    /// selected edge, else the selected ELEMENTS (enclosed routes
    /// follow). Every key press is its own command — Ctrl+Z walks the
    /// nudges one by one. No snap on the way: the step IS the geometry.
    pub(super) fn nudge_selection(&mut self, dx: f64, dy: f64) {
        // A grab lives only while its edge stays the selection.
        let grabbed = self.grabbed.clone().filter(
            |(id, _)| matches!(&self.selection, Some(paint::Selection::Edge(edge)) if edge == id),
        );
        if let Some((id, index)) = grabbed {
            let overlay = self.overlay_for().clone();
            let command = self.scene.as_ref().and_then(|scene| {
                let from = overlay.interior(scene, &id);
                commands::drag_waypoint(&id, &from, index, (dx, dy), false)
            });
            if let Some(command) = command {
                self.edit_current_view(command);
            }
            return;
        }
        let ids: Vec<ModelId> = match &self.selection {
            Some(paint::Selection::Items(ids)) => ids.clone(),
            _ => return,
        };
        let origins = self.press_origins(&ids);
        if origins.is_empty() {
            return;
        }
        let carries = {
            let overlay = self.overlay_for().clone();
            self.scene.as_ref().map_or_else(Vec::new, |scene| {
                commands::carried_interiors(scene, &overlay, &ids)
            })
        };
        if let Some(command) = commands::drag_group(&origins, &carries, (dx, dy), false, true) {
            self.edit_current_view(command);
        }
    }

    /// A non-drag route edit (double-click insert, Alt+press removal,
    /// straighten) shifts or deletes vertices: a grab on THAT edge
    /// would nudge the wrong vertex, so drop it.
    fn drop_grab(&mut self, id: &ModelId) {
        if self
            .grabbed
            .as_ref()
            .is_some_and(|(grabbed, _)| grabbed == id)
        {
            self.grabbed = None;
        }
    }

    /// Ctrl+Shift+K on the selected edge: drop ALL
    /// its interior waypoints — the inverse of double-click insert, one
    /// history step so Ctrl+Z restores the route. `set_waypoints(..,
    /// vec![])` straightens (the "remove vertices" edit); a plain
    /// click-select or hover keeps every other state intact. (Del went
    /// to deleting the relationship itself once model editing landed.)
    pub(super) fn clear_selected_waypoints(&mut self) {
        let Some(paint::Selection::Edge(id)) = self.selection.clone() else {
            return;
        };
        self.drop_grab(&id);
        let overlay = self.overlay_for().clone();
        let command = self
            .scene
            .as_ref()
            .and_then(|scene| commands::set_waypoints(scene, &overlay, &id, Vec::new()));
        if let Some(command) = command {
            self.edit_current_view(command);
        }
    }

    /// Double-click on an edge splices a new waypoint into the segment
    /// nearest the click . The scene polyline is the effective
    /// route, so its splice index maps directly onto the INTERIOR list
    /// `set_waypoints` commits.
    fn insert_waypoint_at(&mut self, screen: (f64, f64)) {
        let picked = self
            .scene
            .as_ref()
            .and_then(|scene| hit::pick(scene, self.viewport, screen));
        let Some(hit::Hit::Edge(index)) = picked else {
            return;
        };
        let Some((id, points)) = self.scene.as_ref().and_then(|scene| {
            scene
                .edges
                .get(index)
                .map(|edge| (edge.id.clone(), edge.points.clone()))
        }) else {
            return;
        };
        if points.len() < 2 {
            return;
        }
        self.drop_grab(&id);
        let world = self.viewport.screen_to_world(screen);
        let splice = hit::insert_waypoint(world, &points).saturating_sub(1);
        let mut interior = points[1..points.len() - 1].to_vec();
        interior.insert(splice.min(interior.len()), world);
        let overlay = self.overlay_for().clone();
        let command = self
            .scene
            .as_ref()
            .and_then(|scene| commands::set_waypoints(scene, &overlay, &id, interior));
        if let Some(command) = command {
            self.edit_current_view(command);
        }
    }

    /// Press-time effective top-lefts of `ids`, in scene content order
    /// (the order the preview and the committed command walk alike).
    fn press_origins(&self, ids: &[ModelId]) -> Vec<(ModelId, (f64, f64))> {
        let Some(scene) = &self.scene else {
            return Vec::new();
        };
        let overlay = self
            .view_key
            .as_deref()
            .and_then(|key| self.overlays.get(key));
        scene
            .items
            .iter()
            .filter(|item| ids.contains(&item.id))
            .map(|item| {
                let from = overlay
                    .and_then(|o| o.position(scene, &item.id))
                    .unwrap_or((item.rect.x, item.rect.y));
                (item.id.clone(), from)
            })
            .collect()
    }

    /// Live drag preview frame: write exactly the targets the release
    /// will commit ([`commands::drag_target`] is shared with
    /// [`commands::drag_move`] for this reason) into the session
    /// overlay and repaint — preview frames EQUAL the committed
    /// geometry, so releasing is an idempotent re-apply.
    fn preview_move(
        &mut self,
        origins: &[(ModelId, (f64, f64))],
        carries: &[(ModelId, Vec<(f64, f64)>)],
        carry: bool,
        delta: (f64, f64),
    ) {
        {
            let snap = self.toggles.snap;
            let overlay = self.overlay_for();
            let mut applied = None;
            for (id, origin) in origins {
                let to = commands::drag_target(*origin, delta, snap);
                if applied.is_none() {
                    applied = Some((to.0 - origin.0, to.1 - origin.1));
                }
                overlay.elements.insert(id.clone(), to);
            }
            // the same shift rides the press-time routes
            // of fully-contained edges — `drag_group` commits exactly
            // this translation (both measure off the FIRST origin).
            if carry && let Some(shift) = applied {
                for (id, wps) in carries {
                    overlay.relationships.insert(
                        id.clone(),
                        wps.iter()
                            .map(|(x, y)| (x + shift.0, y + shift.1))
                            .collect(),
                    );
                }
            }
        }
        self.rebuild_scene();
    }

    /// Live lasso frame: `base` plus every box INTERSECTING the
    /// anchor→current screen rect, mapped to world px and resolved by
    /// [`hit::items_in_rect`] (touching edges do not select).
    fn lasso_select(&mut self, anchor: Pos2, current: Pos2, base: &[ModelId]) {
        let from = self
            .viewport
            .screen_to_world((f64::from(anchor.x), f64::from(anchor.y)));
        let to = self
            .viewport
            .screen_to_world((f64::from(current.x), f64::from(current.y)));
        let rect = scene::Rect {
            x: from.0.min(to.0),
            y: from.1.min(to.1),
            width: (to.0 - from.0).abs(),
            height: (to.1 - from.1).abs(),
        };
        let mut ids: Vec<ModelId> = base.to_vec();
        if let Some(scene) = &self.scene {
            for index in hit::items_in_rect(scene, rect) {
                let id = &scene.items[index].id;
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
        }
        self.selection = (!ids.is_empty()).then_some(paint::Selection::Items(ids));
    }

    /// Resolve a canvas click: ctrl/⌘ opens the element URL; a plain
    /// click selects (item over edge already decided by [`hit::pick`]),
    /// Shift-click TOGGLES membership (multi-select) and a click
    /// on empty canvas clears — Shift keeps the selection.
    fn canvas_click(&mut self, point: Pos2, command: bool, shift: bool, ctx: &egui::Context) {
        let hit = self.scene.as_ref().and_then(|scene| {
            hit::pick(
                scene,
                self.viewport,
                (f64::from(point.x), f64::from(point.y)),
            )
        });
        if command {
            let url = match (hit, self.scene.as_ref()) {
                (Some(hit), Some(scene)) => url_at(scene, hit).map(str::to_owned),
                _ => None,
            };
            if let Some(url) = url {
                ctx.open_url(egui::OpenUrl::new_tab(url));
                return;
            }
        }
        match hit {
            Some(hit::Hit::Item(index)) => {
                let Some(id) = self
                    .scene
                    .as_ref()
                    .and_then(|scene| scene.items.get(index))
                    .map(|item| item.id.clone())
                else {
                    return;
                };
                let mut ids = if shift {
                    match &self.selection {
                        Some(paint::Selection::Items(ids)) => ids.clone(),
                        _ => Vec::new(),
                    }
                } else {
                    Vec::new()
                };
                match ids.iter().position(|existing| *existing == id) {
                    Some(index) if shift => {
                        ids.remove(index);
                    }
                    _ => ids.push(id),
                }
                self.selection = Some(paint::Selection::Items(ids));
            }
            Some(hit::Hit::Edge(index)) => {
                self.selection = self
                    .scene
                    .as_ref()
                    .and_then(|scene| scene.edges.get(index))
                    .map(|edge| paint::Selection::Edge(edge.id.clone()));
            }
            None if !shift => self.selection = None,
            None => {}
        }
    }
}
