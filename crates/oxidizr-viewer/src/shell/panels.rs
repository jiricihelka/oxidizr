//! The eframe shell's chrome panels: the status bar, the session
//! tag-filter chips, the key/legend panel, the info block and the
//! diagnostics window. The App `ui` assembly in the parent shows them
//! every frame; the canvas and browser legs live in the sibling files.

use crate::{commands, key, overview, paint};
use eframe::egui::{self};

use super::widgets::{edge_swatch, element_swatch, icon_cell, log_row, snip, truncate};
use super::{FilterMode, Viewer};

impl Viewer {
    pub(super) fn status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // the opened path shows exactly ONCE — the
            // toolbar edit bar — so this bar opens with the view title
            // instead of repeating it (only the no-workspace hint and
            // the selection stay).
            if self.display.is_empty() {
                ui.weak("No workspace — enter a path and press Open");
            }
            if let Some(scene) = &self.scene {
                ui.label(format!(
                    "{} · zoom {:.0}%",
                    scene.title,
                    self.viewport.scale() * 100.0
                ));
            }
            if let Some(selection) = &self.selection {
                ui.separator();
                ui.label(match selection {
                    paint::Selection::Items(ids) => match ids.len() {
                        1 => format!("selected: {}", ids[0]),
                        count => format!("selected: {count} elements"),
                    },
                    paint::Selection::Edge(id) => format!("selected relationship: {id}"),
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let issues = self.open_log.len() + self.icon_errors.len();
                if issues > 0 {
                    let label = egui::RichText::new(format!("⚠ {issues} diagnostics"))
                        .color(egui::Color32::from_rgb(160, 130, 0));
                    if ui.button(label).clicked() {
                        self.toggles.diagnostics = !self.toggles.diagnostics;
                    }
                }
                if let Some(note) = &self.save_note {
                    // Real failures are the SaveError texts ("serialize:
                    // …", "write <path>: …") and export notes
                    // ("export blocked/failed: …"); "save blocked"/"nothing to
                    // save" are guarded no-ops, "saved …"/"exported …" success.
                    let bad = note.starts_with("serialize:")
                        || note.starts_with("write ")
                        || note.starts_with("export ");
                    ui.colored_label(
                        if bad {
                            egui::Color32::DARK_RED
                        } else {
                            egui::Color32::DARK_GREEN
                        },
                        truncate(note, 90),
                    )
                    .on_hover_text(note.clone());
                }
                if let Some(note) = &self.reload_note {
                    // "reload failed"/"reload watch" are the bad lines;
                    // a reconcile summary ("reloaded — …") is the
                    // happy-path report and stays green.
                    let bad = note.starts_with("reload failed") || note.starts_with("reload watch");
                    ui.colored_label(
                        if bad {
                            egui::Color32::DARK_RED
                        } else {
                            egui::Color32::DARK_GREEN
                        },
                        truncate(note, 90),
                    )
                    .on_hover_text(note.clone());
                }
                if self.model_dirty {
                    // Unsaved MODEL edits — the write clears it with the
                    // geometry dots (one Ctrl+S carries both).
                    ui.label(
                        egui::RichText::new("model edited")
                            .color(egui::Color32::from_rgb(160, 130, 0)),
                    )
                    .on_hover_text("Ctrl+S writes the model edits to the opened .json");
                }
                if let Some(note) = &self.model_note {
                    // Refused or blocked model ops (read-only source,
                    // duplicate arrow): always a refusal, always red.
                    ui.colored_label(egui::Color32::DARK_RED, truncate(note, 90))
                        .on_hover_text(note.clone());
                }
                if let Some(error) = self
                    .open_error
                    .as_ref()
                    .or(self.scene_error.as_ref())
                    .or(self.layout_error.as_ref())
                {
                    ui.colored_label(egui::Color32::DARK_RED, truncate(error, 90))
                        .on_hover_text(error.clone());
                }
            });
        });
    }

    /// Session tag-filter chips: the chip set + mode are
    /// SESSION state — the mask flows into `scene::build` via
    /// [`key::session_mask`] and nothing is ever written back.
    pub(super) fn tag_filter(&mut self, ui: &mut egui::Ui) {
        if self.tag_inventory.is_empty() {
            return;
        }
        let mut toggle: Option<String> = None;
        let mut mode: Option<FilterMode> = None;
        let mut clear = false;
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong("Tag filter");
            if ui
                .selectable_label(
                    self.tag_mode == FilterMode::Exclude,
                    egui::RichText::new("exclude").small(),
                )
                .on_hover_text("Hide tagged items (official UI: absent mode)")
                .clicked()
            {
                mode = Some(FilterMode::Exclude);
            }
            if ui
                .selectable_label(
                    self.tag_mode == FilterMode::Include,
                    egui::RichText::new("include").small(),
                )
                .on_hover_text("Show only items/relationships carrying a selected tag")
                .clicked()
            {
                mode = Some(FilterMode::Include);
            }
            clear = !self.tag_sel.is_empty() && ui.small_button("clear").clicked();
        });
        egui::ScrollArea::horizontal()
            .max_height(72.0)
            .show(ui, |ui| {
                for row in &self.tag_inventory {
                    let on = self.tag_sel.contains(&row.tag);
                    let text = format!("{} · {}", row.tag, row.elements + row.relationships);
                    if ui
                        .selectable_label(on, egui::RichText::new(text).small())
                        .on_hover_text(format!(
                            "{} elements · {} relationships store this tag",
                            row.elements, row.relationships
                        ))
                        .clicked()
                    {
                        toggle = Some(row.tag.clone());
                    }
                }
            });
        let mut dirty = false;
        if let Some(new_mode) = mode
            && self.tag_mode != new_mode
        {
            self.tag_mode = new_mode;
            dirty = true;
        }
        if let Some(tag) = toggle {
            match self.tag_sel.iter().position(|t| *t == tag) {
                Some(at) => {
                    self.tag_sel.remove(at);
                }
                None => self.tag_sel.push(tag),
            }
            dirty = true;
        }
        if clear {
            self.tag_sel.clear();
            dirty = true;
        }
        if dirty {
            self.rebuild_scene();
        }
    }

    /// Right panel: the SELECTED object's facts on top,
    /// then the key/legend — the scene's distinct appearances,
    /// folded headlessly by [`key::legend`] (swatches + labels). One
    /// scroll area so a long Info block never squeezes the legend.
    pub(super) fn key_panel(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                self.info_block(ui);
                ui.heading(egui::RichText::new("Key").size(14.0).weak());
                let legend = &self.legend;
                if legend.elements.is_empty()
                    && legend.edges.is_empty()
                    && legend.boundaries.is_empty()
                {
                    ui.weak("Nothing rendered.");
                }
                for entry in &legend.elements {
                    ui.horizontal(|ui| {
                        element_swatch(ui, entry);
                        icon_cell(
                            ui,
                            entry.icon.as_deref(),
                            &self.icon_uv,
                            self.icon_texture.as_ref(),
                        );
                        ui.label(format!("{:?} · {:?}", entry.kind, entry.shape))
                            .on_hover_text(entry.background.clone());
                    });
                }
                if !legend.edges.is_empty() {
                    ui.separator();
                    for entry in &legend.edges {
                        ui.horizontal(|ui| {
                            edge_swatch(ui, entry);
                            ui.label(format!("{:?}", entry.line_style))
                                .on_hover_text(entry.color.clone());
                        });
                    }
                }
                if !legend.boundaries.is_empty() {
                    ui.separator();
                    for entry in &legend.boundaries {
                        ui.label(format!(
                            "boundary: {} ({:?})",
                            key::boundary_label(entry.kind),
                            entry.border
                        ));
                    }
                }
            });
    }

    /// The Info block (user: "When I click an object, info about that
    /// object should appear in the right sidebar"): rows built
    /// headlessly by [`overview::selection_info`], laid out as a
    /// label/value grid. Absent for an empty/off-scene selection.
    pub(super) fn info_block(&self, ui: &mut egui::Ui) {
        let (Some(scene), Some(selection)) = (&self.scene, &self.selection) else {
            return;
        };
        let rows = overview::selection_info(scene, selection);
        if rows.is_empty() {
            return;
        }
        ui.heading(egui::RichText::new("Selection").size(14.0).weak());
        egui::Grid::new("selection-info")
            .num_columns(2)
            .spacing([8.0, 2.0])
            .max_col_width(170.0)
            .show(ui, |ui| {
                for (label, value) in rows {
                    ui.weak(label);
                    ui.label(value);
                    ui.end_row();
                }
            });
        ui.separator();
    }

    /// Full diagnostics list in a dismissible window;
    /// the status-bar count toggles it.
    pub(super) fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.toggles.diagnostics {
            return;
        }
        let mut open = true;
        egui::Window::new("Diagnostics")
            .open(&mut open)
            .default_pos([120.0, 96.0])
            .default_size([580.0, 260.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if self.open_log.is_empty() && self.icon_errors.is_empty() {
                            ui.weak("No diagnostics — the workspace opened cleanly.");
                        }
                        for item in &self.open_log {
                            let (color, text) = log_row(item);
                            ui.colored_label(color, text);
                        }
                        for (icon, message) in &self.icon_errors {
                            ui.colored_label(
                                egui::Color32::from_rgb(160, 130, 0),
                                format!("icon {}: {message}", snip(icon)),
                            );
                        }
                    });
            });
        self.toggles.diagnostics = open;
    }

    /// The editing overlay of the CURRENT view, created on first use
    /// (: one overlay + history per view key, — the
    /// workspace model itself is never touched).
    pub(super) fn overlay_for(&mut self) -> &mut commands::LayoutOverlay {
        let view_key = self.view_key.clone().unwrap_or_default();
        self.overlays
            .entry(view_key.clone())
            .or_insert_with(|| commands::LayoutOverlay::for_view(&view_key))
    }
}
