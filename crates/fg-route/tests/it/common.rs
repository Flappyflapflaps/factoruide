//! What the integration tests share: the repository's place and the editions' databases.

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The Classic database, unpacked next to the packed one when missing or older.
pub fn database() -> Connection {
    database_of("classic")
}

/// An edition's database (classic, tbc, forever), unpacked the same way.
pub fn database_of(edition: &str) -> Connection {
    static UNPACK: Mutex<()> = Mutex::new(());
    let _guard = UNPACK.lock().unwrap();
    let (path, packed) = (
        repo().join(format!("data/{edition}.sqlite")),
        repo().join(format!("data/{edition}.sqlite.zst")),
    );
    let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if !path.exists() || modified(&packed) > modified(&path) {
        let data = zstd::decode_all(std::fs::File::open(&packed).unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();
    }
    Connection::open(path).unwrap()
}
