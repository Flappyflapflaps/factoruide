//! Creatures from the game client cache (`Cache/WDB/<locale>/creaturecache.wdb`, magic WMOB).
//!
//! Record (the server's creature query answer):
//!   bit-packed, most significant bit first: title length 11 | alternate title length 11 |
//!   cursor name length 6 | [civilian 1] | leader 1 | 4 x (name length 11, alternate name length
//!   11), padded to a byte; lengths count the ending NUL, 0 for an absent string
//!   the 4 names and alternate names, NUL terminated
//!   3 x u32 flags | u8 creature type | i32 family | i8 classification | [u32 pet spell list] |
//!   2 x u32 proxy creature | u32 display count n | f32 total probability | n x (u32 display, f32
//!   scale, f32 probability) | f32 health multiplier | f32 power multiplier | u32 quest item
//!   count q | u32 quest currency count c | 8 x i32 (movement, health scaling expansion,
//!   required expansion, vignette, class, difficulty, widget set, widget condition)
//!   title, alternate title, cursor name | q x i32 quest items | c x i32 currencies
//! The bracketed fields are those of Classic Era and TBC, not Forever. The record must end
//! exactly at its last field, which tells the two layouts apart (checked on every record of the
//! Classic Era, TBC Anniversary and Forever caches at hand, and type, family and classification
//! against the database).

use super::LocaleCache;
use super::reader::{Bits, Reader, Result, WdbError, text};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Caps of the lists of a record (the game sends a few entries at most).
const MAX_DISPLAYS: usize = 64;
const MAX_QUEST_ITEMS: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedCreature {
    pub name: String,
    /// 0 normal, 1 elite, 2 rare elite, 3 world boss, 4 rare (5 trivial, 6 minus on later
    /// clients).
    pub classification: u8,
    /// CreatureType: 1 beast, 2 dragonkin, 3 demon, 4 elemental, 5 giant, 6 undead, 7 humanoid,
    /// 8 critter, 9 mechanical, 10 not specified, 11 totem, 12 non-combat pet...
    pub creature_type: u8,
    /// CreatureFamily of beasts and pets, 0 for none.
    pub family: i32,
    /// Quest items the creature can drop (only Forever servers send them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quest_items: Vec<u32>,
}

pub type CreatureCache = LocaleCache<CachedCreature>;

/// A creature record, with the layout that reads it exactly.
pub fn parse_creature(rec: &[u8]) -> Result<CachedCreature> {
    parse(rec, false).or_else(|e| parse(rec, true).map_err(|_| e))
}

/// Every creature cache of a game client folder (`_classic_era_`...), one per locale. Records
/// that no layout reads are skipped.
pub fn read_creature_cache(client_dir: &Path) -> Vec<CreatureCache> {
    super::read_locales(client_dir, "creaturecache.wdb", "BOMW", |_, _, rec| {
        parse_creature(rec).ok()
    })
}

/// `classic`: the Classic Era and TBC layout (civilian bit, pet spell list).
fn parse(rec: &[u8], classic: bool) -> Result<CachedCreature> {
    let mut bits = Bits::new(rec);
    let title = bits.read(11)? as usize;
    let title_alt = bits.read(11)? as usize;
    let cursor = bits.read(6)? as usize;
    bits.read(u32::from(classic) + 1)?; // [civilian], leader
    let mut names = [(0, 0); 4];
    for (name, alt) in &mut names {
        *name = bits.read(11)? as usize;
        *alt = bits.read(11)? as usize;
    }
    let mut r = Reader::at(rec, bits.flush())?;
    let mut name = String::new();
    for (i, (len, alt)) in names.into_iter().enumerate() {
        let s = string(&mut r, len)?;
        if i == 0 {
            name = text(s, "name")?;
        }
        string(&mut r, alt)?;
    }
    r.skip(12)?; // flags
    let creature_type = r.u8()?;
    let family = r.i32()?;
    let classification = u8::try_from(r.i8()?)
        .ok()
        .filter(|c| *c <= 6)
        .ok_or(WdbError::Invalid("classification"))?;
    r.skip(if classic { 4 } else { 0 } + 8)?; // [pet spell list], proxy creatures
    let displays = r.count(MAX_DISPLAYS, "displays")?;
    r.skip(4 + 12 * displays + 8)?; // total probability, displays, multipliers
    let quest_items = r.count(MAX_QUEST_ITEMS, "quest items")?;
    let currencies = r.count(MAX_QUEST_ITEMS, "quest currencies")?;
    r.skip(32)?;
    for len in [title, title_alt, cursor] {
        string(&mut r, len)?;
    }
    let quest_items = (0..quest_items)
        .map(|_| {
            u32::try_from(r.i32()?)
                .ok()
                .filter(|i| *i > 0)
                .ok_or(WdbError::Invalid("quest item"))
        })
        .collect::<Result<Vec<u32>>>()?;
    r.skip(4 * currencies)?;
    r.finish()?;
    Ok(CachedCreature {
        name,
        classification,
        creature_type,
        family,
        quest_items,
    })
}

