//! Style `icon` rasters and the shared icon atlas (P3-M1-5, §P3-D7).
//!
//! Headless (P3-D1): every decode and rasterization happens here — the
//! shell only uploads the finished RGBA buffer as ONE texture and draws
//! atlas cells. Sources are exactly what D10 lets the model store:
//!
//! - `data:` URIs (the DSL embeds `icon <file>` values as data URIs):
//!   base64 or percent-encoded payloads, sniffed by content;
//! - bare file paths (JSON workspaces may keep raw paths): read relative
//!   to the workspace file's directory ([`build_atlas`]'s `base_dir`);
//! - `http(s)` URLs — never fetched (offline viewer, the D12 zero-network
//!   posture); the caller keeps the dashed [`crate::paint::icon_slot`]
//!   placeholder and surfaces the [`IconError`] as a warning diagnostic;
//! - anything that sniffs neither PNG nor SVG →
//!   [`IconError::Unsupported`].
//!
//! Rasterization rules (§P3-D7, seam recorded on the §P3-M1 M1-5 row):
//!
//! - PNGs decode to RGBA8 through `png` (16-bit stripped, palettes
//!   expanded); they keep their native size until atlas packing, which
//!   down-scales oversized cells with a box filter;
//! - foreign SVGs go usvg → resvg → tiny-skia (the pinned 0.50/0.50/0.12
//!   trio — tiny-skia 0.12 removed the old usvg `Renderer`, resvg is the
//!   sanctioned seam) and rasterize fitted into an [`ICON_RASTER_PX`]
//!   box;
//! - SVGs containing text render text-less ON PURPOSE: the raster stack
//!   carries no font features and no fontdb surface (§P3-D7 — "the
//!   reference never loads web fonts at all"; V3 CLOSED verified the
//!   bundled themes contain zero SVG-with-text icons).
//!
//! The whole visible icon set packs into a single [`Atlas`] (RGBA8,
//! shelf-packed, keyed `icon × size × scheme` per §P3-D7 — icons are
//! never re-tinted by scheme today, the key slot is reserved). Entries
//! that failed rasterization are absent from [`Atlas::entries`] (the
//! placeholder stands in) and reported once each in [`Atlas::errors`].

use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use oxidizr_core::views::ColorScheme;

/// Rasterization box for SVG icons, and the down-scale cap the atlas
/// applies to oversized rasters (px).
pub const ICON_RASTER_PX: u32 = 64;

/// Shelf-width cap of the atlas (icons are tens; this only bounds junk).
const ATLAS_MAX_WIDTH: u32 = 1024;
/// Height cap — beyond it remaining icons report an "atlas full" error.
const ATLAS_MAX_HEIGHT: u32 = 2048;
/// Gap kept at the right/bottom edge of every cell (0 is fine: the
/// shell insets its uv rects by half a pixel against LINEAR bleed).
const ATLAS_GAP: u32 = 1;

/// Why one icon could not be rasterized.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IconError {
    /// The icon string is blank.
    #[error("icon is empty")]
    Empty,
    /// Format posture rejects the source (remote URL, foreign bytes).
    #[error("icon is not supported: {0}")]
    Unsupported(&'static str),
    /// A file-path icon could not be read.
    #[error("icon file could not be read: {0}")]
    Read(String),
    /// The `data:` URI cannot be split or its payload decoded.
    #[error("icon data URI is malformed")]
    BadDataUri,
    /// A recognized container (PNG magic / `<svg`) failed to decode.
    #[error("icon could not be decoded: {0}")]
    Decode(String),
}

/// One decoded RGBA8 raster (straight, non-premultiplied alpha).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA8 bytes (`width × height × 4`).
    pub rgba: Vec<u8>,
}

