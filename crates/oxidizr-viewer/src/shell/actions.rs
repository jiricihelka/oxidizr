//! The eframe shell's action legs: the editing-command funnel and
//! history, workspace save/export, the source-watch reload cycle, the
//! layout runs + layout window and the icon-atlas upload. The App wiring
//! in the parent fires them; every geometry mutation lands in
//! `edit_current_view` (editor rule).

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use crate::{commands, export, icons, layout, paint, reload, scene};
use eframe::egui;
use oxidizr_api::{ApiOptions, Input, Opened};
use oxidizr_core::Id as ModelId;
use oxidizr_core::views::{AutomaticLayout, RankDirection, View};

use super::widgets::{direction_label, summarize_log};
use super::{Viewer, companion_dsl, newest_by_mtime};

impl Viewer {
    /// Run one editing command on the current view: apply through the
    /// SESSION history (the step carries the view), mark the view
    /// dirty, repaint. EVERY geometry mutation in the shell lands here
    /// (editor rule).
    pub(super) fn edit_current_view(&mut self, command: commands::Command) {
        let view_key = self.view_key.clone().unwrap_or_default();
        self.history
            .run_geometry(&mut self.overlays, &view_key, command);
        self.dirty.insert(view_key);
        self.rebuild_scene();
    }

    /// Save every dirty view's overlay into the workspace's stored
    /// geometry and write the JSON: the target comes from
    /// [`commands::save_target`] (opened `.json` in place, `<stem>.json`
    /// sidecar for DSL sources — the DSL text is never rewritten,
    ///), the bytes from the SHARED `oxidizr_json` writer through
    /// [`commands::write_workspace`] (temp+rename, posture). The
    /// current view being FILTERED blocks the save: a filtered view has
    /// no content of its own, its base view owns the layout (a silent
    /// save there would write the wrong geometry or nothing). A failed
    /// write keeps `dirty` — the note explains, the edit survives.
    pub(super) fn save_workspace(&mut self) {
        self.save_note = None;
        let Some(workspace) = &self.workspace else {
            self.save_note = Some("nothing to save — open a workspace first".to_owned());
            return;
        };
        if self.current_view_filtered() {
            self.save_note =
                Some("save blocked: a filtered view has no layout of its own".to_owned());
            return;
        }
        let Some(target) = commands::save_target(&self.display) else {
            self.save_note = Some("save blocked: stdin has no file to write".to_owned());
            return;
        };
        let keys: Vec<String> = self
            .dirty
            .iter()
            .filter(|key| self.overlays.contains_key(key.as_str()))
            .cloned()
            .collect();
        if keys.is_empty() {
            self.save_note = Some("nothing to save — no layout edits".to_owned());
            return;
        }
        let mut applied = workspace.clone();
        for key in &keys {
            let Some(overlay) = self.overlays.get(key) else {
                continue;
            };
            //: a view that opened un-laid-out DISPLAYS engine
            // geometry — save persists what the user saw: seed the ids
            // the overlay does not cover with that geometry, so the
            // written JSON renders exactly like the screen did.
            let seeded = layout::seed_overlay(workspace, key, overlay);
            applied = commands::apply_overlay(&applied, key, seeded.as_ref().unwrap_or(overlay));
        }
        match commands::write_workspace(&target, &applied) {
            Ok(()) => {
                self.dirty.clear();
                self.save_note = Some(format!("saved {}", target.display()));
                // Re-baseline the reload watch on the bytes we just
                // wrote (and adopt the sidecar the save may have
                // created): the rename events our save fired hash-match
                // against the fresh baseline, so saving never
                // self-triggers a reload .
                self.restart_watch();
            }
            Err(error) => self.save_note = Some(error.to_string()),
        }
    }

