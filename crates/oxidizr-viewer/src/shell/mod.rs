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
mod actions;
mod browser;
mod canvas;
mod panels;
mod widgets;

use widgets::{detect_os_scheme, parse_scheme, summarize_log};

use crate::{commands, hit, key, layout, overview, paint, reload, scene, statefile};
use clap::ValueEnum;
use eframe::egui::{self, Pos2};
use oxidizr_api::{ApiOptions, Input, LogItem, Opened, Outcome};
use oxidizr_core::views::{AutomaticLayout, ColorScheme, FilterMode};
use oxidizr_core::{Id as ModelId, Workspace};

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
//: four INDEPENDENT window/behavior toggles — no state machine fits,
// and folding them would hide which window is which (same posture the
// oxidizr-cli allow records).
#[allow(clippy::struct_excessive_bools)]
struct Toggles {
    /// Diagnostics window visibility (session).
    diagnostics: bool,
    /// Help overlay visibility (session) — the rendered [`crate::shortcuts`]
    /// table (F1 / `?` / the ? chip).
    help: bool,
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
    /// Shared markdown cache of the details panel's `egui_commonmark`
    /// renderer: ONE cache serves every rendered text (the crate keys
    /// its internals per widget id itself).
    md: egui_commonmark::CommonMarkCache,
    /// Draft text of the details-panel field that currently holds
    /// keyboard focus `(slot, text)` — the edit is committed as ONE
    /// model command on Enter or focus-loss, so mid-typing never
    /// touches the workspace. Only one field can focus at a time.
    field_draft: Option<(FieldKey, String)>,
    /// Field slots the details panel rendered this frame — a draft whose
    /// field VANISHED (selection moved on) commits like a focus-loss.
    field_seen: Vec<FieldKey>,
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
    /// Undo/redo history for the WHOLE session: geometry steps carry
    /// their view, so one Ctrl+Z stack walks through views (a global
    /// model edit needs exactly this — one stack, no per-view swap).
    history: commands::History,
    /// The in-flight canvas interaction (pan / drag / lasso).
    drag: Drag,
    /// The last-touched waypoint handle: `(edge, INTERIOR index)` —
    /// the arrow keys nudge it while that edge stays selected (a
    /// waypoint-drag or insert-dot commit grabs, Esc / adopt drop).
    grabbed: Option<(ModelId, usize)>,
    /// Views edited since the last adopt (dirty marker; save).
    dirty: BTreeSet<String>,
    /// Unsaved MODEL edits (create / delete / reconnect / relabel) —
    /// separate from the geometry dirty set; one write clears both.
    model_dirty: bool,
    /// Last model-op outcome line (status bar, red): blocked source,
    /// refused duplicate. Cleared by the next attempt and by any adopt.
    model_note: Option<String>,
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

/// Which editable text slot a details-panel field binds: a relationship's
/// model `description`, its `technology`, or the `description` override
/// of the current view's `RelationshipView` (keyed by the view it belongs
/// to, so switching views can never commit a draft into the wrong one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldKey {
    /// The model row's `description`.
    Description(ModelId),
    /// The model row's `technology`.
    Technology(ModelId),
    /// `view_key`'s `RelationshipView.description` override.
    ViewLabel(String, ModelId),
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
    /// Press began on a segment-midpoint INSERT dot of the selected
    /// edge: the route `from` is captured at press, the dot of segment
    /// `segment` sits at `at` (world px) and drags to the new vertex —
    /// the commit is ONE `SetWaypoints` splicing it in (undo removes
    /// exactly the insert).
    Insert {
        /// The edge whose route gains the vertex.
        id: ModelId,
        /// Press-time INTERIOR waypoints (world px).
        from: Vec<(f64, f64)>,
        /// SEGMENT whose dot was pressed (insert index into `from`).
        segment: usize,
        /// Dot position at press (world px) — target measures from it.
        at: (f64, f64),
        /// Press point (world px) — deltas measure from here.
        press: (f64, f64),
        /// Latest preview delta (world px); release commits exactly it.
        delta: (f64, f64),
    },
    /// Press began on a PORT dot (a box side midpoint) of an element:
    /// dragging onto another element CREATES a `source → destination`
    /// relationship when released there (Esc / empty release cancels; the
    /// new edge lands selected). The preview is a transient line drawn
    /// over the canvas — the overlay never sees it, so the geometry
    /// preview==commit invariant is untouched.
    Create {
        /// The pressed element (the new relationship's source).
        source: ModelId,
        /// The pressed port (world px) — the preview line starts here.
        from: (f64, f64),
    },
    /// Press began on an ENDPOINT handle of the selected edge: dragging
    /// onto an element moves that END when released there (empty release
    /// cancels). Transient preview line, same invariant as [`Self::Create`].
    Reconnect {
        /// The selected edge whose endpoint moves.
        id: ModelId,
        /// Which end the press grabbed.
        endpoint: commands::Endpoint,
    },
    /// Press began INSIDE the minimap box: every frame the
    /// pointer position re-centers the viewport (click = one recenter,
    /// drag = continuous). No scene edit, no history entry.
    Minimap,
}