/// Decode and rasterize one style `icon` value. `base_dir` resolves
/// relative file paths (the workspace file's directory); `None` treats
/// them as CWD-relative.
///
/// # Errors
///
/// [`IconError`] variant matching the failure: blank string, rejected
/// source (remote URL, foreign bytes), unreadable file, malformed data
/// URI, or a PNG/SVG container that fails to decode.
pub fn rasterize(icon: &str, base_dir: Option<&Path>) -> Result<Raster, IconError> {
    let bytes = payload(icon, base_dir)?;
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        decode_png(&bytes)
    } else if looks_like_svg(&bytes) {
        rasterize_svg(&bytes, base_dir)
    } else {
        Err(IconError::Unsupported("icon bytes are neither PNG nor SVG"))
    }
}

/// Discriminator of a [`ColorScheme`] inside an [`IconKey`].
#[must_use]
pub const fn scheme_code(scheme: ColorScheme) -> u8 {
    match scheme {
        ColorScheme::Light => 0,
        ColorScheme::Dark => 1,
    }
}

/// Atlas cell request: icon text × rasterization box × color scheme
/// (the §P3-D7 atlas key). Ord — the atlas packs in this order, so the
/// packing is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IconKey {
    /// Style `icon` value, stored verbatim (D10).
    pub icon: String,
    /// Rasterization box edge (`ICON_RASTER_PX` in the viewer).
    pub size: u32,
    /// [`scheme_code`] of the active scheme — reserved, icons are never
    /// re-tinted today.
    pub scheme: u8,
}

/// Placement of one icon inside the atlas, in atlas pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasEntry {
    /// Cell origin x.
    pub x: u32,
    /// Cell origin y.
    pub y: u32,
    /// Cell width.
    pub width: u32,
    /// Cell height.
    pub height: u32,
}

/// One RGBA8 texture holding every rasterized icon of the current scene.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Atlas {
    /// Texture width (`0` when empty).
    pub width: u32,
    /// Texture height (`0` when empty).
    pub height: u32,
    /// Row-major RGBA8 bytes (`width × height × 4`).
    pub pixels: Vec<u8>,
    /// Packed cells, atlas-key order.
    pub entries: BTreeMap<IconKey, AtlasEntry>,
    /// `(icon, message)` of every icon that failed, one row per distinct
    /// icon string — the shell surfaces these as warning diagnostics.
    pub errors: Vec<(String, String)>,
}

impl Atlas {
    /// Cell of one icon (`None` = absent: never requested or failed).
    #[must_use]
    pub fn entry(&self, icon: &str, size: u32, scheme: ColorScheme) -> Option<AtlasEntry> {
        self.entries
            .get(&IconKey {
                icon: icon.to_owned(),
                size,
                scheme: scheme_code(scheme),
            })
            .copied()
    }
}

/// Rasterize and pack `keys` into one [`Atlas`]. Duplicate keys collapse
/// (the shell passes a set); iteration is sorted, so two calls on equal
/// input produce byte-identical atlases. A failing icon never aborts the
/// atlas — it lands in [`Atlas::errors`] once and its cell is skipped.
pub fn build_atlas(keys: impl IntoIterator<Item = IconKey>, base_dir: Option<&Path>) -> Atlas {
    let mut atlas = Atlas::default();
    let mut placed: Vec<(IconKey, Raster, u32, u32)> = Vec::new();
    let mut failed: BTreeSet<String> = BTreeSet::new();
    let (mut x, mut y, mut shelf) = (ATLAS_GAP, ATLAS_GAP, 0_u32);
    for key in BTreeSet::from_iter(keys) {
        let raster = match rasterize(&key.icon, base_dir).map(|r| fit(&r, key.size)) {
            Ok(raster) => raster,
            Err(error) => {
                if failed.insert(key.icon.clone()) {
                    atlas.errors.push((key.icon, error.to_string()));
                }
                continue;
            }
        };
        if raster.width == 0 || raster.height == 0 {
            if failed.insert(key.icon.clone()) {
                atlas
                    .errors
                    .push((key.icon, "rasterized to zero px".to_owned()));
            }
            continue;
        }
        if x > ATLAS_GAP && x + raster.width + ATLAS_GAP > ATLAS_MAX_WIDTH {
            x = ATLAS_GAP;
            y += shelf + ATLAS_GAP;
            shelf = 0;
        }
        if y + raster.height + ATLAS_GAP > ATLAS_MAX_HEIGHT {
            if failed.insert(key.icon.clone()) {
                atlas
                    .errors
                    .push((key.icon, "icon atlas is full".to_owned()));
            }
            continue;
        }
        // Advance the shelf cursor BEFORE `raster` moves into `placed`.
        let (cell_w, cell_h) = (raster.width, raster.height);
        placed.push((key, raster, x, y));
        x += cell_w + ATLAS_GAP;
        shelf = shelf.max(cell_h);
    }
    atlas.width = placed
        .iter()
        .map(|(_, raster, px, _)| px + raster.width + ATLAS_GAP)
        .max()
        .unwrap_or(0);
    atlas.height = placed
        .iter()
        .map(|(_, raster, _, py)| py + raster.height + ATLAS_GAP)
        .max()
        .unwrap_or(0);
    let area = usize::try_from(atlas.width.saturating_mul(atlas.height).saturating_mul(4));
    atlas.pixels = vec![0_u8; area.unwrap_or(0)];
    for (key, raster, px, py) in placed {
        blit(&mut atlas.pixels, atlas.width, &raster, px, py);
        atlas.entries.insert(
            key,
            AtlasEntry {
                x: px,
                y: py,
                width: raster.width,
                height: raster.height,
            },
        );
    }
    atlas
}

