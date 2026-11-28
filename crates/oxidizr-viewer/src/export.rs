//! SVG/PNG export with the key: a flat
//! [`epaint::Shape`] list → self-contained SVG → PNG, all headless
//! (the shell only orchestrates the font view and the file write).
//!
//! - **Own Shape→SVG translator.** epaint has no SVG serializer,
//!   and `paint` emits a CLOSED subset — Rect/Circle/Ellipse/LineSegment/
//!   Path(open|closed)/Text (dashes are already expanded to segments at
//!   paint time) — so this module maps exactly that subset and skips
//!   anything else (Mesh/Bézier/Band/Callback are never produced).
//! - **Text is glyph OUTLINES as paths.** The raster stack is built
//!   WITHOUT text features, so resvg cannot resolve `<text>`, and epaint
//!   renders glyphs via skrifa, not `ab_glyph`. `ab_glyph`
//!   is therefore a companion dep parsing the SAME bundled TTF bytes
//!   epaint embeds (`FontDefinitions::default.font_data`), producing
//!   quadratic path segments. Result: the SVG is self-contained (no
//!   `@font-face`, no font dependency in viewers) and PNG renders
//!   identically without fonts.
//! - **The key rides the same shape pipeline** ([`compose`]): composed
//!   BELOW the diagram, on margins of ours, reusing
//!   [`crate::key::legend`] rows — no second legend model.
//! - Export bytes are NOT goldens (visual parity is out of contract):
//!   the snapshot gate is XML well-formedness (usvg parse) +
//!   PNG magic bytes.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ab_glyph::{Font as _, FontArc, GlyphId, OutlineCurve};
use epaint::text::{FontDefinitions, FontId, FontsView};
use epaint::{
    CircleShape, Color32, ColorMode, EllipseShape, PathShape, Pos2, Rect as ERect, RectShape,
    Shape, Stroke, TextShape, Vec2, pos2,
};
use oxidizr_core::views::LineStyle;

use crate::hit::Viewport;
use crate::key::{self, Key};
use crate::paint::{self, PaintOptions};
use crate::scene::{Rect, Scene};

/// Diagram margin around the scene bounds (export px).
pub(crate) const EXPORT_MARGIN: f32 = 40.0;
/// Soft cap for the diagram side (px): big canvases shrink toward
/// [`EXPORT_MIN_SCALE`] first, and only exceed the cap when even that
/// scale would (huge canvases stay readable, at the user's file size).
pub(crate) const EXPORT_MAX_SIDE: f32 = 4096.0;
/// Floor for the fit scale: at 0.25 the default 24 px box font still
/// clears [`paint::MIN_TEXT_PX`] so text survives the export.
pub(crate) const EXPORT_MIN_SCALE: f64 = 0.25;
/// Hard pixel cap for the whole document (~80 MP): protects the PNG
/// raster from absurd canvases.
pub(crate) const EXPORT_MAX_PIXELS: f64 = 80e6;
/// Key row height (export px).
pub(crate) const KEY_ROW_H: f32 = 26.0;
/// Key swatch column x (export px).
pub(crate) const KEY_SWATCH_X: f32 = 20.0;
/// Key swatch box width (export px).
pub(crate) const KEY_SWATCH_W: f32 = 28.0;
/// Key text column x (export px).
pub(crate) const KEY_TEXT_X: f32 = 56.0;
/// Key text size (export px).
pub(crate) const KEY_TEXT_PX: f32 = 15.0;
/// Heading size (export px).
pub(crate) const KEY_HEADING_PX: f32 = 20.0;
/// Gap between the diagram bottom and the key band (export px).
pub(crate) const KEY_TOP_PAD: f32 = 36.0;

/// Export failures (values, not panics). File writing is the shell's
/// (`commands::write_atomic`), so no `Io` arm here.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The composed SVG did not parse back through `usvg` (a
    /// translator bug — the well-formedness gate).
    #[error("svg: {0}")]
    Svg(String),
    /// The raster step failed (zero size, encode error).
    #[error("png: {0}")]
    Png(String),
}

