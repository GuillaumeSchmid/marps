//! File menu: import/export the forms library as a zipped `.svforms` file.

use crate::forms::FormLibrary;
use egui::{Context, Ui};
use std::time::{SystemTime, UNIX_EPOCH};

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

    pub fn menu_button(&mut self, ui: &mut Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("Import forms…").clicked() {
                self.dialog = egui_file_dialog::FileDialog::new()
                    .add_file_filter_extensions("ScaleView forms", vec!["svforms"]);
                self.dialog.pick_file();
                self.op = Some(FileOp::Import);
                ui.close();
            }
            if ui.button("Export forms…").clicked() {
                self.dialog = egui_file_dialog::FileDialog::new()
                    .add_save_extension("ScaleView forms", "svforms")
                    .default_save_extension("svforms")
                    .default_file_name(&format!("forms-{}", today_ymd()));
                self.dialog.save_file();
                self.op = Some(FileOp::Export);
                ui.close();
            }
        });
    }

    /// Poll the dialog; once a path is picked, run the pending import or export.
    pub fn update(&mut self, ctx: &Context, forms: &mut FormLibrary) {
        self.dialog.update(ctx);
        let Some(path) = self.dialog.take_picked() else { return };
        match self.op.take() {
            Some(FileOp::Export) => {
                if let Err(e) = forms.export_zip(&path) {
                    eprintln!("could not export forms to {}: {e}", path.display());
                }
            }
            Some(FileOp::Import) => match FormLibrary::import_zip(&path) {
                Ok(imported) => forms.merge(imported),
                Err(e) => eprintln!("could not import forms from {}: {e}", path.display()),
            },
            None => {}
        }
    }
}

fn today_ymd() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
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