/// Down-scale `raster` into a `max`-box (no-op when it already fits).
fn fit(raster: &Raster, max: u32) -> Raster {
    if max == 0 || (raster.width <= max && raster.height <= max) {
        return raster.clone();
    }
    let (tw, th) = fit_within(raster.width, raster.height, max);
    Raster {
        width: tw,
        height: th,
        rgba: box_scale(&raster.rgba, raster.width, raster.height, tw, th),
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
// Icon geometry: dims ≤ 64k, aspect ratios sane — rounding is exact
// enough for a slot no bigger than 64 px; results are `.max(1.0)`-ed,
// so the float→u32 cast can never see a negative.
fn fit_within(w: u32, h: u32, max: u32) -> (u32, u32) {
    let scale = max as f32 / w.max(h) as f32;
    (
        (w as f32 * scale).round().max(1.0) as u32,
        (h as f32 * scale).round().max(1.0) as u32,
    )
}

/// Integer box-average down-scale of an RGBA8 buffer.
fn box_scale(rgba: &[u8], w: u32, h: u32, nw: u32, nh: u32) -> Vec<u8> {
    let (Some(w), Some(h), Some(nw), Some(nh)) = (
        usize::try_from(w).ok(),
        usize::try_from(h).ok(),
        usize::try_from(nw).ok(),
        usize::try_from(nh).ok(),
    ) else {
        return Vec::new();
    };
    let mut out = vec![0_u8; nw * nh * 4];
    for oy in 0..nh {
        let y0 = oy * h / nh;
        let y1 = ((oy + 1) * h / nh).max(y0 + 1);
        for ox in 0..nw {
            let x0 = ox * w / nw;
            let x1 = ((ox + 1) * w / nw).max(x0 + 1);
            let mut acc = [0_u32; 4];
            let mut n = 0_u32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    if let Some(px) = slice_at(rgba, (sy * w + sx) * 4, 4) {
                        for (a, p) in acc.iter_mut().zip(px) {
                            *a += u32::from(*p);
                        }
                    }
                    n += 1;
                }
            }
            let n = n.max(1);
            if let Some(dst) = out.get_mut((oy * nw + ox) * 4..(oy * nw + ox) * 4 + 4) {
                for (d, a) in dst.iter_mut().zip(acc) {
                    *d = u8::try_from(a / n).unwrap_or(u8::MAX);
                }
            }
        }
    }
    out
}

