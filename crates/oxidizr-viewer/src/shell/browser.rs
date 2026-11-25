//! The eframe shell's left-rail browser: the top toolbar, the view list
//! and the view switch. The App `ui` assembly in the parent drives the
//! toolbar and the list every frame; the canvas and panel legs live in
//! the sibling files.

use crate::{overview, paint};
use eframe::egui;

use oxidizr_core::views::ColorScheme;

use super::widgets::{NavJump, nav_element_rows, view_label, view_row};
use super::{Drag, Viewer};

impl Viewer {
    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.path_edit)
                    .hint_text("workspace.dsl / workspace.json")
                    .desired_width(360.0),
            );
            self.text_focus = edit.has_focus();
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let clicked = ui.button("Open").clicked();
            if enter || clicked {
                self.open_path();
            }
            ui.separator();
            ui.add_enabled_ui(self.scene.is_some(), |ui| {
                if ui.button("−").clicked() {
                    self.zoom_center(0.8);
                }
                if ui.button("+").clicked() {
                    self.zoom_center(1.25);
                }
                if ui.button("Fit").clicked() {
                    self.latches.needs_fit = true;
                }
                if ui.button("Layout").clicked() {
                    self.toggles.layout = true;
                }
                ui.toggle_value(&mut self.toggles.snap, "Snap")
                    .on_hover_text("Grid-snap element drags to 10 px steps");
                let edits = !self.dirty.is_empty();
                if ui
                    .add_enabled(edits, egui::Button::new("Save"))
                    .on_hover_text(
                        "Write the layout back as JSON (Ctrl+S): the opened .json in \
                         place, a <stem>.json sidecar for DSL sources — the DSL text \
                         is never rewritten",
                    )
                    .clicked()
                {
                    self.save_workspace();
                }
                if ui
                    .button("Export")
                    .on_hover_text(
                        "Write <stem>.svg + <stem>.png with the key beside the save \
                         file — the CURRENT view exactly as painted (no selection), \
                         no dialog",
                    )
                    .clicked()
                {
                    self.export_files();
                }
            });
            ui.separator();
            let dark = self.scheme == ColorScheme::Dark;
            if ui
                .button(if dark { "☀ light" } else { "🌙 dark" })
                .clicked()
            {
                self.scheme = if dark {
                    ColorScheme::Light
                } else {
                    ColorScheme::Dark
                };
                self.apply_visuals(&ui.ctx().clone());
                self.rebuild_scene();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label("?").on_hover_text(
                    "Drag box: move — its arrow handles follow (Ctrl detaches) · \
                     Drag empty: pan · Right-drag: lasso · \
                     Middle/Space-drag: pan too · Wheel: zoom · \
                     Trackpad: pinch zooms, two-finger scroll pans · \
                     Click: select (info in the right panel) · Shift-click: add/toggle · \
                     Click edge: select · Drag handle: reroute · Double-click edge: \
                     insert handle · Alt+click handle: delete · Delete: drop all \
                     handles of the selected edge · Ctrl+Z / Ctrl+Y: \
                     undo/redo · Ctrl+S: save layout JSON · Layout dialog: \
                     align/distribute/bring-back · \
                     Ctrl/⌘-click: open element URL · Esc: deselect · edits to the \
                     file on disk (incl. !includes) auto-reload, layout kept · \
                     a saved .json sidecar becomes the open & reload source until \
                     the .dsl is edited again (newest wins) · left-panel search \
                     jumps to elements/views · minimap click/drag recenters · \
                     Export writes <stem>.svg + <stem>.png (with key) beside \
                     the save file · per-view zoom/pan is remembered per \
                     workspace in <stem>.oxidizr.json (a pure cache — delete \
                     it and nothing is lost)",
                );
            });
        });
    }

    pub(super) fn view_list(&mut self, ui: &mut egui::Ui) {
        if self.workspace.is_none() {
            return;
        }
        ui.heading(egui::RichText::new("Views").size(14.0).weak());
        // quick-nav: one search field over element names/ids
        // (the CURRENT view's scene — selection is view-local) and all
        // view titles/keys. A click jumps: element = center + select,
        // view = switch. Match cap keeps the list scannable.
        let nav_edit = ui.add(
            egui::TextEdit::singleline(&mut self.nav_query)
                .hint_text("find element / view")
                .desired_width(f32::INFINITY),
        );
        // The nav field owns the keyboard too (Ctrl+Z inside it is
        // egui's, not the history's) — toolbar ran first, OR here.
        self.text_focus = self.text_focus || nav_edit.has_focus();
        let mut nav_view: Option<String> = None;
        let mut nav_item: Option<NavJump> = None;
        let query = self.nav_query.clone();
        let cap = 8_usize;
        if !query.trim().is_empty() {
            let mut hits = 0_usize;
            if let Some(scene) = &self.scene {
                let (rows, jump) = nav_element_rows(
                    ui,
                    scene,
                    &query,
                    cap,
                    &self.icon_uv,
                    self.icon_texture.as_ref(),
                );
                hits += rows;
                nav_item = jump;
            }
            if let Some(workspace) = &self.workspace {
                for view in workspace.views.all_views() {
                    if hits == 2 * cap {
                        break;
                    }
                    if !overview::nav_matches(&query, &view_label(view), view.key()) {
                        continue;
                    }
                    hits += 1;
                    let label = format!("⧉ {}", view_label(view));
                    if ui
                        .selectable_label(
                            self.view_key.as_deref() == Some(view.key()),
                            egui::RichText::new(label).small(),
                        )
                        .clicked()
                    {
                        nav_view = Some(view.key().to_owned());
                    }
                }
            }
        }
        let Some(workspace) = &self.workspace else {
            return;
        };
        //: the list is graphically split per view KIND (C4
        // granularity first) — `overview::grouped` owns the partition +
        // the (order, key) ranking inside each section.
        let views = workspace.views.all_views();
        let groups = overview::grouped(&views);
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (header, members) in &groups {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(header.to_ascii_uppercase())
                            .size(10.0)
                            .weak(),
                    );
                    for view in members {
                        let selected = self.view_key.as_deref() == Some(view.key());
                        let dirty = self.dirty.contains(view.key());
                        if view_row(ui, *view, selected, dirty) {
                            chosen = Some(view.key().to_owned());
                        }
                    }
                }
            });
        if let Some(key) = chosen.or(nav_view) {
            self.show_view(key);
        }
        if let Some((id, center)) = nav_item {
            // Quick-nav jump: the element's view is already on screen
            // (search ran over its scene), so center + select.
            self.selection = Some(paint::Selection::Items(vec![id]));
            self.center_world(center);
        }
        self.tag_filter(ui);
    }

    /// Show view `key` (the list and the quick-nav share this): session
    /// overlays are PER VIEW — switching keeps them; only the
    /// transient interaction state resets.
    fn show_view(&mut self, key: String) {
        // the view we are LEAVING keeps its camera in
        // the sidecar map before the switch resets the viewport.
        self.record_view();
        self.view_key = Some(key);
        self.selection = None;
        self.latches.needs_fit = true;
        self.restore_scale = None;
        self.drag = Drag::Idle;
        self.layout_error = None;
        self.rebuild_scene();
    }
}
