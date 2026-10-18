# Forms Drag-Reorder & File Menu Import/Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users drag-reorder their saved forms within a scale/chord, and add a File menu to export/import the whole forms library as a `.svforms` zip.

**Architecture:** Three additions to `FormLibrary` (`src/forms.rs`) for reorder/merge/zip (de)serialization, each split into a pure in-memory helper (unit-testable, no disk I/O) plus a public autosaving wrapper matching the existing `put`/`remove` convention. A new `src/gui/file_menu.rs` owns the `egui-file-dialog` instance and polls it once per frame. UI wiring touches `src/gui/forms_panel.rs` (drag handles) and `src/gui/controls.rs` (File menu button).

**Tech Stack:** Rust, egui/eframe 0.36.1, `egui-file-dialog` 0.15.0, `zip` 9.0.5 (deflate only), serde_json.

## Global Constraints

- Do not bump `egui`/`eframe` past the pinned `0.36.1` — both new crates were verified to build against that exact pin.
- `zip` dependency: `default-features = false, features = ["deflate"]` only — the crate's full default feature set pulls in bzip2/aes/lzma/zstd/xz/ppmd, none of which this single-file archive needs.
- `.svforms` is a real zip archive containing one entry, `forms.json`, serialized exactly as `FormLibrary`'s existing `Serialize` impl already produces (no new schema).
- Import = merge into the existing library; a form with the same name in the same tuning/scale key is replaced in place by the imported one (imported wins); anything new is appended. This is NOT a wholesale replace.
- Reorder is per-key (per `"{tuning}|{shape_key}"`), matching how forms are already stored — never reorder across keys.
- On any import/export I/O or (de)serialization failure: `eprintln!` and leave the in-memory library untouched. No popup/toast UI — matches the existing error handling in `src/storage.rs`.
- Any `FormLibrary` mutation that is reachable from the running app must still call `self.save()` (existing convention in `put`/`remove`) — but unit tests must never trigger a real `self.save()`, because that writes to the user's actual `~/.config/scaleview/forms.json`. This is why reorder/merge are split into a private `_in_memory` helper (tested directly, no I/O) plus a public autosaving wrapper (not unit-tested, trivial one-line composition).

---

## Task 1: `FormLibrary::reorder`

**Files:**
- Modify: `src/forms.rs`

**Interfaces:**
- Produces: `FormLibrary::reorder(&mut self, key: &str, from: usize, to: usize)` — public, autosaves. `FormLibrary::reorder_in_memory(&mut self, key: &str, from: usize, to: usize)` — private, no I/O, used by tests and by `reorder`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src/forms.rs` (after the existing `transposition_prefers_small_moves_and_stays_on_neck` test, inside the same `mod tests { use super::*; ... }`):

```rust
    #[test]
    fn reorder_moves_item_within_key() {
        let key = "standard|Cmajor";
        let mk = |n: &str| Form { name: n.into(), root_pc: 0, positions: vec![] };
        let mut lib = FormLibrary {
            forms: BTreeMap::from([(key.to_string(), vec![mk("a"), mk("b"), mk("c")])]),
        };
        lib.reorder_in_memory(key, 0, 2);
        let names: Vec<_> = lib.forms(key).iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
    }
```

This builds the fixture via the struct literal (not `put`, which calls `self.save()` and would write to the real `~/.config/scaleview/forms.json`) and calls the not-yet-existing private `reorder_in_memory` directly, so no disk I/O happens anywhere in this test.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib reorder_moves_item_within_key`
Expected: FAIL to compile — `no method named reorder_in_memory found for struct FormLibrary`.

- [ ] **Step 3: Implement**

In `src/forms.rs`, inside `impl FormLibrary { ... }`, add after the existing `remove` method (just before the closing `}` of the `impl` block, i.e. right before the blank line that precedes `#[cfg(test)]`):

