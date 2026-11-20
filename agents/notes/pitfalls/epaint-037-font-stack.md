# epaint 0.37 fonts: default_fonts is OFF by default; skrifa not ab_glyph

Verified 2026-11-20 against the vendored sources (`epaint-0.37.0`,
`ab_glyph-0.2.32`) while building `oxidizr-viewer/src/export.rs` (M4-5).

- epaint 0.37 `default = []` — **`default_fonts` is not a default feature**.
  A `gui` build gets the bundled Ubuntu-Light/Hack TTFs only because egui
  turns the feature on; a headless epaint-only lib gets
  `FontDefinitions::default()` with `font_data: {}` (empty!) — layout still
  "works" (galleys carry the job text, advances are placeholders), so nothing
  fails loudly. Fix: pin `epaint = { version = "0.37", features =
  ["default_fonts"] }` in the workspace.
- epaint 0.37 rasterizes glyphs via **skrifa/swash**, not ab_glyph (older
  egui used ab_glyph). Want glyph OUTLINES (e.g. SVG export where resvg is
  built without text features): add `ab_glyph = "0.2"` as a companion dep and
  feed it the SAME bytes — `FontDefinitions::default().font_data
  ["Ubuntu-Light"].bytes()`. Independent parser over the same TTF, no
  version-unification.
- `ab_glyph` `Font::outline(GlyphId)` → curves are UNSCALED, y-UP font space;
  `move_to` emits NO curve (new contour ⇒ `curve.start != previous end` —
  that is the contour boundary); `close` pushes a line back to the start.
  `GlyphId(0)` = `.notdef` → skip.
- epaint `Shape::line` is an OPEN `Path` — the `LineSegment` variant needs
  its own constructor; a Shape→SVG translator must cover both.
- `CornerRadius` fields are `nw/ne/sw/se: u8` (not left/top/…, not f32).