    ///: export the CURRENT view as `<stem>.svg` + `<stem>.png`
    /// beside the save target (the `export.jspf` analogue) — the scene
    /// exactly as painted plus the key band; the selection is never
    /// exported. Writes are atomic (the temp+rename: neither file
    /// name is in the watch set, so exporting cannot self-trigger a
    /// reload). Dialog-free (V8 posture): success/blocked/failed rides
    /// the status note.
    pub(super) fn export_files(&mut self) {
        self.save_note = None;
        let Some(base) = commands::save_target(&self.display) else {
            self.save_note = Some("export blocked: stdin has no file to write".to_owned());
            return;
        };
        let Some(ctx) = self.repaint_ctx.clone() else {
            return;
        };
        let rendered = self.scene.as_ref().map(|scene| {
            ctx.fonts_mut(|fonts| {
                let composed = export::compose(scene, fonts);
                let svg = export::shapes_to_svg(&composed.shapes, composed.size);
                export::svg_to_png(&svg).map(|png| (svg, png))
            })
        });
        let rendered = match rendered {
            Some(Ok(rendered)) => rendered,
            Some(Err(error)) => {
                self.save_note = Some(format!("export failed: {error}"));
                return;
            }
            None => {
                self.save_note = Some("export blocked: no view is shown".to_owned());
                return;
            }
        };
        let svg_path = base.with_extension("svg");
        let png_path = base.with_extension("png");
        let written = commands::write_atomic(&svg_path, rendered.0.as_bytes())
            .and_then(|()| commands::write_atomic(&png_path, &rendered.1));
        match written {
            Ok(()) => {
                self.save_note = Some(format!(
                    "exported {} + {}",
                    svg_path.display(),
                    png_path.display()
                ));
            }
            Err(error) => self.save_note = Some(format!("export failed: {error}")),
        }
    }

    /// Whether the CURRENT view is a `FilteredView` (no content slot —
    /// the save guard).
    fn current_view_filtered(&self) -> bool {
        self.view_key.as_deref().is_some_and(|key| {
            self.workspace.as_ref().is_some_and(|workspace| {
                workspace
                    .views
                    .all_views()
                    .iter()
                    .any(|view| view.key() == key && matches!(view, View::Filtered(_)))
            })
        })
    }

    /// The reload watch closure: the display file, the
    /// sidecar once one EXISTS (the save may have created it),
    /// and the DSL `!include` set. stdin has no files. The driver
    /// canonicalizes and filters parent-dir events to exactly these
    /// paths, so the `.{pid}.tmp` siblings our saves churn can never
    /// match.
    fn watch_set(&self) -> Vec<PathBuf> {
        if self.display == "-" {
            return Vec::new();
        }
        let display = PathBuf::from(&self.display);
        let mut set = vec![display.clone()];
        if let Some(sidecar) = commands::save_target(&self.display)
            && sidecar != display
            && sidecar.exists()
        {
            set.push(sidecar);
        }
        // The source DSL of an opened sidecar stays watched:
        // text edits must reach the window even while the sidecar is
        // the display.
        set.extend(companion_dsl(&self.display));
        set.extend(self.includes.iter().map(PathBuf::from));
        set
    }

    /// Which file a reload re-opens: the NEWEST of display,
    /// sidecar and companion DSL — "sidecar preferred": once a save
    /// wrote the sidecar it holds the freshest model AND layout, so it
    /// wins until the source DSL is edited again. Ties keep the
    /// display (a reload re-reads what it already shows).
    fn reload_source(&self) -> PathBuf {
        let display = PathBuf::from(&self.display);
        let mut candidates = vec![display.clone()];
        if let Some(sidecar) = commands::save_target(&self.display)
            && sidecar != display
        {
            candidates.push(sidecar);
        }
        candidates.extend(companion_dsl(&self.display));
        newest_by_mtime(&candidates).unwrap_or(display)
    }

    /// (Re-)arm the reload watch on the current source set: RESET the
    /// live driver (re-baseline — the bytes a save just wrote are
    /// truth and the rename events they fired hash-match, so no
    /// self-triggered reload), or build a fresh one (an adopt
    /// dropped the old, taking its dir watches with it). Failure
    /// degrades to "no reload for this open", never worse.
    pub(super) fn restart_watch(&mut self) {
        let Some(ctx) = self.repaint_ctx.clone() else {
            return; // before the first frame: the frame guard retries
        };
        let set = self.watch_set();
        if set.is_empty() {
            self.reload = None;
            return;
        }
        if let Some(driver) = &self.reload {
            driver.reset(&set);
            return;
        }
        let wake: reload::Wake = Arc::new(move || ctx.request_repaint());
        match reload::ReloadDriver::watch(&set, wake) {
            Ok(driver) => self.reload = Some(driver),
            Err(error) => {
                self.reload_disabled = true;
                self.reload_note = Some(format!("reload watch failed: {error}"));
            }
        }
    }

