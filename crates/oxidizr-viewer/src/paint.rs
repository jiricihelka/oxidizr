//! The paint step (M1-4): [`Scene`] + [`Viewport`] → one frame's canvas
//! as an inert [`epaint::Shape`] list — P3-D3's "build once, paint cheap"
//! transform: no workspace access, no state, no windowing.
//!
//! `epaint` is allowed in the lib core (the R-2 boundary guard forbids
//! only the windowing/GPU stack): it is pure software raster geometry
//! and text layout, and [`paint_scene`] merely returns data — the shell
//! draws the list verbatim, so the whole paint path is testable
//! headlessly (Category A).
//!
//! Visual-parity approximations (out of contract per P3-D0, recorded on
//! the §P3-M1 M1-4 row):
//!
//! - Opacity becomes an alpha byte on the fill, stroke and text colors
//!   (the reference composites whole boxes via canvas group opacity).
//! - `Dashed`/`Dotted` strokes expand to dash segments — `epaint` 0.37
//!   has no dashed stroke style, `Shape::dashed_line_many` is the
//!   mechanism; dash periods are our screen-pixel constants.
//! - Text is the name / metadata / description stack centred in the
//!   box; the reference's bold name falls back to the same bundled
//!   face (P3-D2: bundled fonts only, no bold variant).
//! - Element glyphs are polygon approximations of the reference SVG
//!   paths (arcs sampled at [`ARC_SEGMENTS`] points); arrowheads are
//!   filled triangles at the destination end.
//! - Below [`MIN_TEXT_PX`] text is skipped, and anything outside the
//!   painted screen rect is culled — cheap paints at any zoom.

use std::sync::Arc;

use epaint::emath::Align2;
use epaint::text::{FontId, FontsView};
use epaint::{Color32, PathShape, Pos2, Rect as ERect, Shape, Stroke, Vec2, pos2};

use crate::hit::Viewport;
use crate::scene::{Boundary, Edge, Item, Rect, Scene};
use oxidizr_core::Id;
use oxidizr_core::views::{IconPosition, LineStyle, Shape as ElementShape};

/// Selection accent outline color (ours — the reference uses its CSS
/// accent; visual parity is out of contract).
pub const SELECTION_COLOR: Color32 = Color32::from_rgb(0, 150, 215);
/// Text below this many SCREEN pixels is not painted at all (zoomed-out
/// culling; shapes stay, runs vanish).
pub const MIN_TEXT_PX: f32 = 5.0;
/// `Dashed` dash length in screen px (period constant of ours).
pub const DASH_PX: f32 = 8.0;
/// `Dashed` gap length in screen px.
pub const DASH_GAP_PX: f32 = 6.0;
/// Arrowhead length in WORLD px (clamped into 5..=40 screen px at paint
/// time; reference marker ≈ 10).
pub const ARROW_LEN: f32 = 10.0;
/// Box-interior text padding in world px (scaled).
pub const TEXT_PAD: f32 = 8.0;
/// Arc sampling resolution for ellipse-derived outlines.
pub const ARC_SEGMENTS: usize = 32;
/// Fallback fill when a scene color fails [`parse_color`].
pub const FALLBACK_FILL: Color32 = Color32::from_rgb(135, 180, 220);
/// Fallback ink (stroke/text) when a scene color fails [`parse_color`].
pub const FALLBACK_INK: Color32 = Color32::BLACK;
/// Screen-space radius of the waypoint handles painted on the SELECTED
/// edge (P3-M4) — fixed pixels, so they stay grabbable at any zoom.
pub const WAYPOINT_HANDLE_PX: f32 = 4.0;

/// What the shell selects (id-based so the selection survives scene
/// rebuilds across zoom-band/filter changes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Element boxes — every id gets an accent outline (single click
    /// selects one; box-select and Shift-click select many, M3-3).
    Items(Vec<Id>),
    /// A relationship polyline — a [`Scene::edges`] entry with this id.
    Edge(Id),
}

/// Text style bundle for the box/edge text passes (keeps the helpers
/// within the pedantic argument budget).
#[derive(Clone, Copy)]
struct TextStyle {
    px: f32,
    ink: Color32,
    wrap: f32,
}

/// Inputs of one [`paint_scene`] call.
#[derive(Debug, Clone, Copy)]
pub struct PaintOptions<'a> {
    /// The screen rect being painted into (egui points).
    pub screen: ERect,
    /// The current selection, painted as an accent outline.
    pub selection: Option<&'a Selection>,
    /// Edge whose waypoint handles paint WITHOUT a selection (§P3-M11
    /// M11-2): hovering advertises the handles, so they can be dragged
    /// or Alt+deleted before the edge is even selected.
    pub hover_edge: Option<&'a Id>,
}

/// Paint one scene into a shape list, background first, then boundaries,
/// items, edges and the selection accent — the layer order the reference
/// paints (boundaries under boxes, arrows over boxes).
#[must_use]
pub fn paint_scene(
    scene: &Scene,
    viewport: &Viewport,
    fonts: &mut FontsView<'_>,
    options: &PaintOptions<'_>,
) -> Vec<Shape> {
    let background = parse_color(&scene.background).unwrap_or(Color32::from_rgb(255, 255, 255));
    let mut out = vec![Shape::rect_filled(options.screen, 0.0, background)];
    for boundary in &scene.boundaries {
        paint_boundary(&mut out, boundary, viewport, fonts, options);
    }
    for item in &scene.items {
        paint_item(&mut out, item, viewport, fonts, options);
    }
    for edge in &scene.edges {
        paint_edge(&mut out, edge, viewport, fonts, background, options);
    }
    out
}