/// The composed export document: one flat shape list in FINAL document
/// coordinates plus the document size (export px).
pub struct Composed {
    /// Shapes in paint order (background first).
    pub shapes: Vec<Shape>,
    /// Document size (px) — feed to `shapes_to_svg` unchanged.
    pub size: (f64, f64),
}

/// Fit scale for `bounds`: 1:1 until [`EXPORT_MAX_SIDE`] binds, floor
/// [`EXPORT_MIN_SCALE`], then a final pixel-cap squeeze.
#[must_use]
pub(crate) fn export_scale(bounds: Rect) -> f64 {
    let max_side = bounds.width.max(bounds.height).max(1.0);
    let fit = (f64::from(EXPORT_MAX_SIDE) - 2.0 * f64::from(EXPORT_MARGIN)) / max_side;
    let mut scale = 1.0_f64.min(fit).max(EXPORT_MIN_SCALE);
    let w = bounds.width * scale + 2.0 * f64::from(EXPORT_MARGIN);
    let h = bounds.height * scale + 2.0 * f64::from(EXPORT_MARGIN);
    if w * h > EXPORT_MAX_PIXELS {
        scale = (scale * (EXPORT_MAX_PIXELS / (w * h)).sqrt()).max(0.02);
    }
    scale
}

/// Compose the whole export (diagram + key: key BELOW,
/// margins ours) through the given font view. The diagram rides
/// [`paint::paint_scene`] at a fitted [`Viewport`]; the key band reuses
/// [`crate::key::legend`] rows. The selection is NEVER painted (export
/// is the diagram, not the UI state).
pub fn compose(scene: &Scene, fonts: &mut FontsView<'_>) -> Composed {
    let scale = export_scale(scene.bounds);
    let degenerate = scene.bounds.width <= 0.0 || scene.bounds.height <= 0.0;
    let (fig_w, fig_h) = if degenerate {
        (400.0_f64, 240.0_f64)
    } else {
        (
            scene.bounds.width * scale + 2.0 * f64::from(EXPORT_MARGIN),
            scene.bounds.height * scale + 2.0 * f64::from(EXPORT_MARGIN),
        )
    };
    let viewport = if degenerate {
        Viewport::new(scale, (200.0, 120.0))
    } else {
        Viewport::new(
            scale,
            (
                f64::from(EXPORT_MARGIN) - scene.bounds.x * scale,
                f64::from(EXPORT_MARGIN) - scene.bounds.y * scale,
            ),
        )
    };
    let screen = ERect::from_min_size(
        Pos2::ZERO,
        vec2(
            to_f32(fig_w, EXPORT_MAX_SIDE),
            to_f32(fig_h, EXPORT_MAX_SIDE),
        ),
    );
    let diagram = paint::paint_scene(
        scene,
        &viewport,
        fonts,
        &PaintOptions {
            screen,
            selection: None,
            hover_edge: None,
            //: exports carry no editing affordances (selection is
            // already dropped — the route kit never paints).
            model_editing: false,
        },
    );

    let background = paint::parse_color(&scene.background).unwrap_or(Color32::WHITE);
    let ink = contrast_ink(background);
    let key = key_shapes(scene, fonts, ink);
    let Composed {
        shapes: key_shapes_out,
        size: (key_w, key_h),
    } = key;
    let width = fig_w.max(key_w + 2.0 * f64::from(KEY_SWATCH_X));
    let height = fig_h + f64::from(KEY_TOP_PAD) + key_h + f64::from(EXPORT_MARGIN);
    // `key_shapes` composes at the document origin — slide the whole
    // band into its place BELOW the diagram.
    let key_top = to_f32(fig_h + f64::from(KEY_TOP_PAD), EXPORT_MAX_SIDE);
    let mut key_shapes_out = key_shapes_out;
    for shape in &mut key_shapes_out {
        shape.translate(epaint::vec2(0.0, key_top));
    }

    let mut shapes = Vec::with_capacity(key_shapes_out.len() + diagram.len() + 2);
    shapes.push(Shape::rect_filled(
        ERect::from_min_size(
            Pos2::ZERO,
            vec2(
                to_f32(width, EXPORT_MAX_SIDE),
                to_f32(height, EXPORT_MAX_SIDE),
            ),
        ),
        0.0,
        background,
    ));
    shapes.extend(diagram);
    let divider_y = to_f32(fig_h + f64::from(KEY_TOP_PAD) / 2.0 - 10.0, EXPORT_MAX_SIDE);
    shapes.push(Shape::line(
        vec![
            pos2(KEY_SWATCH_X, divider_y),
            pos2(to_f32(width, EXPORT_MAX_SIDE) - EXPORT_MARGIN, divider_y),
        ],
        Stroke::new(1.0, ink),
    ));
    shapes.extend(key_shapes_out);
    Composed {
        shapes,
        size: (width, height),
    }
}