```rust
    /// Move the form at `from` to `to` within one key's list, then persist.
    pub fn reorder(&mut self, key: &str, from: usize, to: usize) {
        self.reorder_in_memory(key, from, to);
        self.save();
    }

    fn reorder_in_memory(&mut self, key: &str, from: usize, to: usize) {
        if let Some(list) = self.forms.get_mut(key) {
            if from < list.len() && to < list.len() && from != to {
                let item = list.remove(from);
                list.insert(to, item);
            }
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib reorder_moves_item_within_key`
Expected: `test theory::tests::reorder_moves_item_within_key ... ok` (module path may read `forms::tests::...` — either way, 1 passed, 0 failed).

- [ ] **Step 5: Commit**

```bash
git add src/forms.rs
git commit -m "Add FormLibrary::reorder"
```

---

## Task 2: `FormLibrary::merge`

**Files:**
- Modify: `src/forms.rs`

**Interfaces:**
- Produces: `FormLibrary::merge(&mut self, other: FormLibrary)` — public, autosaves. `FormLibrary::merge_in_memory(&mut self, other: FormLibrary)` — private, no I/O.

- [ ] **Step 1: Write the failing test**

Add to the same `mod tests` block in `src/forms.rs`:

```rust
    #[test]
    fn merge_imported_form_replaces_same_name_in_same_key() {
        let key = "standard|Cmajor";
        let mut lib = FormLibrary {
            forms: BTreeMap::from([(
                key.to_string(),
                vec![
                    Form { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(0, 0)] },
                    Form { name: "shape2".into(), root_pc: 0, positions: vec![] },
                ],
            )]),
        };
        let other = FormLibrary {
            forms: BTreeMap::from([(
                key.to_string(),
                vec![
                    Form { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(1, 1)] },
                    Form { name: "shape3".into(), root_pc: 0, positions: vec![] },
                ],
            )]),
        };
        lib.merge_in_memory(other);
        let forms = lib.forms(key);
        assert_eq!(forms.len(), 3);
        assert_eq!(forms[0].name, "shape1");
        assert_eq!(forms[0].positions, vec![Pos::new(1, 1)]); // imported wins, replaced in place
        assert_eq!(forms[1].name, "shape2");
        assert_eq!(forms[2].name, "shape3"); // new form appended
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib merge_imported_form_replaces_same_name_in_same_key`
Expected: FAIL to compile — `no method named merge_in_memory found for struct FormLibrary`.

- [ ] **Step 3: Implement**

In `src/forms.rs`, inside `impl FormLibrary { ... }`, add right after the `reorder`/`reorder_in_memory` pair from Task 1:

```rust
    /// Merge another library in: a form with the same name in the same key is
    /// replaced in place by the incoming one; anything new is appended.
    pub fn merge(&mut self, other: FormLibrary) {
        self.merge_in_memory(other);
        self.save();
    }

    fn merge_in_memory(&mut self, other: FormLibrary) {
        for (key, incoming) in other.forms {
            let list = self.forms.entry(key).or_default();
            for form in incoming {
                match list.iter_mut().find(|f| f.name == form.name) {
                    Some(slot) => *slot = form,
                    None => list.push(form),
                }
            }
        }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib merge_imported_form_replaces_same_name_in_same_key`
Expected: 1 passed, 0 failed.

- [ ] **Step 5: Commit**

```bash
git add src/forms.rs
git commit -m "Add FormLibrary::merge"
```

---

## Task 3: Zip export/import of the forms library

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/forms.rs`

**Interfaces:**
- Consumes: `FormLibrary` already derives `Serialize`/`Deserialize` (`src/forms.rs:45`).
- Produces: `FormLibrary::export_zip(&self, path: &Path) -> std::io::Result<()>`. `FormLibrary::import_zip(path: &Path) -> std::io::Result<FormLibrary>` (associated function, not a method — there's no existing instance to import into).

- [ ] **Step 1: Add the `zip` dependency**

In `Cargo.toml`, in `[dependencies]`, add this line after `directories = "6.0.0"`:

```toml
zip = { version = "9", default-features = false, features = ["deflate"] }
```

Run: `cargo build` — expected to succeed (confirms the dependency resolves; `zip` 9.0.5 was locked and this exact `deflate`-only build was verified to compile and round-trip a file during planning).

- [ ] **Step 2: Write the failing test**

Add to the `mod tests` block in `src/forms.rs`:

```rust
    #[test]
    fn export_then_import_round_trips() {
        let key = "standard|Cmajor";
        let lib = FormLibrary {
            forms: BTreeMap::from([(
                key.to_string(),
                vec![Form { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(2, 3)] }],
            )]),
        };
        let path = std::env::temp_dir().join("scaleview_export_test.svforms");
        lib.export_zip(&path).unwrap();
        let imported = FormLibrary::import_zip(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(imported.forms(key), lib.forms(key));
    }