    /// Live reload: re-open the display file, fold the session
    /// overlays into the layout side and remap through the SHARED
    /// `oxidizr-api::reconcile` engine. Surviving views keep their
    /// on-screen geometry — overlays re-key onto the new ids from the
    /// report's remap lists (uncontested pairs only: ambiguous ids are
    /// NEVER guessed, so their entries drop, R-3). Histories reset (a
    /// command's `from` values name pre-reload ids), `dirty` clears
    /// (the geometry lands baked into the reconciled workspace). A
    /// failed reparse keeps the last-good workspace on screen: the
    /// driver's hashes already advanced with the change, so the note
    /// tells the story and the window stays open (no retry storm).
    pub(super) fn reload_from_disk(&mut self) {
        let source = self.reload_source();
        let opened = match oxidizr_api::open(&Input::File(source), &ApiOptions::default()) {
            Ok(opened) => opened,
            Err(outcome) => {
                self.reload_note = Some(format!(
                    "reload failed — last-good kept: {}",
                    summarize_log(&outcome.log, "open failed")
                ));
                return;
            }
        };
        // The CURRENT on-screen layout is reconcile's layout side:
        // stored geometry plus every session overlay.
        let Some(current) = self.workspace.clone() else {
            return;
        };
        let mut layout = current;
        for (key, overlay) in &self.overlays {
            layout = commands::apply_overlay(&layout, key, overlay);
        }
        let (reconciled, report) = oxidizr_api::reconcile::reconcile(&layout, &opened.workspace);
        // Re-key the overlays onto the new ids. Element and
        // relationship id spaces OVERLAP (both mint "1", "2", …), so
        // each kind looks up in its OWN list; views that died take
        // their overlays with them.
        let view_keys: BTreeSet<&str> = opened
            .workspace
            .views
            .all_views()
            .iter()
            .map(|view| view.key())
            .collect();
        let remap = |pairs: &[(ModelId, ModelId)], old: &ModelId| {
            pairs
                .binary_search_by(|(from, _)| from.cmp(old))
                .ok()
                .map(|index| pairs[index].1.clone())
        };
        let mut restored: HashMap<String, commands::LayoutOverlay> = HashMap::new();
        for (key, overlay) in &self.overlays {
            if !view_keys.contains(key.as_str()) {
                continue;
            }
            let mut next = commands::LayoutOverlay::for_view(&overlay.view_key);
            for (old, at) in &overlay.elements {
                if let Some(new) = remap(&report.remapped_elements, old) {
                    next.elements.insert(new, *at);
                }
            }
            for (old, points) in &overlay.relationships {
                if let Some(new) = remap(&report.remapped_relationships, old) {
                    next.relationships.insert(new, points.clone());
                }
            }
            next.dimensions = overlay.dimensions;
            restored.insert(key.clone(), next);
        }
        let note = format!(
            "reloaded — {} · {} ambiguous",
            report.summary(),
            report.ambiguous.len()
        );
        // a reload keeps the user's view — adopt
        // otherwise falls back to the FIRST view (pending_view is empty
        // after startup), which would strand the remembered camera of
        // the view actually on screen.
        self.pending_view = self.view_key.clone();
        self.adopt(Opened {
            workspace: reconciled,
            ..opened
        });
        self.overlays = restored;
        self.rebuild_scene();
        self.reload_note = Some(note);
    }

    /// Undo/redo one SESSION step (Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y). The
    /// step carries its view: undoing can move a view the user has
    /// switched away from — that view is marked dirty and the scene
    /// rebuilt; the current view repaints unchanged, the dirty dots in
    /// the view list tell the rest.
    pub(super) fn step_history(&mut self, forward: bool) {
        let moved = if forward {
            self.history.redo(&mut self.overlays)
        } else {
            self.history.undo(&mut self.overlays)
        };
        if let Some(view_key) = moved {
            self.dirty.insert(view_key);
            self.rebuild_scene();
        }
    }

    /// One headless engine run over the current scene: [`layout::apply`]
    /// plus [`commands::apply_layout`] — the result lands as ONE
    /// undoable command (positions + routes revert together), then a
    /// refit. A failing engine keeps the previous geometry and lands in
    /// [`Self::layout_error`] — errors as values (P3 charter).
    fn run_layout(&mut self, params: AutomaticLayout) {
        let applied = self
            .scene
            .as_ref()
            .map(|scene| layout::apply_with(self.layout_engine, scene, &params));
        match applied {
            Some(Ok(overrides)) => {
                self.layout_error = None;
                let overlay = self.overlay_for().clone();
                let command = self
                    .scene
                    .as_ref()
                    .and_then(|scene| commands::apply_layout(scene, &overlay, &overrides));
                if let Some(command) = command {
                    self.edit_current_view(command);
                    self.latches.needs_fit = true;
                }
            }
            Some(Err(error)) => self.layout_error = Some(error.to_string()),
            None => {}
        }
    }

