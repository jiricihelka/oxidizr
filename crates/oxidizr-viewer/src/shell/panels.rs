//! The eframe shell's chrome panels: the status bar, the session
//! tag-filter chips, the key/legend panel, the info block and the
//! diagnostics window. The App `ui` assembly in the parent shows them
//! every frame; the canvas and browser legs live in the sibling files.

use crate::{commands, key, overview, paint};
use eframe::egui::{self};
use oxidizr_core::Id as ModelId;

use super::widgets::{edge_swatch, element_swatch, icon_cell, log_row, snip, truncate};
use super::{FieldKey, FilterMode, Viewer};

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
    pub(super) fn key_panel(&mut self, ui: &mut egui::Ui) {
        self.field_seen.clear();
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
        //: A field whose widget VANISHED without an egui focus-loss
        // event — the selection moved on, or the panel closed — commits
        // its draft exactly like a click-away: one command, one step.
        if let Some((key, draft)) = self.field_draft.take() {
            if self.field_seen.contains(&key) {
                self.field_draft = Some((key, draft));
            } else {
                self.commit_field(&key, &draft);
            }
        }
    }

    /// The Info block (user: "When I click an object, info about that
    /// object should appear in the right sidebar"): rows built
    /// headlessly by [`overview::selection_info`] laid out as a
    /// label/value grid, plus what [`Self::selection_details`] gathers
    /// on top — for a single ELEMENT the model's stored tags, properties,
    /// MARKDOWN description and URL link; for an EDGE on a
    /// model-editing source the editable text slots. Absent for an
    /// empty/off-scene selection.
    pub(super) fn info_block(&mut self, ui: &mut egui::Ui) {
        let Some(Details {
            mut rows,
            description_md,
            tags,
            properties,
            url,
            fields,
        }) = self.selection_details()
        else {
            return;
        };
        // The Url row becomes a live link; the plain Description row
        // yields to the markdown rendering when the model has text.
        rows.retain(|(label, _)| match *label {
            "Url" => url.is_none(),
            "Description" => description_md.is_none(),
            _ => true,
        });
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
        if !tags.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.weak("Tags:");
                ui.label(tags.join(", "));
            });
        }
        if !properties.is_empty() {
            egui::Grid::new("selection-properties")
                .num_columns(2)
                .spacing([8.0, 2.0])
                .max_col_width(170.0)
                .show(ui, |ui| {
                    for (name, value) in &properties {
                        ui.weak(name);
                        ui.label(value);
                        ui.end_row();
                    }
                });
        }
        if let Some(text) = &description_md {
            ui.weak("Description");
            egui_commonmark::CommonMarkViewer::new().show(ui, &mut self.md, text);
        }
        if let Some(link) = &url {
            ui.hyperlink_to(truncate(link, 60), link);
        }
        if let Some(edge) = fields {
            ui.separator();
            self.field_row(
                ui,
                "Description",
                &FieldKey::Description(edge.id.clone()),
                &edge.description,
            );
            self.field_row(
                ui,
                "Technology",
                &FieldKey::Technology(edge.id.clone()),
                &edge.technology,
            );
            if let Some((view_key, current)) = &edge.view_label {
                self.field_row(
                    ui,
                    "View label",
                    &FieldKey::ViewLabel(view_key.clone(), edge.id),
                    current,
                );
            }
        }
        ui.separator();
    }

    /// Snapshot of the selection's panel data, gathered while only
    /// immutable borrows live: the headless fact rows plus what the scene
    /// does NOT carry — the MODEL item of a single element (full
    /// description text, STORED tags, properties) and the editable values
    /// of a selected edge. The view-label value rides only when the
    /// CURRENT view carries a `RelationshipView` row — a view without the
    /// row has nothing to override (the same guard the factory applies).
    fn selection_details(&self) -> Option<Details> {
        let (scene, selection) = (self.scene.as_ref()?, self.selection.as_ref()?);
        let rows = overview::selection_info(scene, selection);
        if rows.is_empty() {
            return None;
        }
        let single = match selection {
            paint::Selection::Items(ids) if ids.len() == 1 => Some(&ids[0]),
            _ => None,
        };
        let (description_md, tags, properties) = single
            .zip(self.workspace.as_ref())
            .and_then(|(id, workspace)| key::element(&workspace.model, id))
            .map_or_else(Default::default, |item| {
                (
                    item.description
                        .clone()
                        .filter(|text| !text.trim().is_empty()),
                    item.tags.stored().to_vec(),
                    item.properties
                        .iter()
                        .map(|(name, value)| (name.to_owned(), value.to_owned()))
                        .collect::<Vec<_>>(),
                )
            });
        let url = rows
            .iter()
            .find(|(label, _)| *label == "Url")
            .map(|(_, url)| url.clone());
        let fields = match selection {
            paint::Selection::Edge(id) if self.can_edit_model() => {
                self.workspace.as_ref().and_then(|workspace| {
                    let row = workspace
                        .model
                        .relationships
                        .iter()
                        .find(|row| row.id == *id)?;
                    let view_label = self.view_key.as_ref().and_then(|view_key| {
                        workspace
                            .views
                            .all_views()
                            .iter()
                            .find(|view| view.key() == view_key.as_str())?
                            .content()?
                            .relationships
                            .iter()
                            .find(|rv| rv.id == *id)
                            .map(|rv| rv.description.clone().unwrap_or_default())
                            .map(|current| (view_key.clone(), current))
                    });
                    Some(EdgeFields {
                        id: id.clone(),
                        description: row.description.clone().unwrap_or_default(),
                        technology: row.technology.clone().unwrap_or_default(),
                        view_label,
                    })
                })
            }
            _ => None,
        };
        Some(Details {
            rows,
            description_md,
            tags,
            properties,
            url,
            fields,
        })
    }

    /// One editable text slot of the selected relationship: the field
    /// shows the model value; FOCUS starts a draft (kept in
    /// [`Self::field_draft`]), typing edits ONLY the draft, and Enter or
    /// focus-loss commits exactly ONE model command (empty text clears
    /// the slot). While focused the field owns the keyboard:
    /// `text_focus` yields the canvas chords.
    fn field_row(&mut self, ui: &mut egui::Ui, label: &str, key: &FieldKey, base: &str) {
        self.field_seen.push(key.clone());
        let editing = self
            .field_draft
            .as_ref()
            .is_some_and(|(slot, _)| slot == key);
        let mut text = self
            .field_draft
            .as_ref()
            .filter(|(slot, _)| slot == key)
            .map_or_else(|| base.to_owned(), |(_, draft)| draft.clone());
        ui.horizontal(|ui| {
            ui.weak(label);
            let response = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .id_salt(format!("{key:?}"))
                    .desired_width(150.0),
            );
            if response.has_focus() {
                self.text_focus = true;
                if editing {
                    if let Some((_, draft)) = &mut self.field_draft {
                        draft.clone_from(&text);
                    }
                } else {
                    self.field_draft = Some((key.clone(), text));
                }
            }
            let enter = response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                response.surrender_focus();
            }
            //: Enter commits and surrenders focus; a click away lands
            // here as egui's focus-loss. `take_if` matches the slot
            // first — another field's draft is never stolen.
            if (enter || response.lost_focus())
                && let Some((slot, draft)) = self.field_draft.take_if(|(slot, _)| slot == key)
            {
                self.commit_field(&slot, &draft);
            }
        });
    }

    /// Land a committed field draft as ONE undoable model step through
    /// the model funnel: the description/technology slots write the
    /// model row, the view-label slot writes the `RelationshipView`
    /// override it was keyed to. No text or an unchanged value builds
    /// no command — the draft just dies.
    fn commit_field(&mut self, key: &FieldKey, text: &str) {
        let text = text.to_owned();
        match key {
            FieldKey::Description(id) => {
                let id = id.clone();
                self.run_model_edit(move |workspace, _| {
                    commands::set_relationship_text(
                        workspace,
                        &id,
                        commands::TextField::Description,
                        Some(text),
                    )
                });
            }
            FieldKey::Technology(id) => {
                let id = id.clone();
                self.run_model_edit(move |workspace, _| {
                    commands::set_relationship_text(
                        workspace,
                        &id,
                        commands::TextField::Technology,
                        Some(text),
                    )
                });
            }
            FieldKey::ViewLabel(view_key, id) => {
                let (view_key, id) = (view_key.clone(), id.clone());
                self.run_model_edit(move |workspace, _| {
                    commands::set_relationship_view_label(workspace, &view_key, &id, Some(text))
                });
            }
        }
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

/// The details panel's owned snapshot of the current selection — built by
/// [`Viewer::selection_details`] so the render leg owns nothing borrowed
/// (a field commit can rebuild the scene mid-frame).
struct Details {
    /// Headless fact rows ([`overview::selection_info`]).
    rows: Vec<(&'static str, String)>,
    /// Full description text (markdown) of a single element.
    description_md: Option<String>,
    /// STORED (custom) tags of a single element.
    tags: Vec<String>,
    /// Properties of a single element, model order.
    properties: Vec<(String, String)>,
    /// URL of the element or edge (the row value — rendered as a link).
    url: Option<String>,
    /// Editable edge values — present only when the source may be
    /// model-edited.
    fields: Option<EdgeFields>,
}

/// The editable text values of a selected edge: the model row's slots
/// plus the CURRENT view's label override (`None` when that view does not
/// show the relationship — nothing to override).
struct EdgeFields {
    /// The relationship these values belong to.
    id: ModelId,
    /// Model row `description` ("" when absent).
    description: String,
    /// Model row `technology` ("" when absent).
    technology: String,
    /// `(view_key, override)` of the current view's row ("" override when
    /// absent).
    view_label: Option<(String, String)>,
}
