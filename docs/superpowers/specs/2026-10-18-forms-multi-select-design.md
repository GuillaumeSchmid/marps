# Forms list: toggle buttons, multiple forms visible at once

Date: 2026-10-18

## Scope

The forms list (`src/gui/forms_panel.rs`) currently behaves like a radio group:
clicking a form sets it as the single `active_form`, clicking another replaces it.
This changes it to toggle buttons: any number of forms can be active
simultaneously, and the fretboard shows the union of their positions.

## Data model

- `ScaleViewApp.active_form: Option<usize>` → `active_forms: BTreeSet<usize>`
  (`src/gui/app.rs`).
- `Settings.last_form: BTreeMap<String, String>` → `last_forms: BTreeMap<String, BTreeSet<String>>`
  (`src/settings.rs`). The old field simply disappears from saved JSON on next
  save; the new one defaults to empty — no migration code, matching how the
  codebase already tolerates settings shape changes (`#[serde(default)]` at
  the struct level, unknown fields ignored).

## Behavior

- `ScaleViewApp::toggle_form(&mut self, index: usize)` replaces `set_active_form`:
  flips `index`'s membership in `active_forms`, then rewrites
  `settings.last_forms[form_key]` from the resulting set of names (removing the
  key entirely if the set is now empty).
- `ScaleViewApp::clear_forms(&mut self)`: empties `active_forms` and removes
  `form_key` from `last_forms`. Replaces the old `set_active_form(None)` call
  for the "All notes" pill.
- `form_positions()`: `None` if `active_forms` is empty; otherwise the union
  (deduplicated via `Pos`'s existing `Ord`) of every active form's
  `transposed()` positions. Everything downstream (`focus()`'s ghosting,
  `play()`'s position source) keeps its current `Option<Vec<Pos>>` shape, so
  no other call site changes.
- `sync_forms()` (on `form_key` change): resolve `last_forms[form_key]` (a set
  of names) to indices by name match against the new key's forms; names that
  don't resolve (form renamed/deleted) are silently dropped — same tolerance
  the single-form version already had.
- Deleting a form: capture the active set's *names* before `forms.remove`,
  then re-resolve all of them to new indices afterward (generalizes the
  single-index remap `reorder_and_reselect` already does for drag-reorder).
- Reordering a form (drag-drop): same name-based remap, generalized from one
  index to the whole set.
- Saving a new or edited form (`recording_controls`): toggle the saved form
  **on** afterward (add its new index to `active_forms`) — matches today's
  behavior of selecting the form you just recorded.

## UI (`src/gui/forms_panel.rs`)

- "All notes" pill: `selectable_label(app.active_forms.is_empty(), ...)`,
  click → `clear_forms()`.
- Each form pill: `selectable_label(app.active_forms.contains(&i), text)`,
  click → `toggle_form(i)` (no longer clears the others).
- Edit (✏) and Delete (🗑) move from the shared buttons below the list to a
  per-row icon pair next to each form's grip, acting on that row's index
  directly — they no longer depend on what's toggled on.
- Left/Right arrow-key form-cycling in `ScaleViewApp::shortcuts()` is removed
  (no replacement — multi-select has no natural "next form").

## Header (`src/gui/header.rs`)

- Status line: "Form: {name}" → "Forms: {names joined by \", \"}" when
  `active_forms` is non-empty; the "◀ ▶ arrow keys switch forms" hint line is
  removed (no longer true).

## Testing

No new automated tests. This is UI/state wiring with no logic complex enough
to warrant one, and the codebase has no UI test harness — same situation as
the drag-and-drop reorder work (Task 4 of the previous feature), which also
shipped without one. Verified instead by `cargo build`/`cargo test` (no
regressions) plus a manual run-through.

## Out of scope

- No keyboard shortcut replacement for cycling forms.
- No limit on how many forms can be active at once.
- No change to how a single active form's positions are computed or
  transposed — only how many contribute to the union.