/// A `#rrggbb` color from the resolved styles; `None` on anything else
/// (the scene guarantees the format, so `None` means a bug upstream —
/// the paint falls back, never panics).
#[must_use]
pub fn parse_color(text: &str) -> Option<Color32> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok();
    Some(Color32::from_rgb(byte(0)?, byte(1)?, byte(2)?))
}

/// Scene opacity (0-100 %) as an alpha-adjusted color.
#[must_use]
pub fn with_opacity(color: Color32, opacity: i32) -> Color32 {
    let alpha = u8::try_from(i32::from(color.a()) * opacity.clamp(0, 100) / 100).unwrap_or(u8::MAX);
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

// ---- layers ---------------------------------------------------------------

fn paint_boundary(
    out: &mut Vec<Shape>,
    boundary: &Boundary,
    viewport: &Viewport,
    fonts: &mut FontsView<'_>,
    options: &PaintOptions<'_>,
) {
    let screen_rect = to_rect(boundary.rect, viewport);
    if !visible(screen_rect, options.screen) {
        return;
    }
    let stroke = stroke_for(
        boundary.stroke.as_str(),
        boundary.stroke_width,
        boundary.opacity,
        viewport,
    );
    stroke_polygon(out, &rect_points(screen_rect), stroke, boundary.border);
    let px = scaled_px(boundary.font_size, viewport);
    if px < MIN_TEXT_PX {
        return;
    }
    let ink = with_opacity(
        parse_color(&boundary.color).unwrap_or(FALLBACK_INK),
        boundary.opacity,
    );
    let mut y = screen_rect.min.y + 2.0;
    for line in [Some(boundary.label.as_str()), boundary.metadata.as_deref()]
        .into_iter()
        .flatten()
    {
        let galley = fonts.layout_no_wrap(line.to_owned(), FontId::proportional(px), ink);
        let height = galley.size().y;
        let pos = pos2(screen_rect.min.x + 6.0, y);
        out.push(galley_shape(Align2::LEFT_TOP, pos, galley));
        y += height + 1.0;
    }
}

fn paint_item(
    out: &mut Vec<Shape>,
    item: &Item,
    viewport: &Viewport,
    fonts: &mut FontsView<'_>,
    options: &PaintOptions<'_>,
) {
    let screen_rect = to_rect(item.rect, viewport);
    if !visible(screen_rect, options.screen) {
        return;
    }
    let ink = with_opacity(
        parse_color(&item.color).unwrap_or(FALLBACK_INK),
        item.opacity,
    );
    let fill = with_opacity(
        parse_color(&item.background).unwrap_or(FALLBACK_FILL),
        item.opacity,
    );
    let stroke = stroke_for(
        item.stroke.as_str(),
        item.stroke_width,
        item.opacity,
        viewport,
    );
    let (outline, body) = element_geometry(item.shape, screen_rect, fill, stroke.color);
    out.extend(body);
    stroke_polygon(out, &outline, stroke, item.border);
    if let Some(Selection::Items(ids)) = options.selection
        && ids.contains(&item.id)
    {
        out.push(Shape::rect_stroke(
            screen_rect.expand(4.0),
            4.0,
            Stroke::new(3.0, SELECTION_COLOR),
            epaint::StrokeKind::Outside,
        ));
    }
    let px = scaled_px(item.font_size, viewport);
    if px < MIN_TEXT_PX {
        return;
    }
    let area = item.icon.as_ref().map_or(screen_rect, |_| {
        icon_placeholder(out, screen_rect, item.icon_position, stroke.color)
    });
    let mut lines: Vec<&str> = Vec::with_capacity(3);
    if !item.name.is_empty() {
        lines.push(&item.name);
    }
    if let Some(metadata) = &item.metadata {
        lines.push(metadata);
    }
    if let Some(description) = &item.description {
        lines.push(description);
    }
    text_block(out, fonts, area, &lines, px, ink);
}

fn paint_edge(
    out: &mut Vec<Shape>,
    edge: &Edge,
    viewport: &Viewport,
    fonts: &mut FontsView<'_>,
    background: Color32,
    options: &PaintOptions<'_>,
) {
    if edge.points.len() < 2 {
        return;
    }
    let points: Vec<Pos2> = edge.points.iter().map(|p| to_pos(*p, viewport)).collect();
    let bounds = ERect::from_points(&points);
    if !visible(bounds, options.screen) {
        return;
    }
    let color = with_opacity(
        parse_color(&edge.color).unwrap_or(FALLBACK_INK),
        edge.opacity,
    );
    let width = scaled_px(edge.thickness, viewport).max(0.75);
    let stroke = Stroke::new(width, color);
    match edge.line_style {
        LineStyle::Solid => out.push(Shape::line(points.clone(), stroke)),
        LineStyle::Dashed => {
            Shape::dashed_line_many(&points, stroke, DASH_PX, DASH_GAP_PX, out);
        }
        LineStyle::Dotted => {
            Shape::dashed_line_many(&points, stroke, width * 1.2, width * 2.4, out);
        }
    }
    if let Some((tip, from)) = tip_and_from(&points) {
        let head_len = (ARROW_LEN * f(viewport.scale())).clamp(5.0, 40.0);
        out.push(arrow_head(tip, from, color, head_len));
    }
    let selected = matches!(options.selection, Some(Selection::Edge(id)) if *id == edge.id);
    if selected {
        out.push(Shape::line(
            points.clone(),
            Stroke::new(width + 2.0, SELECTION_COLOR),
        ));
    }
    // P3-M4: one handle circle per INTERIOR vertex — the endpoints
    // belong to the boxes and never get one. §P3-M11: the hovered edge
    // shows them too (delete/drag without selecting first).
    if (selected || options.hover_edge == Some(&edge.id)) && points.len() >= 3 {
        for at in &points[1..points.len() - 1] {
            out.push(Shape::circle_filled(
                *at,
                WAYPOINT_HANDLE_PX,
                SELECTION_COLOR,
            ));
            out.push(Shape::circle_stroke(
                *at,
                WAYPOINT_HANDLE_PX,
                Stroke::new(1.0, Color32::WHITE),
            ));
        }
    }
    let px = scaled_px(edge.font_size, viewport);
    let label_at = point_at(
        &points,
        f(f64::from(edge.label_position.clamp(0, 100)) / 100.0),
    );
    if px >= MIN_TEXT_PX
        && let Some(at) = label_at
    {
        let lines: Vec<&str> = [&edge.label, &edge.metadata]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        edge_label(
            out,
            fonts,
            at,
            &lines,
            &TextStyle {
                px,
                ink: color,
                wrap: scaled_px(edge.label_width, viewport).max(16.0),
            },
            background,
        );
    }
    if let Some(order) = &edge.order
        && px >= MIN_TEXT_PX
        && let Some(at) = point_at(&points, 0.3)
    {
        let radius = f(8.0 * viewport.scale()).clamp(5.0, 24.0);
        out.push(Shape::circle_filled(at, radius, background));
        out.push(Shape::circle_stroke(at, radius, Stroke::new(1.5, color)));
        let digits = fonts.layout_no_wrap(
            order.clone(),
            FontId::proportional((radius * 1.3).max(MIN_TEXT_PX)),
            color,
        );
        out.push(galley_shape(Align2::CENTER_CENTER, at, digits));
    }
}

// ---- element glyph geometry -------------------------------------------------

/// Outline polygon + painted body of one element glyph (the polygon
/// doubles as the dashed-border overlay path).
// One arm per glyph — the table IS the glyph set.
#[allow(clippy::too_many_lines)]
fn element_geometry(
    shape: ElementShape,
    r: ERect,
    fill: Color32,
    ink: Color32,
) -> (Vec<Pos2>, Vec<Shape>) {
    let stroke = Stroke::new(1.0, ink);
    let (w, h) = r.size().into();
    let cx = r.center().x;
    // The hexagon's indices (≤ 6) are exact in f32.
    #[allow(clippy::cast_precision_loss)]
    match shape {
        ElementShape::Box => (rect_points(r), vec![Shape::rect_filled(r, 0.0, fill)]),
        ElementShape::RoundedBox => {
            let rounding = (w.min(h) * 0.1).min(24.0);
            (rect_points(r), vec![Shape::rect_filled(r, rounding, fill)])
        }
        ElementShape::Circle => {
            let radius = w.min(h) / 2.0;
            (
                ellipse_points(r.center(), radius, radius),
                vec![Shape::circle_filled(r.center(), radius, fill)],
            )
        }
        ElementShape::Ellipse => {
            let (rx, ry) = (w / 2.0, h / 2.0);
            (
                ellipse_points(r.center(), rx, ry),
                vec![polygon(ellipse_points(r.center(), rx, ry), fill)],
            )
        }
        ElementShape::Hexagon => {
            let hex: Vec<Pos2> = (0..6)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / 6.0;
                    pos2(cx + (w / 2.0) * a.cos(), r.center().y + (h / 2.0) * a.sin())
                })
                .collect();
            (hex.clone(), vec![polygon(hex, fill)])
        }
        ElementShape::Diamond => {
            let points = vec![
                pos2(cx, r.min.y),
                pos2(r.max.x, r.center().y),
                pos2(cx, r.max.y),
                pos2(r.min.x, r.center().y),
            ];
            (points.clone(), vec![polygon(points, fill)])
        }
        ElementShape::Cylinder => {
            let ry = (h * 0.1).min(w / 2.0);
            let points = cylinder_points(r, ry, false);
            let top = half_ellipse(r.center().x, r.min.y + ry, w / 2.0, ry, true);
            (
                points.clone(),
                vec![
                    polygon(points, fill),
                    Shape::line(top, Stroke::new(1.0, ink)),
                ],
            )
        }
        ElementShape::Bucket => {
            let ry = (h * 0.08).min(w / 2.0);
            let inset = w * 0.18;
            let mut points = half_ellipse(r.center().x, r.min.y + ry, w / 2.0, ry, false);
            points.push(pos2(r.min.x + inset, r.max.y));
            points.push(pos2(r.max.x - inset, r.max.y));
            points.extend(half_ellipse(r.center().x, r.min.y + ry, w / 2.0, ry, true));
            (points.clone(), vec![polygon(points, fill)])
        }
        ElementShape::Pipe => {
            let rx = (w * 0.1).min(h / 2.0);
            let points = cylinder_points(r, rx, true);
            (points.clone(), vec![polygon(points, fill)])
        }
        ElementShape::Person => {
            let head_r = (w * 0.16).min(h * 0.16);
            let head_center = pos2(cx, r.min.y + h * 0.16 + head_r);
            let body = vec![
                pos2(cx - w * 0.32, r.min.y + h * 0.52),
                pos2(cx - w * 0.2, r.min.y + h * 0.38),
                pos2(cx + w * 0.2, r.min.y + h * 0.38),
                pos2(cx + w * 0.32, r.min.y + h * 0.52),
                pos2(cx + w * 0.26, r.max.y),
                pos2(cx - w * 0.26, r.max.y),
            ];
            (
                body.clone(),
                vec![
                    Shape::circle_filled(head_center, head_r, fill),
                    polygon(body, fill),
                ],
            )
        }
        ElementShape::Robot => {
            let head = erct(r.min.x + w * 0.3, r.min.y + h * 0.08, w * 0.4, h * 0.2);
            let body = erct(r.min.x + w * 0.14, r.min.y + h * 0.36, w * 0.72, h * 0.6);
            (
                rect_points(body),
                vec![
                    Shape::line_segment([pos2(cx, r.min.y + h * 0.08), pos2(cx, r.min.y)], stroke),
                    Shape::rect_filled(head, 1.0, fill),
                    Shape::rect_filled(body, 1.0, fill),
                ],
            )
        }
        ElementShape::Folder => {
            let points = vec![
                pos2(r.min.x, r.min.y + h * 0.25),
                pos2(r.min.x + w * 0.35, r.min.y + h * 0.25),
                pos2(r.min.x + w * 0.45, r.min.y + h * 0.1),
                pos2(r.max.x, r.min.y + h * 0.1),
                pos2(r.max.x, r.max.y),
                pos2(r.min.x, r.max.y),
            ];
            (points.clone(), vec![polygon(points, fill)])
        }
        ElementShape::WebBrowser => {
            let bar_y = r.min.y + h * 0.14;
            (
                rect_points(r),
                vec![
                    Shape::rect_filled(r, 2.0, fill),
                    Shape::line(
                        vec![
                            pos2(r.min.x, bar_y),
                            pos2(r.max.x, bar_y),
                            pos2(r.max.x, r.min.y),
                        ],
                        stroke,
                    ),
                ],
            )
        }
        ElementShape::Window => {
            let bar_y = r.min.y + h * 0.12;
            let close = erct(r.max.x - w * 0.08, bar_y - h * 0.08, w * 0.05, h * 0.05);
            (
                rect_points(r),
                vec![
                    Shape::rect_filled(r, 1.0, fill),
                    Shape::line_segment([pos2(r.min.x, bar_y), pos2(r.max.x, bar_y)], stroke),
                    Shape::rect_stroke(close, 0.0, stroke, epaint::StrokeKind::Middle),
                ],
            )
        }
        ElementShape::Terminal => {
            let points = vec![
                pos2(r.min.x + w * 0.1, r.min.y + h * 0.3),
                pos2(r.min.x + w * 0.25, r.min.y + h * 0.45),
                pos2(r.min.x + w * 0.1, r.min.y + h * 0.6),
            ];
            (
                rect_points(r),
                vec![
                    Shape::rect_filled(r, 2.0, fill),
                    Shape::line(points, stroke),
                ],
            )
        }
        ElementShape::Shell => {
            let prompt = vec![
                pos2(r.min.x + w * 0.1, r.min.y + h * 0.3),
                pos2(r.min.x + w * 0.25, r.min.y + h * 0.45),
                pos2(r.min.x + w * 0.1, r.min.y + h * 0.6),
                pos2(r.min.x + w * 0.32, r.min.y + h * 0.6),
            ];
            (
                rect_points(r),
                vec![
                    Shape::rect_filled(r, 2.0, fill),
                    Shape::line(prompt, stroke),
                ],
            )
        }
        ElementShape::MobileDevicePortrait | ElementShape::MobileDeviceLandscape => {
            let portrait = shape == ElementShape::MobileDevicePortrait;
            let (dot_a, dot_b) = if portrait {
                (cx, r.max.y - h * 0.05)
            } else {
                (r.max.x - w * 0.05, r.center().y)
            };
            (
                rect_points(r),
                vec![
                    Shape::rect_filled(r, w.min(h) * 0.12, fill),
                    Shape::circle_filled(pos2(dot_a, dot_b), 1.5_f32.max(w.min(h) * 0.015), ink),
                ],
            )
        }
        ElementShape::Component => {
            let circle_y = [0.32, 0.68];
            let mut body = vec![
                Shape::rect_filled(r, 1.0, fill),
                Shape::line_segment(
                    [
                        pos2(r.min.x + w * 0.24, r.min.y + h * 0.2),
                        pos2(r.min.x + w * 0.24, r.min.y + h * 0.8),
                    ],
                    stroke,
                ),
            ];
            body.extend(circle_y.map(|cy| {
                Shape::circle_stroke(
                    pos2(r.min.x + w * 0.12, r.min.y + h * cy),
                    (w.min(h) * 0.05).max(1.5),
                    stroke,
                )
            }));
            (rect_points(r), body)
        }
    }
}