/// The key band, composed at the document origin BELOW the diagram:
/// heading + one row per [`crate::key::legend`] entry (element swatch +
/// `kind · shape`, edge line swatch + `style · color`, boundary swatch +
/// label). Returns the shapes + content extent (width, height).
#[must_use]
pub(crate) fn key_shapes(scene: &Scene, fonts: &mut FontsView<'_>, ink: Color32) -> Composed {
    let legend: Key = key::legend(scene);
    let mut shapes = Vec::new();
    let mut width = 0.0_f32;
    let mut y = 0.0_f32;
    push_text(
        &mut shapes,
        fonts,
        "Key",
        KEY_SWATCH_X,
        y,
        ink,
        KEY_HEADING_PX,
        &mut width,
    );
    y += KEY_ROW_H + 6.0;
    for entry in &legend.elements {
        let fill = paint::parse_color(&entry.background).unwrap_or(paint::FALLBACK_FILL);
        let stroke = paint::parse_color(&entry.stroke).unwrap_or(paint::FALLBACK_INK);
        let mut label = format!("{:?} · {:?}", entry.kind, entry.shape);
        if let Some(icon) = &entry.icon {
            label.push_str(" · ");
            label.push_str(icon);
        }
        swatch_box(&mut shapes, y, fill, stroke);
        push_text(
            &mut shapes,
            fonts,
            &label,
            KEY_TEXT_X,
            y,
            ink,
            KEY_TEXT_PX,
            &mut width,
        );
        y += KEY_ROW_H;
    }
    for entry in &legend.edges {
        let color = paint::parse_color(&entry.color).unwrap_or(paint::FALLBACK_INK);
        let style = entry.line_style;
        swatch_line(&mut shapes, y, color, style);
        push_text(
            &mut shapes,
            fonts,
            &format!("{:?} · {}", entry.line_style, entry.color),
            KEY_TEXT_X,
            y,
            ink,
            KEY_TEXT_PX,
            &mut width,
        );
        y += KEY_ROW_H;
    }
    for entry in &legend.boundaries {
        swatch_box(&mut shapes, y, Color32::TRANSPARENT, Color32::GRAY);
        push_text(
            &mut shapes,
            fonts,
            &format!(
                "boundary: {} ({:?})",
                key::boundary_label(entry.kind),
                entry.border
            ),
            KEY_TEXT_X,
            y,
            ink,
            KEY_TEXT_PX,
            &mut width,
        );
        y += KEY_ROW_H;
    }
    Composed {
        shapes,
        size: (f64::from(width) + f64::from(KEY_TEXT_X), f64::from(y)),
    }
}

/// One laid-out key text run (LEFT-TOP at `x`, centered in
/// [`KEY_ROW_H`] of the row at `row_y`), tracked into the running
/// content `width`.
#[allow(clippy::too_many_arguments)]
fn push_text(
    shapes: &mut Vec<Shape>,
    fonts: &mut FontsView<'_>,
    text: &str,
    x: f32,
    row_y: f32,
    ink: Color32,
    px: f32,
    width: &mut f32,
) {
    let galley = fonts.layout_no_wrap(text.to_owned(), FontId::proportional(px), ink);
    *width = (*width).max(galley.size().x);
    let pos = pos2(x, row_y + (KEY_ROW_H - galley.size().y) / 2.0);
    shapes.push(Shape::galley(pos, galley, Color32::PLACEHOLDER));
}

