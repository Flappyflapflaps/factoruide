use std::fmt::Write as _;
use std::path::Path;

/// `(name, include_bytes!(path))` entries for the files of `dir` with one of `extensions`, and
/// those of its subfolders (named `folder/file`), but `skip`.
fn files(dir: &Path, prefix: &str, extensions: &[&str], skip: &[&str], out: &mut Vec<(String, std::path::PathBuf)>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
    {
        let p = entry.path();
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if p.is_dir() {
            files(&p, &format!("{name}/"), extensions, skip, out);
        } else if p.extension().is_some_and(|e| extensions.iter().any(|x| e == *x)) && !skip.contains(&name.as_str()) {
            out.push((name, p));
        }
    }
}

fn list(entries: &mut [(String, std::path::PathBuf)]) -> String {
    entries.sort();
    entries.iter().fold(String::new(), |mut out, (name, p)| {
        let path = p.canonicalize().unwrap();
        let _ = writeln!(out, "    ({name:?}, include_bytes!({:?})),", path.to_str().unwrap());
        out
    })
}

/// Embeds the overrides in the backend; the databases and maps are static files the page
/// serves.
fn main() {
    println!("cargo:rerun-if-env-changed=FG_VERSION");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut overrides = Vec::new();
    files(&root.join("overrides"), "", &["toml"], &[], &mut overrides);
    let out = format!(
        "pub const OVERRIDES: &[(&str, &[u8])] = &[\n{}];\n",
        list(&mut overrides)
    );
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("embedded.rs"), out).unwrap();
}
