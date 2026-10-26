//! The SavedVariables of the released addons (tests/saved-variables, one per release whose saved
//! data changed, see its README) are still read by the desktop app: the character's profile and
//! the events the collector recorded.
#![cfg(feature = "local")]
#![expect(clippy::unwrap_used, reason = "fixtures that cannot be read fail the test")]

use fg_contrib::{collector, profile};
use std::path::PathBuf;

fn fixtures() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/saved-variables");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "lua"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no SavedVariables in {}", dir.display());
    files
}

#[test]
fn the_profiles_of_released_addons_are_read() {
    for file in fixtures() {
        let what = file.display().to_string();
        let profiles = profile::read(std::slice::from_ref(&file)).unwrap();
        assert_eq!(profiles.len(), 1, "{what}");
        let p = &profiles[0];
        assert_eq!(p["name"], "Brouz-Realm", "{what}");
        assert_eq!((p["level"].as_i64(), p["race"].as_i64()), (Some(4), Some(7)), "{what}");
        assert_eq!(p["class"], "MAGE", "{what}");
        assert_eq!(p["flights"], serde_json::json!([2, 12]), "{what}: the known ones");
        assert_eq!(p["completed"], serde_json::json!([179, 233]), "{what}");
        assert_eq!(p["bind"]["name"], "Kharanos", "{what}");
    }
}

#[test]
fn the_recordings_of_released_addons_are_read() {
    for file in fixtures() {
        let what = file.display().to_string();
        let characters = collector::read(std::slice::from_ref(&file)).unwrap();
        assert_eq!(characters.len(), 1, "{what}");
        let c = &characters[0];
        assert_eq!(c.character, "Brouz-Realm", "{what}");
        assert_eq!(c.class.as_deref(), Some("MAGE"), "{what}");
        // Rows as text and as a table (the first addons'), each with its number and event.
        let rows: Vec<_> = c.events.iter().map(|e| (e.seq, e.event.as_deref())).collect();
        assert_eq!(
            rows,
            [
                (Some(101), Some("kill")),
                (Some(102), Some("kill")),
                (Some(103), Some("kill")),
                (Some(104), Some("loot"))
            ],
            "{what}"
        );
        let first = &c.events[0];
        assert_eq!((first.entity_id, first.ui_map_id), (Some(6), Some(1426)), "{what}");
        assert_eq!(
            (first.x, first.y, first.level),
            (Some(30.0), Some(70.0), Some(1)),
            "{what}"
        );
    }
}