fn swatch_box(out: &mut Vec<Shape>, row_y: f32, fill: Color32, stroke: Color32) {
    let r = ERect::from_min_size(
        pos2(KEY_SWATCH_X, row_y + (KEY_ROW_H - 18.0) / 2.0),
        vec2(KEY_SWATCH_W, 18.0),
    );
    out.push(Shape::rect_filled(r, 2.0, fill));
    out.push(Shape::rect_stroke(
        r,
        2.0,
        Stroke::new(1.25, stroke),
        epaint::StrokeKind::Inside,
    ));
}

fn swatch_line(out: &mut Vec<Shape>, row_y: f32, color: Color32, style: LineStyle) {
    let start = pos2(KEY_SWATCH_X, row_y + KEY_ROW_H / 2.0);
    let end = pos2(KEY_SWATCH_X + KEY_SWATCH_W, row_y + KEY_ROW_H / 2.0);
    let stroke = Stroke::new(2.0, color);
    match style {
        LineStyle::Solid => out.push(Shape::line(vec![start, end], stroke)),
        LineStyle::Dashed => Shape::dashed_line_many(&[start, end], stroke, 8.0, 6.0, out),
        LineStyle::Dotted => Shape::dashed_line_many(&[start, end], stroke, 2.4, 4.8, out),
    }
}

/// Luma-contrast ink for a background (export key text — a legibility
/// choice of ours, not a contract).
fn contrast_ink(background: Color32) -> Color32 {
    let [r, g, b, _] = background.to_array();
    if u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 127_000 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// The SAME bundled faces the viewer paints with, as an
/// `ab_glyph` font for the outline pass. `None` only when epaint's
/// `default_fonts` feature is off (no embedded bytes) — text is then
/// skipped.
fn outline_font() -> Option<FontArc> {
    let defs = FontDefinitions::default();
    let data = defs.font_data.get("Ubuntu-Light")?;
    FontArc::try_from_vec(data.bytes().to_vec()).ok()
}

/// Translate a flat shape list (document px) into a self-contained SVG
/// document. Text becomes per-glyph outline paths (module docs).
#[must_use]
pub(crate) fn shapes_to_svg(shapes: &[Shape], size: (f64, f64)) -> String {
    let font = outline_font();
    let mut cache: BTreeMap<char, Option<GlyphBody>> = BTreeMap::new();
    let (w, h) = (n64(size.0), n64(size.1));
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\">\n"
    );
    for shape in shapes {
        shape_svg(shape, font.as_ref(), &mut cache, &mut out);
    }
    out.push_str("</svg>\n");
    out
}

/// SVG → PNG (module docs): `usvg` parse → `resvg` paint at 1:1 →
/// `tiny_skia` PNG encode.
///
/// # Errors
///
/// [`ExportError::Svg`] when the document does not parse (the
/// well-formedness gate); [`ExportError::Png`] when the raster or the
/// PNG encode fails (degenerate size).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // document px are positive (usvg sizes) and fit u32 at export sizes
pub(crate) fn svg_to_png(svg: &str) -> Result<Vec<u8>, ExportError> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| ExportError::Svg(e.to_string()))?;
    let size = tree.size();
    let (w, h) = (size.width().max(1.0), size.height().max(1.0));
    let mut pixmap = tiny_skia::Pixmap::new(w as u32, h as u32)
        .ok_or_else(|| ExportError::Png("zero-size raster".to_owned()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(1.0, 1.0),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| ExportError::Png(e.to_string()))
}

