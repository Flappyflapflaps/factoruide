//! The library over two kinds of files: the disk, and a map in memory (as the browser's).
#![expect(clippy::unwrap_used, reason = "the helpers of a test fail it")]

use fg_library::{DiskFiles, Files, Library, LibraryError, SharedGuide};
use fg_route::xp::Edition;
use std::collections::BTreeMap;
use std::io;
use std::sync::Mutex;

#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, Vec<u8>>>);

impl Files for Memory {
    fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        self.0
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "not found"))
    }

    fn write(&self, path: &str, data: &[u8]) -> io::Result<()> {
        self.0.lock().unwrap().insert(path.to_owned(), data.to_vec());
        Ok(())
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{dir}/");
        let mut names: Vec<String> = self
            .0
            .lock()
            .unwrap()
            .keys()
            .filter_map(|k| k.strip_prefix(&prefix)?.split('/').next().map(str::to_owned))
            .collect();
        names.dedup();
        names
    }

    fn remove_dir(&self, dir: &str) -> io::Result<()> {
        let prefix = format!("{dir}/");
        let mut files = self.0.lock().unwrap();
        let before = files.len();
        files.retain(|k, _| !k.starts_with(&prefix));
        if files.len() == before {
            return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
        }
        Ok(())
    }
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fg-library-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_library_without_files_has_no_guides() {
    let files = Memory::default();
    let library = Library::new(&files, Edition::Classic);
    assert_eq!(library.ids().len(), 0);
    assert_eq!(library.list().len(), 0);
    assert_eq!(library.followed().len(), 0);
    assert_eq!(library.walk_memories().len(), 0);
}

#[test]
fn a_missing_guide_is_named_in_the_error() {
    let files = Memory::default();
    let library = Library::new(&files, Edition::Classic);
    let e = library.delete_guide("nobody").unwrap_err();
    assert!(matches!(e, LibraryError::NoGuide(_)));
    assert_eq!(e.to_string(), "no guide nobody");
    let e = library.version("nobody", 1).unwrap_err();
    assert_eq!(e.to_string(), "guides/classic/nobody/v1/outcome.json: not found");
}

#[test]
fn a_summary_needs_the_guide_file() {
    let files = Memory::default();
    files
        .write("guides/classic/a/guide.json", br#"{"name":"A","installed":2}"#)
        .unwrap();
    let library = Library::new(&files, Edition::Classic);
    let summary = library.summary("a").unwrap();
    assert_eq!((summary.name.as_str(), summary.installed), ("A", Some(2)));
    assert_eq!(summary.versions.len(), 0);
    // A guide without a version is not listed.
    assert_eq!(library.list().len(), 0);
    library.set_installed("a", None).unwrap();
    assert_eq!(library.info("a").unwrap().installed, None);
}

#[test]
fn a_corrupt_file_says_which() {
    let files = Memory::default();
    files.write("guides/tbc/a/guide.json", b"{").unwrap();
    let e = Library::new(&files, Edition::Tbc).info("a").unwrap_err();
    assert!(matches!(e, LibraryError::Corrupt { .. }));
    assert!(e.to_string().starts_with("guides/tbc/a/guide.json: "), "{e}");
}

#[test]
fn editions_do_not_share_guides() {
    let files = Memory::default();
    files.write("guides/classic/a/guide.json", br#"{"name":"A"}"#).unwrap();
    assert_eq!(Library::new(&files, Edition::Classic).ids(), ["a"]);
    assert_eq!(Library::new(&files, Edition::Tbc).ids().len(), 0);
}

#[test]
fn files_on_the_disk_make_their_folders_and_report_the_missing() {
    let root = temp_root("disk");
    let files = DiskFiles::new(&root);
    files.write("guides/classic/a/guide.json", br#"{"name":"A"}"#).unwrap();
    assert_eq!(files.list("guides/classic"), ["a"]);
    assert_eq!(files.list("guides/none").len(), 0);
    let library = Library::new(&files, Edition::Classic);
    assert_eq!(library.info("a").unwrap().name, "A");
    library.delete_guide("a").unwrap();
    assert!(matches!(library.delete_guide("a"), Err(LibraryError::NoGuide(_))));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_file_that_is_not_a_guide_is_refused() {
    for content in ["", "[]", r#"{"format":"other"}"#, "not json"] {
        assert!(
            matches!(SharedGuide::parse(content), Err(LibraryError::NotAGuideFile(_))),
            "{content:?}"
        );
    }
}

proptest::proptest! {
    /// A file received is a guide or is refused, whatever it holds.
    #[test]
    fn any_text_is_a_guide_or_refused(text in ".{0,512}") {
        let _ = SharedGuide::parse(&text);
    }
}
