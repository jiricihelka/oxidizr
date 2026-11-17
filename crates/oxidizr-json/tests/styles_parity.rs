//! Style-array parity against the reference JSON round-trip (Phase 2 P2-V1
//! oracle probes, 2026-11-17 — witnesses `/tmp/nix-shell.uni100/p2/v1/`):
//!
//! * Jackson binds `Styles.elements`/`relationships` by FIELD (no setter), so
//!   the reference JSON reader keeps DOCUMENT order there — unlike every other
//!   collection, whose package-private setter re-wraps into a `TreeSet`
//!   (WRITE-CATALOG §5). The writer must therefore emit styles in model order.
//! * numeric style fields pass through the `JavaBean` setters, which CLAMP on
//!   read (§6 "read normalizes"): `strokeWidth` → [1, 10], `opacity` → [0,
//!   100] (both styles), relationship `position` → [0, 100]; `width`/`height`/
//!   `fontSize`/`thickness` are NOT clamped.

use oxidizr_json::{from_slice, to_vec};

fn workspace(styles_json: &str) -> oxidizr_core::Workspace {
    let json = format!(
        r#"{{ "configuration": {{ "name": "Probe" }}, "views": {{ "configuration": {{ "styles": {styles_json} }} }} }}"#
    );
    from_slice(json.as_bytes()).expect("probe workspace reads")
}

#[test]
fn style_arrays_keep_document_order_through_a_round_trip() {
    // Reference witness: out5/order.json — [Alpha(Dark), Zeta, Mid] in, the
    // same order out (a `TreeSet` would have yielded Mid, Zeta, Alpha(Dark)).
    let ws = workspace(
        r##"{ "elements": [
               { "tag": "Alpha", "colorScheme": "Dark", "background": "#ff0000" },
               { "tag": "Zeta", "background": "#000000" },
               { "tag": "Mid", "background": "#00ff00" }
           ] }"##,
    );
    let tags: Vec<&str> = ws
        .views
        .configuration
        .styles
        .element_styles
        .iter()
        .map(|s| s.tag.as_str())
        .collect();
    assert_eq!(tags, ["Alpha", "Zeta", "Mid"], "document order survives");

    let bytes = to_vec(&ws).expect("write");
    let text = String::from_utf8(bytes).expect("UTF-8 out");
    let alpha = text.find("Alpha").unwrap();
    let zeta = text.find("Zeta").unwrap();
    let mid = text.find("Mid").unwrap();
    assert!(alpha < zeta && zeta < mid, "writer replays model order");
}

#[test]
fn numeric_style_fields_clamp_on_read() {
    // Witnesses: out3/styles-theme.json (`strokeWidth 0` → `1`) and
    // out6/order-theme.json (`position 150` → `100`).
    let ws = workspace(
        r#"{ "elements": [
               { "tag": "E", "strokeWidth": 0, "opacity": 200, "width": -5, "fontSize": 0 }
           ],
           "relationships": [
               { "tag": "R", "position": 150, "opacity": -1, "thickness": 0 }
           ] }"#,
    );
    let s = &ws.views.configuration.styles;
    let e = &s.element_styles[0];
    assert_eq!(e.stroke_width, Some(1), "setStrokeWidth clamps to 1");
    assert_eq!(e.opacity, Some(100), "setOpacity clamps to 100");
    assert_eq!(e.width, Some(-5), "width is NOT clamped");
    assert_eq!(e.font_size, Some(0), "fontSize is NOT clamped");
    let r = &s.relationship_styles[0];
    assert_eq!(r.position, Some(100), "setPosition clamps to 100");
    assert_eq!(r.opacity, Some(0), "setOpacity clamps to 0");
    assert_eq!(r.thickness, Some(0), "thickness is NOT clamped");

    // The clamp is visible in the written bytes (json2json echoes the
    // NORMALIZED value, exactly like the reference).
    let bytes = to_vec(&ws).expect("write");
    let text = String::from_utf8(bytes).expect("UTF-8 out");
    assert!(text.contains(r#""strokeWidth" : 1"#), "{text}");
    assert!(text.contains(r#""position" : 100"#), "{text}");
}