/// Icon slot geometry: `(box, text-area shrink vector)` — the shell's
/// M1-5 icon-texture overlay reuses the exact box the placeholder draws.
fn icon_box(r: ERect, position: IconPosition) -> (ERect, Vec2) {
    let slot = (r.size().min_elem() * 0.28).clamp(8.0, 64.0);
    match position {
        IconPosition::Top => (
            erct(r.center().x - slot / 2.0, r.min.y + 4.0, slot, slot),
            Vec2::new(0.0, slot + 4.0),
        ),
        IconPosition::Bottom => (
            erct(r.center().x - slot / 2.0, r.max.y - slot - 4.0, slot, slot),
            Vec2::new(0.0, -(slot + 4.0)),
        ),
        IconPosition::Left => (
            erct(r.min.x + 4.0, r.center().y - slot / 2.0, slot, slot),
            Vec2::new(slot + 4.0, 0.0),
        ),
    }
}

/// The icon slot rect inside a screen rect — where the shell overlays a
/// rasterized icon texture (M1-5) and where the placeholder below draws.
#[must_use]
pub fn icon_slot(r: ERect, position: IconPosition) -> ERect {
    icon_box(r, position).0
}

/// World-space item/boundary rect → screen rect through `viewport` —
/// paint's own mapping, shared with the shell's icon overlay (M1-5).
#[must_use]
pub fn to_screen_rect(rect: Rect, viewport: &Viewport) -> ERect {
    to_rect(rect, viewport)
}

