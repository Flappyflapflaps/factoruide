use std::fmt::Write as _;
use std::path::Path;

/// `(name, include_bytes!(path))` entries for the files of `dir` with one of `extensions`; with
/// `subdirs`, also those one folder down, named `folder/file`.
fn files(dir: &Path, extensions: &[&str], skip: &[&str], subdirs: bool) -> String {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
    {
        let p = entry.path();
        if p.is_dir() && subdirs {
            println!("cargo:rerun-if-changed={}", p.display());
            for sub in std::fs::read_dir(&p).unwrap().flatten() {
                let s = sub.path();
                if s.extension().is_some_and(|e| extensions.iter().any(|x| e == *x)) {
                    entries.push((
                        format!(
                            "{}/{}",
                            entry.file_name().to_string_lossy(),
                            sub.file_name().to_string_lossy()
                        ),
                        s,
                    ));
                }
            }
        } else if p.extension().is_some_and(|e| extensions.iter().any(|x| e == *x))
            && !skip.iter().any(|s| p.file_name().is_some_and(|n| n == *s))
        {
            entries.push((entry.file_name().to_string_lossy().into_owned(), p));
        }
    }
    entries.sort();
    entries.iter().fold(String::new(), |mut out, (name, p)| {
        let path = p.canonicalize().unwrap();
        let _ = writeln!(out, "    ({name:?}, include_bytes!({:?})),", path.to_str().unwrap());
        out
    })
}

/// Embeds the game databases (one per edition, from `fg --edition <e> bundle`) and their map
/// images, the overrides and the addon in the app.
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("data");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut databases = String::new();
    for edition in ["forever", "classic", "tbc"] {
        let db = dir.join(format!("{edition}.sqlite.zst"));
        println!("cargo:rerun-if-changed={}", db.display());
        if let Ok(path) = db.canonicalize() {
            let _ = writeln!(
                databases,
                "    ({edition:?}, include_bytes!({:?})),",
                path.to_str().unwrap()
            );
        }
    }
    assert!(
        !databases.is_empty(),
        "no database in {}: run `fg bundle`",
        dir.display()
    );
    let out = format!(
        "/// (edition, zstd database) of every edition bundled.\n\
         pub const DATABASES: &[(&str, &[u8])] = &[\n{databases}];\n\
         /// (\"<edition>/<map id>.webp\", image) of every edition's maps.\n\
         pub const MAPS: &[(&str, &[u8])] = &[\n{}];\n\
         pub const OVERRIDES: &[(&str, &[u8])] = &[\n{}];\n\
         /// The addon's two parts: the profile and recordings, and the guide.\n\
         pub const PROFILE_ADDON: &[(&str, &[u8])] = &[\n{}];\n\
         pub const GUIDE_ADDON: &[(&str, &[u8])] = &[\n{}];\n",
        files(&dir.join("maps"), &["webp"], &[], true),
        files(&root.join("overrides"), &["toml"], &[], true),
        // Routes.lua, Config.lua and Sent.lua are written on install; textures are in Media/.
        files(
            &root.join("addon/Factoruide_Profile"),
            &["lua", "toc", "xml"],
            &["Sent.lua"],
            true
        ),
        files(
            &root.join("addon/Factoruide_Guide"),
            &["lua", "toc", "xml", "tga"],
            &["Routes.lua", "Config.lua"],
            true
        ),
    );
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs"), out).unwrap();
    tauri_build::build();
}
