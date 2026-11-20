//! The thin presentation shell (P3-D1): the ONLY code in this crate
//! allowed to reference the GUI stack, compiled only with `--features gui`.
//! All behavior lives in the library core (`scene`/`hit`/`paint`/`key`/
//! `icons`/`commands`/`reload`), which is tested headlessly — this file opens a
//! workspace through `oxidizr_api::open`, wires `scene::build` +
//! `hit::pick` + `paint::paint_scene` to pan/zoom/click events, and owns
//! no logic of its own beyond presentation.
//!
//! M1-4 scope (a/b split of the §P3-M1 row): canvas — open (argv +
//! toolbar), view list, pan/zoom/select, tooltips, element URLs
//! (ctrl/⌘-click), light/dark (+ OS preference at startup, §P3-D7),
//! Fit — plus the overlays: key/legend panel (right), session
//! tag-filter chips (left, V7) and the diagnostics window. M1-5 adds the
//! icon atlas: `icons` rasterizes style icons headlessly, the shell
//! uploads ONE texture and overlays its cells on the paint placeholders.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use eframe::egui::{self, Id, Pos2, Vec2};
use oxidizr_api::{ApiOptions, Input, LogItem, Opened, Outcome};
use oxidizr_core::views::{
    AutomaticLayout, ColorScheme, FilterMode, LineStyle, RankDirection, View,
};
use oxidizr_core::{Id as ModelId, Workspace};
use oxidizr_diag::Severity;
use oxidizr_viewer::{commands, hit, icons, key, layout, paint, scene};

/// Storage keys (§P3-D2: eframe storage only, no config files).
const KEY_THEME: &str = "oxidizr.viewer/theme";
const KEY_ZOOM: &str = "oxidizr.viewer/zoom";
const KEY_VIEW: &str = "oxidizr.viewer/view";
const KEY_LAYOUT: &str = "oxidizr.viewer/layout";

/// CLI arguments (flags + positional only, `agents/plan/WORKFLOW.md`).
#[derive(Parser)]
#[command(
    name = "oxidizr-viewer",
    version,
    about = "Read-only Structurizr workspace viewer (P3-M1)"
)]
struct Args {
    /// Workspace file to open at startup (`.dsl` or `.json`).
    file: Option<PathBuf>,
    /// Color scheme; `system` consults the OS preference, then the
    /// persisted runtime toggle (§P3-D7).
    #[arg(long, value_enum, default_value_t = ThemeArg::System)]
    theme: ThemeArg,
}

/// `--theme` values (§P3-D7).
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ThemeArg {
    /// Force the light scheme.
    Light,
    /// Force the dark scheme.
    Dark,
    /// Persisted toggle, else the OS preference, else light.
    System,
}

fn main() -> eframe::Result {
    let args = Args::parse();
    // Open before the window comes up: failures become in-app state
    // (the status bar), never a crash — errors are values (P3 charter).
    let opened = args
        .file
        .as_ref()
        .map(|path| oxidizr_api::open(&Input::File(path.clone()), &ApiOptions::default()));
    let path_shown = args
        .file
        .as_ref()
        .map_or_else(String::new, |path| path.display().to_string());
    let theme_arg = args.theme;
    let mut viewer = Viewer::new();
    eframe::run_native(
        "oxidizr-viewer",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1360.0, 860.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            viewer.startup(cc.storage, theme_arg, path_shown, opened);
            Ok(Box::new(viewer))
        }),
    )
}