/// The dashed placeholder box standing in for a style `icon` whose
/// raster is absent from the atlas (missing, broken or unsupported);
/// returns the text area (the box shrunk by the slot).
fn icon_placeholder(out: &mut Vec<Shape>, r: ERect, position: IconPosition, ink: Color32) -> ERect {
    let (box_rect, shrink) = icon_box(r, position);
    out.push(Shape::rect_stroke(
        box_rect,
        2.0,
        Stroke::new(1.0, with_opacity(ink, 60)),
        epaint::StrokeKind::Middle,
    ));
    ERect::from_min_max(r.min + shrink * 0.5, r.max - shrink * 0.5)
}

// ---- text -------------------------------------------------------------------

/// The stacked text block of a box (name / metadata / description),
/// centred in `area`, wrapped at the area width.
fn text_block(
    out: &mut Vec<Shape>,
    fonts: &mut FontsView<'_>,
    area: ERect,
    lines: &[&str],
    px: f32,
    ink: Color32,
) {
    if lines.is_empty() {
        return;
    }
    let wrap = (area.width() - TEXT_PAD * 2.0).max(16.0);
    let galleys: Vec<Arc<epaint::Galley>> = lines
        .iter()
        .map(|line| fonts.layout(line.to_string(), FontId::proportional(px), ink, wrap))
        .collect();
    let gap = px * 0.3;
    let total = galleys.iter().enumerate().fold(0.0_f32, |acc, (i, g)| {
        acc + g.size().y + f32::from(i > 0) * gap
    });
    let mut y = area.center().y - total / 2.0;
    for galley in galleys {
        let height = galley.size().y;
        let pos = pos2(area.center().x, y);
        out.push(galley_shape(Align2::CENTER_TOP, pos, galley));
        y += height + gap;
    }
}