fn shape_svg(
    shape: &Shape,
    font: Option<&FontArc>,
    cache: &mut BTreeMap<char, Option<GlyphBody>>,
    out: &mut String,
) {
    match shape {
        Shape::Vec(inner) => {
            for s in inner {
                shape_svg(s, font, cache, out);
            }
        }
        Shape::Rect(r) => rect_svg(r, out),
        Shape::Circle(c) => circle_svg(c, out),
        Shape::Ellipse(e) => ellipse_svg(e, out),
        Shape::LineSegment { points, stroke } => segment_svg(*points, *stroke, out),
        Shape::Path(p) => path_svg(p, out),
        Shape::Text(t) => text_svg(t, font, cache, out),
        // `Noop` plus the shapes `paint` never emits (module docs): skip.
        Shape::Noop
        | Shape::Mesh(_)
        | Shape::QuadraticBezier(_)
        | Shape::CubicBezier(_)
        | Shape::Band(_)
        | Shape::Callback(_) => {}
    }
}

fn rect_svg(r: &RectShape, out: &mut String) {
    if !r.rect.is_finite() || (r.fill == Color32::TRANSPARENT && !visible(r.stroke)) {
        return;
    }
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
        n(r.rect.min.x),
        n(r.rect.min.y),
        n(r.rect.width()),
        n(r.rect.height())
    );
    // RoundedBox elements carry px corner radii; SVG rounds both axes
    // with one value, so the largest non-zero corner wins.
    let rx = [
        r.corner_radius.nw,
        r.corner_radius.ne,
        r.corner_radius.sw,
        r.corner_radius.se,
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    if rx > 0 {
        let _ = write!(out, " rx=\"{}\"", n(f32::from(rx)));
    }
    fill_attrs(out, r.fill);
    stroke_attrs(out, r.stroke);
    out.push_str("/>\n");
}

fn circle_svg(c: &CircleShape, out: &mut String) {
    if !(c.center.is_finite()
        && c.radius.is_finite()
        && (c.fill != Color32::TRANSPARENT || visible(c.stroke)))
    {
        return;
    }
    let _ = write!(
        out,
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
        n(c.center.x),
        n(c.center.y),
        n(c.radius)
    );
    fill_attrs(out, c.fill);
    stroke_attrs(out, c.stroke);
    out.push_str("/>\n");
}

fn ellipse_svg(e: &EllipseShape, out: &mut String) {
    if e.fill == Color32::TRANSPARENT && !visible(e.stroke) {
        return;
    }
    let _ = write!(
        out,
        "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",
        n(e.center.x),
        n(e.center.y),
        n(e.radius.x),
        n(e.radius.y)
    );
    if e.angle != 0.0 {
        let deg = e.angle * 180.0 / std::f32::consts::PI;
        let _ = write!(
            out,
            " transform=\"rotate({} {} {})\"",
            n(deg),
            n(e.center.x),
            n(e.center.y)
        );
    }
    fill_attrs(out, e.fill);
    stroke_attrs(out, e.stroke);
    out.push_str("/>\n");
}

fn segment_svg(points: [Pos2; 2], stroke: Stroke, out: &mut String) {
    if !visible(stroke) {
        return;
    }
    let _ = write!(
        out,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
        n(points[0].x),
        n(points[0].y),
        n(points[1].x),
        n(points[1].y)
    );
    stroke_attrs(out, stroke);
    out.push_str("/>\n");
}

/// One text shape → per-glyph outline `<path>`s (module docs): the
/// galley is a single-format job (`paint` lays one run per call), so
/// `sections[0].format` carries the font size and ink for every glyph.
fn text_svg(
    t: &TextShape,
    font: Option<&FontArc>,
    cache: &mut BTreeMap<char, Option<GlyphBody>>,
    out: &mut String,
) {
    let Some(font) = font else { return };
    let Some(section) = t.galley.job.sections.first() else {
        return;
    };
    let ink = section.format.color;
    if ink == Color32::TRANSPARENT {
        return;
    }
    let scale = f64::from(section.format.font_id.size);
    let ppp = f64::from(t.galley.pixels_per_point).max(f64::EPSILON);
    for row in &t.galley.rows {
        for glyph in &row.row.glyphs {
            if glyph.chr.is_whitespace() {
                continue;
            }
            let x = f64::from(t.pos.x) + (f64::from(row.pos.x) + f64::from(glyph.pos.x)) / ppp;
            let baseline =
                f64::from(t.pos.y) + (f64::from(row.pos.y) + f64::from(glyph.pos.y)) / ppp;
            let body = cache
                .entry(glyph.chr)
                .or_insert_with(|| glyph_body(font, glyph.chr))
                .clone();
            let Some(body) = body else { continue };
            out.push_str("<path d=\"");
            write_body(out, &body, scale, x, baseline);
            out.push('"');
            fill_attrs(out, ink);
            out.push_str(" fill-rule=\"evenodd\"/>\n");
        }
    }
}