/// The viewer app state: a workspace, one selected view, its rendered
/// scene, the canvas viewport and the theme toggle. No GUI types outside
/// presentation fields (the lib core owns the rest).
// Session flags (fit/theme/diagnostics/icons), not state to model as enums
// — same reading as the P1-D8 CLI flags precedent (oxidizr-cli lib.rs).
#[allow(clippy::struct_excessive_bools)]
struct Viewer {
    /// Loaded workspace (after a successful open).
    workspace: Option<Workspace>,
    /// Input display name (`-`/path, from [`Opened`]).
    display: String,
    /// Diagnostics of the current workspace or failed open (panel = M1-4b).
    open_log: Vec<LogItem>,
    /// Last open failure (status bar, red).
    open_error: Option<String>,
    /// Toolbar path editor.
    path_edit: String,
    /// The path field owns the keyboard (undo/redo keys skip then).
    path_focus: bool,
    /// Selected view key.
    view_key: Option<String>,
    /// Scene rendered from workspace + view + scheme.
    scene: Option<scene::Scene>,
    /// Scene-build failure (centered, red).
    scene_error: Option<String>,
    /// Effective color scheme.
    scheme: ColorScheme,
    /// Selected item/edge (id-based, survives rebuilds).
    selection: Option<paint::Selection>,
    /// Hovered hit (tooltip + cursor).
    hover: Option<hit::Hit>,
    /// World↔screen transform.
    viewport: hit::Viewport,
    /// Re-fit the viewport into the canvas on the next paint.
    needs_fit: bool,
    /// Zoom restored from storage, applied on the first painted frame
    /// (the canvas size is unknown before that).
    restore_scale: Option<f64>,
    /// View key restored from storage (honored by the next adopt).
    pending_view: Option<String>,
    /// Canvas rect from the previous frame (zoom-button pivot).
    last_rect: Option<egui::Rect>,
    /// Last window title pushed to the OS (change detection).
    shown_title: String,
    /// Visuals pushed to egui once (the creator closure has no Context).
    visuals_ready: bool,
    /// Stored-tag inventory for the filter chips (recomputed on open).
    tag_inventory: Vec<key::TagCount>,
    /// Chip semantics: hide (Exclude) or show only (Include) tagged.
    tag_mode: FilterMode,
    /// Selected filter tags — SESSION state (P3-V7), never persisted.
    tag_sel: Vec<String>,
    /// Legend of the current scene (folded by [`key::legend`]).
    legend: key::Key,
    /// Diagnostics window visibility (session).
    show_diagnostics: bool,
    /// Workspace file's directory — resolves file-path icon strings.
    base_dir: Option<PathBuf>,
    /// The single icon-atlas texture (uploaded when pixels change, M1-5).
    icon_texture: Option<egui::TextureHandle>,
    /// Atlas uv rect per icon string (rebuilt with every atlas).
    icon_uv: BTreeMap<String, egui::Rect>,
    /// Atlas pixels last uploaded (skips redundant GPU uploads).
    icon_pixels: Vec<u8>,
    /// `snip(icon)` → reason, one row per failing icon (M1-5, warnings).
    icon_errors: Vec<(String, String)>,
    /// Icons are stale wrt the scene (set on open/rebuild).
    icons_dirty: bool,
    /// Session editing overlays, one per view key (P3-M3); NEVER
    /// persisted — M3-5 owns saving (editor rule, P3-D0). Kept across
    /// view switches, cleared when a workspace is adopted.
    overlays: HashMap<String, commands::LayoutOverlay>,
    /// Undo/redo history per view key, parallel to [`Self::overlays`].
    histories: HashMap<String, commands::History>,
    /// Grid-snap drags to multiples of [`commands::snapped`]'s step.
    snap: bool,
    /// The in-flight canvas interaction (pan / drag / lasso).
    drag: Drag,
    /// Views edited since the last adopt (dirty marker; M3-5 save).
    dirty: BTreeSet<String>,
    /// Last auto-layout failure (dialog + status bar).
    layout_error: Option<String>,
    /// Auto-layout dialog visibility.
    show_layout: bool,
    /// Dialog parameters — a persisted preference (§P3-D2), mirroring
    /// the reference `auto-layout.jspf` fields.
    layout_params: AutomaticLayout,
}

/// The in-flight left-button canvas interaction (P3-M3). The mode is
/// decided at PRESS time and can no longer change: dragging a box
/// moves the selection, dragging empty canvas lassos, panning moved to
/// middle-button/Space so the left button is free for editing.
enum Drag {
    /// No press, or a press still undecided (a click is still
    /// possible): nothing mutates the scene.
    Idle,
    /// Pan: the plain-drag behavior of M1, unchanged.
    Pan,
    /// Move the selected boxes: `origins` are their effective top-lefts
    /// captured at press; every frame writes `drag_target(origin,
    /// delta, snap)` into the overlay, so preview frames equal the
    /// geometry the release commits.
    Move {
        /// Press-time selection: id + effective top-left (content order).
        origins: Vec<(ModelId, (f64, f64))>,
        /// Press point (screen px) — frame deltas measure from here.
        press: (f64, f64),
        /// Latest preview delta (world px); release commits exactly it.
        delta: (f64, f64),
    },
    /// Lasso box-select: `anchor` is the press point (screen px);
    /// `base` is the selection Shift extends (empty without Shift).
    Lasso {
        /// Press point (egui screen points).
        anchor: Pos2,
        /// Selection to keep and add to (Shift-drag), empty otherwise.
        base: Vec<ModelId>,
    },
}