```

This writes only to `std::env::temp_dir()`, never to the real config path, so it is safe to run repeatedly.

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib export_then_import_round_trips`
Expected: FAIL to compile — `no method named export_zip found for struct FormLibrary`.

- [ ] **Step 4: Implement**

At the top of `src/forms.rs`, change the import lines from:

```rust
use crate::storage;
use crate::theory::Pos;
use crate::theory::tuning::FRETS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
```

to:

```rust
use crate::storage;
use crate::theory::Pos;
use crate::theory::tuning::FRETS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
```

Then, inside `impl FormLibrary { ... }`, add after the `merge`/`merge_in_memory` pair from Task 2:

```rust
    /// Serialize to JSON and write it as `forms.json` inside a new zip archive at `path`.
    pub fn export_zip(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let file = std::fs::File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("forms.json", options)?;
        zip.write_all(json.as_bytes())?;
        zip.finish()?;
        Ok(())
    }

    /// Read a zip archive at `path` and deserialize its `forms.json` entry.
    pub fn import_zip(path: &Path) -> std::io::Result<FormLibrary> {
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut entry = archive.by_name("forms.json")?;
        let mut s = String::new();
        entry.read_to_string(&mut s)?;
        serde_json::from_str(&s).map_err(std::io::Error::other)
    }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib export_then_import_round_trips`
Expected: 1 passed, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/forms.rs
git commit -m "Add FormLibrary zip export/import"
```

---

## Task 4: Drag-and-drop reorder in the forms list UI

**Files:**
- Modify: `src/gui/forms_panel.rs`

**Interfaces:**
- Consumes: `FormLibrary::reorder(&mut self, key: &str, from: usize, to: usize)` (Task 1). `ScaleViewApp::{forms_here, active_form, form_key, set_active_form}` (all pre-existing, `src/gui/app.rs`).
- Produces: nothing new consumed by later tasks — this is pure UI wiring.

No automated test for this task: it's egui input-handling glue over already-unit-tested logic (`reorder`), and egui widget interaction isn't exercised by `cargo test` without a UI test harness this codebase doesn't have. Verify with a build check plus a short manual run-through (Step 3).

- [ ] **Step 1: Replace `form_list` and add `reorder_and_reselect`**

In `src/gui/forms_panel.rs`, replace the whole `form_list` function (currently lines 34-68) with:

```rust
fn form_list(app: &mut ScaleViewApp, ui: &mut Ui) {
    if ui.selectable_label(app.active_form.is_none(), "All notes").clicked() {
        app.set_active_form(None);
    }
    let key = app.form_key.clone();
    let names: Vec<String> = app.forms_here().iter().map(|f| f.name.clone()).collect();
    for (i, name) in names.iter().enumerate() {
        let (_, dropped) = ui.dnd_drop_zone::<usize, ()>(Frame::new(), |ui| {
            ui.horizontal(|ui| {
                let grip_id = egui::Id::new(("form_grip", &key, i));
                ui.dnd_drag_source(grip_id, i, |ui| {
                    ui.label(RichText::new("⠿").color(TEXT_DIM));
                });
                let text = RichText::new(name).strong();
                if ui.selectable_label(app.active_form == Some(i), text).clicked() {
                    app.set_active_form(Some(i));
                }
            });
        });
        if let Some(from) = dropped {
            reorder_and_reselect(app, *from, i);
        }
    }
    if names.is_empty() {
        ui.label(RichText::new("none yet — record one to save a fingering").color(TEXT_DIM).small());
    }
    ui.separator();
    if ui.button(RichText::new("⏺ New form").color(RECORD)).on_hover_text("Pick notes on the neck and name them").clicked() {
        app.recording = Some(Recording { positions: Default::default(), name: String::new(), editing: None });
        app.highlight = None;
    }
    if let Some(i) = app.active_form {
        let form = app.forms_here()[i].clone();
        if ui.button("✏ Edit").clicked() {
            app.recording = Some(Recording {
                positions: form.transposed(app.settings.selection.root.pc()).into_iter().collect(),
                name: form.name.clone(),
                editing: Some(i),
            });
        }
        if ui.button("🗑 Delete").clicked() {
            let key = app.form_key.clone();
            app.forms.remove(&key, i);
            app.set_active_form(None);
        }
    }
}