    /// Run one headless editing factory against the current scene and
    /// overlay and land its result as ONE undoable edit; a
    /// `None` command (nothing moves) is silently ignored.
    // The factory signature IS the seam between the dialog buttons and
    // the `commands` factories — factoring it out would only name it.
    #[allow(clippy::type_complexity)]
    fn run_edit(
        &mut self,
        make: impl FnOnce(&scene::Scene, &commands::LayoutOverlay) -> Option<commands::Command>,
    ) {
        let overlay = self.overlay_for().clone();
        let command = self.scene.as_ref().and_then(|scene| make(scene, &overlay));
        if let Some(command) = command {
            self.edit_current_view(command);
        }
    }

    /// The the official UI editor's parity row for the layout dialog
    ///: align/distribute cues for a multi-select, plus the
    /// footer's two magnet buttons — bring-back and remove-all-vertices.
    /// Every button runs its `commands` factory through [`Self::run_edit`]
    /// (session overlay only, one undoable step each).
    fn editing_buttons(&mut self, ui: &mut egui::Ui) {
        let ids: Vec<ModelId> = match &self.selection {
            Some(paint::Selection::Items(ids)) => ids.clone(),
            _ => Vec::new(),
        };
        ui.horizontal(|ui| {
            ui.label("Align:");
            for (label, tip, edge) in [
                ("L", "common left edges", commands::AlignEdge::Left),
                (
                    "Cx",
                    "common vertical centres",
                    commands::AlignEdge::CenterX,
                ),
                ("R", "common right edges", commands::AlignEdge::Right),
                ("T", "common top edges", commands::AlignEdge::Top),
                (
                    "Cy",
                    "common horizontal centres",
                    commands::AlignEdge::CenterY,
                ),
                ("B", "common bottom edges", commands::AlignEdge::Bottom),
            ] {
                let enabled = ids.len() >= 2;
                if ui
                    .add_enabled(enabled, egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
                {
                    let ids = ids.clone();
                    self.run_edit(move |scene, overlay| {
                        commands::align(scene, overlay, &ids, edge)
                    });
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Space:");
            for (label, tip, axis) in [
                ("↔", "even horizontal gaps", commands::Axis::Horizontal),
                ("↕", "even vertical gaps", commands::Axis::Vertical),
            ] {
                let enabled = ids.len() >= 3;
                if ui
                    .add_enabled(enabled, egui::Button::new(label))
                    .on_hover_text(tip)
                    .clicked()
                {
                    let ids = ids.clone();
                    self.run_edit(move |scene, overlay| {
                        commands::distribute(scene, overlay, &ids, axis)
                    });
                }
            }
            if ui
                .button("Centre")
                .on_hover_text("centre the selection in the view")
                .clicked()
            {
                let ids = ids.clone();
                self.run_edit(move |scene, overlay| commands::centre_view(scene, overlay, &ids));
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Bring back")
                .on_hover_text(
                    "pull off-canvas elements onto the visible page (official UI: magnet)",
                )
                .clicked()
            {
                let page = self.canvas_page();
                self.run_edit(move |scene, overlay| {
                    page.and_then(|page| commands::bring_back(scene, overlay, page))
                });
            }
            if ui
                .button("Remove vertices")
                .on_hover_text(
                    "straighten every routed relationship (official UI: remove all vertices)",
                )
                .clicked()
            {
                self.run_edit(commands::remove_all_vertices);
            }
        });
    }

    /// The visible canvas as a WORLD rect — the "page" the official UI’s
    /// bring-back button pulls off-canvas boxes onto .
    fn canvas_page(&self) -> Option<scene::Rect> {
        let rect = self.last_rect?;
        let min = self
            .viewport
            .screen_to_world((f64::from(rect.min.x), f64::from(rect.min.y)));
        let max = self
            .viewport
            .screen_to_world((f64::from(rect.max.x), f64::from(rect.max.y)));
        Some(scene::Rect {
            x: min.0,
            y: min.1,
            width: max.0 - min.0,
            height: max.1 - min.1,
        })
    }

    /// The auto-layout dialog — mirrors the official UI the official layout dialog
    /// (direction, rank/node/edge separation, route vertices). `Apply`
    /// runs the engine on the CURRENT scene; `Reset` drops the session
    /// geometry. Only the PARAMETERS persist; the engine
    /// ignores edge separation at MVP scope (parity field only).
    pub(super) fn layout_window(&mut self, ctx: &egui::Context) {
        if !self.toggles.layout {
            return;
        }
        let mut open = true;
        egui::Window::new("Automatic layout")
            .open(&mut open)
            .resizable(false)
            .default_pos([420.0, 120.0])
            .show(ctx, |ui| {
                let mut params = self.layout_params;
                let engines = layout::Engine::available();
                if engines.len() > 1 {
                    //: the choice appears exactly when a second
                    // engine CAN run (feature + $OXIDZR_DOT_BIN).
                    ui.horizontal(|ui| {
                        ui.label("Engine:");
                        egui::ComboBox::from_id_salt("layout-engine")
                            .selected_text(self.layout_engine.label())
                            .show_ui(ui, |ui| {
                                for engine in engines {
                                    ui.selectable_value(
                                        &mut self.layout_engine,
                                        engine,
                                        engine.label(),
                                    );
                                }
                            });
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Direction:");
                    egui::ComboBox::from_id_salt("layout-direction")
                        .selected_text(direction_label(params.rank_direction))
                        .show_ui(ui, |ui| {
                            for direction in [
                                RankDirection::TopBottom,
                                RankDirection::BottomTop,
                                RankDirection::LeftRight,
                                RankDirection::RightLeft,
                            ] {
                                ui.selectable_value(
                                    &mut params.rank_direction,
                                    Some(direction),
                                    direction_label(Some(direction)),
                                );
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Rank sep:");
                    ui.add(egui::DragValue::new(&mut params.rank_separation).range(0..=1000));
                    ui.label("Node sep:");
                    ui.add(egui::DragValue::new(&mut params.node_separation).range(0..=1000));
                    ui.label("Edge sep:");
                    ui.add(egui::DragValue::new(&mut params.edge_separation).range(0..=1000));
                });
                ui.checkbox(&mut params.vertices, "Route vertices");
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        self.layout_params = params;
                        self.run_layout(params);
                    }
                    if ui.button("Reset").clicked() {
                        let command = commands::clear_view(self.overlay_for());
                        self.edit_current_view(command);
                        self.layout_error = None;
                    }
                });
                ui.separator();
                self.editing_buttons(ui);
                if let Some(error) = &self.layout_error {
                    ui.colored_label(egui::Color32::DARK_RED, error);
                }
                ui.weak("Session geometry — not saved to the workspace.");
            });
        self.toggles.layout = open;
    }

    /// Rebuild the icon atlas when the scene changed: rasterize
    /// headlessly with [`icons::build_atlas`], re-upload the single
    /// texture only when its pixels actually changed, and remap the
    /// per-icon uv rects (inset half a pixel against LINEAR bleed).
    /// Failures land in [`Self::icon_errors`] — the canvas keeps the
    /// dashed placeholder for exactly those icons.
    #[allow(clippy::cast_precision_loss)]
    // Atlas caps are 1024×2048 px — every u32 here is exactly f32.
    pub(super) fn sync_icons(&mut self, ctx: &egui::Context) {
        if !self.latches.icons_dirty {
            return;
        }
        self.latches.icons_dirty = false;
        self.icon_uv.clear();
        self.icon_errors.clear();
        let scheme = icons::scheme_code(self.scheme);
        let keys: BTreeSet<icons::IconKey> = self
            .scene
            .iter()
            .flat_map(|scene| scene.items.iter())
            .filter_map(|item| item.icon.clone())
            .map(|icon| icons::IconKey {
                icon,
                size: icons::ICON_RASTER_PX,
                scheme,
            })
            .collect();
        if keys.is_empty() {
            return;
        }
        let atlas = icons::build_atlas(keys, self.base_dir.as_deref());
        atlas.errors.clone_into(&mut self.icon_errors);
        if atlas.width > 0 && (self.icon_texture.is_none() || self.icon_pixels != atlas.pixels) {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [
                    usize::try_from(atlas.width).unwrap_or(0),
                    usize::try_from(atlas.height).unwrap_or(0),
                ],
                &atlas.pixels,
            );
            self.icon_texture =
                Some(ctx.load_texture("oxidizr-icons", image, egui::TextureOptions::LINEAR));
            self.icon_pixels.clone_from(&atlas.pixels);
        }
        let (w, h) = (atlas.width as f32, atlas.height as f32);
        for (key, entry) in &atlas.entries {
            if entry.width == 0 || entry.height == 0 || w == 0.0 || h == 0.0 {
                continue;
            }
            self.icon_uv.insert(
                key.icon.clone(),
                egui::Rect::from_min_max(
                    egui::pos2((entry.x as f32 + 0.5) / w, (entry.y as f32 + 0.5) / h),
                    egui::pos2(
                        ((entry.x + entry.width) as f32 - 0.5) / w,
                        ((entry.y + entry.height) as f32 - 0.5) / h,
                    ),
                ),
            );
        }
    }
}
