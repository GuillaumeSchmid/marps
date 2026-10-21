//! Where settings and shapes live: ~/.config/scaleview on desktop, localStorage in the browser.

use serde::{Serialize, de::DeserializeOwned};

pub fn load<T: DeserializeOwned + Default>(file: &str) -> T {
    try_load(file).unwrap_or_default()
}

/// Like `load`, but None when `file` is missing or unreadable.
pub fn try_load<T: DeserializeOwned>(file: &str) -> Option<T> {
    read(file).and_then(|s| match serde_json::from_str(&s) {
        Ok(v) => Some(v),
        Err(e) => {
            log_error(&format!("ignoring unreadable {file}: {e}"));
            None
        }
    })
}

pub fn save<T: Serialize>(file: &str, value: &T) {
    if let Err(e) = write(file, &serde_json::to_string_pretty(value).unwrap()) {
        log_error(&format!("could not save {file}: {e}"));
    }
}

pub fn log_error(msg: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{msg}");
    #[cfg(target_arch = "wasm32")]
    log::error!("{msg}");
}

#[cfg(not(target_arch = "wasm32"))]
fn path(file: &str) -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "scaleview").map(|d| d.config_dir().join(file))
}

#[cfg(not(target_arch = "wasm32"))]
fn read(file: &str) -> Option<String> {
    path(file).and_then(|p| std::fs::read_to_string(p).ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn write(file: &str, contents: &str) -> Result<(), String> {
    let p = path(file).ok_or("no config directory")?;
    p.parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(&p, contents))
        .map_err(|e| format!("{}: {e}", p.display()))
}

#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn read(file: &str) -> Option<String> {
    local_storage()?.get_item(&format!("scaleview/{file}")).ok()?
}

#[cfg(target_arch = "wasm32")]
fn write(file: &str, contents: &str) -> Result<(), String> {
    local_storage()
        .ok_or("localStorage unavailable")?
        .set_item(&format!("scaleview/{file}"), contents)
        .map_err(|e| format!("{e:?}"))
}
