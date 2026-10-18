# Forms List Multi-Select Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the forms list from single-select (radio) into toggle buttons, so multiple forms can be shown on the fretboard at once.

**Architecture:** `Option<usize>` → `BTreeSet<usize>` for the active-forms state, with the same name-based persistence/re-resolution pattern the codebase already uses for drag-reorder, generalized from one name to a set of names. This is one cohesive change across `src/settings.rs`, `src/gui/app.rs`, `src/gui/forms_panel.rs`, `src/gui/header.rs` — none of the four compiles correctly on its own once the others change, so it's a single task rather than independently gated ones.

**Tech Stack:** Rust, egui 0.36.1 (unchanged — no new dependency).

## Global Constraints

- `Settings.last_form: BTreeMap<String, String>` becomes `last_forms: BTreeMap<String, BTreeSet<String>>`. The old field just disappears from saved JSON on next save (unknown fields are ignored on load, same as always) — no migration code.
- `form_positions()` keeps its `Option<Vec<Pos>>` signature — it's the union of every active form's positions now, not one form's, so `focus()`/`play()` need no changes at their call sites.
- Edit/Delete move to per-row icon buttons; the old shared buttons (which only made sense for a single active form) are removed.
- Left/Right arrow-key form-cycling is removed outright, no replacement.
- Deleting a form only removes that form from the active set (if it was in it) — other active forms stay active. (This is a deliberate improvement over the old behavior, which cleared the entire selection on any delete; flag it if you disagree.)
- No new automated tests — this is UI/state wiring with no pure logic complex enough to warrant one, consistent with how the drag-and-drop reorder task in the previous feature shipped (verified by `cargo build`/`cargo test` + manual run-through only).

---

## Task 1: Forms list as toggle buttons (multi-select)

**Files:**
- Modify: `src/settings.rs`
- Modify: `src/gui/app.rs`
- Modify: `src/gui/forms_panel.rs`
- Modify: `src/gui/header.rs`

**Interfaces:**
- Produces on `ScaleViewApp`: `pub active_forms: BTreeSet<usize>` (was `active_form: Option<usize>`); `pub fn toggle_form(&mut self, index: usize)`; `pub fn clear_forms(&mut self)`; `pub fn set_active_forms(&mut self, forms: BTreeSet<usize>)` (was `set_active_form`); `pub fn form_positions(&self) -> Option<Vec<Pos>>` (signature unchanged, now a union).
- Produces on `Settings`: `pub last_forms: BTreeMap<String, BTreeSet<String>>` (was `last_form: BTreeMap<String, String>`).

- [ ] **Step 1: `src/settings.rs` — rename and retype the persisted field**

Change the import line:

```rust
use std::collections::BTreeMap;
```

to:

```rust
use std::collections::{BTreeMap, BTreeSet};
```

Change the struct field:

```rust
    /// Last form chosen for each selection key, so forms come back as they were left.
    pub last_form: BTreeMap<String, String>,
```

to:

```rust
    /// Forms toggled on for each selection key, so they come back as they were left.
    pub last_forms: BTreeMap<String, BTreeSet<String>>,
```

Change the `Default` impl's field:

```rust
            last_form: BTreeMap::new(),
```

to:

```rust
            last_forms: BTreeMap::new(),
```

- [ ] **Step 2: `src/gui/app.rs` — state field**

Change the struct field (in `ScaleViewApp`):

```rust
    pub active_form: Option<usize>,
```

to:

```rust
    pub active_forms: BTreeSet<usize>,
```

Change the initializer in `ScaleViewApp::new`:

```rust
            active_form: None,
```

to:

```rust
            active_forms: BTreeSet::new(),
```

(`BTreeSet` is already imported in this file — `use std::collections::{BTreeSet, HashSet};` — no import change needed here.)

- [ ] **Step 3: `src/gui/app.rs` — replace `form_positions`/`set_active_form` with the multi-select API**

Replace:

```rust
    /// Positions of the active form, transposed to the current root.
    pub fn form_positions(&self) -> Option<Vec<Pos>> {
        let form = self.forms_here().get(self.active_form?)?;
        Some(form.transposed(self.settings.selection.root.pc()))
    }

    pub fn set_active_form(&mut self, index: Option<usize>) {
        self.active_form = index;
        match index.and_then(|i| self.forms_here().get(i)) {
            Some(f) => {
                let name = f.name.clone();
                self.settings.last_form.insert(self.form_key.clone(), name);
            }
            None => {
                self.settings.last_form.remove(&self.form_key);
            }
        }
    }
```

with:

