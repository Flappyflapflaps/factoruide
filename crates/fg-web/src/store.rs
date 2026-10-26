//! The app's files (settings, guides, characters...), kept in memory and saved by the page in
//! the browser's storage at each change. Paths are relative, separated by `/`.

use fg_route::sync::LockExt;
use std::collections::BTreeMap;
use std::sync::Mutex;

static FILES: Mutex<BTreeMap<String, Vec<u8>>> = Mutex::new(BTreeMap::new());

/// A file the page had saved, given back at startup (not saved again).
#[cfg(any(target_os = "wasi", test))]
pub fn load(path: &str, data: Vec<u8>) {
    FILES.locked().insert(path.to_owned(), data);
}

pub fn read(path: &str) -> Option<Vec<u8>> {
    FILES.locked().get(path).cloned()
}

pub fn read_string(path: &str) -> Result<String, String> {
    let bytes = read(path).ok_or_else(|| format!("{path}: not found"))?;
    String::from_utf8(bytes).map_err(|e| format!("{path}: {e}"))
}

pub fn write(path: &str, data: impl Into<Vec<u8>>) {
    let data = data.into();
    crate::host::persist(path, Some(&data));
    FILES.locked().insert(path.to_owned(), data);
}

/// Remove a folder: every file under `dir`. Returns whether there was one.
pub fn remove_dir(dir: &str) -> bool {
    let prefix = format!("{}/", dir.trim_end_matches('/'));
    let gone: Vec<String> = {
        let mut files = FILES.locked();
        let gone: Vec<String> = files.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        for k in &gone {
            files.remove(k);
        }
        gone
    };
    for k in &gone {
        crate::host::persist(k, None);
    }
    !gone.is_empty()
}

/// The names in a folder (files and folders, once each, sorted).
pub fn list(dir: &str) -> Vec<String> {
    let prefix = format!("{}/", dir.trim_end_matches('/'));
    let files = FILES.locked();
    let mut out: Vec<String> = files
        .range(prefix.clone()..)
        .take_while(|(k, _)| k.starts_with(&prefix))
        .filter_map(|(k, _)| k[prefix.len()..].split('/').next().map(str::to_owned))
        .collect();
    out.dedup();
    out
}

#[cfg(test)]
pub fn clear() {
    FILES.locked().clear();
}