/// A string whose length counts its ending NUL, without it.
fn string<'a>(r: &mut Reader<'a>, len: usize) -> Result<&'a [u8]> {
    if len == 0 {
        return Ok(&[]);
    }
    match r.take(len)?.split_last() {
        Some((0, s)) => Ok(s),
        _ => Err(WdbError::Invalid("string end")),
    }
}

#[cfg(test)]
mod tests {
    use super::super::reader::pack_bits;
    use super::*;

    /// A creature record of either layout.
    fn record(classic: bool, name: &str, title: &str, classification: i8, quest_items: &[i32]) -> Vec<u8> {
        let len = |s: &str| if s.is_empty() { 0 } else { s.len() as u32 + 1 };
        let mut header = vec![(len(title), 11), (0, 11), (0, 6)];
        if classic {
            header.push((1, 1));
        }
        header.push((0, 1));
        header.extend([(len(name), 11), (0, 11)]);
        header.extend([(0, 11); 6]);
        let mut rec = pack_bits(&header);
        rec.extend(name.bytes().chain([0]));
        rec.extend([0; 12]);
        rec.push(7);
        rec.extend(0i32.to_le_bytes());
        rec.push(classification as u8);
        if classic {
            rec.extend([0; 4]);
        }
        rec.extend([0; 8]);
        rec.extend(1u32.to_le_bytes());
        rec.extend(1f32.to_le_bytes());
        rec.extend(3381u32.to_le_bytes());
        rec.extend(1f32.to_le_bytes());
        rec.extend(1f32.to_le_bytes());
        rec.extend(1f32.to_le_bytes());
        rec.extend(1f32.to_le_bytes());
        rec.extend((quest_items.len() as u32).to_le_bytes());
        rec.extend(0u32.to_le_bytes());
        rec.extend([0; 32]);
        if !title.is_empty() {
            rec.extend(title.bytes().chain([0]));
        }
        for item in quest_items {
            rec.extend(item.to_le_bytes());
        }
        rec
    }

    #[test]
    fn decodes_both_layouts_with_quest_items() {
        for classic in [false, true] {
            let c = parse_creature(&record(classic, "Defias Convict", "", 1, &[2909, 7906])).unwrap();
            assert_eq!(
                c,
                CachedCreature {
                    name: "Defias Convict".into(),
                    classification: 1,
                    creature_type: 7,
                    family: 0,
                    quest_items: vec![2909, 7906],
                }
            );
        }
        let trainer = parse_creature(&record(true, "Pelturas Whitemoon", "Druid Trainer", 0, &[])).unwrap();
        assert_eq!(trainer.name, "Pelturas Whitemoon");
        assert_eq!(trainer.quest_items, Vec::<u32>::new());
    }

    #[test]
    fn rejects_truncated_and_trailing_records() {
        let rec = record(false, "Kobold Vermin", "", 0, &[772]);
        for len in 0..rec.len() {
            assert!(parse_creature(&rec[..len]).is_err(), "decoded {len} bytes");
        }
        let mut longer = rec.clone();
        longer.push(0);
        assert!(parse_creature(&longer).is_err());
    }

    #[test]
    fn caps_counts_and_values() {
        let mut rec = record(false, "Kobold Vermin", "", 0, &[]);
        // The quest item count (then the currency count and 32 bytes) set to a huge value.
        let at = rec.len() - 36 - 4;
        rec[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            parse(&rec, false),
            Err(WdbError::Limit("quest items")),
            "a huge count is refused before reading"
        );
        assert!(parse_creature(&record(false, "Kobold Vermin", "", 9, &[])).is_err());
        assert!(parse_creature(&record(false, "Kobold Vermin", "", 0, &[-5])).is_err());
    }
}
