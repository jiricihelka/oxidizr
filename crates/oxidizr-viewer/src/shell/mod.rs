//! The eframe presentation shell: the ONLY module of
//! this crate allowed to the official UI the GUI stack, compiled only with
//! `--features gui`; the `oxidizr-viewer` binary (`src/main.rs`) is a
//! thin argv shim over [`run`], which `oxidizr-cli`'s `view`
//! subcommand (cli feature `gui`) calls in-process — one entry, no
//! drift. All behavior lives in the library core
//! (`scene`/`hit`/`paint`/`key`/`icons`/`commands`/`reload`), which is
//! tested headlessly — this module opens a workspace through
//! `oxidizr_api::open`, wires `scene::build` + `hit::pick` +
//! `paint::paint_scene` to pan/zoom/click events, and owns no logic of
//! its own beyond presentation.
//!
//! Canvas scope: canvas — open (argv +
//! toolbar), view list, pan/zoom/select, tooltips, element URLs
//! (ctrl/⌘-click), light/dark (+ OS preference at startup),
//! Fit — plus the overlays: key/legend panel (right), session
//! tag-filter chips (left) and the diagnostics window. it the
//! icon atlas: `icons` rasterizes style icons headlessly, the shell
//! uploads ONE texture and overlays its cells on the paint placeholders.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
mod canvas;

use crate::{commands, export, hit, icons, key, layout, overview, paint, reload, scene, statefile};
use clap::ValueEnum;
use eframe::egui::{self, Pos2};
use oxidizr_api::{ApiOptions, Input, LogItem, Opened, Outcome};
use oxidizr_core::views::{
    AutomaticLayout, ColorScheme, FilterMode, LineStyle, RankDirection, View,
};
use oxidizr_core::{Id as ModelId, Workspace};
use oxidizr_diag::Severity;

/// Storage keys .
const KEY_THEME: &str = "oxidizr.viewer/theme";
const KEY_ZOOM: &str = "oxidizr.viewer/zoom";
const KEY_VIEW: &str = "oxidizr.viewer/view";
const KEY_LAYOUT: &str = "oxidizr.viewer/layout";

/// `--theme` values — shared by the `oxidizr-viewer` argv and
/// the `oxidizr view` subcommand .
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ThemeArg {
    /// Force the light scheme.
    Light,
    /// Force the dark scheme.
    Dark,
    /// Persisted toggle, else the OS preference, else light.
    System,
}

/// Open `file` (if any) and run the viewer window until it closes — the
/// ONE entry shared by the `oxidizr-viewer` binary and the
/// `oxidizr view` subcommand .
///
/// # Errors
///
/// [`eframe::Error`] when the window cannot be created (no display,
/// renderer failure). WORKSPACE open failures are NOT errors here: they
/// become in-app state (the status bar) — errors are values (P3
/// charter). A `.dsl` with an existing sidecar opens through the
/// SIDECAR: the saved model + layout is the preferred source.
pub fn run(file: Option<&Path>, theme: ThemeArg) -> eframe::Result {
    let target = file.map(|path| open_target(&path.to_string_lossy()));
    let opened = target
        .clone()
        .map(|path| oxidizr_api::open(&Input::File(path), &ApiOptions::default()));
    let path_shown = target.map_or_else(String::new, |path| path.display().to_string());
    let mut viewer = Viewer::new();
    eframe::run_native(
        "oxidizr-viewer",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1360.0, 860.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            viewer.startup(cc.storage, theme, path_shown, opened);
            Ok(Box::new(viewer))
        }),
    )
}

/// The user-facing window/canvas toggles (grouping — six loose
/// bools on [`Viewer`] tripped `struct_excessive_bools`; they remain
/// session flags, not state to model as enums — same reading as the
/// CLI flags precedent (oxidizr-cli lib.rs)).
struct Toggles {
    /// Diagnostics window visibility (session).
    diagnostics: bool,
    /// Auto-layout dialog visibility.
    layout: bool,
    /// Grid-snap drags to multiples of [`commands::snapped`]'s step.
    snap: bool,
}