/// Copy `raster` into the atlas buffer at `(px, py)`.
fn blit(out: &mut [u8], out_width: u32, raster: &Raster, px: u32, py: u32) {
    let (Some(w), Some(h), Some(x), Some(y), Some(width)) = (
        usize::try_from(raster.width).ok(),
        usize::try_from(raster.height).ok(),
        usize::try_from(px).ok(),
        usize::try_from(py).ok(),
        usize::try_from(out_width).ok(),
    ) else {
        return;
    };
    for line in 0..h {
        let Some(src) = slice_at(&raster.rgba, line * w * 4, w * 4) else {
            continue;
        };
        if let Some(slot) = out.get_mut((y + line) * width * 4 + x * 4..) {
            if let Some(dst) = slot.get_mut(..w * 4) {
                dst.copy_from_slice(src);
            }
        }
    }
}

/// `data.get(start..start+len)` without overflowing `start + len`.
fn slice_at(data: &[u8], start: usize, len: usize) -> Option<&[u8]> {
    start.checked_add(len).and_then(|end| data.get(start..end))
}

/// Icon string → raw image bytes (D10 storage forms, see module docs).
fn payload(icon: &str, base_dir: Option<&Path>) -> Result<Vec<u8>, IconError> {
    let text = icon.trim();
    if text.is_empty() {
        return Err(IconError::Empty);
    }
    if let Some(rest) = text.strip_prefix("data:") {
        let (header, body) = rest.split_once(',').ok_or(IconError::BadDataUri)?;
        if header.contains(";base64") {
            return base64::engine::general_purpose::STANDARD
                .decode(body.trim())
                .map_err(|e| IconError::Decode(format!("base64: {e}")));
        }
        return Ok(percent_decode(body));
    }
    if text.starts_with("http://") || text.starts_with("https://") {
        return Err(IconError::Unsupported(
            "remote icon URLs are never fetched (offline viewer)",
        ));
    }
    let path: PathBuf = match base_dir {
        Some(dir) if !Path::new(text).is_absolute() => dir.join(text),
        _ => PathBuf::from(text),
    };
    std::fs::read(&path).map_err(|e| IconError::Read(format!("{}: {e}", path.display())))
}

/// Percent-decode a `data:` payload (subset: `%XX` hex escapes).
fn percent_decode(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let hex = |c: u8| match c {
        b'0'..=b'9' => Some(u16::from(c - b'0')),
        b'a'..=b'f' => Some(u16::from(c - b'a') + 10),
        b'A'..=b'F' => Some(u16::from(c - b'A') + 10),
        _ => None,
    };
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(u8::try_from(hi * 16 + lo).unwrap_or(b'%'));
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Whitespace/BOM-skipped `<svg` or `<?xml` prefix.
fn looks_like_svg(bytes: &[u8]) -> bool {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace() && *b != 0xEF && *b != 0xBB && *b != 0xBF)
        .unwrap_or(bytes.len());
    let head = &bytes[start..];
    head.starts_with(b"<svg") || head.starts_with(b"<?xml")
}

/// PNG magic → RGBA8, via `EXPAND | ALPHA | STRIP_16` (palettes and
/// low-bit depths expanded, 16-bit stripped); RGB/Gray8 outputs (truecolor
/// never gains alpha) get opaque/replicated alpha here.
fn decode_png(bytes: &[u8]) -> Result<Raster, IconError> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::ALPHA | png::Transformations::EXPAND | png::Transformations::STRIP_16,
    );
    let mut reader = decoder
        .read_info()
        .map_err(|e| IconError::Decode(format!("png: {e}")))?;
    let Some(len) = reader.output_buffer_size() else {
        return Err(IconError::Decode("png: unknown output size".to_owned()));
    };
    let mut buf = vec![0_u8; len];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| IconError::Decode(format!("png: {e}")))?;
    let data = slice_at(&buf, 0, info.buffer_size()).unwrap_or(&[]);
    let rgba = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => data.to_vec(),
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            let mut out = Vec::with_capacity(data.len() / 3 * 4);
            for px in data.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], u8::MAX]);
            }
            out
        }
        (png::ColorType::Grayscale, png::BitDepth::Eight) => {
            let mut out = Vec::with_capacity(data.len() * 4);
            for &g in data {
                out.extend_from_slice(&[g, g, g, u8::MAX]);
            }
            out
        }
        (color, depth) => {
            return Err(IconError::Decode(format!(
                "png: unsupported {color:?}/{depth:?} after transforms"
            )));
        }
    };
    let expected = usize::try_from(u64::from(info.width) * u64::from(info.height) * 4);
    if expected.ok() != Some(rgba.len()) {
        return Err(IconError::Decode(
            "png: unexpected output length".to_owned(),
        ));
    }
    Ok(Raster {
        width: info.width,
        height: info.height,
        rgba,
    })
}