/// Edge label (description + metadata) with a readable background plate,
/// centred at `at`.
fn edge_label(
    out: &mut Vec<Shape>,
    fonts: &mut FontsView<'_>,
    at: Pos2,
    lines: &[&str],
    style: &TextStyle,
    background: Color32,
) {
    if lines.is_empty() {
        return;
    }
    let (px, ink, wrap) = (style.px, style.ink, style.wrap);
    let galleys: Vec<Arc<epaint::Galley>> = lines
        .iter()
        .map(|line| fonts.layout(line.to_string(), FontId::proportional(px), ink, wrap))
        .collect();
    let gap = style.px * 0.2;
    let total = galleys.iter().enumerate().fold(0.0_f32, |acc, (i, g)| {
        acc + g.size().y + f32::from(i > 0) * gap
    });
    let widest = galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
    let plate = ERect::from_center_size(at, Vec2::new(widest + 6.0, total + 4.0));
    out.push(Shape::rect_filled(plate, 1.0, with_opacity(background, 80)));
    let mut y = at.y - total / 2.0;
    for galley in galleys {
        let height = galley.size().y;
        let pos = pos2(at.x, y);
        out.push(galley_shape(Align2::CENTER_TOP, pos, galley));
        y += height + gap;
    }
}

fn galley_shape(anchor: Align2, pos: Pos2, galley: Arc<epaint::Galley>) -> Shape {
    let rect_at = anchor.anchor_size(pos, galley.size());
    Shape::galley(rect_at.min, galley, Color32::PLACEHOLDER)
}

// ---- small geometry / color helpers ------------------------------------------

/// World-px size (font / stroke / label width) in screen px, scaled by
/// the viewport zoom.
fn scaled_px(value: i32, viewport: &Viewport) -> f32 {
    f(f64::from(value.clamp(1, i32::MAX)) * viewport.scale()).max(0.5)
}