impl Viewer {
    fn new() -> Self {
        Self {
            workspace: None,
            display: String::new(),
            open_log: Vec::new(),
            open_error: None,
            path_edit: String::new(),
            path_focus: false,
            view_key: None,
            scene: None,
            scene_error: None,
            scheme: ColorScheme::Light,
            selection: None,
            hover: None,
            viewport: hit::Viewport::new(1.0, (0.0, 0.0)),
            needs_fit: false,
            restore_scale: None,
            pending_view: None,
            last_rect: None,
            shown_title: String::new(),
            visuals_ready: false,
            tag_inventory: Vec::new(),
            tag_mode: FilterMode::Exclude,
            tag_sel: Vec::new(),
            legend: key::Key::default(),
            show_diagnostics: false,
            base_dir: None,
            icon_texture: None,
            icon_uv: BTreeMap::new(),
            icon_pixels: Vec::new(),
            icon_errors: Vec::new(),
            icons_dirty: false,
            overlays: HashMap::new(),
            histories: HashMap::new(),
            snap: false,
            drag: Drag::Idle,
            dirty: BTreeSet::new(),
            layout_error: None,
            show_layout: false,
            layout_params: AutomaticLayout::default(),
        }
    }

    /// Resolve theme + persisted state, then adopt the argv open result.
    fn startup(
        &mut self,
        storage: Option<&dyn eframe::Storage>,
        theme: ThemeArg,
        path_shown: String,
        opened: Option<Result<Opened, Outcome>>,
    ) {
        self.scheme = match theme {
            ThemeArg::Light => ColorScheme::Light,
            ThemeArg::Dark => ColorScheme::Dark,
            ThemeArg::System => storage
                .and_then(|s| s.get_string(KEY_THEME))
                .map_or_else(detect_os_scheme, |stored| parse_scheme(&stored)),
        };
        self.restore_scale = storage
            .and_then(|s| s.get_string(KEY_ZOOM))
            .and_then(|zoom| zoom.parse::<f64>().ok());
        self.pending_view = storage.and_then(|s| s.get_string(KEY_VIEW));
        self.layout_params = storage
            .and_then(|s| s.get_string(KEY_LAYOUT))
            .as_deref()
            .and_then(layout::decode)
            .unwrap_or_default();
        self.path_edit = path_shown;
        match opened {
            Some(Ok(opened)) => self.adopt(opened),
            Some(Err(outcome)) => {
                self.open_error = Some(summarize_log(&outcome.log, "open failed"));
                self.open_log = outcome.log;
            }
            None => {}
        }
    }

    /// Take over a freshly opened workspace: first (or restored) view,
    /// re-fit unless a zoom is pending.
    fn adopt(&mut self, opened: Opened) {
        self.display = opened.display.clone();
        self.path_edit = opened.display;
        self.open_log = opened.log;
        self.open_error = None;
        self.selection = None;
        let mut views = opened.workspace.views.all_views();
        views.sort_by_key(|view| (view.base().order, view.key().to_owned()));
        self.view_key = self
            .pending_view
            .take()
            .filter(|key| views.iter().any(|view| view.key() == key))
            .or_else(|| views.first().map(|view| view.key().to_owned()));
        self.base_dir = if self.display == "-" {
            None
        } else {
            PathBuf::from(&self.display).parent().map(PathBuf::from)
        };
        self.tag_inventory = key::tags(&opened.workspace);
        self.tag_sel.clear();
        // Fresh workspace: every session edit dies with the old one
        // (scope cut §P3-M3 — overlays survive VIEW switches, not OPENs).
        self.overlays.clear();
        self.histories.clear();
        self.dirty.clear();
        self.drag = Drag::Idle;
        self.layout_error = None;
        self.workspace = Some(opened.workspace);
        self.needs_fit = self.restore_scale.is_none();
        self.rebuild_scene();
        self.autolayout_once();
    }

    /// Re-run `scene::build` for the current workspace/view/scheme,
    /// intersected with the session tag mask (empty selection = no
    /// mask), and refresh the legend.
    fn rebuild_scene(&mut self) {
        let built = match (&self.workspace, &self.view_key) {
            (Some(workspace), Some(view_key)) => {
                let mask = if self.tag_sel.is_empty() {
                    None
                } else {
                    key::session_mask(workspace, view_key, self.tag_mode, &self.tag_sel)
                };
                let overrides = self
                    .view_key
                    .as_deref()
                    .and_then(|key| self.overlays.get(key))
                    .map(commands::LayoutOverlay::as_overrides);
                Some(scene::build(
                    workspace,
                    view_key,
                    self.scheme,
                    mask.as_ref(),
                    overrides.as_ref(),
                ))
            }
            _ => None,
        };
        match built {
            Some(Ok(scene)) => {
                self.legend = key::legend(&scene);
                self.icons_dirty = true;
                self.scene = Some(scene);
                self.scene_error = None;
            }
            Some(Err(error)) => {
                self.legend = key::Key::default();
                self.scene = None;
                self.scene_error = Some(error.to_string());
            }
            None => {
                self.legend = key::Key::default();
                self.scene = None;
                self.scene_error = None;
            }
        }
    }