fn path_svg(p: &PathShape, out: &mut String) {
    let stroke_color = match p.stroke.color {
        ColorMode::Solid(c) => c,
        ColorMode::UV(_) => Color32::TRANSPARENT,
    };
    let stroke_visible = p.stroke.width > 0.0 && stroke_color != Color32::TRANSPARENT;
    if p.points.len() < 2 || (p.fill == Color32::TRANSPARENT && !stroke_visible) {
        return;
    }
    if !p.points.iter().all(|pt| pt.is_finite()) {
        return;
    }
    let tag = if p.closed { "polygon" } else { "polyline" };
    let _ = write!(out, "<{tag} points=\"");
    for (i, pt) in p.points.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&n(pt.x));
        out.push(',');
        out.push_str(&n(pt.y));
    }
    out.push('"');
    fill_attrs(
        out,
        if p.closed {
            p.fill
        } else {
            Color32::TRANSPARENT
        },
    );
    if stroke_visible {
        stroke_attrs(
            out,
            Stroke {
                width: p.stroke.width,
                color: stroke_color,
            },
        );
    }
    out.push_str("/>\n");
}

/// A glyph outline in UNSCALED font-space (y up): commands and their
/// absolute points — cached per char, formatted per placement.
type GlyphBody = Vec<(&'static str, Vec<(f32, f32)>)>;

/// `ab_glyph`'s `Point` → our tuple.
fn pt(p: ab_glyph::Point) -> (f32, f32) {
    (p.x, p.y)
}

/// One glyph's outline via `ab_glyph` (the bundled TTF; module docs).
/// `.notdef` (id 0), whitespace and empty outlines are `None`.
fn glyph_body(font: &FontArc, chr: char) -> Option<GlyphBody> {
    let id = font.glyph_id(chr);
    if id == GlyphId(0) {
        return None;
    }
    let outline = font.outline(id)?;
    let mut body: GlyphBody = Vec::new();
    let mut last: Option<(f32, f32)> = None;
    for curve in &outline.curves {
        let (start, cmd, controls) = match curve {
            OutlineCurve::Line(a, b) => (pt(*a), "L", vec![pt(*b)]),
            OutlineCurve::Quad(a, c, b) => (pt(*a), "Q", vec![pt(*c), pt(*b)]),
            OutlineCurve::Cubic(a, c1, c2, b) => (pt(*a), "C", vec![pt(*c1), pt(*c2), pt(*b)]),
        };
        // `move_to` emits NO curve (new contour), so a start that does
        // not continue the previous end opens an explicit `M`.
        if last != Some(start) {
            body.push(("M", vec![start]));
        }
        let end = controls.last().copied().unwrap_or(start);
        body.push((cmd, controls));
        last = Some(end);
    }
    (!body.is_empty()).then_some(body)
}

/// Font-space (y up) → document px (y down, baseline at `baseline`).
fn write_body(out: &mut String, body: &GlyphBody, scale: f64, x: f64, baseline: f64) {
    let point = |out: &mut String, &(fx, fy): &(f32, f32)| {
        out.push_str(&n64(x + f64::from(fx) * scale));
        out.push(' ');
        out.push_str(&n64(baseline - f64::from(fy) * scale));
    };
    for (cmd, pts) in body {
        out.push_str(cmd);
        out.push(' ');
        for (i, pt) in pts.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            point(out, pt);
        }
        out.push(' ');
    }
    out.push('Z');
}

fn visible(stroke: Stroke) -> bool {
    stroke.width > 0.0 && stroke.color != Color32::TRANSPARENT
}