```rust
    /// Union of every active form's positions, transposed to the current root.
    pub fn form_positions(&self) -> Option<Vec<Pos>> {
        if self.active_forms.is_empty() {
            return None;
        }
        let root_pc = self.settings.selection.root.pc();
        let forms_here = self.forms_here();
        let mut positions: BTreeSet<Pos> = BTreeSet::new();
        for &i in &self.active_forms {
            if let Some(form) = forms_here.get(i) {
                positions.extend(form.transposed(root_pc));
            }
        }
        Some(positions.into_iter().collect())
    }

    /// Flip whether `index` is one of the active (shown) forms.
    pub fn toggle_form(&mut self, index: usize) {
        let mut forms = self.active_forms.clone();
        if !forms.remove(&index) {
            forms.insert(index);
        }
        self.set_active_forms(forms);
    }

    /// Deactivate every form for the current selection.
    pub fn clear_forms(&mut self) {
        self.set_active_forms(BTreeSet::new());
    }

    /// Replace the active set outright (e.g. after a delete/reorder re-resolves
    /// indices by name) and persist it.
    pub fn set_active_forms(&mut self, forms: BTreeSet<usize>) {
        self.active_forms = forms;
        let names: BTreeSet<String> =
            self.active_forms.iter().filter_map(|&i| self.forms_here().get(i)).map(|f| f.name.clone()).collect();
        if names.is_empty() {
            self.settings.last_forms.remove(&self.form_key);
        } else {
            self.settings.last_forms.insert(self.form_key.clone(), names);
        }
    }
```

- [ ] **Step 4: `src/gui/app.rs` — `sync_forms` resolves a set of names, not one**

Replace:

```rust
    fn sync_forms(&mut self) {
        let key = FormLibrary::key(&self.settings.tuning, &self.settings.selection.shape_key());
        if key != self.form_key {
            self.form_key = key;
            self.recording = None;
            let last = self.settings.last_form.get(&self.form_key);
            self.active_form = last.and_then(|name| self.forms_here().iter().position(|f| &f.name == name));
        }
    }
```

with:

```rust
    fn sync_forms(&mut self) {
        let key = FormLibrary::key(&self.settings.tuning, &self.settings.selection.shape_key());
        if key != self.form_key {
            self.form_key = key;
            self.recording = None;
            let names = self.settings.last_forms.get(&self.form_key).cloned().unwrap_or_default();
            self.active_forms = self
                .forms_here()
                .iter()
                .enumerate()
                .filter(|(_, f)| names.contains(&f.name))
                .map(|(i, _)| i)
                .collect();
        }
    }
```

- [ ] **Step 5: `src/gui/app.rs` — drop arrow-key form-cycling in `shortcuts`**

Replace:

```rust
        let (space, esc, left, right) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
            )
        });
        let now = ctx.input(|i| i.time);
        if space {
            if self.is_playing() { self.stop() } else { self.play(now) }
        }
        if esc {
            self.highlight = None;
            self.recording = None;
        }
        let n = self.forms_here().len();
        if n > 0 && (left || right) && self.recording.is_none() {
            // Cycle through "All notes" (None) and each form.
            let cur = self.active_form.map_or(0, |i| i + 1) as i32;
            let next = (cur + if right { 1 } else { -1 }).rem_euclid(n as i32 + 1);
            self.set_active_form((next > 0).then(|| next as usize - 1));
        }
    }
```

with:

```rust
        let (space, esc) = ctx.input(|i| (i.key_pressed(Key::Space), i.key_pressed(Key::Escape)));
        let now = ctx.input(|i| i.time);
        if space {
            if self.is_playing() { self.stop() } else { self.play(now) }
        }
        if esc {
            self.highlight = None;
            self.recording = None;
        }
    }
```

- [ ] **Step 6: `src/gui/app.rs` — `chord_shapes` checks the set, not the option**

Change:

```rust
        let chord_shapes = self.settings.selection.kind == ViewKind::Chord
            && self.recording.is_none()
            && self.active_form.is_none();
```

to:

```rust
        let chord_shapes = self.settings.selection.kind == ViewKind::Chord
            && self.recording.is_none()
            && self.active_forms.is_empty();
```

- [ ] **Step 7: `src/gui/forms_panel.rs` — toggle buttons, per-row edit/delete, set-based reorder**

Add to the imports:

```rust
use std::collections::BTreeSet;
```

Replace the whole `form_list` function and the `reorder_and_reselect` function (everything from `fn form_list` through the end of `reorder_and_reselect`, i.e. the code between `pub fn show` and `fn recording_controls`) with:

