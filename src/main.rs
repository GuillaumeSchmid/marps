//! ScaleView — learn scales, arpeggios and chords on a 24-fret guitar neck.
//!
//! Layers, each with one job:
//! - `theory`  — the ScaleEngine: notes, scales, chords, CAGED, fretboard model (pure).
//! - `audio`   — the AudioEngine: plucked-string synth playing timed notes.
//! - `shapes` / `settings` — user data persisted as JSON.
//! - `gui`     — egui views; the only layer that knows about the other three.

mod audio;
mod shapes;
mod gui;
mod settings;
mod storage;
mod theory;

#[cfg(not(target_arch = "wasm32"))]
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

/// Browser entry point: mounts the app on the `<canvas id="scaleview">` in index.html.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Warn).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().and_then(|w| w.document()).expect("no document");
        let canvas = document
            .get_element_by_id("scaleview")
            .expect("no #scaleview canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#scaleview is not a canvas");
        let result = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(gui::ScaleViewApp::new(cc)))))
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => loading.set_inner_html(&format!("ScaleView failed to start: {e:?}")),
            }
        }
    });
}
