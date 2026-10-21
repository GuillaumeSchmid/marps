//! Shapes: named, user-recorded ways of playing a scale, arpeggio or chord.
//! A shape is stored with the root it was recorded on and transposed to any other root.

use crate::storage;
use crate::theory::Pos;
use crate::theory::tuning::FRETS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use zip::write::SimpleFileOptions;

const FILE: &str = "shapes.json";
/// Where shapes were saved before they were renamed from "forms"; still read once to migrate.
const LEGACY_FILE: &str = "forms.json";
/// Name of the JSON entry inside an exported zip (`LEGACY_ENTRY` in older exports).
const ENTRY: &str = "shapes.json";
const LEGACY_ENTRY: &str = "forms.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub name: String,
    pub root_pc: u8,
    pub positions: Vec<Pos>,
}

impl Shape {
    /// The shape's positions for another root: shifted by the smallest move that
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

/// All shapes, keyed by tuning + root-independent selection key.
#[derive(Default, Serialize, Deserialize)]
pub struct ShapeLibrary {
    #[serde(alias = "forms")]
    shapes: BTreeMap<String, Vec<Shape>>,
}

impl ShapeLibrary {
    pub fn load() -> Self {
        storage::try_load(FILE).or_else(|| storage::try_load(LEGACY_FILE)).unwrap_or_default()
    }

    fn save(&self) {
        storage::save(FILE, self);
    }

    pub fn key(tuning: &str, shape_key: &str) -> String {
        format!("{tuning}|{shape_key}")
    }

    pub fn shapes(&self, key: &str) -> &[Shape] {
        self.shapes.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Insert or replace (by index) a shape, returning its index.
    pub fn put(&mut self, key: &str, index: Option<usize>, shape: Shape) -> usize {
        let list = self.shapes.entry(key.to_string()).or_default();
        let i = match index {
            Some(i) if i < list.len() => {
                list[i] = shape;
                i
            }
            _ => {
                list.push(shape);
                list.len() - 1
            }
        };
        self.save();
        i
    }

    pub fn remove(&mut self, key: &str, index: usize) {
        if let Some(list) = self.shapes.get_mut(key) {
            if index < list.len() {
                list.remove(index);
            }
            if list.is_empty() {
                self.shapes.remove(key);
            }
        }
        self.save();
    }

    /// Move the shape at `from` to `to` within one key's list, then persist.
    pub fn reorder(&mut self, key: &str, from: usize, to: usize) {
        self.reorder_in_memory(key, from, to);
        self.save();
    }

    fn reorder_in_memory(&mut self, key: &str, from: usize, to: usize) {
        if let Some(list) = self.shapes.get_mut(key)
            && from < list.len()
            && to < list.len()
            && from != to
        {
            let item = list.remove(from);
            list.insert(to, item);
        }
    }

    /// Merge another library in: a shape with the same name in the same key is
    /// replaced in place by the incoming one; anything new is appended.
    pub fn merge(&mut self, other: ShapeLibrary) {
        self.merge_in_memory(other);
        self.save();
    }

    fn merge_in_memory(&mut self, other: ShapeLibrary) {
        for (key, incoming) in other.shapes {
            let list = self.shapes.entry(key).or_default();
            for shape in incoming {
                match list.iter_mut().find(|f| f.name == shape.name) {
                    Some(slot) => *slot = shape,
                    None => list.push(shape),
                }
            }
        }
    }

    /// Serialize to JSON and pack it as `shapes.json` inside a new zip archive.
    pub fn export_zip(&self) -> std::io::Result<Vec<u8>> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(ENTRY, options)?;
        zip.write_all(json.as_bytes())?;
        Ok(zip.finish()?.into_inner())
    }

    /// Read a zip archive and deserialize its `shapes.json` (or older `forms.json`) entry.
    pub fn import_zip(bytes: &[u8]) -> std::io::Result<ShapeLibrary> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        let name = if archive.index_for_name(ENTRY).is_some() { ENTRY } else { LEGACY_ENTRY };
        let mut entry = archive.by_name(name)?;
        let mut s = String::new();
        entry.read_to_string(&mut s)?;
        serde_json::from_str(&s).map_err(std::io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transposition_prefers_small_moves_and_stays_on_neck() {
        let f = Shape { name: "x".into(), root_pc: 0, positions: vec![Pos::new(1, 3), Pos::new(2, 5)] };
        // C -> D: up two frets.
        assert_eq!(f.transposed(2), [Pos::new(1, 5), Pos::new(2, 7)]);
        // C -> A: down three frets would go to 0 and 2 — fine.
        assert_eq!(f.transposed(9), [Pos::new(1, 0), Pos::new(2, 2)]);
        // C -> Bb: down two frets.
        assert_eq!(f.transposed(10), [Pos::new(1, 1), Pos::new(2, 3)]);
        let low = Shape { name: "y".into(), root_pc: 0, positions: vec![Pos::new(0, 0)] };
        // Can't go below the nut, so B is played 11 frets up.
        assert_eq!(low.transposed(11), [Pos::new(0, 11)]);
    }

    #[test]
    fn reorder_moves_item_within_key() {
        let key = "standard|Cmajor";
        let mk = |n: &str| Shape { name: n.into(), root_pc: 0, positions: vec![] };
        let mut lib = ShapeLibrary {
            shapes: BTreeMap::from([(key.to_string(), vec![mk("a"), mk("b"), mk("c")])]),
        };
        lib.reorder_in_memory(key, 0, 2);
        let names: Vec<_> = lib.shapes(key).iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
    }

    #[test]
    fn merge_imported_shape_replaces_same_name_in_same_key() {
        let key = "standard|Cmajor";
        let mut lib = ShapeLibrary {
            shapes: BTreeMap::from([(
                key.to_string(),
                vec![
                    Shape { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(0, 0)] },
                    Shape { name: "shape2".into(), root_pc: 0, positions: vec![] },
                ],
            )]),
        };
        let other = ShapeLibrary {
            shapes: BTreeMap::from([(
                key.to_string(),
                vec![
                    Shape { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(1, 1)] },
                    Shape { name: "shape3".into(), root_pc: 0, positions: vec![] },
                ],
            )]),
        };
        lib.merge_in_memory(other);
        let shapes = lib.shapes(key);
        assert_eq!(shapes.len(), 3);
        assert_eq!(shapes[0].name, "shape1");
        assert_eq!(shapes[0].positions, vec![Pos::new(1, 1)]); // imported wins, replaced in place
        assert_eq!(shapes[1].name, "shape2");
        assert_eq!(shapes[2].name, "shape3"); // new shape appended
    }

    #[test]
    fn export_then_import_round_trips() {
        let key = "standard|Cmajor";
        let lib = ShapeLibrary {
            shapes: BTreeMap::from([(
                key.to_string(),
                vec![Shape { name: "shape1".into(), root_pc: 0, positions: vec![Pos::new(2, 3)] }],
            )]),
        };
        let imported = ShapeLibrary::import_zip(&lib.export_zip().unwrap()).unwrap();
        assert_eq!(imported.shapes(key), lib.shapes(key));
    }

    #[test]
    fn imports_export_from_before_the_rename() {
        let json = r#"{"forms":{"standard|Cmajor":[{"name":"old","root_pc":0,"positions":[]}]}}"#;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(LEGACY_ENTRY, SimpleFileOptions::default()).unwrap();
        zip.write_all(json.as_bytes()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let imported = ShapeLibrary::import_zip(&bytes).unwrap();
        assert_eq!(imported.shapes("standard|Cmajor")[0].name, "old");
    }
}
