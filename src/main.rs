//! ScaleView — learn scales, arpeggios and chords on a 24-fret guitar neck.
//!
//! Layers, each with one job:
//! - `theory`  — the ScaleEngine: notes, scales, chords, CAGED, fretboard model (pure).
//! - `audio`   — the AudioEngine: plucked-string synth playing timed notes.
//! - `forms` / `settings` — user data persisted as JSON.
//! - `gui`     — egui views; the only layer that knows about the other three.

mod audio;
mod forms;
mod gui;
mod settings;
mod storage;
mod theory;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("ScaleView")
            .with_inner_size([1600.0, 820.0])
            .with_min_inner_size([1000.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native("ScaleView", options, Box::new(|cc| Ok(Box::new(gui::ScaleViewApp::new(cc)))))
}
