//! The thin presentation shell (P3-D1): the ONLY code in this crate allowed
//! to reference the GUI stack, compiled only with `--features gui`. All
//! behavior lives in the library core (`scene`/`hit`/`commands`/`reload`),
//! which is tested headlessly.

use eframe::egui;

/// The scaffold app: proves the eframe/wgpu stack builds and opens (M1
/// replaces the label with the scene).
#[derive(Default)]
struct ScaffoldApp;

fn main() -> eframe::Result {
    eframe::run_native(
        "oxidizr-viewer",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(ScaffoldApp))),
    )
}

impl eframe::App for ScaffoldApp {
    // eframe 0.37 trait shape: the root `Ui` is handed in (not the Context).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("oxidizr-viewer — P3-M0 scaffold (the scene lands in M1)");
        });
    }
}