/// SVG bytes → RGBA8 fitted into [`ICON_RASTER_PX`] (usvg parse →
/// resvg paint → demultiplied tiny-skia pixmap). `resources_dir` lets a
/// file-path SVG reference siblings.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
// Icon geometry: the raster box is 64 px; source sizes are checked
// finite and > 0 above and the results `.max(1.0)`-rounded — the
// float→u32 casts cannot see a negative.
fn rasterize_svg(bytes: &[u8], base_dir: Option<&Path>) -> Result<Raster, IconError> {
    let options = usvg::Options {
        resources_dir: base_dir.map(PathBuf::from),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options)
        .map_err(|e| IconError::Decode(format!("svg: {e}")))?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return Err(IconError::Decode("svg: zero-size viewport".to_owned()));
    }
    let scale = ICON_RASTER_PX as f32 / w.max(h);
    let tw = (w * scale).max(1.0).round() as u32;
    let th = (h * scale).max(1.0).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(tw, th)
        .ok_or_else(|| IconError::Decode("svg: zero-size pixmap".to_owned()))?;
    // `render` takes `&mut PixmapMut`; `as_mut()` hands out one by value.
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(Raster {
        width: tw,
        height: th,
        rgba: pixmap.take_demultiplied(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Deterministic test scheme codes for the icon key (Light).
    const SCHEME: u8 = 0;

    fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(rgba)
                .unwrap();
        }
        out
    }

    fn png_uri(bytes: &[u8]) -> String {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    const RED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect x="10" y="10" width="80" height="80" fill="#ff0000"/></svg>"##;

    fn pixel(r: &Raster, x: u32, y: u32) -> [u8; 4] {
        let i = usize::try_from((y * r.width + x) * 4).unwrap();
        r.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn png_data_uri_decodes_verbatim() {
        let rgba = vec![255, 0, 0, 255, 0, 255, 0, 200, 0, 0, 255, 128, 0, 0, 0, 0];
        let icon = png_uri(&encode_png(2, 2, &rgba));
        let raster = rasterize(&icon, None).unwrap();
        assert_eq!((raster.width, raster.height), (2, 2));
        assert_eq!(raster.rgba, rgba);
    }

    #[test]
    fn svg_data_uri_rasterizes_shape() {
        let svg_uri = format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(RED_SVG)
        );
        let raster = rasterize(&svg_uri, None).unwrap();
        assert_eq!((raster.width, raster.height), (64, 64));
        let center = pixel(&raster, 32, 32);
        assert!(
            center[0] > 200 && center[1] < 60 && center[3] == 255,
            "{center:?}"
        );
        // Outside the rect (viewBox corner): fully transparent.
        assert_eq!(pixel(&raster, 1, 1)[3], 0);
    }

    #[test]
    fn svg_with_text_still_rasterizes() {
        // The §P3-D7 limitation: text is skipped (no font stack), the
        // shapes around it render.
        let svg = RED_SVG.replace(
            "</svg>",
            "<text x=\"50\" y=\"50\" font-size=\"40\">hi</text></svg>",
        );
        let uri = format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(svg)
        );
        let raster = rasterize(&uri, None).unwrap();
        assert!(
            pixel(&raster, 32, 32)[3] > 0,
            "shape survives text skipping"
        );
    }

    #[test]
    fn percent_payloads_and_rejects() {
        assert_eq!(percent_decode("a%20b%3Cb%3E%41%zz"), b"a b<b>A%zz");
        // Neither PNG nor SVG magic ("GIF89a" magic in base64).
        assert_eq!(
            rasterize("data:image/gif;base64,R0lGODlh", None),
            Err(IconError::Unsupported("icon bytes are neither PNG nor SVG"))
        );
        // Broken XML: recognized as SVG, fails to parse.
        assert!(matches!(
            rasterize("data:image/svg+xml,%3Csvg%3E%3Cbroken", None),
            Err(IconError::Decode(_))
        ));
        // Broken base64.
        assert!(matches!(
            rasterize("data:image/png;base64,!!!!not b64!!!!", None),
            Err(IconError::Decode(_))
        ));
        // Malformed URI, blank icon, remote URL never fetched.
        assert_eq!(
            rasterize("data:image/png", None),
            Err(IconError::BadDataUri)
        );
        assert_eq!(rasterize("   ", None), Err(IconError::Empty));
        assert!(matches!(
            rasterize("https://example.com/icon.png", None),
            Err(IconError::Unsupported(_))
        ));
    }

    #[test]
    fn file_paths_resolve_against_base_dir() {
        let dir = std::env::temp_dir().join(format!("oxidizr-icons-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("icon.png"),
            encode_png(2, 2, &[1, 2, 3, 4].repeat(4)),
        )
        .unwrap();
        let raster = rasterize("icon.png", Some(&dir)).unwrap();
        assert_eq!((raster.width, raster.height), (2, 2));
        // Without a base dir the CWD has no such file.
        assert!(matches!(
            rasterize("definitely-missing-icon.png", None),
            Err(IconError::Read(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atlas_packs_dedupes_and_reports() {
        let svg_uri = format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(RED_SVG)
        );
        let png_uri = png_uri(&encode_png(2, 2, &[7, 7, 7, 255].repeat(4)));
        let key = |icon: &str| IconKey {
            icon: icon.to_owned(),
            size: ICON_RASTER_PX,
            scheme: SCHEME,
        };
        let keys = vec![
            key(&svg_uri),
            key(&svg_uri), // duplicate collapses
            key(&png_uri),
            key("https://example.com/i.png"),
        ];
        let atlas = build_atlas(keys.clone(), None);
        assert_eq!(atlas.entries.len(), 2);
        assert_eq!(atlas.errors.len(), 1);
        assert!(atlas.errors[0].0.starts_with("https://"));
        assert_eq!(
            usize::try_from(atlas.width * atlas.height * 4).unwrap(),
            atlas.pixels.len()
        );
        assert_eq!(
            atlas
                .entry(&svg_uri, ICON_RASTER_PX, ColorScheme::Light)
                .unwrap()
                .width,
            64
        );
        assert!(
            atlas
                .entry(&svg_uri, ICON_RASTER_PX, ColorScheme::Dark)
                .is_none()
        );
        // The PNG cell carries the decoded pixels.
        let png = atlas
            .entry(&png_uri, ICON_RASTER_PX, ColorScheme::Light)
            .unwrap();
        let i = usize::try_from((png.y * atlas.width + png.x) * 4).unwrap();
        assert_eq!(&atlas.pixels[i..i + 4], &[7, 7, 7, 255]);
        // Determinism: same input, same atlas.
        assert_eq!(build_atlas(keys, None), atlas);
    }

    #[test]
    fn atlas_downscales_oversized_png() {
        let big = encode_png(128, 128, &[3, 4, 5, 255].repeat(128 * 128));
        let uri = png_uri(&big);
        let atlas = build_atlas(
            [IconKey {
                icon: uri.clone(),
                size: ICON_RASTER_PX,
                scheme: SCHEME,
            }],
            None,
        );
        let entry = atlas
            .entry(&uri, ICON_RASTER_PX, ColorScheme::Light)
            .unwrap();
        assert_eq!((entry.width, entry.height), (64, 64));
        let i = usize::try_from((entry.y * atlas.width + entry.x) * 4).unwrap();
        assert_eq!(&atlas.pixels[i..i + 4], &[3, 4, 5, 255]);
    }

    #[test]
    fn empty_atlas_is_inert() {
        let atlas = build_atlas([], None);
        assert_eq!((atlas.width, atlas.height), (0, 0));
        assert!(atlas.pixels.is_empty() && atlas.entries.is_empty() && atlas.errors.is_empty());
    }
}