/// Frame-level latches: signals set at one event and consumed by the
/// next paint (grouping).
struct Latches {
    /// Re-fit the viewport into the canvas on the next paint.
    needs_fit: bool,
    /// Visuals pushed to egui once (the creator closure has no Context).
    visuals_ready: bool,
    /// Icons are stale wrt the scene (set on open/rebuild).
    icons_dirty: bool,
}

/// The viewer app state: a workspace, one selected view, its rendered
/// scene, the canvas viewport and the theme toggle. No GUI types outside
/// presentation fields (the lib core owns the rest).
struct Viewer {
    /// Loaded workspace (after a successful open).
    workspace: Option<Workspace>,
    /// Input display name (`-`/path, from [`Opened`]).
    display: String,
    /// Diagnostics of the current workspace or failed open (panel window).
    open_log: Vec<LogItem>,
    /// Last open failure (status bar, red).
    open_error: Option<String>,
    /// Toolbar path editor.
    path_edit: String,
    /// A text field (path bar or quick-nav) owns the keyboard
    /// (undo/redo/save keys skip then).
    text_focus: bool,
    /// Selected view key.
    view_key: Option<String>,
    /// Quick-nav search text (the left panel): matches
    /// element names/ids and view titles; session-only, not persisted.
    nav_query: String,
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
    /// User-facing toggles (diagnostics / layout dialog / grid snap).
    toggles: Toggles,
    /// Frame latches (fit request / first-visuals / icon cache invalid).
    latches: Latches,
    /// World↔screen transform.
    viewport: hit::Viewport,
    /// Zoom restored from storage, applied on the first painted frame
    /// (the canvas size is unknown before that).
    restore_scale: Option<f64>,
    /// per-file sidecar state — view key → last
    /// camera (zoom + world centre). Seeded from the
    /// `<stem>.oxidizr.json` cache on adopt, flushed back to it on every
    /// file swap and on close. Pure cache: NEVER an open/reload source.
    sidecar: statefile::ViewerState,
    /// Sidecar camera awaiting the first painted frame after an adopt
    /// (consumed by `canvas`; takes precedence over `restore_scale`).
    pending_viewport: Option<statefile::ViewMemory>,
    /// View key restored from storage (honored by the next adopt).
    pending_view: Option<String>,
    /// Canvas rect from the previous frame (zoom-button pivot).
    last_rect: Option<egui::Rect>,
    /// Last window title pushed to the OS (change detection).
    shown_title: String,
    /// Stored-tag inventory for the filter chips (recomputed on open).
    tag_inventory: Vec<key::TagCount>,
    /// Chip semantics: hide (Exclude) or show only (Include) tagged.
    tag_mode: FilterMode,
    /// Selected filter tags — SESSION state, never persisted.
    tag_sel: Vec<String>,
    /// Legend of the current scene (folded by [`key::legend`]).
    legend: key::Key,
    /// Workspace file's directory — resolves file-path icon strings.
    base_dir: Option<PathBuf>,
    /// The single icon-atlas texture (uploaded when pixels change).
    icon_texture: Option<egui::TextureHandle>,
    /// Atlas uv rect per icon string (rebuilt with every atlas).
    icon_uv: BTreeMap<String, egui::Rect>,
    /// Atlas pixels last uploaded (skips redundant GPU uploads).
    icon_pixels: Vec<u8>,
    /// `snip(icon)` → reason, one row per failing icon (warnings).
    icon_errors: Vec<(String, String)>,
    /// Session editing overlays, one per view key; NEVER
    /// persisted — owns saving (editor rule). Kept across
    /// view switches, cleared when a workspace is adopted.
    overlays: HashMap<String, commands::LayoutOverlay>,
    /// Undo/redo history per view key, parallel to [`Self::overlays`].
    histories: HashMap<String, commands::History>,
    /// The in-flight canvas interaction (pan / drag / lasso).
    drag: Drag,
    /// Views edited since the last adopt (dirty marker; save).
    dirty: BTreeSet<String>,
    /// Last Save outcome line: green in the status bar, cleared
    /// by the next save attempt and by any adopt.
    save_note: Option<String>,
    /// File set behind the current open (`Opened.includes`) — the
    /// reload watch closure next to the display file .
    includes: Vec<String>,
    /// Live-reload driver for the current sources; `None`
    /// = not watching: stdin, or awaiting re-arm after an adopt.
    reload: Option<reload::ReloadDriver>,
    /// Watch setup failed once for this open — stop retrying per frame.
    reload_disabled: bool,
    /// Last reload outcome line (status bar): the reconcile report
    /// summary, or the last-good notice after a failed reparse.
    reload_note: Option<String>,
    /// Paint-context clone handed to the notify thread as its wake
    /// handle (egui 0.37: `request_repaint` IS the cross-thread
    /// signal — there is no `RepaintSignal`).
    repaint_ctx: Option<egui::Context>,
    /// Last auto-layout failure (dialog + status bar).
    layout_error: Option<String>,
    /// Dialog parameters — a persisted preference, mirroring
    /// the official UI the official layout dialog fields.
    layout_params: AutomaticLayout,
    /// Dialog engine choice — SESSION-only: the selection
    /// deliberately stays out of the persisted parameter string.
    layout_engine: layout::Engine,
}

