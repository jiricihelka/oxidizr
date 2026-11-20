//! The thin presentation shell (P3-D1): the ONLY code in this crate
//! allowed to reference the GUI stack, compiled only with `--features gui`.
//! All behavior lives in the library core (`scene`/`hit`/`paint`/
//! `commands`/`reload`), which is tested headlessly — this file opens a
//! workspace through `oxidizr_api::open`, wires `scene::build` +
//! `hit::pick` + `paint::paint_scene` to pan/zoom/click events, and owns
//! no logic of its own beyond presentation.
//!
//! M1-4a scope (the a/b split of the §P3-M1 M1-4 row): open (argv +
//! toolbar), view list, pan/zoom/select, tooltips, element URLs
//! (ctrl/⌘-click), light/dark (+ OS preference at startup, §P3-D7) and
//! Fit. The key/legend panel, tag-filter chips and the diagnostics
//! panel land with M1-4b.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use eframe::egui::{self, Id, Pos2, Vec2};
use oxidizr_api::{ApiOptions, Input, LogItem, Opened, Outcome};
use oxidizr_core::Workspace;
use oxidizr_core::views::{ColorScheme, View};
use oxidizr_viewer::{hit, paint, scene};

/// Storage keys (§P3-D2: eframe storage only, no config files).
const KEY_THEME: &str = "oxidizr.viewer/theme";
const KEY_ZOOM: &str = "oxidizr.viewer/zoom";
const KEY_VIEW: &str = "oxidizr.viewer/view";

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
}

impl Viewer {
    fn new() -> Self {
        Self {
            workspace: None,
            display: String::new(),
            open_log: Vec::new(),
            open_error: None,
            path_edit: String::new(),
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
        self.workspace = Some(opened.workspace);
        self.needs_fit = self.restore_scale.is_none();
        self.rebuild_scene();
    }

    /// Re-run `scene::build` for the current workspace/view/scheme.
    fn rebuild_scene(&mut self) {
        let built = match (&self.workspace, &self.view_key) {
            (Some(workspace), Some(key)) => Some(scene::build(workspace, key, self.scheme, None)),
            _ => None,
        };
        match built {
            Some(Ok(scene)) => {
                self.scene = Some(scene);
                self.scene_error = None;
            }
            Some(Err(error)) => {
                self.scene = None;
                self.scene_error = Some(error.to_string());
            }
            None => {
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
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status(ui));
        if self.workspace.is_some() {
            egui::Panel::left("views")
                .default_size(240.0)
                .show(ui, |ui| self.view_list(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
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
                    "Drag: pan · Wheel: zoom · Click: select · Ctrl/⌘-click: open \
                     element URL · Esc: deselect · keys work on the canvas",
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
                    let label = view_label(view);
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
            self.rebuild_scene();
        }
    }

    #[allow(clippy::too_many_lines)] // the canvas pass: input → fit → paint → tip
    fn canvas(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        self.last_rect = Some(rect);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        if response.dragged() {
            let delta = response.drag_delta();
            self.viewport
                .pan_by((f64::from(delta.x), f64::from(delta.y)));
        }
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON && response.hovered() {
            if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                let factor = (scroll * 0.005_f32).exp();
                self.viewport.zoom_at(
                    f64::from(factor),
                    (f64::from(pointer.x), f64::from(pointer.y)),
                );
            }
        }
        if response.clicked() {
            if let Some(point) = response.interact_pointer_pos() {
                let command = ui.input(|i| i.modifiers.command);
                self.canvas_click(point, command, &ctx);
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

    /// Resolve a canvas click: ctrl/⌘ opens the element URL, a plain
    /// click selects (item over edge already decided by [`hit::pick`]).
    fn canvas_click(&mut self, point: Pos2, command: bool, ctx: &egui::Context) {
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
        self.selection = hit.and_then(|hit| match hit {
            hit::Hit::Item(index) => self
                .scene
                .as_ref()
                .and_then(|scene| scene.items.get(index))
                .map(|item| paint::Selection::Item(item.id.clone())),
            hit::Hit::Edge(index) => self
                .scene
                .as_ref()
                .and_then(|scene| scene.edges.get(index))
                .map(|edge| paint::Selection::Edge(edge.id.clone())),
        });
    }

    fn status(&self, ui: &mut egui::Ui) {
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
                    paint::Selection::Item(id) => format!("selected: {id}"),
                    paint::Selection::Edge(id) => format!("selected relationship: {id}"),
                });
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.open_log.is_empty() {
                    let count = self.open_log.len();
                    ui.colored_label(
                        egui::Color32::from_rgb(160, 130, 0),
                        format!("⚠ {count} diagnostics"),
                    )
                    .on_hover_ui(|ui| {
                        ui.set_max_width(520.0);
                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                for item in &self.open_log {
                                    ui.label(log_line(item));
                                }
                            });
                    });
                }
                if let Some(error) = self.open_error.as_ref().or(self.scene_error.as_ref()) {
                    ui.colored_label(egui::Color32::DARK_RED, truncate(error, 90))
                        .on_hover_text(error.clone());
                }
            });
        });
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

/// One human line for a diagnostic entry (full panel comes with M1-4b).
fn log_line(item: &LogItem) -> String {
    match item {
        LogItem::Issue(issue) => format!("{:?}: {}", issue.severity, issue.message),
        LogItem::OkLine(line) | LogItem::NoteLine(line) => line.clone(),
    }
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