    /// Open `path_edit` through the api entry (M1-1); failures keep the
    /// previous workspace on screen and surface in the status bar.
    fn open_path(&mut self) {
        let raw = self.path_edit.trim().to_owned();
        if raw.is_empty() {
            self.open_error = Some("enter a workspace path first".to_owned());
            return;
        }
        match oxidizr_api::open(&Input::File(PathBuf::from(&raw)), &ApiOptions::default()) {
            Ok(opened) => self.adopt(opened),
            Err(outcome) => {
                self.open_error = Some(summarize_log(&outcome.log, "open failed"));
                self.open_log = outcome.log;
            }
        }
    }

    /// Zoom at the middle of the last painted canvas (toolbar buttons).
    fn zoom_center(&mut self, factor: f64) {
        let pivot = self.last_rect.map_or((0.0, 0.0), |rect| {
            (f64::from(rect.center().x), f64::from(rect.center().y))
        });
        self.viewport.zoom_at(factor, pivot);
    }

    /// Push scheme visuals; egui keeps its own defaults until then.
    fn apply_visuals(&self, ctx: &egui::Context) {
        ctx.set_visuals(if self.scheme == ColorScheme::Dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.visuals_ready {
            self.apply_visuals(&ctx);
            self.visuals_ready = true;
        }
        let title = format!(
            "oxidizr-viewer — {}",
            if self.display.is_empty() {
                "no workspace"
            } else {
                &self.display
            }
        );
        if self.shown_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shown_title = title;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
        }
        // Undo/redo (M3-3): Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y walk the
        // CURRENT view's history — skipped while the toolbar path field
        // owns the keyboard (Ctrl+Z inside the path edit is egui's).
        if self.scene.is_some() && !self.path_focus {
            let (undo, redo) = ctx.input(|i| {
                let command = i.modifiers.command;
                let z = i.key_pressed(egui::Key::Z);
                (
                    command && z && !i.modifiers.shift,
                    (command && z && i.modifiers.shift) || (command && i.key_pressed(egui::Key::Y)),
                )
            });
            if undo {
                self.step_history(false);
            } else if redo {
                self.step_history(true);
            }
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));
        if self.workspace.is_some() {
            egui::Panel::left("views")
                .default_size(240.0)
                .show(ui, |ui| self.view_list(ui));
        }
        if self.scene.is_some() {
            egui::Panel::right("key")
                .default_size(220.0)
                .show(ui, |ui| self.key_panel(ui));
        }
        self.sync_icons(&ctx);
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
        self.diagnostics_window(&ctx);
        self.layout_window(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // §P3-D2: only the toggleable state persists (window geometry is
        // eframe's own `persist_window`).
        storage.set_string(
            KEY_THEME,
            if self.scheme == ColorScheme::Dark {
                "dark".to_owned()
            } else {
                "light".to_owned()
            },
        );
        storage.set_string(KEY_ZOOM, self.viewport.scale().to_string());
        if let Some(key) = &self.view_key {
            storage.set_string(KEY_VIEW, key.clone());
        }
        storage.set_string(KEY_LAYOUT, layout::encode(&self.layout_params));
    }
}

// ---- panels ----------------------------------------------------------------