/// The in-flight canvas interaction . The mode is decided at
/// PRESS time and can no longer change: a left-drag moves the boxes
/// under it or pans (empty canvas), a right-drag lassos, Middle and
/// Space+Left pan too — swapped the plain-drag roles (pan ↔
/// lasso) at the user's request.
enum Drag {
    /// No press, or a press still undecided (a click is still
    /// possible): nothing mutates the scene.
    Idle,
    /// Pan: the plain-drag behavior, unchanged.
    Pan,
    /// Move the selected boxes: `origins` are their effective top-lefts
    /// captured at press; every frame writes `drag_target(origin,
    /// delta, snap)` into the overlay, so preview frames equal the
    /// geometry the release commits.: `carries` are the press-time
    /// routes of fully-contained edges (`commands::carried_interiors`) —
    /// by default they translate with the boxes, `carry: false`
    /// (Ctrl+drag) leaves them behind.
    Move {
        /// Press-time selection: id + effective top-left (content order).
        origins: Vec<(ModelId, (f64, f64))>,
        /// Press-time interior routes that move along .
        carries: Vec<(ModelId, Vec<(f64, f64)>)>,
        /// Whether the drag carries those routes (false = Ctrl held).
        carry: bool,
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
    /// Drag one INTERIOR waypoint of the selected edge: the
    /// press captured the edge's route `from` and the grabbed vertex
    /// `index` — the same preview/commit contract as [`Self::Move`].
    Waypoint {
        /// The edge whose route is dragged.
        id: ModelId,
        /// Press-time INTERIOR waypoints (world px).
        from: Vec<(f64, f64)>,
        /// Grabbed vertex within `from`.
        index: usize,
        /// Press point (world px) — deltas measure from here.
        press: (f64, f64),
        /// Latest preview delta (world px); release commits exactly it.
        delta: (f64, f64),
    },
    /// Press began INSIDE the minimap box: every frame the
    /// pointer position re-centers the viewport (click = one recenter,
    /// drag = continuous). No scene edit, no history entry.
    Minimap,
}

/// What a press on a waypoint handle finds: the edge id, its
/// press-time INTERIOR route, and the route-relative vertex index.
type WaypointHit = (ModelId, Vec<(f64, f64)>, usize);

impl Viewer {
    fn new() -> Self {
        Self {
            workspace: None,
            display: String::new(),
            open_log: Vec::new(),
            open_error: None,
            path_edit: String::new(),
            text_focus: false,
            view_key: None,
            nav_query: String::new(),
            scene: None,
            scene_error: None,
            scheme: ColorScheme::Light,
            selection: None,
            hover: None,
            viewport: hit::Viewport::new(1.0, (0.0, 0.0)),
            toggles: Toggles {
                diagnostics: false,
                layout: false,
                snap: false,
            },
            latches: Latches {
                needs_fit: false,
                visuals_ready: false,
                icons_dirty: false,
            },
            restore_scale: None,
            sidecar: statefile::ViewerState::default(),
            pending_viewport: None,
            pending_view: None,
            last_rect: None,
            shown_title: String::new(),
            tag_inventory: Vec::new(),
            tag_mode: FilterMode::Exclude,
            tag_sel: Vec::new(),
            legend: key::Key::default(),
            base_dir: None,
            icon_texture: None,
            icon_uv: BTreeMap::new(),
            icon_pixels: Vec::new(),
            icon_errors: Vec::new(),
            overlays: HashMap::new(),
            histories: HashMap::new(),
            drag: Drag::Idle,
            dirty: BTreeSet::new(),
            save_note: None,
            includes: Vec::new(),
            reload: None,
            reload_disabled: false,
            reload_note: None,
            repaint_ctx: None,
            layout_error: None,
            layout_params: AutomaticLayout::default(),
            layout_engine: layout::Engine::default(),
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
        // sidecar swap: BOTH an open and a live reload
        // funnel through here, so this is the single choke-point — the
        // OUTGOING file's camera map is recorded and flushed to its
        // `<stem>.oxidizr.json` before the swap, the INCOMING file's
        // map is then seeded from its own cache (a missing or unusable
        // file simply starts empty).
        self.record_view();
        self.flush_sidecar();
        self.display = opened.display.clone();
        self.path_edit = opened.display;
        self.sidecar = statefile::sidecar_path(&self.display)
            .map(|path| statefile::load(&path))
            .unwrap_or_default();
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
        // (scope cut — overlays survive VIEW switches, not OPENs).
        self.overlays.clear();
        self.histories.clear();
        self.dirty.clear();
        self.save_note = None;
        // New workspace, new source set: the driver dies with the old
        // one and the frame guard re-arms the watch from `display` +
        // `includes` on the next frame .
        self.reload = None;
        self.reload_disabled = false;
        self.includes = opened.includes.clone();
        self.drag = Drag::Idle;
        self.layout_error = None;
        self.workspace = Some(opened.workspace);
        // a remembered camera beats both the startup
        // zoom restore and the re-fit; a view nobody panned fits fresh.
        self.pending_viewport = self
            .view_key
            .as_ref()
            .and_then(|key| self.sidecar.views.get(key))
            .copied();
        self.latches.needs_fit = self.pending_viewport.is_none() && self.restore_scale.is_none();
        self.rebuild_scene();
    }

    /// Record the current viewport into the in-memory sidecar map
    ///: the zoom plus the WORLD point at the canvas
    /// centre, derived from the last painted canvas rect. Before the
    /// first frame there is no rect and nothing worth remembering.
    fn record_view(&mut self) {
        let (Some(key), Some(rect)) = (&self.view_key, self.last_rect) else {
            return;
        };
        let centre = self.viewport.screen_to_world((
            f64::from(rect.width()) / 2.0,
            f64::from(rect.height()) / 2.0,
        ));
        if !(centre.0.is_finite() && centre.1.is_finite()) {
            return;
        }
        self.sidecar.views.insert(
            key.clone(),
            statefile::ViewMemory {
                zoom: self.viewport.scale(),
                centre,
            },
        );
    }

    /// Write the current file's sidecar map to disk . Silent
    /// best-effort: this is a pure cache, a failed write may never
    /// disturb the session.
    fn flush_sidecar(&self) {
        if let Some(path) = statefile::sidecar_path(&self.display) {
            let _ = statefile::store(&path, &self.sidecar);
        }
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
                let built = scene::build(
                    workspace,
                    view_key,
                    self.scheme,
                    mask.as_ref(),
                    overrides.as_ref(),
                );
                // display auto-layout: a view with NO stored
                // coordinates renders with the bundled engine's geometry
                // UNDER the user overlay (structurizr-web's on-display
                // pass) — session display only, never dirty; the dialog
                // and the seeded save persist it explicitly.
                let displayed = built
                    .as_ref()
                    .ok()
                    .and_then(|built| layout::auto_display(workspace, view_key, built))
                    .map(|auto| {
                        scene::build(
                            workspace,
                            view_key,
                            self.scheme,
                            mask.as_ref(),
                            Some(&scene::Overrides::display(&auto, overrides.as_ref())),
                        )
                    });
                displayed.or(Some(built))
            }
            _ => None,
        };
        match built {
            Some(Ok(scene)) => {
                self.legend = key::legend(&scene);
                self.latches.icons_dirty = true;
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

    /// Open `path_edit` through the api entry; failures keep the
    /// previous workspace on screen and surface in the status bar.
    /// Opening a `.dsl` whose sidecar already exists opens the SIDECAR
    /// instead: it carries the last saved model AND layout.
    fn open_path(&mut self) {
        let raw = self.path_edit.trim().to_owned();
        if raw.is_empty() {
            self.open_error = Some("enter a workspace path first".to_owned());
            return;
        }
        match oxidizr_api::open(&Input::File(open_target(&raw)), &ApiOptions::default()) {
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

    /// The minimap box for a canvas rect: bottom-right, fixed size
    ///, `None` when the canvas is too small to earn one.
    fn minimap_rect(rect: egui::Rect) -> Option<egui::Rect> {
        let (w, h, m) = (
            overview::MAP_WIDTH_PX,
            overview::MAP_HEIGHT_PX,
            overview::MAP_MARGIN_PX,
        );
        (rect.width() > 2.0 * (w + m) && rect.height() > 2.0 * (h + m)).then(|| {
            egui::Rect::from_min_max(
                egui::pos2(rect.max.x - m - w, rect.max.y - m - h),
                egui::pos2(rect.max.x - m, rect.max.y - m),
            )
        })
    }

    /// Whether a screen point sits inside this frame's minimap (input
    /// triage: presses there steer the map, never the canvas).
    fn minimap_contains(&self, point: egui::Pos2) -> bool {
        self.scene.is_some()
            && self
                .last_rect
                .and_then(Self::minimap_rect)
                .is_some_and(|mini| mini.contains(point))
    }

    /// The world point a minimap click/drag selects — `Some` only for
    /// points INSIDE the map box (the projection otherwise extrapolates
    /// any canvas point), `None` when the scene has no bounds.
    fn minimap_world(&self, point: egui::Pos2) -> Option<(f64, f64)> {
        let rect = self.last_rect?;
        let mini = Self::minimap_rect(rect)?;
        if !mini.contains(point) {
            return None;
        }
        let scene = self.scene.as_ref()?;
        let map = overview::Overview::new(scene.bounds, mini)?;
        Some(map.map_to_world((f64::from(point.x), f64::from(point.y))))
    }

    /// Put `world` at the middle of the last canvas, scale unchanged —
    /// the minimap recenter and the quick-nav jump share this.
    fn center_world(&mut self, world: (f64, f64)) {
        let Some(rect) = self.last_rect else {
            return;
        };
        let scale = self.viewport.scale();
        self.viewport = hit::Viewport::new(
            scale,
            (
                f64::from(rect.center().x) - world.0 * scale,
                f64::from(rect.center().y) - world.1 * scale,
            ),
        );
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
        if !self.latches.visuals_ready {
            self.apply_visuals(&ctx);
            self.latches.visuals_ready = true;
        }
        let title = format!(
            "oxidizr-viewer — {}",
            if self.display.is_empty() {
                "no workspace"
            } else {
                // (user: the full path is in 3 places,
                // the edit bar is enough): the title keeps just the
                // FILE NAME; the toolbar edit bar carries the path.
                Path::new(&self.display)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&self.display)
            }
        );
        if self.shown_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.shown_title = title;
        }
        if self.repaint_ctx.is_none() {
            self.repaint_ctx = Some(ctx.clone());
        }
        // Live reload: the notify thread only SCHEDULES a
        // check — the debounced edge is consumed HERE, on the UI
        // thread, so a reload can never interleave with a drag. The
        // reload's adopt empties `reload`, and the guard below re-arms
        // the watch on the fresh source set next frame.
        if self.reload.as_ref().is_some_and(reload::ReloadDriver::poll) {
            self.reload_from_disk();
        }
        if self.reload.is_none()
            && !self.reload_disabled
            && self.workspace.is_some()
            && self.display != "-"
        {
            self.restart_watch();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection = None;
        }
        // Undo/redo: Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y walk the
        // CURRENT view's history — skipped while the toolbar path field
        // owns the keyboard (Ctrl+Z inside the path edit is egui's).
        // Ctrl+S saves under the same focus guard.
        if self.scene.is_some() && !self.text_focus {
            let (undo, redo, save) = ctx.input(|i| {
                let command = i.modifiers.command;
                let z = i.key_pressed(egui::Key::Z);
                (
                    command && z && !i.modifiers.shift,
                    (command && z && i.modifiers.shift) || (command && i.key_pressed(egui::Key::Y)),
                    command && i.key_pressed(egui::Key::S),
                )
            });
            if save {
                self.save_workspace();
            }
            if undo {
                self.step_history(false);
            } else if redo {
                self.step_history(true);
            }
            // Delete/Backspace: strip the selected
            // edge's handles — the discoverable counterpart of the
            // Alt+click-per-handle and double-click-to-insert bindings.
            let del = ctx
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
            if del {
                self.clear_selected_waypoints();
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
        //: only the toggleable state persists (window geometry is
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
        // closing the window flushes the per-file
        // viewport memory to the sidecar beside the workspace.
        self.record_view();
        self.flush_sidecar();
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

    fn view_list(&mut self, ui: &mut egui::Ui) {
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

    fn status(&mut self, ui: &mut egui::Ui) {
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
    fn key_panel(&self, ui: &mut egui::Ui) {
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
    fn info_block(&self, ui: &mut egui::Ui) {
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
    fn diagnostics_window(&mut self, ctx: &egui::Context) {
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
    fn overlay_for(&mut self) -> &mut commands::LayoutOverlay {
        let view_key = self.view_key.clone().unwrap_or_default();
        self.overlays
            .entry(view_key.clone())
            .or_insert_with(|| commands::LayoutOverlay::for_view(&view_key))
    }

    /// Run one editing command on the current view: apply through the
    /// history, mark the view dirty, repaint. EVERY geometry mutation
    /// in the shell lands here (editor rule).
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
    fn save_workspace(&mut self) {
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
    fn export_files(&mut self) {
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
    fn restart_watch(&mut self) {
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
    fn reload_from_disk(&mut self) {
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
    fn layout_window(&mut self, ctx: &egui::Context) {
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
    fn sync_icons(&mut self, ctx: &egui::Context) {
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

// ---- free helpers ------------------------------------------------------------

/// Sidecar-preferred open target: a `.dsl` whose
/// `<stem>.json` sidecar exists (the save writes one) opens
/// through the SIDECAR — it carries the last saved model AND layout.
/// An mtime tie opens the sidecar (listed FIRST in the candidate
/// list); a strictly newer source file wins, so hand edits made
/// after a save still open. stdin (`-`) and plain `.json` paths
/// open exactly as given.
fn open_target(raw: &str) -> PathBuf {
    let given = PathBuf::from(raw);
    match commands::save_target(raw) {
        Some(sidecar) if sidecar != given && sidecar.exists() => {
            newest_by_mtime(&[sidecar.clone(), given]).unwrap_or(sidecar)
        }
        _ => given,
    }
}

/// The `.dsl` sibling of an opened sidecar JSON: when the
/// viewer's own sidecar is the display, its source DSL stays watched
/// so later text edits still reach the window. Exact stem, the
/// lowercase extension `save_target` writes; no sibling ⇒ `None`.
fn companion_dsl(display: &str) -> Option<PathBuf> {
    let path = Path::new(display);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        return None;
    }
    let dsl = path.with_extension("dsl");
    dsl.exists().then_some(dsl)
}

/// File mtime, `None` when un-stat-able.
fn mtime(path: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

/// First entry with the NEWEST modification time; un-stat-able
/// entries lose to stat-able ones (all unusable ⇒ `None`). Ties keep
/// the earlier-listed candidate — how the sidecar "wins by
/// preference" while the companion only wins when genuinely edited.
fn newest_by_mtime(paths: &[PathBuf]) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for path in paths {
        let Some(at) = mtime(path) else {
            continue;
        };
        if best.as_ref().is_none_or(|(_, best_at)| at > *best_at) {
            best = Some((path.clone(), at));
        }
    }
    best.map(|(path, _)| path)
}

/// List label for a view: title, else the key (the official UI switcher).
fn view_label(view: View<'_>) -> String {
    view.base()
        .title
        .clone()
        .unwrap_or_else(|| view.key().to_owned())
}

/// One sidebar row: title + dirty dot, selection state, the
/// key/description hover detail; `true` when clicked. Split out of
/// `view_list` (the kind sections) for the line budget.
fn view_row(ui: &mut egui::Ui, view: View<'_>, selected: bool, dirty: bool) -> bool {
    let mut label = view_label(view);
    if dirty {
        label.push_str(" ●");
    }
    let response = ui.selectable_label(selected, egui::RichText::new(label).small());
    let clicked = response.clicked();
    response.on_hover_text(format!(
        "key: {}\n{}",
        view.key(),
        view.base()
            .description
            .as_deref()
            .unwrap_or("(no description)")
    ));
    clicked
}

/// Quick-nav jump target: element id + world centre .
type NavJump = (ModelId, (f64, f64));

/// Quick-nav element rows (user: "Icons for results in
/// search do not work"): each hit shows its REAL icon through the same
/// 16 px atlas chip the key panel uses (iconless items keep the `▪`
/// glyph; a broken icon gets the chip's hollow placeholder). Returns
/// the row count (against the caller's shared cap) and the clicked
/// jump target (id + centre). Split out of `view_list` for the line
/// budget.
fn nav_element_rows(
    ui: &mut egui::Ui,
    scene: &scene::Scene,
    query: &str,
    cap: usize,
    icon_uv: &BTreeMap<String, egui::Rect>,
    icon_texture: Option<&egui::TextureHandle>,
) -> (usize, Option<NavJump>) {
    let mut hits = 0_usize;
    let mut jump = None;
    for item in scene
        .items
        .iter()
        .filter(|item| overview::nav_matches(query, &item.name, item.id.as_str()))
    {
        if hits == cap {
            break;
        }
        hits += 1;
        let label = format!("{} ({})", item.name, item.id);
        let clicked = ui.horizontal(|ui| {
            if item.icon.is_some() {
                icon_cell(ui, item.icon.as_deref(), icon_uv, icon_texture);
            } else {
                ui.label("▪");
            }
            ui.selectable_label(false, egui::RichText::new(label).small())
                .clicked()
        });
        if clicked.inner {
            jump = Some((item.id.clone(), item.rect.center()));
        }
    }
    (hits, jump)
}

/// Dialog label for a rank direction (`None` reads as the official UI
/// default, Left-Right —).
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

/// Key-row icon chip: the atlas cell when rasterized, a hollow
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

/// Press snapshot starting a LASSO (the right button's job):
/// Shift keeps the current Items selection
/// as the additive base, anything else lassos from empty.
fn lasso_drag(selection: Option<&paint::Selection>, anchor: Pos2, shift: bool) -> Drag {
    let base = if shift {
        match selection {
            Some(paint::Selection::Items(ids)) => ids.clone(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    Drag::Lasso { anchor, base }
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

/// `--theme system` order: persisted toggle, OS preference,
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