```rust
fn form_list(app: &mut ScaleViewApp, ui: &mut Ui) {
    if ui.selectable_label(app.active_forms.is_empty(), "All notes").clicked() {
        app.clear_forms();
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
                if ui.selectable_label(app.active_forms.contains(&i), text).clicked() {
                    app.toggle_form(i);
                }
                if ui.small_button("✏").on_hover_text("Edit this form").clicked() {
                    let form = app.forms_here()[i].clone();
                    app.recording = Some(Recording {
                        positions: form.transposed(app.settings.selection.root.pc()).into_iter().collect(),
                        name: form.name.clone(),
                        editing: Some(i),
                    });
                }
                if ui.small_button("🗑").on_hover_text("Delete this form").clicked() {
                    delete_and_reresolve(app, i);
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
}

/// Active forms' names, so a mutation that shifts indices can re-resolve them afterward.
fn active_names(app: &ScaleViewApp) -> BTreeSet<String> {
    app.active_forms.iter().filter_map(|&i| app.forms_here().get(i)).map(|f| f.name.clone()).collect()
}

/// Indices of forms whose name is in `names`, under the current key.
fn indices_named(app: &ScaleViewApp, names: &BTreeSet<String>) -> BTreeSet<usize> {
    app.forms_here().iter().enumerate().filter(|(_, f)| names.contains(&f.name)).map(|(i, _)| i).collect()
}

/// After a drag-drop reorder, keep whatever forms were active still active
/// (their indices may have shifted) by re-resolving them by name.
fn reorder_and_reselect(app: &mut ScaleViewApp, from: usize, to: usize) {
    if from == to {
        return;
    }
    let key = app.form_key.clone();
    let names = active_names(app);
    app.forms.reorder(&key, from, to);
    app.set_active_forms(indices_named(app, &names));
}

/// Delete form `i`, then re-resolve the remaining active forms by name
/// (deleting shifts every later index down by one).
fn delete_and_reresolve(app: &mut ScaleViewApp, i: usize) {
    let key = app.form_key.clone();
    let names = active_names(app);
    app.forms.remove(&key, i);
    app.set_active_forms(indices_named(app, &names));
}
```

- [ ] **Step 8: `src/gui/forms_panel.rs` — toggle the saved form on, in `recording_controls`**

Change:

```rust
        let i = app.forms.put(&key, editing, form);
        app.recording = None;
        app.set_active_form(Some(i));
        return;
```

to:

```rust
        let i = app.forms.put(&key, editing, form);
        app.recording = None;
        let mut forms = app.active_forms.clone();
        forms.insert(i);
        app.set_active_forms(forms);
        return;
```

- [ ] **Step 9: `src/gui/header.rs` — status line shows every active form's name**

Change:

```rust
        } else if let Some(f) = app.active_form.and_then(|i| app.forms_here().get(i)) {
            ui.label(RichText::new(format!("Form: {}", f.name)).color(ACCENT).strong());
            ui.label(RichText::new("◀ ▶ arrow keys switch forms").color(TEXT_DIM).small());
        } else if let Some(shape) = app.caged {
```

to:

```rust
        } else if !app.active_forms.is_empty() {
            let names: Vec<&str> =
                app.active_forms.iter().filter_map(|&i| app.forms_here().get(i)).map(|f| f.name.as_str()).collect();
            ui.label(RichText::new(format!("Forms: {}", names.join(", "))).color(ACCENT).strong());
        } else if let Some(shape) = app.caged {
```

- [ ] **Step 10: Build and test check**

Run: `cargo build`
Expected: builds with zero errors.

Run: `cargo clippy --all-targets`
Expected: no new warnings from this crate's own code (pre-existing warnings, if any, from dependencies are not this task's concern).

Run: `cargo test`
Expected: all existing tests still pass (13 passed, 0 failed) — none of them touch forms-selection state, so this is purely a regression check.

- [ ] **Step 11: Manual verification**

Run: `cargo run`. Record two forms for the same scale/arpeggio/chord. Then:
- Click both form pills — confirm both stay highlighted (toggle, not radio) and the fretboard shows the union of their positions (ghosting everything else).
- Click one again — confirm it deactivates while the other stays active.
- Click "All notes" — confirm both deactivate.
- Click ✏ on one row — confirm it opens that form for editing regardless of whether it was toggled on.
- Click 🗑 on a row while another form is toggled on — confirm only the deleted one disappears from the active set; the other stays active.
- Drag-reorder a toggled-on form — confirm it's still toggled on afterward (per `reorder_and_reselect`'s name-based re-resolution).
- Quit and relaunch — confirm the toggled-on set for that selection comes back as left (persisted via `last_forms`).

- [ ] **Step 12: Commit**

```bash
git add src/settings.rs src/gui/app.rs src/gui/forms_panel.rs src/gui/header.rs
git commit -m "Make forms list toggle buttons so multiple forms can be shown at once"
```
