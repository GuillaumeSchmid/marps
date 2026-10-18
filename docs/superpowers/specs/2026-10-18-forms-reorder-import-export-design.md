# Forms: drag-reorder, startup load, File menu import/export

Date: 2026-10-18

## Scope

Three asks:

1. Drag-and-drop reorder of the forms list shown for the current tuning/scale/chord key.
2. Load user-created forms at startup.
3. A File menu with Import/Export of the forms library as a zipped JSON file, extension `.svforms`, using an egui file-dialog crate for the save/open prompt.

## Already done

Forms are already loaded at startup: `ScaleViewApp::new` calls `FormLibrary::load()`
(`src/gui/app.rs:65`), which reads `~/.config/scaleview/forms.json` via `storage::load`.
No change needed for ask 2 — noting it so nothing gets re-built by accident.

## 1. Drag-and-drop reorder

**Scope:** per-key, as now. `FormLibrary` already stores forms keyed by
`"{tuning}|{shape_key}"` (`src/forms.rs:46`); reordering only touches the vec for the
current key, matching how everything else about forms already works.

**Mechanism:** egui 0.36 ships drag-and-drop natively (`Ui::dnd_drag_source`,
`Ui::dnd_drop_zone`, `DragAndDrop`) — confirmed in the vendored docs, no new
dependency. `dnd_drag_source` swallows plain clicks (it owns the sense for its whole
contents), so a drag handle has to be a separate small widget next to the existing
`selectable_label`, not wrapped around it — otherwise clicking a form to select it
would stop working.

Each row in `form_list` (`src/gui/forms_panel.rs:34`) becomes:

```
[⠿ grip: dnd_drag_source(payload = index)]  [existing selectable_label]
```

wrapped in a `dnd_drop_zone::<usize, ()>` per row. On drop, the zone reports the
dragged index; the row calls `app.forms.reorder(&key, dragged_index, this_index)`.

**New `FormLibrary` method:**

```rust
pub fn reorder(&mut self, key: &str, from: usize, to: usize) {
    if let Some(list) = self.forms.get_mut(key) {
        if from < list.len() && to < list.len() && from != to {
            let item = list.remove(from);
            list.insert(to, item);
        }
    }
    self.save();
}
```

**Keeping the active selection stable:** `active_form` is an index into the current
key's vec. After a reorder, remember the active form's *name* before moving, reorder,
then re-resolve the index by name and call the existing `app.set_active_form(...)` —
reuses the method that already keeps `settings.last_form` in sync, no new bookkeeping.

## 2. File menu: Import / Export as `.svforms`

**Menu placement:** no menu bar exists yet. Add `ui.menu_button("File", |ui| {...})`
as the first widget in the existing top panel's row (`controls::selection_bar`,
`src/gui/controls.rs:24`) rather than a new panel strip — one less band of chrome.

**File dialog:** `egui-file-dialog` v0.15, confirmed to build against egui/eframe
0.36.1. One `FileDialog` instance stored on `ScaleViewApp`, reused for both Import and
Export, with a mode flag (`enum FileOp { Import, Export }`) tracking which action is
in flight, since the same dialog surfaces a single picked path either way.

- Filter: `.add_file_filter_extensions("ScaleView forms", vec!["svforms"])`.
- Export opens `save_file()` with a default filename stamped from the current time:
  `forms-YYYY-MM-DD`. (`.svforms` appended by the dialog/export code, not typed by the
  user.)
- Import opens `pick_file()`.
- `file_dialog.update(ctx)` runs every frame (from `ScaleViewApp::ui`); once
  `take_picked()` returns a path, the pending `FileOp` decides whether to export or
  import, then clears.

**Zip format:** the `.svforms` file is a real zip archive (openable by any unzip
tool) containing one entry, `forms.json`, holding `FormLibrary`'s existing
`Serialize`/`Deserialize` output verbatim — no new schema. Uses the `zip` crate,
default features off, only `deflate` enabled (pure-Rust backend, no system zlib, and
it skips the bzip2/aes/lzma/zstd/xz/ppmd codecs the crate otherwise defaults to for a
one-file archive that doesn't need them).

```rust
impl FormLibrary {
    // Serializes self to JSON, writes it as the single entry "forms.json" in a
    // new zip archive at `path` (zip::ZipWriter + start_file + write_all + finish).
    pub fn export_zip(&self, path: &Path) -> std::io::Result<()>;

    // Opens `path` as a zip archive, reads the "forms.json" entry, deserializes it.
    pub fn import_zip(path: &Path) -> std::io::Result<FormLibrary>;
}
```

**Import behavior: merge, imported wins.** For every `(key, form)` in the imported
library: if a form with the same name already exists under that key, replace it
in place (same index, so any active selection by that name is unaffected); otherwise
append it. One new method:

```rust
impl FormLibrary {
    pub fn merge(&mut self, other: FormLibrary) {
        for (key, incoming) in other.forms {
            let list = self.forms.entry(key).or_default();
            for form in incoming {
                match list.iter_mut().find(|f| f.name == form.name) {
                    Some(slot) => *slot = form,
                    None => list.push(form),
                }
            }
        }
        self.save();
    }
}
```

**Errors:** on a bad zip/JSON or I/O failure, `eprintln!` and leave the library
untouched — matches the existing pattern in `storage::save`/`storage::load`, which
already swallow errors to stderr rather than surfacing UI popups. No toast/error UI
is being introduced for this.

## New dependencies

```toml
egui-file-dialog = "0.15"
zip = { version = "9", default-features = false, features = ["deflate"] }
```

Both verified to compile against the existing `egui`/`eframe` 0.36.1 pin.

## Out of scope

- No drag-reorder across different tuning/scale keys (forms stay keyed as today).
- No in-app error dialog for import failures (stderr only, as elsewhere in the app).
- No partial/selective import UI — merge is all-or-nothing per the rule above.