/// After a drag-drop reorder, keep whatever form was selected still selected
/// (its index may have shifted) by re-resolving it by name.
fn reorder_and_reselect(app: &mut ScaleViewApp, from: usize, to: usize) {
    if from == to {
        return;
    }
    let key = app.form_key.clone();
    let active_name = app.active_form.and_then(|i| app.forms_here().get(i)).map(|f| f.name.clone());
    app.forms.reorder(&key, from, to);
    let new_index = active_name.and_then(|name| app.forms_here().iter().position(|f| f.name == name));
    app.set_active_form(new_index);
}
```

The grip (`⠿`, via `dnd_drag_source`) is a separate widget from the `selectable_label` — `dnd_drag_source` claims drag-sense over its own contents' rect, so if the label itself were wrapped in it, plain clicks to select a form would stop working. Keeping them side by side in one `ui.horizontal` preserves both.

- [ ] **Step 2: Build check**

Run: `cargo build`
Expected: builds with no errors (warnings, if any, should be pre-existing — don't introduce new ones; `cargo build` output should mention nothing about `forms_panel.rs`).

- [ ] **Step 3: Manual verification**

Run: `cargo run`. Record at least two forms for the same scale (click "⏺ New form", pick a fret, type a name, Save — repeat once more so there are 2+ forms listed). Then:
- Drag the `⠿` grip of the second form onto the first form's row. Confirm the list order swaps.
- Click a form's name label (not the grip) — confirm it still selects/highlights that form on the fretboard (plain click still works).
- Quit and relaunch (`cargo run` again) — confirm the new order persisted (it was saved to `~/.config/scaleview/forms.json` by `reorder`'s autosave).

- [ ] **Step 4: Commit**

```bash
git add src/gui/forms_panel.rs
git commit -m "Allow drag-and-drop reordering of saved forms"
```

---

## Task 5: File menu — import/export as `.svforms`

**Files:**
- Modify: `Cargo.toml`
- Create: `src/gui/file_menu.rs`
- Modify: `src/gui/mod.rs`
- Modify: `src/gui/app.rs`
- Modify: `src/gui/controls.rs`

**Interfaces:**
- Consumes: `FormLibrary::{export_zip, import_zip, merge}` (Tasks 2 and 3).
- Produces: `file_menu::FileMenu` with `pub fn new() -> Self` and `pub fn menu_button(&mut self, ui: &mut egui::Ui)` and `pub fn update(&mut self, ctx: &egui::Context, forms: &mut FormLibrary)`. `ScaleViewApp.file_menu: file_menu::FileMenu` (new public field).

- [ ] **Step 1: Add the `egui-file-dialog` dependency**

In `Cargo.toml`, in `[dependencies]`, add this line after the `zip` line added in Task 3:

```toml
egui-file-dialog = "0.15"
```

Run: `cargo build` — expected to succeed (this exact version was verified during planning to compile against the pinned `egui`/`eframe` 0.36.1).

- [ ] **Step 2: Write the failing test for the date helper**

Create `src/gui/file_menu.rs` with just this much (the test first, implementation in the next step):

```rust
//! File menu: import/export the forms library as a zipped `.svforms` file.

