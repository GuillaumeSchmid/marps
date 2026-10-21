//! File menu: import/export the shapes library as a zipped `.svshapes` file
//! (`.svforms` files from before the rename still import).
//! On desktop through a file dialog; in the browser as an upload and a download.

use crate::shapes::ShapeLibrary;
use crate::storage::log_error;
use egui::{Context, Ui};

pub use platform::FileMenu;

impl FileMenu {
    pub fn menu_button(&mut self, ui: &mut Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("Import shapes…").clicked() {
                self.start_import(ui.ctx());
                ui.close();
            }
            if ui.button("Export shapes…").clicked() {
                self.start_export(ui.ctx());
                ui.close();
            }
        });
    }
}

fn export_file_name() -> String {
    format!("shapes-{}.svshapes", today_ymd())
}

fn import_bytes(bytes: &[u8], shapes: &mut ShapeLibrary) {
    match ShapeLibrary::import_zip(bytes) {
        Ok(imported) => shapes.merge(imported),
        Err(e) => log_error(&format!("could not import shapes: {e}")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::*;

    enum FileOp {
        Import,
        Export,
    }

    pub struct FileMenu {
        dialog: egui_file_dialog::FileDialog,
        op: Option<FileOp>,
    }

    impl FileMenu {
        pub fn new() -> Self {
            Self { dialog: egui_file_dialog::FileDialog::new(), op: None }
        }

        pub(super) fn start_import(&mut self, _ctx: &Context) {
            self.dialog = egui_file_dialog::FileDialog::new()
                .add_file_filter_extensions("ScaleView shapes", vec!["svshapes", "svforms"]);
            self.dialog.pick_file();
            self.op = Some(FileOp::Import);
        }

        pub(super) fn start_export(&mut self, _ctx: &Context) {
            self.dialog = egui_file_dialog::FileDialog::new()
                .add_save_extension("ScaleView shapes", "svshapes")
                .default_save_extension("svshapes")
                .default_file_name(export_file_name().trim_end_matches(".svshapes"));
            self.dialog.save_file();
            self.op = Some(FileOp::Export);
        }

        /// Poll the dialog; once a path is picked, run the pending import or export.
        pub fn update(&mut self, ctx: &Context, shapes: &mut ShapeLibrary) {
            self.dialog.update(ctx);
            let Some(path) = self.dialog.take_picked() else { return };
            match self.op.take() {
                Some(FileOp::Export) => {
                    if let Err(e) = shapes.export_zip().and_then(|bytes| std::fs::write(&path, bytes)) {
                        log_error(&format!("could not export shapes to {}: {e}", path.display()));
                    }
                }
                Some(FileOp::Import) => match std::fs::read(&path) {
                    Ok(bytes) => import_bytes(&bytes, shapes),
                    Err(e) => log_error(&format!("could not read {}: {e}", path.display())),
                },
                None => {}
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    pub struct FileMenu {
        export_requested: bool,
        /// Filled by the file input's async read; drained in `update`.
        uploaded: Rc<RefCell<Option<Vec<u8>>>>,
    }

    impl FileMenu {
        pub fn new() -> Self {
            Self { export_requested: false, uploaded: Rc::default() }
        }

        pub(super) fn start_import(&mut self, ctx: &Context) {
            if let Err(e) = open_upload(ctx.clone(), self.uploaded.clone()) {
                log_error(&format!("could not open file picker: {e:?}"));
            }
        }

        pub(super) fn start_export(&mut self, _ctx: &Context) {
            self.export_requested = true;
        }

        pub fn update(&mut self, _ctx: &Context, shapes: &mut ShapeLibrary) {
            if std::mem::take(&mut self.export_requested) {
                match shapes.export_zip() {
                    Ok(bytes) => {
                        if let Err(e) = download(&bytes, &export_file_name()) {
                            log_error(&format!("could not export shapes: {e:?}"));
                        }
                    }
                    Err(e) => log_error(&format!("could not export shapes: {e}")),
                }
            }
            if let Some(bytes) = self.uploaded.borrow_mut().take() {
                import_bytes(&bytes, shapes);
            }
        }
    }

    fn document() -> web_sys::Document {
        web_sys::window().and_then(|w| w.document()).expect("no document")
    }

    /// Hand `bytes` to the browser as a file download.
    fn download(bytes: &[u8], name: &str) -> Result<(), wasm_bindgen::JsValue> {
        let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/zip");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)?;
        let a: web_sys::HtmlAnchorElement = document().create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        web_sys::Url::revoke_object_url(&url)
    }

    /// Show the browser's file picker; the chosen file's bytes land in `slot`.
    fn open_upload(ctx: Context, slot: Rc<RefCell<Option<Vec<u8>>>>) -> Result<(), wasm_bindgen::JsValue> {
        let input: web_sys::HtmlInputElement = document().create_element("input")?.dyn_into()?;
        input.set_type("file");
        input.set_accept(".svshapes,.svforms");
        let picker = input.clone();
        let on_change = Closure::once_into_js(move || {
            let Some(file) = picker.files().and_then(|f| f.get(0)) else { return };
            wasm_bindgen_futures::spawn_local(async move {
                match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
                    Ok(buf) => {
                        *slot.borrow_mut() = Some(js_sys::Uint8Array::new(&buf).to_vec());
                        ctx.request_repaint();
                    }
                    Err(e) => log_error(&format!("could not read {}: {e:?}", file.name())),
                }
            });
        });
        input.set_onchange(Some(on_change.unchecked_ref()));
        input.click();
        Ok(())
    }
}

fn today_ymd() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    #[cfg(target_arch = "wasm32")]
    let secs = (js_sys::Date::now() / 1000.0) as u64;
    ymd_from_days((secs / 86400) as i64)
}

/// Gregorian civil date from a day count since the Unix epoch (Howard Hinnant's
/// `civil_from_days`) — avoids pulling in a full date/time crate for one filename.
fn ymd_from_days(days: i64) -> String {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ymd_from_days_matches_known_dates() {
        assert_eq!(ymd_from_days(0), "1970-01-01");
        assert_eq!(ymd_from_days(19645), "2023-10-15");
        assert_eq!(ymd_from_days(20000), "2024-10-04");
        assert_eq!(ymd_from_days(-1), "1969-12-31");
        assert_eq!(ymd_from_days(10957), "2000-01-01");
    }
}