/// What a press on a waypoint handle finds: the edge id, its
/// press-time INTERIOR route, and the route-relative vertex index.
type WaypointHit = (ModelId, Vec<(f64, f64)>, usize);

/// What a press on an INSERT dot of the selected edge finds: the edge
/// id, its press-time INTERIOR route, the hit SEGMENT index (the route
/// insert position) and the dot's world position.
type InsertDotHit = (ModelId, Vec<(f64, f64)>, usize, (f64, f64));

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
                help: false,
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
            md: egui_commonmark::CommonMarkCache::default(),
            field_draft: None,
            field_seen: Vec::new(),
            base_dir: None,
            icon_texture: None,
            icon_uv: BTreeMap::new(),
            icon_pixels: Vec::new(),
            icon_errors: Vec::new(),
            overlays: HashMap::new(),
            history: commands::History::default(),
            drag: Drag::Idle,
            grabbed: None,
            dirty: BTreeSet::new(),
            model_dirty: false,
            model_note: None,
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
        self.history.reset();
        self.grabbed = None;
        self.dirty.clear();
        self.model_dirty = false;
        self.model_note = None;
        self.save_note = None;
        // An in-flight field draft belongs to the OLD workspace — it
        // dies with it, it never commits into the new one.
        self.field_draft = None;
        self.field_seen.clear();
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

    /// The keyboard leg of one frame: Escape, then the canvas chords.
    /// Undo/redo: Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y walk the SESSION
    /// history — the step carries its view, so undoing can land on a
    /// view the user has since switched away from. Skipped while the
    /// toolbar path field owns the keyboard (Ctrl+Z inside the path
    /// edit is egui's). Ctrl+S saves under the same focus guard.
    fn canvas_keys(&mut self, ctx: &egui::Context) {
        //: F1 and `?` toggle the help overlay wherever the keyboard is
        // free — an open workspace is NOT required (the table describes
        // the app, not one view). egui has no `?` key, so it is the
        // shifted slash.
        if !self.text_focus
            && ctx.input(|i| {
                i.key_pressed(egui::Key::F1)
                    || (i.modifiers.shift && i.key_pressed(egui::Key::Slash))
            })
        {
            self.toggles.help = !self.toggles.help;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            //: help first — Esc closes the overlay BEFORE its other
            // duties (drag cancel, selection clear): dismissing a help
            // screen never costs the selection.
            if self.toggles.help {
                self.toggles.help = false;
            } else {
                //: Esc cancels an in-flight model drag first — its preview is
                // transient (no overlay write), so cancelling is dropping the
                // drag; the selection clear below stays the second duty.
                if matches!(self.drag, Drag::Create { .. } | Drag::Reconnect { .. }) {
                    self.drag = Drag::Idle;
                }
                self.selection = None;
                self.grabbed = None;
            }
        }
        if self.scene.is_none() || self.text_focus {
            return;
        }
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
        // Delete/Backspace DELETES the selected relationship (undoable
        // model edit; blocked sources only see the status note).
        // Ctrl+Shift+K owns the waypoint-strip chord alone — Del kept
        // that meaning until the model ops landed.
        let (del, straighten) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::K),
            )
        });
        if del {
            self.delete_selected_edge();
        }
        if straighten {
            self.clear_selected_waypoints();
        }
        // Arrow keys: nudge by one grid step (Shift = 10x) — the
        // GRABBED waypoint of the selected edge if there is one,
        // else the selected elements (enclosed routes follow).
        let nudge = ctx.input(|i| {
            let step = if i.modifiers.shift { 10.0 } else { 1.0 };
            let mut nudge = (0.0, 0.0);
            for (key, (dx, dy)) in [
                (egui::Key::ArrowLeft, (-1.0, 0.0)),
                (egui::Key::ArrowRight, (1.0, 0.0)),
                (egui::Key::ArrowUp, (0.0, -1.0)),
                (egui::Key::ArrowDown, (0.0, 1.0)),
            ] {
                if i.key_pressed(key) {
                    nudge.0 += dx * step;
                    nudge.1 += dy * step;
                }
            }
            nudge
        });
        if nudge != (0.0, 0.0) {
            self.nudge_selection(nudge.0 * commands::GRID_PX, nudge.1 * commands::GRID_PX);
        }
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
        self.canvas_keys(&ctx);
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
        self.help_window(&ctx);
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