fn fill_attrs(out: &mut String, fill: Color32) {
    if fill == Color32::TRANSPARENT {
        out.push_str(" fill=\"none\"");
        return;
    }
    let _ = write!(out, " fill=\"{}\"", hex(fill));
    if fill.a() < 255 {
        let _ = write!(out, " fill-opacity=\"{:.3}\"", f32::from(fill.a()) / 255.0);
    }
}

fn stroke_attrs(out: &mut String, stroke: Stroke) {
    if !visible(stroke) {
        return;
    }
    let _ = write!(
        out,
        " stroke=\"{}\" stroke-width=\"{}\"",
        hex(stroke.color),
        n(stroke.width)
    );
    if stroke.color.a() < 255 {
        let _ = write!(
            out,
            " stroke-opacity=\"{:.3}\"",
            f32::from(stroke.color.a()) / 255.0
        );
    }
}

fn hex(color: Color32) -> String {
    let [r, g, b, _] = color.to_array();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Compact number: integers lose the fraction, else two decimals.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // rounding at 2 dp is the intent
fn n(v: f32) -> String {
    if v.is_finite() && (f64::from(v) - f64::from(v).round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

/// [`n`] for f64 document coordinates.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // rounding at 2 dp is the intent
fn n64(v: f64) -> String {
    if v.is_finite() && (v - v.round()).abs() < 0.005 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// Saturating f64→f32 for document geometry: beyond the f32 range (an
/// absurd canvas) the `fallback` cap keeps the document drawable.
#[allow(clippy::cast_possible_truncation)] // in-range cast is the intent
fn to_f32(v: f64, fallback: f32) -> f32 {
    if v.is_finite() && v <= f64::from(f32::MAX) {
        v as f32
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Boundary, BoundaryKind, Edge, Item};
    use epaint::text::{FontDefinitions, Fonts, TextOptions};
    use oxidizr_core::Id;
    use oxidizr_core::validate::ElementKind;
    use oxidizr_core::views::{IconPosition, Shape as ElementShape};

    fn fonts() -> Fonts {
        Fonts::new(TextOptions::default(), FontDefinitions::default())
    }

    fn text_shape(text: &str, at: Pos2) -> Shape {
        let mut fonts = fonts();
        let galley = fonts.with_pixels_per_point(1.0).layout_no_wrap(
            text.to_owned(),
            FontId::proportional(20.0),
            Color32::BLACK,
        );
        Shape::galley(at, galley, Color32::PLACEHOLDER)
    }

    fn wr(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn item(id: &str, x: f64, y: f64) -> Item {
        Item {
            id: Id::new(id),
            kind: ElementKind::Person,
            name: format!("Box {id}"),
            description: None,
            metadata: None,
            group: None,
            url: None,
            rect: wr(x, y, 350.0, 150.0),
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

    fn scene() -> Scene {
        Scene {
            view_key: "v".to_string(),
            title: "V".to_string(),
            background: "#ffffff".to_string(),
            boundaries: vec![Boundary {
                kind: BoundaryKind::Group,
                id: "group:g".to_string(),
                label: "g".to_string(),
                metadata: None,
                rect: wr(0.0, 0.0, 1000.0, 600.0),
                members: vec![Id::new("1"), Id::new("2")],
                background: "#fafafa".to_string(),
                stroke: "#c0c0c0".to_string(),
                stroke_width: 2,
                border: LineStyle::Solid,
                color: "#000000".to_string(),
                font_size: 24,
                opacity: 100,
            }],
            items: vec![item("1", 50.0, 50.0), item("2", 500.0, 300.0)],
            edges: vec![Edge {
                id: Id::new("e"),
                source: Id::new("1"),
                destination: Id::new("2"),
                points: vec![(225.0, 125.0), (675.0, 375.0)],
                label: Some("Uses".to_string()),
                metadata: Some("[HTTPS]".to_string()),
                order: None,
                color: "#333333".to_string(),
                thickness: 2,
                line_style: LineStyle::Dashed,
                font_size: 14,
                label_width: 200,
                label_position: 50,
                opacity: 100,
                url: None,
            }],
            bounds: wr(0.0, 0.0, 1000.0, 600.0),
        }
    }

    #[test]
    fn svg_round_trips_through_the_parser() {
        let shapes = vec![
            Shape::rect_filled(
                ERect::from_min_size(pos2(1.0, 2.0), vec2(30.0, 20.0)),
                0.0,
                Color32::RED,
            ),
            Shape::circle_filled(pos2(50.0, 50.0), 10.0, Color32::BLUE),
            // `Shape::line` is an OPEN Path → <polyline>; <line> needs
            // the LineSegment variant (paint never emits it — covered
            // here so the translator branch stays honest).
            Shape::LineSegment {
                points: [pos2(0.0, 0.0), pos2(10.0, 10.0)],
                stroke: Stroke::new(2.0, Color32::GREEN),
            },
            Shape::line(
                vec![pos2(0.0, 5.0), pos2(10.0, 5.0)],
                Stroke::new(2.0, Color32::GREEN),
            ),
            Shape::closed_line(
                vec![pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(5.0, 8.0)],
                Stroke::new(1.0, Color32::BLACK),
            ),
        ];
        let svg = shapes_to_svg(&shapes, (100.0, 100.0));
        assert_eq!(svg.matches("<rect").count(), 1);
        assert_eq!(svg.matches("<circle").count(), 1);
        assert_eq!(svg.matches("<line ").count(), 1);
        assert_eq!(svg.matches("<polyline").count(), 1);
        assert_eq!(svg.matches("<polygon").count(), 1);
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("well-formed SVG");
    }

    #[test]
    fn text_becomes_glyph_outline_paths() {
        let svg = shapes_to_svg(&[text_shape("Ab", pos2(5.0, 5.0))], (120.0, 40.0));
        assert!(svg.contains("<path d=\"M"), "glyph outlines: {svg}");
        assert!(svg.contains("fill-rule=\"evenodd\""));
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("well-formed SVG");
    }

    #[test]
    fn png_carries_the_magic_bytes() {
        let svg = shapes_to_svg(
            &[Shape::rect_filled(
                ERect::from_min_size(Pos2::ZERO, vec2(8.0, 6.0)),
                0.0,
                Color32::RED,
            )],
            (8.0, 6.0),
        );
        let png = svg_to_png(&svg).expect("png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn png_gate_rejects_broken_svg() {
        assert!(matches!(
            svg_to_png("<svg><bogus"),
            Err(ExportError::Svg(_))
        ));
    }

    #[test]
    fn compose_places_diagram_key_and_text() {
        let scene = scene();
        let mut fonts = fonts();
        let composed = compose(&scene, &mut fonts.with_pixels_per_point(1.0));
        assert!(composed.size.0 > 0.0 && composed.size.1 > composed.size.0 * 0.5);
        let svg = shapes_to_svg(&composed.shapes, composed.size);
        usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("composed SVG parses");
        // The key rides the diagram: heading + element/edge/boundary rows
        // appear as glyph outlines; the boundary row is the textual tell.
        assert!(svg.contains("fill-rule=\"evenodd\""), "text present");
        // Layout guard: the key band sits BELOW the figure — some text
        // shapes must start past the diagram's bottom edge (the compose
        // translate, not the key_shapes origin).
        let fig_bottom = to_f32(
            scene.bounds.height * export_scale(scene.bounds) + 2.0 * f64::from(EXPORT_MARGIN),
            EXPORT_MAX_SIDE,
        );
        let below = composed
            .shapes
            .iter()
            .filter(|s| matches!(s, Shape::Text(t) if t.pos.y > fig_bottom))
            .count();
        assert!(below > 0, "key text placed below the diagram");
    }

    #[test]
    fn outline_font_maps_real_glyphs() {
        let font = outline_font().expect("bundled font");
        assert_ne!(font.glyph_id('A'), GlyphId(0));
        assert!(glyph_body(&font, 'A').is_some_and(|b| !b.is_empty()));
        assert!(glyph_body(&font, ' ').is_none(), "whitespace never draws");
    }
}