impl Viewer {
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.path_edit)
                    .hint_text("workspace.dsl / workspace.json")
                    .desired_width(360.0),
            );
            self.path_focus = edit.has_focus();
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
                    self.needs_fit = true;
                }
                if ui.button("Layout").clicked() {
                    self.show_layout = true;
                }
                ui.toggle_value(&mut self.snap, "Snap")
                    .on_hover_text("Grid-snap element drags to 10 px steps");
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
                    "Drag box: move · Drag empty: lasso · Middle/Space-drag: pan · \
                     Wheel: zoom · Click: select · Shift-click: add/toggle · \
                     Ctrl/⌘-click: open element URL · Ctrl+Z / Ctrl+Y: undo/redo · \
                     Esc: deselect · keys work on the canvas",
                );
            });
        });
    }

    fn view_list(&mut self, ui: &mut egui::Ui) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        ui.heading(egui::RichText::new("Views").size(14.0).weak());
        let mut views = workspace.views.all_views();
        views.sort_by_key(|view| (view.base().order, view.key().to_owned()));
        let mut chosen: Option<String> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for view in views {
                    let mut label = view_label(view);
                    if self.dirty.contains(view.key()) {
                        label.push_str(" ●");
                    }
                    let response = ui.selectable_label(
                        self.view_key.as_deref() == Some(view.key()),
                        egui::RichText::new(label).small(),
                    );
                    if response.clicked() {
                        chosen = Some(view.key().to_owned());
                    }
                    response.on_hover_text(format!(
                        "key: {}\n{}",
                        view.key(),
                        view.base()
                            .description
                            .as_deref()
                            .unwrap_or("(no description)")
                    ));
                }
            });
        if let Some(key) = chosen {
            self.view_key = Some(key);
            self.selection = None;
            self.needs_fit = true;
            self.restore_scale = None;
            // Session overlays are PER VIEW (§P3-M3) — switching keeps
            // them; only the transient interaction state resets.
            self.drag = Drag::Idle;
            self.layout_error = None;
            self.rebuild_scene();
            self.autolayout_once();
        }
        self.tag_filter(ui);
    }

    #[allow(clippy::too_many_lines)] // the canvas pass: input → fit → paint → tip
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        self.last_rect = Some(rect);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        // §P3-M3 input state machine: the drag MODE is decided at press
        // (pan = middle button or Space, move = left on a box, lasso =
        // left on background) and every frame of that mode acts on it.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.drag_started_by(egui::PointerButton::Middle)
            || (space && response.drag_started_by(egui::PointerButton::Primary))
        {
            self.drag = Drag::Pan;
        } else if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(origin) = ui.input(|i| i.pointer.press_origin()) {
                let shift = ui.input(|i| i.modifiers.shift);
                self.drag = self.press_mode(origin, shift);
            }
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
                    self.preview_move(&origins, world_delta);
                    self.drag = Drag::Move {
                        origins,
                        press,
                        delta: world_delta,
                    };
                } else if release {
                    // Released: commit the SAME absolute geometry the
                    // preview wrote (idempotent re-apply through the
                    // history, so one Ctrl+Z reverts the whole drag).
                    if let Some(command) = commands::drag_move(&origins, world_delta, self.snap) {
                        self.edit_current_view(command);
                    } else {
                        // Nothing moved (a snap pulled the target back
                        // onto its origin): restore the press geometry
                        // the live preview had written into the overlay.
                        {
                            let overlay = self.overlay_for();
                            for (id, origin) in &origins {
                                overlay.elements.insert(id.clone(), *origin);
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
        }
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON && response.hovered() {
            if let Some(pt) = ui.input(|i| i.pointer.hover_pos()) {
                let factor = (scroll * 0.005_f32).exp();
                self.viewport
                    .zoom_at(f64::from(factor), (f64::from(pt.x), f64::from(pt.y)));
            }
        }
        if response.clicked() {
            if let Some(pt) = response.interact_pointer_pos() {
                let (command, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                self.canvas_click(pt, command, shift, &ctx);
            }
        }
        self.hover = match (response.hovered(), ui.input(|i| i.pointer.hover_pos())) {
            (true, Some(pointer)) => self.scene.as_ref().and_then(|scene| {
                hit::pick(
                    scene,
                    self.viewport,
                    (f64::from(pointer.x), f64::from(pointer.y)),
                )
            }),
            _ => None,
        };
        ctx.set_cursor_icon(if self.hover.is_some() {
            egui::CursorIcon::PointingHand
        } else {
            egui::CursorIcon::Default
        });

        let screen = (f64::from(rect.width()), f64::from(rect.height()));
        if let Some(scale) = self.restore_scale.take() {
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
        } else if self.needs_fit {
            if let Some(scene) = &self.scene {
                self.viewport = hit::Viewport::fit(scene.bounds, screen);
                self.needs_fit = false;
            }
        }

        let painter = ui.painter_at(rect);
        if let Some(scene) = &self.scene {
            let options = paint::PaintOptions {
                screen: rect,
                selection: self.selection.as_ref(),
            };
            let shapes =
                ctx.fonts_mut(|fonts| paint::paint_scene(scene, &self.viewport, fonts, &options));
            for shape in shapes {
                painter.add(shape);
            }
            // M1-5: rasterized style icons over the placeholders the
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
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Open a workspace: enter a path in the toolbar (or launch\n\
                 with `oxidizr-viewer <file>`)",
                egui::FontId::default(),
                ui.visuals().weak_text_color(),
            );
        }
        // §P3-M3: lasso rubber band — accent rect from anchor to pointer.
        if let Drag::Lasso { anchor, .. } = &self.drag {
            if let Some(pos) = cursor {
                let band = egui::Rect::from_two_pos(*anchor, pos);
                painter.rect_stroke(
                    band,
                    0.0,
                    egui::Stroke::new(1.0, paint::SELECTION_COLOR),
                    egui::StrokeKind::Inside,
                );
            }
        }
        // Hover tooltip (fg order, next to the cursor).
        if let (Some(hit), Some(pos), Some(scene)) =
            (self.hover, ui.input(|i| i.pointer.hover_pos()), &self.scene)
        {
            let text = tooltip_text(scene, hit);
            if !text.is_empty() {
                egui::Area::new(Id::unique("hover-tip"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos + Vec2::new(12.0, 12.0))
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(text);
                        });
                    });
            }
        }
    }

    /// Decide what a fresh left-button press starts (M3-3): a box
    /// under the cursor begins a group MOVE (the box joins the
    /// selection first; Shift keeps the group, a plain press elsewhere
    /// selects just it), anything else begins a LASSO rubber band.
    fn press_mode(&mut self, origin: Pos2, shift: bool) -> Drag {
        let screen = (f64::from(origin.x), f64::from(origin.y));
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
            let base = if shift {
                match &self.selection {
                    Some(paint::Selection::Items(ids)) => ids.clone(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            return Drag::Lasso {
                anchor: origin,
                base,
            };
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
            Drag::Move {
                origins,
                press: self.viewport.screen_to_world(screen),
                delta: (0.0, 0.0),
            }
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
    fn preview_move(&mut self, origins: &[(ModelId, (f64, f64))], delta: (f64, f64)) {
        {
            let snap = self.snap;
            let overlay = self.overlay_for();
            for (id, origin) in origins {
                let to = commands::drag_target(*origin, delta, snap);
                overlay.elements.insert(id.clone(), to);
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
    /// Shift-click TOGGLES membership (M3-3 multi-select) and a click
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

    fn status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.display.is_empty() {
                ui.weak("No workspace — enter a path and press Open");
            } else {
                ui.strong(&self.display);
            }
            if let Some(scene) = &self.scene {
                ui.separator();
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
                        self.show_diagnostics = !self.show_diagnostics;
                    }
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

    /// Session tag-filter chips (M1-4b, P3-V7): the chip set + mode are
    /// SESSION state — the mask flows into `scene::build` via
    /// [`key::session_mask`] and nothing is ever written back.
    fn tag_filter(&mut self, ui: &mut egui::Ui) {
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
                .on_hover_text("Hide tagged items (absent mode in the reference)")
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
        if let Some(new_mode) = mode {
            if self.tag_mode != new_mode {
                self.tag_mode = new_mode;
                dirty = true;
            }
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

    /// Key/legend panel (M1-4b): the scene's distinct appearances,
    /// folded headlessly by [`key::legend`] (swatches + labels).
    fn key_panel(&self, ui: &mut egui::Ui) {
        ui.heading(egui::RichText::new("Key").size(14.0).weak());
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
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

    /// Full diagnostics list in a dismissible window (M1-4b); the
    /// status-bar count toggles it (hover preview was M1-4a).
    fn diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.show_diagnostics {
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
        self.show_diagnostics = open;
    }

    /// §P3-M2 M2-5: a view that CARRIES an `automaticLayout` request
    /// gets EXACTLY ONE engine pass at adopt/view-change; further runs
    /// are the dialog's job. The workspace file is never touched
    /// (session geometry only — M3 owns saving, P3-D0).
    fn autolayout_once(&mut self) {
        let stored = match (&self.workspace, &self.view_key) {
            (Some(workspace), Some(view_key)) => layout::stored_for(workspace, view_key),
            _ => None,
        };
        if let Some(params) = stored {
            self.run_layout(params);
        }
    }

    /// The editing overlay of the CURRENT view, created on first use
    /// (P3-M3: one overlay + history per view key, §P3-D0 — the
    /// workspace model itself is never touched).
    fn overlay_for(&mut self) -> &mut commands::LayoutOverlay {
        let view_key = self.view_key.clone().unwrap_or_default();
        self.overlays
            .entry(view_key.clone())
            .or_insert_with(|| commands::LayoutOverlay::for_view(&view_key))
    }

    /// Run one editing command on the current view: apply through the
    /// history, mark the view dirty, repaint. EVERY geometry mutation
    /// in the shell lands here (editor rule, P3-D0).
    fn edit_current_view(&mut self, command: commands::Command) {
        let view_key = self.view_key.clone().unwrap_or_default();
        let overlay = self
            .overlays
            .entry(view_key.clone())
            .or_insert_with(|| commands::LayoutOverlay::for_view(&view_key));
        let history = self.histories.entry(view_key.clone()).or_default();
        history.run(overlay, command);
        self.dirty.insert(view_key);
        self.rebuild_scene();
    }

    /// Undo/redo one step of the current view (Ctrl+Z / Ctrl+Y).
    fn step_history(&mut self, forward: bool) {
        let moved = match &self.view_key {
            Some(view_key) => {
                match (
                    self.overlays.get_mut(view_key),
                    self.histories.get_mut(view_key),
                ) {
                    (Some(overlay), Some(history)) => {
                        if forward {
                            history.redo(overlay)
                        } else {
                            history.undo(overlay)
                        }
                    }
                    _ => false,
                }
            }
            None => false,
        };
        if moved {
            if let Some(view_key) = &self.view_key {
                self.dirty.insert(view_key.clone());
            }
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
            .map(|scene| layout::apply(scene, &params));
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
                    self.needs_fit = true;
                }
            }
            Some(Err(error)) => self.layout_error = Some(error.to_string()),
            None => {}
        }
    }

    /// The auto-layout dialog — mirrors the reference `auto-layout.jspf`
    /// (direction, rank/node/edge separation, route vertices). `Apply`
    /// runs the engine on the CURRENT scene; `Reset` drops the session
    /// geometry. Only the PARAMETERS persist (§P3-D2); the engine
    /// ignores edge separation at MVP scope (parity field only).
    fn layout_window(&mut self, ctx: &egui::Context) {
        if !self.show_layout {
            return;
        }
        let mut open = true;
        egui::Window::new("Automatic layout")
            .open(&mut open)
            .resizable(false)
            .default_pos([420.0, 120.0])
            .show(ctx, |ui| {
                let mut params = self.layout_params;
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
                if let Some(error) = &self.layout_error {
                    ui.colored_label(egui::Color32::DARK_RED, error);
                }
                ui.weak("Session geometry — not saved to the workspace (M3).");
            });
        self.show_layout = open;
    }

    /// Rebuild the icon atlas when the scene changed (M1-5): rasterize
    /// headlessly with [`icons::build_atlas`], re-upload the single
    /// texture only when its pixels actually changed, and remap the
    /// per-icon uv rects (inset half a pixel against LINEAR bleed).
    /// Failures land in [`Self::icon_errors`] — the canvas keeps the
    /// dashed placeholder for exactly those icons.
    #[allow(clippy::cast_precision_loss)]
    // Atlas caps are 1024×2048 px — every u32 here is exactly f32.
    fn sync_icons(&mut self, ctx: &egui::Context) {
        if !self.icons_dirty {
            return;
        }
        self.icons_dirty = false;
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

// ---- free helpers ------------------------------------------------------------

/// List label for a view: title, else the key (the reference switcher).
fn view_label(view: View<'_>) -> String {
    view.base()
        .title
        .clone()
        .unwrap_or_else(|| view.key().to_owned())
}

/// Dialog label for a rank direction (`None` reads as the reference
/// default, Left-Right — P3-M2-5).
fn direction_label(direction: Option<RankDirection>) -> &'static str {
    match direction.unwrap_or_default() {
        RankDirection::TopBottom => "Top → Bottom",
        RankDirection::BottomTop => "Bottom → Top",
        RankDirection::LeftRight => "Left → Right",
        RankDirection::RightLeft => "Right → Left",
    }
}

/// The click-through URL behind a hit (`None` = plain selection).
fn url_at(scene: &scene::Scene, hit: hit::Hit) -> Option<&str> {
    match hit {
        hit::Hit::Item(index) => scene.items.get(index)?.url.as_deref(),
        hit::Hit::Edge(index) => scene.edges.get(index)?.url.as_deref(),
    }
}

/// Multi-line hover text for a hovered item/edge.
fn tooltip_text(scene: &scene::Scene, hit: hit::Hit) -> String {
    let mut lines: Vec<String> = Vec::new();
    match hit {
        hit::Hit::Item(index) => {
            if let Some(item) = scene.items.get(index) {
                lines.push(format!("{} ({:?})", item.name, item.kind));
                if let Some(metadata) = &item.metadata {
                    lines.push(metadata.clone());
                }
                if let Some(description) = &item.description {
                    lines.push(description.clone());
                }
                if let Some(group) = &item.group {
                    lines.push(format!("group: {group}"));
                }
                if item.url.is_some() {
                    lines.push("Ctrl/⌘-click: open URL".to_owned());
                }
            }
        }
        hit::Hit::Edge(index) => {
            if let Some(edge) = scene.edges.get(index) {
                lines.push(format!("{} → {}", edge.source, edge.destination));
                if let Some(label) = &edge.label {
                    lines.push(label.clone());
                }
                if let Some(metadata) = &edge.metadata {
                    lines.push(metadata.clone());
                }
                if edge.url.is_some() {
                    lines.push("Ctrl/⌘-click: open URL".to_owned());
                }
            }
        }
    }
    lines.join("\n")
}

/// Key-row icon chip (M1-5): the atlas cell when rasterized, a hollow
/// hint box for missing/broken icons (the canvas placeholder is the
/// canvas-side half of the same rule).
fn icon_cell(
    ui: &mut egui::Ui,
    icon: Option<&str>,
    uv_map: &BTreeMap<String, egui::Rect>,
    texture: Option<&egui::TextureHandle>,
) {
    let Some(icon) = icon else {
        return;
    };
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    match (uv_map.get(icon), texture) {
        (Some(uv), Some(texture)) => {
            ui.painter()
                .image(texture.id(), rect, *uv, egui::Color32::WHITE);
        }
        _ => {
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0, egui::Color32::GRAY),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// Shorten a data-URI-ish string for the diagnostics list.
fn snip(text: &str) -> String {
    const LIMIT: usize = 48;
    if text.chars().count() <= LIMIT {
        return text.to_owned();
    }
    let head: String = text.chars().take(LIMIT).collect();
    format!("{head}…")
}

/// Small fixed-size swatch cell painted by `draw` (key panel rows).
fn swatch_cell(ui: &mut egui::Ui, draw: impl FnOnce(egui::Rect, &egui::Painter)) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
    draw(rect, ui.painter());
}

/// Element legend swatch: filled box in the entry's resolved colors.
fn element_swatch(ui: &mut egui::Ui, entry: &key::ElementKey) {
    let fill = paint::parse_color(&entry.background).unwrap_or(paint::FALLBACK_FILL);
    let ink = paint::parse_color(&entry.stroke).unwrap_or(paint::FALLBACK_INK);
    swatch_cell(ui, |rect, painter| {
        let r = rect.shrink(1.5);
        painter.rect_filled(r, 3.0, fill);
        painter.rect_stroke(
            r,
            3.0,
            egui::Stroke::new(1.0, ink),
            egui::StrokeKind::Inside,
        );
    });
}

/// Edge legend swatch: a short line in the entry's color + line style.
fn edge_swatch(ui: &mut egui::Ui, entry: &key::EdgeKey) {
    let color = paint::parse_color(&entry.color).unwrap_or(paint::FALLBACK_INK);
    let stroke = egui::Stroke::new(2.0, color);
    swatch_cell(ui, |rect, painter| {
        let y = rect.center().y;
        let (l, r) = (rect.min.x + 1.0, rect.max.x - 1.0);
        let third = (r - l) / 3.0;
        match entry.line_style {
            LineStyle::Solid => {
                painter.line_segment([egui::pos2(l, y), egui::pos2(r, y)], stroke);
            }
            LineStyle::Dashed => {
                let mid = l + third * 1.5;
                painter.line_segment([egui::pos2(l, y), egui::pos2(mid, y)], stroke);
                painter.line_segment([egui::pos2(mid + 2.0, y), egui::pos2(r, y)], stroke);
            }
            LineStyle::Dotted => {
                let mut x = l;
                for _ in 0..5 {
                    painter.line_segment([egui::pos2(x, y), egui::pos2(x + 1.0, y)], stroke);
                    x += third * 0.6;
                }
            }
        }
    });
}

/// Panel row for a diagnostic entry: severity color + one-line text.
fn log_row(item: &LogItem) -> (egui::Color32, String) {
    match item {
        LogItem::Issue(issue) => {
            let color = match issue.severity {
                Severity::Error => egui::Color32::from_rgb(185, 60, 60),
                Severity::Warning => egui::Color32::from_rgb(160, 130, 0),
                Severity::Info => egui::Color32::GRAY,
            };
            (color, format!("{:?}: {}", issue.severity, issue.message))
        }
        LogItem::OkLine(line) | LogItem::NoteLine(line) => (egui::Color32::GRAY, line.clone()),
    }
}

/// One human line for a diagnostic entry (status-bar summaries).
fn log_line(item: &LogItem) -> String {
    log_row(item).1
}

/// First three log lines inline, the rest counted (status bar text).
fn summarize_log(log: &[LogItem], prefix: &str) -> String {
    use std::fmt::Write as _;
    if log.is_empty() {
        return prefix.to_owned();
    }
    let shown: Vec<String> = log.iter().take(3).map(log_line).collect();
    let mut text = format!("{prefix}: {}", shown.join(" · "));
    if log.len() > 3 {
        let _ = write!(text, " (+{} more)", log.len() - 3);
    }
    text
}

/// Character-trim long messages for the status bar (hover shows all).
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut trimmed: String = text.chars().take(max).collect();
    trimmed.push('…');
    trimmed
}

/// `--theme system` order (§P3-D7): persisted toggle, OS preference,
/// light fallback. `dark-light` keeps its platform-specific code inside
/// the dependency — this call site is platform-neutral (portability rule).
fn detect_os_scheme() -> ColorScheme {
    if matches!(dark_light::detect().ok(), Some(dark_light::Mode::Dark)) {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

/// Stored theme string → scheme (anything but `dark` = light).
fn parse_scheme(stored: &str) -> ColorScheme {
    if stored == "dark" {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}
