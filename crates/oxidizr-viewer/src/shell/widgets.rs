//! Free widgets + text helpers of the eframe shell: view rows and
//! labels, key-panel swatches, log lines, hit-to-text helpers and the
//! light/dark scheme helpers. The Viewer state and the App wiring stay
//! in the parent; the action and panel legs import these via
//! `super::widgets`.

use std::collections::BTreeMap;

use eframe::egui::{self, Pos2};

use crate::{hit, key, overview, paint, scene};
use oxidizr_api::LogItem;
use oxidizr_core::Id as ModelId;
use oxidizr_core::views::{ColorScheme, LineStyle, RankDirection, View};
use oxidizr_diag::Severity;

use super::Drag;

/// List label for a view: title, else the key (the official UI switcher).
pub(super) fn view_label(view: View<'_>) -> String {
    view.base()
        .title
        .clone()
        .unwrap_or_else(|| view.key().to_owned())
}

/// One sidebar row: title + dirty dot, selection state, the
/// key/description hover detail; `true` when clicked. Split out of
/// `view_list` (the kind sections) for the line budget.
pub(super) fn view_row(ui: &mut egui::Ui, view: View<'_>, selected: bool, dirty: bool) -> bool {
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
pub(super) type NavJump = (ModelId, (f64, f64));

/// Quick-nav element rows (user: "Icons for results in
/// search do not work"): each hit shows its REAL icon through the same
/// 16 px atlas chip the key panel uses (iconless items keep the `▪`
/// glyph; a broken icon gets the chip's hollow placeholder). Returns
/// the row count (against the caller's shared cap) and the clicked
/// jump target (id + centre). Split out of `view_list` for the line
/// budget.
pub(super) fn nav_element_rows(
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
pub(super) fn direction_label(direction: Option<RankDirection>) -> &'static str {
    match direction.unwrap_or_default() {
        RankDirection::TopBottom => "Top → Bottom",
        RankDirection::BottomTop => "Bottom → Top",
        RankDirection::LeftRight => "Left → Right",
        RankDirection::RightLeft => "Right → Left",
    }
}

/// The click-through URL behind a hit (`None` = plain selection).
pub(super) fn url_at(scene: &scene::Scene, hit: hit::Hit) -> Option<&str> {
    match hit {
        hit::Hit::Item(index) => scene.items.get(index)?.url.as_deref(),
        hit::Hit::Edge(index) => scene.edges.get(index)?.url.as_deref(),
    }
}

/// Multi-line hover text for a hovered item/edge.
pub(super) fn tooltip_text(scene: &scene::Scene, hit: hit::Hit) -> String {
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
pub(super) fn icon_cell(
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
pub(super) fn lasso_drag(selection: Option<&paint::Selection>, anchor: Pos2, shift: bool) -> Drag {
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
pub(super) fn snip(text: &str) -> String {
    const LIMIT: usize = 48;
    if text.chars().count() <= LIMIT {
        return text.to_owned();
    }
    let head: String = text.chars().take(LIMIT).collect();
    format!("{head}…")
}

/// Small fixed-size swatch cell painted by `draw` (key panel rows).
pub(super) fn swatch_cell(ui: &mut egui::Ui, draw: impl FnOnce(egui::Rect, &egui::Painter)) {
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
    draw(rect, ui.painter());
}

/// Element legend swatch: filled box in the entry's resolved colors.
pub(super) fn element_swatch(ui: &mut egui::Ui, entry: &key::ElementKey) {
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
pub(super) fn edge_swatch(ui: &mut egui::Ui, entry: &key::EdgeKey) {
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
pub(super) fn log_row(item: &LogItem) -> (egui::Color32, String) {
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
pub(super) fn log_line(item: &LogItem) -> String {
    log_row(item).1
}

/// First three log lines inline, the rest counted (status bar text).
pub(super) fn summarize_log(log: &[LogItem], prefix: &str) -> String {
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
pub(super) fn truncate(text: &str, max: usize) -> String {
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
pub(super) fn detect_os_scheme() -> ColorScheme {
    if matches!(dark_light::detect().ok(), Some(dark_light::Mode::Dark)) {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

/// Stored theme string → scheme (anything but `dark` = light).
pub(super) fn parse_scheme(stored: &str) -> ColorScheme {
    if stored == "dark" {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}
