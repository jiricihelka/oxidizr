//! `oxidizr-viewer` — native workspace viewer + layout editor (P3).
//!
//! Architecture promise (P3-D1, `agents/plan/build/PHASE_3.md`): all
//! non-trivial logic lives in the library modules below — [`scene`],
//! [`hit`], [`commands`], [`reload`] — which import NOTHING from the eframe
//! shell. The `oxidizr-viewer` binary (feature `gui`) is a thin presentation
//! layer, so Category A tests build and exercise the whole core headlessly:
//! zero GPU, zero windowing (`cargo test --workspace` never enables `gui`).
//!
//! The editor rule (P3-D0): editing features change only layout JSON values
//! — the workspace model and any DSL text are immutable here.
//!
//! # Dependency pins (P3-D2)
//!
//! `eframe` = 0.37 (wgpu renderer — better HiDPI/text path; glow is not
//! used), `notify` = 8 (9 is rc as of 2026-11-19), `usvg` = 0.50 +
//! `tiny-skia` = 0.12 (SVG icon rasterization, P3-D7), `dark-light` = 3
//! (OS light/dark preference, P3-D7). Event-driven repaint only (D2).

pub mod commands;
pub mod hit;
pub mod reload;
pub mod scene;

#[cfg(test)]
mod tests {
    /// R-2 boundary guard (P3-D1): the lib core must never reference the
    /// eframe shell — no import, no qualified path, in any lib source.
    /// `src/main.rs` (the `gui`-gated shell) is the only allowed place.
    #[test]
    fn lib_core_never_references_the_gui_shell() {
        // Built at runtime: the literals themselves must not appear in this
        // file, or the guard would flag its own source.
        let (e, g) = ("eframe", "egui");
        let forbidden = [
            format!("use {e}"),
            format!("{e}::"),
            format!("use {g}"),
            format!("{g}::"),
        ];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let entries = std::fs::read_dir(&src).expect("src readable");
        let mut checked = 0_usize;
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|e| e != "rs")
                || path.file_name().is_some_and(|n| n == "main.rs")
            {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source readable");
            for token in &forbidden {
                assert!(
                    !text.contains(token.as_str()),
                    "{} references the GUI shell via `{token}`",
                    path.display()
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 5, "lib core = lib.rs + four modules");
    }
}