use crate::forms::FormLibrary;
use egui::{Context, Ui};
use std::time::{SystemTime, UNIX_EPOCH};

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
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib ymd_from_days_matches_known_dates`
Expected: FAIL to compile — `cannot find function ymd_from_days in this scope`.

- [ ] **Step 4: Implement the full module**

Replace the contents of `src/gui/file_menu.rs` with:

```rust
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
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib ymd_from_days_matches_known_dates`
Expected: 1 passed, 0 failed.

- [ ] **Step 6: Wire the module into `src/gui/mod.rs`**

In `src/gui/mod.rs`, change:

```rust
mod app;
mod controls;
mod forms_panel;
mod fretboard;
mod header;
mod theme;
```

to:

```rust
mod app;
mod controls;
mod file_menu;
mod forms_panel;
mod fretboard;
mod header;
mod theme;
```

- [ ] **Step 7: Add the `file_menu` field to `ScaleViewApp`**

In `src/gui/app.rs`:

Change the import line:

```rust
use super::{controls, forms_panel, header};
```

to:

```rust
use super::{controls, file_menu, forms_panel, header};
```

Add a field to the `ScaleViewApp` struct (after `pub forms: FormLibrary,`):

```rust
pub struct ScaleViewApp {
    pub engine: ScaleEngine,
    pub audio: AudioEngine,
    pub settings: Settings,
    saved_settings: Settings,
    pub forms: FormLibrary,
    pub file_menu: file_menu::FileMenu,
    pub model: FretboardModel,
    ...
```

(i.e. insert `pub file_menu: file_menu::FileMenu,` right after the existing `pub forms: FormLibrary,` line — leave every other field as-is.)

In `ScaleViewApp::new`, add the field to the struct literal (after `forms: FormLibrary::load(),`):

```rust
            forms: FormLibrary::load(),
            file_menu: file_menu::FileMenu::new(),
            model,
```

(i.e. insert `file_menu: file_menu::FileMenu::new(),` right after the existing `forms: FormLibrary::load(),` line — leave every other field as-is.)

In `impl eframe::App for ScaleViewApp`'s `ui` method, right after the existing:

```rust
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.refresh();
```

add:

```rust
        self.file_menu.update(&ctx, &mut self.forms);
```

so the block reads:

```rust
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);
        self.refresh();
        self.file_menu.update(&ctx, &mut self.forms);
```

- [ ] **Step 8: Add the File menu button to the selection bar**

In `src/gui/controls.rs`, change the start of `selection_bar`:

```rust
pub fn selection_bar(app: &mut ScaleViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("ScaleView").strong().size(18.0).color(theme::ACCENT));
        ui.add_space(10.0);
```

to:

```rust
pub fn selection_bar(app: &mut ScaleViewApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        app.file_menu.menu_button(ui);
        ui.label(RichText::new("ScaleView").strong().size(18.0).color(theme::ACCENT));
        ui.add_space(10.0);
```

- [ ] **Step 9: Build check**

Run: `cargo build`
Expected: builds with no errors.

- [ ] **Step 10: Manual verification**

Run: `cargo run`.
- Click **File > Export forms…**. Confirm the egui file dialog opens with a default filename like `forms-2026-10-18` and only accepts/appends `.svforms`. Save it somewhere (e.g. `/tmp`).
- Confirm the saved file is a real zip: `unzip -l /tmp/forms-2026-10-18.svforms` should list exactly one entry, `forms.json`.
- Record a new form, or edit/delete one of the existing ones, so the in-app library now differs from the exported file.
- Click **File > Import forms…**, pick the file just exported. Confirm the form(s) from the exported snapshot reappear (merged back in — same-named forms are overwritten with the exported version, nothing else in your current library is lost).

- [ ] **Step 11: Commit**

```bash
git add Cargo.toml Cargo.lock src/gui/file_menu.rs src/gui/mod.rs src/gui/app.rs src/gui/controls.rs
git commit -m "Add File menu to import/export forms as .svforms"
```
