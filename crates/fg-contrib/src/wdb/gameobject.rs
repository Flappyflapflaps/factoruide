//! Objects from the game client cache (`Cache/WDB/<locale>/gameobjectcache.wdb`, magic WGOB).
//!
//! Record (the server's object query answer, the same on Classic Era, TBC and Forever):
//!   i32 type | i32 display | 4 names, icon, cast bar caption and one more string, each NUL
//!   terminated | 35 x i32 data (meaning by type: chests have their loot id at 1) | f32 size |
//!   u8 quest item count n | n x i32 quest items | i32 content tuning | i32 required level
//! The record must end exactly at its last field (checked on every record of the caches at hand,
//! and type, name and chest loot ids against the database).

use super::LocaleCache;
use super::reader::{Reader, Result, WdbError, text};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// GameObjectType of chests (and herbs, veins, quest containers).
pub const CHEST: u8 = 3;
const DATA_FIELDS: usize = 35;
const MAX_STRING: usize = 1024;
const MAX_QUEST_ITEMS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedObject {
    /// GameObjectType: 2 quest giver, 3 chest, 5 generic, 8 spell focus, 10 goober, 25 fishing
    /// hole...
    pub object_type: u8,
    pub name: String,
    /// Quest items it can give.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quest_items: Vec<u32>,
    /// Chests: their loot id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loot: Option<u32>,
}

pub type ObjectCache = LocaleCache<CachedObject>;

/// An object record.
pub fn parse_object(rec: &[u8]) -> Result<CachedObject> {
    let mut r = Reader::new(rec);
    let object_type = u8::try_from(r.i32()?).map_err(|_| WdbError::Invalid("object type"))?;
    r.skip(4)?; // display
    let name = text(r.cstr(MAX_STRING, "name")?, "name")?;
    for _ in 0..6 {
        r.cstr(MAX_STRING, "object text")?;
    }
    let mut data = [0; DATA_FIELDS];
    for d in &mut data {
        *d = r.i32()?;
    }
    r.skip(4)?; // size
    let quest_items = usize::from(r.u8()?);
    if quest_items > MAX_QUEST_ITEMS {
        return Err(WdbError::Limit("quest items"));
    }
    let quest_items = (0..quest_items)
        .map(|_| {
            u32::try_from(r.i32()?)
                .ok()
                .filter(|i| *i > 0)
                .ok_or(WdbError::Invalid("quest item"))
        })
        .collect::<Result<Vec<u32>>>()?;
    r.skip(8)?; // content tuning, required level
    r.finish()?;
    Ok(CachedObject {
        object_type,
        name,
        quest_items,
        loot: (object_type == CHEST)
            .then(|| u32::try_from(data[1]).ok().filter(|l| *l > 0))
            .flatten(),
    })
}

/// Every object cache of a game client folder (`_classic_era_`...), one per locale. Records
/// that do not read exactly are skipped.
pub fn read_object_cache(client_dir: &Path) -> Vec<ObjectCache> {
    super::read_locales(client_dir, "gameobjectcache.wdb", "BOGW", |_, _, rec| {
        parse_object(rec).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(object_type: i32, name: &str, loot: i32, quest_items: &[i32]) -> Vec<u8> {
        let mut rec = Vec::new();
        rec.extend(object_type.to_le_bytes());
        rec.extend(259i32.to_le_bytes());
        rec.extend(name.bytes().chain([0]));
        rec.extend([0; 6]);
        let mut data = [0i32; DATA_FIELDS];
        data[0] = 57;
        data[1] = loot;
        for d in data {
            rec.extend(d.to_le_bytes());
        }
        rec.extend(1f32.to_le_bytes());
        rec.push(quest_items.len() as u8);
        for item in quest_items {
            rec.extend(item.to_le_bytes());
        }
        rec.extend([0; 8]);
        rec
    }

    #[test]
    fn decodes_a_chest_with_quest_items() {
        assert_eq!(
            parse_object(&record(3, "Equipment Boxes", 2385, &[5051, 5052])),
            Ok(CachedObject {
                object_type: 3,
                name: "Equipment Boxes".into(),
                quest_items: vec![5051, 5052],
                loot: Some(2385),
            })
        );
        // Only chests have a loot id.
        let door = parse_object(&record(0, "Doors", 2385, &[])).unwrap();
        assert_eq!((door.loot, door.quest_items.is_empty()), (None, true));
    }

    #[test]
    fn rejects_truncated_and_trailing_records() {
        let rec = record(3, "Sealed Crate", 13, &[4105]);
        for len in 0..rec.len() {
            assert!(parse_object(&rec[..len]).is_err(), "decoded {len} bytes");
        }
        let mut longer = rec;
        longer.push(0);
        assert_eq!(parse_object(&longer), Err(WdbError::Trailing));
    }

    #[test]
    fn caps_strings_counts_and_values() {
        let long_name = "x".repeat(MAX_STRING + 1);
        assert_eq!(
            parse_object(&record(3, &long_name, 1, &[])),
            Err(WdbError::Limit("name"))
        );
        let many = vec![1; MAX_QUEST_ITEMS + 1];
        assert_eq!(
            parse_object(&record(3, "Crate", 1, &many)),
            Err(WdbError::Limit("quest items"))
        );
        assert!(parse_object(&record(3, "Crate", 1, &[-1])).is_err());
        assert!(parse_object(&record(-1, "Crate", 1, &[])).is_err());
        let mut bad_utf8 = record(3, "Crate", 1, &[]);
        bad_utf8[8] = 0xFF;
        assert_eq!(parse_object(&bad_utf8), Err(WdbError::Invalid("name")));
    }
}
