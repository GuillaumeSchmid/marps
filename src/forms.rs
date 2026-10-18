//! Forms: named, user-recorded ways of playing a scale, arpeggio or chord.
//! A form is stored with the root it was recorded on and transposed to any other root.

use crate::storage;
use crate::theory::Pos;
use crate::theory::tuning::FRETS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const FILE: &str = "forms.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Form {
    pub name: String,
    pub root_pc: u8,
    pub positions: Vec<Pos>,
}

impl Form {
    /// The form's positions for another root: shifted by the smallest move that
    /// keeps every note on the neck (or, failing that, the most notes).
    pub fn transposed(&self, root_pc: u8) -> Vec<Pos> {
        let d = (root_pc as i16 - self.root_pc as i16).rem_euclid(12);
        let shifted = |s: i16| -> Vec<Pos> {
            self.positions
                .iter()
                .filter_map(|p| {
                    let f = p.fret as i16 + s;
                    (0..=FRETS as i16).contains(&f).then(|| Pos::new(p.string, f as u8))
                })
                .collect()
        };
        let mut options = [d, d - 12, d + 12, d - 24];
        options.sort_by_key(|s| s.abs());
        options
            .iter()
            .rev() // max_by_key keeps the last maximum: make that the smallest move
            .map(|&s| shifted(s))
            .max_by_key(|v| v.len())
            .unwrap_or_default()
    }
}

/// All forms, keyed by tuning + root-independent selection key.
#[derive(Default, Serialize, Deserialize)]
pub struct FormLibrary {
    forms: BTreeMap<String, Vec<Form>>,
}

impl FormLibrary {
    pub fn load() -> Self {
        storage::load(FILE)
    }

    fn save(&self) {
        storage::save(FILE, self);
    }

    pub fn key(tuning: &str, shape_key: &str) -> String {
        format!("{tuning}|{shape_key}")
    }

    pub fn forms(&self, key: &str) -> &[Form] {
        self.forms.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Insert or replace (by index) a form, returning its index.
    pub fn put(&mut self, key: &str, index: Option<usize>, form: Form) -> usize {
        let list = self.forms.entry(key.to_string()).or_default();
        let i = match index {
            Some(i) if i < list.len() => {
                list[i] = form;
                i
            }
            _ => {
                list.push(form);
                list.len() - 1
            }
        };
        self.save();
        i
    }

    pub fn remove(&mut self, key: &str, index: usize) {
        if let Some(list) = self.forms.get_mut(key) {
            if index < list.len() {
                list.remove(index);
            }
            if list.is_empty() {
                self.forms.remove(key);
            }
        }
        self.save();
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transposition_prefers_small_moves_and_stays_on_neck() {
        let f = Form { name: "x".into(), root_pc: 0, positions: vec![Pos::new(1, 3), Pos::new(2, 5)] };
        // C -> D: up two frets.
        assert_eq!(f.transposed(2), [Pos::new(1, 5), Pos::new(2, 7)]);
        // C -> A: down three frets would go to 0 and 2 — fine.
        assert_eq!(f.transposed(9), [Pos::new(1, 0), Pos::new(2, 2)]);
        // C -> Bb: down two frets.
        assert_eq!(f.transposed(10), [Pos::new(1, 1), Pos::new(2, 3)]);
        let low = Form { name: "y".into(), root_pc: 0, positions: vec![Pos::new(0, 0)] };
        // Can't go below the nut, so B is played 11 frets up.
        assert_eq!(low.transposed(11), [Pos::new(0, 11)]);
    }

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
}
