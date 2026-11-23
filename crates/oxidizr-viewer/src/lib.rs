//! `oxidizr-viewer` — native workspace viewer + layout editor.
//!
//! All non-trivial logic lives in the library modules below — [`scene`],
//! [`hit`], [`paint`], [`key`], [`icons`], [`commands`], [`reload`],
//! [`layout`] — which import NOTHING from the GUI shell. The shell itself
//! is `shell`, compiled only with `feature = "gui"`: the
//! `oxidizr-viewer` binary is its argv shim and `oxidizr view`
//! (the `gui` feature of `oxidizr-cli`) drives the same
//! `shell::run` in-process, so both entries share one window.
//! Tests build and exercise the whole core headlessly: zero GPU, zero
//! windowing (`cargo test --workspace` never enables `gui`).
//! [`paint`] emits `epaint::Shape` data (software raster + text layout,
//! no windowing) — the boundary guard forbids eframe and egui only, so
//! epaint stays allowed exactly because the guard's own test below runs
//! with no GPU.
//!
//! Editing features change only layout JSON values — the workspace model
//! and any DSL text are immutable here.
//!
//! # Rendering
//!
//! eframe's wgpu renderer (better HiDPI/text path; glow is not used), and
//! repaint on demand only — never every frame. Icons are rasterized from
//! SVG and the OS light/dark preference comes from `dark-light`.

pub mod commands;
pub mod export;
pub mod hit;
pub mod icons;
pub mod key;
pub mod layout;
pub mod overview;
pub mod paint;
pub mod reload;
pub mod scene;
#[cfg(feature = "gui")]
pub mod shell;
pub mod statefile;

#[cfg(test)]
mod tests {
    /// R-2 boundary guard: the lib core must never the official UI the
    /// eframe shell — no import, no qualified path, in any lib source.
    /// The two `gui`-gated shell files are the only allowed
    /// places: `src/shell.rs` (the shell itself) and `src/main.rs`
    /// (the binary's argv shim).
    #[test]
    fn lib_core_never_references_the_gui_shell() {
        const SHELL_FILES: [&str; 2] = ["main.rs", "shell.rs"];
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
                || path
                    .file_name()
                    .is_some_and(|n| SHELL_FILES.iter().any(|shell| n == *shell))
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
        assert_eq!(checked, 12, "lib core = lib.rs + eleven modules");
    }
}
