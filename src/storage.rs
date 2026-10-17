//! Where settings and forms live on disk (~/.config/scaleview on Linux).

use serde::{Serialize, de::DeserializeOwned};
use std::path::PathBuf;

pub fn path(file: &str) -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "scaleview").map(|d| d.config_dir().join(file))
}

pub fn load<T: DeserializeOwned + Default>(file: &str) -> T {
    path(file)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| match serde_json::from_str(&s) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("ignoring unreadable {file}: {e}");
                None
            }
        })
        .unwrap_or_default()
}

pub fn save<T: Serialize>(file: &str, value: &T) {
    let Some(p) = path(file) else { return };
    let result = p
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&p, serde_json::to_string_pretty(value).unwrap()));
    if let Err(e) = result {
        eprintln!("could not save {}: {e}", p.display());
    }
}