fn stroke_for(color: &str, width: i32, opacity: i32, viewport: &Viewport) -> Stroke {
    Stroke::new(
        scaled_px(width, viewport).max(0.5),
        with_opacity(parse_color(color).unwrap_or(FALLBACK_INK), opacity),
    )
}

/// f64 world math → f32 screen points; the truncation is subpixel noise
/// at canvas scale (viewer pixels, not the byte contract).
#[expect(clippy::cast_possible_truncation)]
fn f(value: f64) -> f32 {
    value as f32
}

/// `x/y/w/h` → [`ERect`] (epaint 0.37 does not re-export `emath::rect`).
fn erct(x: f32, y: f32, w: f32, h: f32) -> ERect {
    ERect::from_min_size(pos2(x, y), Vec2::new(w, h))
}

fn to_pos(world: (f64, f64), viewport: &Viewport) -> Pos2 {
    let (x, y) = viewport.world_to_screen(world);
    pos2(f(x), f(y))
}

fn to_rect(rect: Rect, viewport: &Viewport) -> ERect {
    ERect::from_min_max(
        to_pos((rect.x, rect.y), viewport),
        to_pos((rect.x + rect.width, rect.y + rect.height), viewport),
    )
}

/// Viewport culling: does `rect` touch `screen` (a generous margin keeps
/// labels/arrowheads popping in just outside the edge)?
fn visible(rect: ERect, screen: ERect) -> bool {
    let margin = 64.0;
    rect.max.x >= screen.min.x - margin
        && rect.min.x <= screen.max.x + margin
        && rect.max.y >= screen.min.y - margin
        && rect.min.y <= screen.max.y + margin
}

fn rect_points(r: ERect) -> Vec<Pos2> {
    vec![
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    ]
}

#[allow(clippy::cast_precision_loss)] // arc indices ≤ ARC_SEGMENTS: exact in f32
fn ellipse_points(center: Pos2, rx: f32, ry: f32) -> Vec<Pos2> {
    (0..ARC_SEGMENTS)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / ARC_SEGMENTS as f32;
            pos2(center.x + rx * a.cos(), center.y + ry * a.sin())
        })
        .collect()
}

/// Half of an ellipse (top or bottom arc), left→right.
#[allow(clippy::cast_precision_loss)] // arc indices: exact in f32
fn half_ellipse(cx: f32, cy: f32, rx: f32, ry: f32, top: bool) -> Vec<Pos2> {
    (0..=ARC_SEGMENTS / 2)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / (ARC_SEGMENTS / 2) as f32;
            let sign = if top { -1.0 } else { 1.0 };
            pos2(cx - rx * a.cos(), cy + sign * ry * a.sin())
        })
        .collect()
}

/// Cylinder / pipe silhouette: two elliptical caps, vertical or horizontal.
fn cylinder_points(r: ERect, cap: f32, horizontal: bool) -> Vec<Pos2> {
    let mut points = if horizontal {
        let mut p = half_ellipse_vertical(r.min.x + cap, r.center().y, cap, r.height() / 2.0, true);
        p.extend(half_ellipse_vertical(
            r.max.x - cap,
            r.center().y,
            cap,
            r.height() / 2.0,
            false,
        ));
        p
    } else {
        let mut p = half_ellipse(r.center().x, r.min.y + cap, r.width() / 2.0, cap, true);
        p.extend(half_ellipse(
            r.center().x,
            r.max.y - cap,
            r.width() / 2.0,
            cap,
            false,
        ));
        p
    };
    if points.len() < 3 {
        points = rect_points(r);
    }
    points
}

#[allow(clippy::cast_precision_loss)] // arc indices: exact in f32
fn half_ellipse_vertical(cx: f32, cy: f32, rx: f32, ry: f32, left: bool) -> Vec<Pos2> {
    (0..=ARC_SEGMENTS / 2)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / (ARC_SEGMENTS / 2) as f32;
            let sign = if left { -1.0 } else { 1.0 };
            pos2(cx + sign * rx * a.sin(), cy - ry * a.cos())
        })
        .collect()
}

fn polygon(points: Vec<Pos2>, fill: Color32) -> Shape {
    Shape::Path(PathShape::convex_polygon(points, fill, Stroke::NONE))
}

fn stroke_polygon(out: &mut Vec<Shape>, points: &[Pos2], stroke: Stroke, style: LineStyle) {
    match style {
        LineStyle::Solid => out.push(Shape::closed_line(points.to_vec(), stroke)),
        LineStyle::Dashed => {
            let mut looped = points.to_vec();
            looped.push(points[0]);
            Shape::dashed_line_many(&looped, stroke, DASH_PX, DASH_GAP_PX, out);
        }
        LineStyle::Dotted => {
            let mut looped = points.to_vec();
            looped.push(points[0]);
            Shape::dashed_line_many(&looped, stroke, stroke.width * 1.2, stroke.width * 2.4, out);
        }
    }
}

/// The destination tip and a distinct earlier point (arrowhead direction;
/// duplicate stored vertices are legal JSON, skip them).
fn tip_and_from(points: &[Pos2]) -> Option<(Pos2, Pos2)> {
    let tip = *points.last()?;
    let from = points
        .iter()
        .rev()
        .skip(1)
        .find(|p| tip.distance(**p) > 0.01)
        .copied()?;
    Some((tip, from))
}

/// Filled arrowhead triangle at `tip` (length `head` screen px),
/// pointing away from `from`.
fn arrow_head(tip: Pos2, from: Pos2, color: Color32, head: f32) -> Shape {
    let delta = tip - from;
    let length = delta.length().max(f32::EPSILON);
    let dir = delta / length;
    let base = tip - dir * head;
    let perp = Vec2::new(-dir.y, dir.x) * head * 0.45;
    polygon(vec![tip, base + perp, base - perp], color)
}

/// Point at `fraction` (0..=1) of the polyline's arc length.
fn point_at(points: &[Pos2], fraction: f32) -> Option<Pos2> {
    let first = *points.first()?;
    if points.len() < 2 {
        return Some(first);
    }
    let segments: Vec<f32> = points.windows(2).map(|w| w[0].distance(w[1])).collect();
    let total: f32 = segments.iter().sum();
    if total <= f32::EPSILON {
        return Some(first);
    }
    let mut target = total * fraction.clamp(0.0, 1.0);
    for (window, length) in points.windows(2).zip(&segments) {
        if target <= *length {
            let t = if *length > f32::EPSILON {
                target / length
            } else {
                0.0
            };
            return Some(window[0].lerp(window[1], t));
        }
        target -= length;
    }
    points.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::BoundaryKind;
    use epaint::text::{FontDefinitions, Fonts, TextOptions};
    use oxidizr_core::Id;
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{LineStyle, Shape as ElementShape};

    fn item(id: &str, x: f64, y: f64) -> Item {
        Item {
            id: Id::new(id),
            kind: ElementKind::Person,
            name: "Alice".to_string(),
            description: Some("A person".to_string()),
            metadata: Some("[Person]".to_string()),
            group: None,
            url: None,
            rect: Rect {
                x,
                y,
                width: 350.0,
                height: 150.0,
            },
            shape: ElementShape::Box,
            background: "#8db4dc".to_string(),
            stroke: "#1a1a1a".to_string(),
            stroke_width: 2,
            border: LineStyle::Solid,
            color: "#000000".to_string(),
            font_size: 24,
            opacity: 100,
            icon: None,
            icon_position: IconPosition::Top,
        }
    }

    fn edge(points: Vec<(f64, f64)>) -> Edge {
        Edge {
            id: Id::new("e"),
            source: Id::new("s"),
            destination: Id::new("d"),
            points,
            label: Some("Uses".to_string()),
            metadata: Some("[HTTPS]".to_string()),
            order: Some("1".to_string()),
            color: "#333333".to_string(),
            thickness: 2,
            line_style: LineStyle::Solid,
            font_size: 24,
            label_width: 200,
            label_position: 50,
            opacity: 100,
            url: None,
        }
    }

    fn scene() -> Scene {
        Scene {
            view_key: "K".to_string(),
            title: "T".to_string(),
            background: "#ffffff".to_string(),
            boundaries: vec![Boundary {
                kind: BoundaryKind::Group,
                id: "group:root/Team".to_string(),
                label: "Team".to_string(),
                metadata: None,
                rect: Rect {
                    x: -20.0,
                    y: -20.0,
                    width: 800.0,
                    height: 200.0,
                },
                background: "#ffffff".to_string(),
                stroke: "#888888".to_string(),
                stroke_width: 2,
                border: LineStyle::Dashed,
                color: "#555555".to_string(),
                font_size: 24,
                opacity: 100,
            }],
            items: vec![item("1", 0.0, 0.0)],
            edges: vec![edge(vec![(175.0, 75.0), (900.0, 75.0)])],
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 900.0,
                height: 150.0,
            },
        }
    }

    #[test]
    fn hover_edge_paints_handles_without_selection() {
        // §P3-M11 M11-2: the hovered edge advertises its route.
        let mut s = scene();
        s.edges[0].points = vec![(175.0, 75.0), (500.0, 300.0), (900.0, 75.0)];
        let hover = s.edges[0].id.clone();
        let circles = |shapes: &[Shape]| {
            shapes
                .iter()
                .filter(|shape| matches!(shape, Shape::Circle(_)))
                .count()
        };
        // The fixture paints a circle-shaped item — measure the DELTA.
        let base = circles(&painted(&s, None));
        let hovered = painted_with(&s, None, Some(&hover));
        assert_eq!(
            circles(&hovered) - base,
            2,
            "one filled + one outlined handle"
        );
        // The selected edge keeps its handles (and the accent line).
        let selected = painted(&s, Some(&Selection::Edge(hover)));
        assert!(circles(&selected) - base >= 2);
    }

    fn painted(s: &Scene, selection: Option<&Selection>) -> Vec<Shape> {
        painted_with(s, selection, None)
    }

    fn painted_with(
        s: &Scene,
        selection: Option<&Selection>,
        hover_edge: Option<&Id>,
    ) -> Vec<Shape> {
        let mut fonts = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let mut view = fonts.with_pixels_per_point(10.0);
        paint_scene(
            s,
            &Viewport::new(1.0, (0.0, 0.0)),
            &mut view,
            &PaintOptions {
                screen: erct(-100.0, -100.0, 2000.0, 1500.0),
                selection,
                hover_edge,
            },
        )
    }

    fn texts(shapes: &[Shape]) -> String {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text(t) => Some(t.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn paints_layers_and_text() {
        let selection = Selection::Items(vec![Id::new("1")]);
        let shapes = painted(&scene(), Some(&selection));
        assert!(shapes.len() > 4, "layers: bg + boundary + item + edge");
        let text = texts(&shapes);
        assert!(text.contains("Alice"), "name painted: {text}");
        assert!(text.contains("[Person]"), "metadata painted: {text}");
        assert!(text.contains("Team"), "group label painted: {text}");
        assert!(text.contains("Uses"), "edge label painted: {text}");
        assert!(text.contains("[HTTPS]"), "edge metadata painted: {text}");
        assert!(text.contains('1'), "dynamic order digit painted: {text}");
        let accents = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Rect(r) if r.stroke.color == SELECTION_COLOR))
            .count();
        assert_eq!(accents, 1, "selection accent painted once");
    }

    #[test]
    fn paint_is_deterministic() {
        let a = painted(&scene(), None);
        let b = painted(&scene(), None);
        assert_eq!(a.len(), b.len());
        assert_eq!(texts(&a), texts(&b));
    }

    #[test]
    fn selected_edge_paints_waypoint_handles() {
        let mut s = scene();
        // Fixture edge "e" gets two INTERIOR vertices; the fixture item
        // is a Box, so no other shape in the scene is a Circle.
        s.edges[0].points = vec![(175.0, 75.0), (400.0, 120.0), (700.0, 40.0), (900.0, 75.0)];
        // The order badge paints its own circle pair — out of scope here.
        s.edges[0].order = None;
        let id = s.edges[0].id.clone();
        let circles = |shapes: &[Shape]| {
            shapes
                .iter()
                .filter(|shape| matches!(shape, Shape::Circle(_)))
                .count()
        };
        // Unselected: no handles at all…
        assert_eq!(circles(&painted(&s, None)), 0);
        // …selected: filled + ring per interior vertex (endpoints skip).
        let picked = painted(&s, Some(&Selection::Edge(id)));
        assert_eq!(circles(&picked), 4);
    }

    #[test]
    fn culls_offscreen_layers() {
        let mut s = scene();
        s.boundaries.clear();
        s.items[0].rect.x = 50_000.0;
        s.edges.clear();
        let shapes = painted(&s, None);
        assert_eq!(shapes.len(), 1, "background only");
    }

    #[test]
    fn skips_text_below_the_zoom_gate() {
        let mut s = scene();
        s.boundaries[0].font_size = 1;
        s.items[0].font_size = 1;
        s.edges[0].font_size = 1;
        let shapes = painted(&s, None);
        assert!(
            !shapes.iter().any(|s| matches!(s, Shape::Text(_))),
            "font_size 1 at zoom 1 is below {MIN_TEXT_PX}px"
        );
    }

    #[test]
    fn parse_and_opacify_colors() {
        assert_eq!(
            parse_color("#1a2b3c"),
            Some(Color32::from_rgb(0x1a, 0x2b, 0x3c))
        );
        assert_eq!(parse_color("1a2b3c"), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#ff00ff"), Some(Color32::from_rgb(255, 0, 255)));
        assert_eq!(with_opacity(Color32::BLACK, 50).a(), 127);
        assert_eq!(with_opacity(Color32::BLACK, 0).a(), 0);
        assert_eq!(with_opacity(Color32::BLACK, 100).a(), 255);
        assert_eq!(with_opacity(Color32::BLACK, -5).a(), 0);
    }

    #[test]
    fn every_element_shape_paints() {
        let shapes = [
            ElementShape::Box,
            ElementShape::RoundedBox,
            ElementShape::Circle,
            ElementShape::Ellipse,
            ElementShape::Hexagon,
            ElementShape::Diamond,
            ElementShape::Cylinder,
            ElementShape::Bucket,
            ElementShape::Pipe,
            ElementShape::Person,
            ElementShape::Robot,
            ElementShape::Folder,
            ElementShape::WebBrowser,
            ElementShape::Window,
            ElementShape::Terminal,
            ElementShape::Shell,
            ElementShape::MobileDevicePortrait,
            ElementShape::MobileDeviceLandscape,
            ElementShape::Component,
        ];
        let mut s = scene();
        s.boundaries.clear();
        s.edges.clear();
        for shape in shapes {
            s.items[0].shape = shape;
            let painted = painted(&s, None);
            assert!(
                painted.iter().any(
                    |p| !matches!(p, Shape::Rect(r) if r.fill == Color32::from_rgb(255, 255, 255))
                ),
                "{shape:?} must paint a body, got {} shapes",
                painted.len()
            );
        }
    }

    #[test]
    fn point_at_walks_the_arc_length() {
        let points = vec![pos2(0.0, 0.0), pos2(100.0, 0.0), pos2(100.0, 100.0)];
        assert_eq!(point_at(&points, 0.0), Some(pos2(0.0, 0.0)));
        assert_eq!(point_at(&points, 0.25), Some(pos2(50.0, 0.0)));
        assert_eq!(point_at(&points, 0.75), Some(pos2(100.0, 50.0)));
        assert_eq!(point_at(&points, 1.0), Some(pos2(100.0, 100.0)));
        assert_eq!(point_at(&[], 0.5), None);
        // Degenerate (zero total length): the single vertex.
        let flat = vec![pos2(5.0, 5.0), pos2(5.0, 5.0)];
        assert_eq!(point_at(&flat, 0.5), Some(pos2(5.0, 5.0)));
    }
}
