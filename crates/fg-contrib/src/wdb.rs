//! The game client's caches (`Cache/WDB/<locale>/*.wdb`): the server's answers to the queries
//! of everything a character has seen, one file per kind and per client locale.
//!   questcache.wdb: quests. It proves the quest exists in the game version and gives exact
//!     objective targets and amounts, the XP reward and more (`QuestInfo`).
//!   creaturecache.wdb: creatures (`creature`), gameobjectcache.wdb: objects (`gameobject`).
//! File: a 24-byte header (magic reversed, build, locale reversed, record size, record version,
//! cache version), then records {u32 id | u32 length | data} until a length of 0.
//!
//! Quest records are read field by field with the layouts of the supported clients (`quest`).
//! When none fits (a client build with another layout), the title, log and XP are still found
//! by the layout-free method below, and the objectives only when it can confirm them:
//!   a fixed part of i32 fields: id | type | quest level | ... | XP difficulty (the column of the
//!   QuestXP table) | XP multiplier (f32: dungeon quests 1.8 to 2.1 on Forever) | money |
//!   money difficulty | money multiplier (f32) | ... The XP difficulty is field 10 on Forever
//!   and field 11 on Classic Era and TBC (one more field before it), told apart by the two
//!   multipliers, the only floats near 1 of either layout (`xp_reward`).
//!   ...
//!   objectives (Forever layout), each: u32 id | u8 type | 4 bytes | i32 object id (+9) | i32
//!                     amount (+13) | ... | i32 visual effect count n (+33) | 4 bytes | n x i32 |
//!                     u8 description length | 1 byte | description
//!   12 bytes of bit-packed string lengths (big endian): title 9, log description 12, quest
//!   description 12, area description 9, giver text 10, giver name 8, turn-in text 10,
//!   turn-in name 8, completion log 11
//!   the strings, ending the record.
//! The string header is found by scanning; the position where a chain of objective records
//! ends exactly at it is preferred (method of Stein-N/WoW-Quest-Database's questcache.py).

pub mod creature;
pub mod gameobject;
mod quest;
mod reader;

pub use creature::{CachedCreature, CreatureCache, parse_creature, read_creature_cache};
pub use gameobject::{CachedObject, ObjectCache, parse_object, read_object_cache};
pub use quest::{ItemDrop, QuestInfo};
pub use reader::WdbError;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

const WIDTHS: [u32; 9] = [9, 12, 12, 9, 10, 8, 10, 8, 11];
const HEADER_LEN: usize = 12;
const OBJ_FIXED: usize = 43;
const MAX_OBJECTIVES: usize = 12;
/// Larger cache files are not read (the largest seen are a few MB).
const MAX_FILE_LEN: u64 = 64 << 20;
/// Larger records are skipped (quests reach 3 KB, creatures and objects 1 KB).
const MAX_RECORD_LEN: usize = 16 << 10;
/// Largest quest record the layout-free method reads.
const MAX_SCANNED_LEN: usize = 4 << 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub kind: u8,
    pub target: i32,
    pub amount: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedQuest {
    pub title: String,
    pub log: String,
    /// Empty when no objective chain could be confirmed.
    pub objectives: Vec<Objective>,
    pub confirmed: bool,
    /// The XP reward, when the fixed part reads as one of the known layouts (absent from
    /// contributions of earlier app versions).
    #[serde(default)]
    pub xp: Option<XpReward>,
    /// The other fields, when a known layout reads the record exactly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<QuestInfo>,
}

/// A quest's XP reward as the server sends it: the game gives `QuestXP[level][difficulty] *
/// multiplier` (rounded, see `round_xp`) before the level difference penalty.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct XpReward {
    pub level: i32,
    pub difficulty: u8,
    pub multiplier: f32,
}

/// The game's rounding of an XP reward: to 5 up to 100, 10 up to 500, 25 up to 1000, 50 above
/// (Forever turn-ins: 405 -> 410, 680 -> 675, 970 -> 975).
pub fn round_xp(xp: f64) -> i64 {
    let xp = xp.max(0.0) as i64;
    let step = match xp {
        0..=100 => 5,
        101..=500 => 10,
        501..=1000 => 25,
        _ => 50,
    };
    (xp + step / 2) / step * step
}

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn i32_at(b: &[u8], p: usize) -> i32 {
    i32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn f32_at(b: &[u8], p: usize) -> f32 {
    f32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}

/// The XP reward of a record's fixed part: the difficulty field followed by the XP multiplier,
/// and the money multiplier three fields further, both floats near 1 (an integer field read as a
/// float is 0 or tiny). Forever has the difficulty at field 10, Classic Era and TBC at 11.
fn xp_reward(rec: &[u8]) -> Option<XpReward> {
    if rec.len() < 64 {
        return None;
    }
    let multiplier = |field: usize| {
        let p = field * 4;
        (rec.len() >= p + 4)
            .then(|| f32_at(rec, p))
            .filter(|m| (0.05..=20.0).contains(m))
    };
    [10, 11].into_iter().find_map(|field| {
        let difficulty = u8::try_from(i32_at(rec, field * 4)).ok().filter(|d| *d <= 10)?;
        let xp_multiplier = multiplier(field + 1)?;
        multiplier(field + 4)?;
        Some(XpReward {
            level: i32_at(rec, 8),
            difficulty,
            multiplier: xp_multiplier,
        })
    })
}

/// (magic, build, {id: bytes}).
pub type WdbRecords<'a> = (String, u32, BTreeMap<u32, &'a [u8]>);

/// Records of a WDB file.
pub fn read_wdb(data: &[u8]) -> Result<WdbRecords<'_>> {
    if data.len() < 24 {
        bail!("not a WDB file");
    }
    let magic = String::from_utf8_lossy(&data[..4]).into_owned();
    let build = u32_at(data, 4);
    let mut records = BTreeMap::new();
    let mut pos = 24;
    while pos + 8 <= data.len() {
        let (id, len) = (u32_at(data, pos), u32_at(data, pos + 4) as usize);
        if len == 0 || len > data.len() - pos - 8 {
            break;
        }
        records.insert(id, &data[pos + 8..pos + 8 + len]);
        pos += 8 + len;
    }
    Ok((magic, build, records))
}

fn lengths(rec: &[u8], pos: usize) -> [usize; 9] {
    let mut value: u128 = 0;
    for b in &rec[pos..pos + HEADER_LEN] {
        value = (value << 8) | u128::from(*b);
    }
    let mut shift = (HEADER_LEN * 8) as u32;
    let mut out = [0; 9];
    for (i, w) in WIDTHS.iter().enumerate() {
        shift -= w;
        out[i] = ((value >> shift) & ((1u128 << w) - 1)) as usize;
    }
    out
}

/// Objectives whose records chain up to end exactly at `header`, the longest chain found.
fn objective_chain(rec: &[u8], header: usize) -> Option<Vec<Objective>> {
    let mut best: Option<Vec<Objective>> = None;
    let lowest = header.saturating_sub(MAX_OBJECTIVES * (OBJ_FIXED + 255));
    let mut start = header.checked_sub(OBJ_FIXED)?;
    loop {
        let (mut pos, mut ids, mut objs) = (start, Vec::new(), Vec::new());
        while pos + OBJ_FIXED <= header {
            if u32_at(rec, pos) < 1000 || rec[pos + 4] > 30 {
                break;
            }
            let effects = i32_at(rec, pos + 33);
            if !(0..=8).contains(&effects) || pos + OBJ_FIXED + 4 * effects as usize > header {
                break;
            }
            let desc_len = rec[pos + 41 + 4 * effects as usize] as usize;
            let desc_start = pos + OBJ_FIXED + 4 * effects as usize;
            let end = desc_start + desc_len;
            if end > header || std::str::from_utf8(&rec[desc_start..end]).is_err() {
                break;
            }
            ids.push(u32_at(rec, pos));
            objs.push(Objective {
                kind: rec[pos + 4],
                target: i32_at(rec, pos + 9),
                amount: i32_at(rec, pos + 13),
            });
            pos = end;
        }
        let ids_ok = ids.windows(2).all(|w| w[1] > w[0] && w[1] - w[0] < 64);
        let amounts_ok = objs.iter().all(|o| o.amount > 0 && o.amount < 10000);
        if pos == header
            && !objs.is_empty()
            && ids_ok
            && amounts_ok
            && best.as_ref().is_none_or(|b| objs.len() > b.len())
        {
            best = Some(objs);
        }
        if start == 0 || start <= lowest {
            break;
        }
        start -= 1;
    }
    best
}

fn looks_like_title(t: &str) -> bool {
    !t.is_empty()
        && t.chars().count() <= 120
        && !t.contains('\n')
        && !t.starts_with(' ')
        && !t.chars().next().is_some_and(char::is_lowercase)
        && t.chars().all(|c| c as u32 >= 32)
}

/// A quest record: read with the first known layout that fits it exactly, else by the
/// layout-free method (title, log and XP; objectives when confirmed).
pub fn parse_quest(rec: &[u8]) -> Option<CachedQuest> {
    quest::decode(rec, None, None)
        .map(|d| d.quest)
        .or_else(|| scan_quest(rec))
}

/// The fields of a quest record beyond the title, objectives and XP, when a known layout reads
/// it exactly.
pub fn parse_quest_info(rec: &[u8]) -> Option<QuestInfo> {
    quest::decode(rec, None, None).map(|d| d.info)
}

/// The layout-free method.
fn scan_quest(rec: &[u8]) -> Option<CachedQuest> {
    // Real quests stay under 3 KB; the scan tries every position, so a larger record costs
    // quadratic time for nothing.
    if rec.len() > MAX_SCANNED_LEN {
        return None;
    }
    let mut found: Vec<(bool, usize, CachedQuest)> = Vec::new();
    for pos in 0..rec.len().saturating_sub(HEADER_LEN) {
        let lens = lengths(rec, pos);
        let total: usize = lens.iter().sum();
        if lens[0] == 0 || total > rec.len() - pos - HEADER_LEN {
            continue;
        }
        let mut offset = rec.len() - total;
        let mut strings = Vec::with_capacity(9);
        let mut ok = true;
        for n in lens {
            if let Ok(s) = std::str::from_utf8(&rec[offset..offset + n]) {
                strings.push(s.to_owned());
            } else {
                ok = false;
                break;
            }
            offset += n;
        }
        if !ok || !looks_like_title(&strings[0]) || strings[1].is_empty() {
            continue;
        }
        let chain = objective_chain(rec, pos);
        let confirmed = chain.is_some();
        found.push((
            confirmed,
            pos,
            CachedQuest {
                title: strings[0].clone(),
                log: strings[1].clone(),
                objectives: chain.unwrap_or_default(),
                confirmed,
                xp: xp_reward(rec),
                info: None,
            },
        ));
    }
    found.sort_by_key(|f| (f.0, f.1));
    found.pop().map(|f| f.2)
}

/// Objective type sent by the server -> our objective kind.
pub fn kind(t: u8) -> Option<&'static str> {
    match t {
        0 => Some("creature"),
        1 => Some("item"),
        2 => Some("object"),
        _ => None,
    }
}

/// The quests of one locale's cache file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheFile {
    /// Client locale (enUS, frFR...).
    pub locale: String,
    pub build: u32,
    pub quests: BTreeMap<u32, CachedQuest>,
}

/// Every quest cache of a game client folder (`_classic_era_`...), one per locale.
pub fn read_cache(client_dir: &Path) -> Vec<CacheFile> {
    read_locales(client_dir, "questcache.wdb", "TSQW", |id, version, rec| {
        quest::decode(rec, Some(id), Some(version))
            .map(|d| CachedQuest {
                info: Some(d.info),
                ..d.quest
            })
            .or_else(|| scan_quest(rec))
    })
    .into_iter()
    .map(|c| CacheFile {
        locale: c.locale,
        build: c.build,
        quests: c.entries,
    })
    .collect()
}

/// The records of one locale's cache file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleCache<T> {
    /// Client locale (enUS, frFR...).
    pub locale: String,
    pub build: u32,
    pub entries: BTreeMap<u32, T>,
}

pub type QuestInfoCache = LocaleCache<QuestInfo>;

/// `QuestInfo` of every quest cache of a game client folder, one per locale (the quests read
/// with a known layout).
pub fn read_quest_info(client_dir: &Path) -> Vec<QuestInfoCache> {
    read_locales(client_dir, "questcache.wdb", "TSQW", |id, version, rec| {
        quest::decode(rec, Some(id), Some(version)).map(|d| d.info)
    })
}

/// One cache file (`file`, with this magic) of every locale folder of a client, its records
/// decoded by `decode(id, record version, record)`; the records it refuses are skipped.
fn read_locales<T>(
    client_dir: &Path,
    file: &str,
    magic: &str,
    mut decode: impl FnMut(u32, u32, &[u8]) -> Option<T>,
) -> Vec<LocaleCache<T>> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir(client_dir.join("Cache/WDB")) else {
        return out;
    };
    for entry in dir.flatten() {
        let locale = entry.file_name().to_string_lossy().into_owned();
        if locale.len() != 4 || !locale.bytes().all(|b| b.is_ascii_alphabetic()) {
            continue;
        }
        let Some(data) = read_file(&entry.path().join(file)) else {
            continue;
        };
        let Ok((file_magic, build, records)) = read_wdb(&data) else {
            continue;
        };
        if file_magic != magic {
            continue;
        }
        let version = u32_at(&data, 16);
        let entries = records
            .into_iter()
            .filter(|(_, rec)| rec.len() <= MAX_RECORD_LEN)
            .filter_map(|(id, rec)| decode(id, version, rec).map(|v| (id, v)))
            .collect();
        out.push(LocaleCache { locale, build, entries });
    }
    out.sort_by(|a, b| a.locale.cmp(&b.locale));
    out
}

/// A file of at most `MAX_FILE_LEN` bytes.
fn read_file(path: &Path) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_FILE_LEN + 1)
        .read_to_end(&mut data)
        .ok()?;
    (data.len() as u64 <= MAX_FILE_LEN).then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record's fixed part from i32 fields, floats given as their bits.
    fn record(fields: &[i32]) -> Vec<u8> {
        let mut rec: Vec<u8> = fields.iter().flat_map(|f| f.to_le_bytes()).collect();
        rec.resize(rec.len() + 40, 0);
        rec
    }

    fn bits(f: f32) -> i32 {
        f.to_bits() as i32
    }

    #[test]
    fn reads_the_xp_reward_of_both_layouts() {
        // Forever (The Color of Blood, Stockade): level 26, difficulty 6, x2.1, 4000 copper.
        let forever = record(&[388, 2, 26, 0, 22, 0, 717, 81, 0, 0, 6, bits(2.1), 4000, 6, bits(1.0), 0]);
        assert_eq!(
            xp_reward(&forever),
            Some(XpReward {
                level: 26,
                difficulty: 6,
                multiplier: 2.1
            })
        );
        // Classic Era (quest 1022): level 30, difficulty 5, one more field.
        let classic = record(&[
            1022,
            2,
            30,
            0,
            255,
            0,
            25,
            331,
            0,
            0,
            1037,
            5,
            bits(1.0),
            2500,
            5,
            bits(1.0),
            1500,
        ]);
        assert_eq!(
            xp_reward(&classic),
            Some(XpReward {
                level: 30,
                difficulty: 5,
                multiplier: 1.0
            })
        );
        // Neither layout: no multipliers where they would be.
        assert_eq!(
            xp_reward(&record(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])),
            None
        );
    }

    #[test]
    fn rounds_as_the_game() {
        assert_eq!(round_xp(405.0), 410);
        assert_eq!(round_xp(270.0), 270);
        assert_eq!(round_xp(680.0), 675);
        assert_eq!(round_xp(970.0), 975);
        assert_eq!(round_xp(2000.0 * 2.1), 4200);
        assert_eq!(round_xp(2650.0 * 2.1), 5550);
        assert_eq!(round_xp(83.0), 85);
    }

    /// Counts of what the caches of real clients give: `FG_WOW_DIR=<World of Warcraft folder>
    /// cargo test -p fg-contrib -- --ignored --nocapture real_caches`.
    #[test]
    #[ignore = "needs game clients (FG_WOW_DIR)"]
    fn real_caches() {
        let root = std::path::PathBuf::from(std::env::var("FG_WOW_DIR").expect("FG_WOW_DIR"));
        let mut clients: Vec<_> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.path()).collect();
        clients.sort();
        let records = |dir: &Path, locale: &str, file: &str| {
            let data = std::fs::read(dir.join("Cache/WDB").join(locale).join(file)).unwrap_or_default();
            read_wdb(&data).map_or(0, |(_, _, r)| r.len())
        };
        for dir in clients.iter().filter(|d| d.join("Cache/WDB").is_dir()) {
            let name = dir.file_name().unwrap().to_string_lossy();
            let infos = read_quest_info(dir);
            for file in read_cache(dir) {
                let info = infos
                    .iter()
                    .find(|i| i.locale == file.locale)
                    .map_or(0, |i| i.entries.len());
                let with_objectives = file.quests.values().filter(|q| !q.objectives.is_empty()).count();
                let with_xp = file.quests.values().filter(|q| q.xp.is_some()).count();
                let total = records(dir, &file.locale, "questcache.wdb");
                println!(
                    "{name} {} quests: {total} records, {} read, {info} exact, {with_objectives} with objectives, \
                     {with_xp} with XP",
                    file.locale,
                    file.quests.len()
                );
            }
            for file in read_creature_cache(dir) {
                let total = records(dir, &file.locale, "creaturecache.wdb");
                let items = file.entries.values().filter(|c| !c.quest_items.is_empty()).count();
                println!(
                    "{name} {} creatures: {total} records, {} read, {items} with quest items",
                    file.locale,
                    file.entries.len()
                );
            }
            for file in read_object_cache(dir) {
                let total = records(dir, &file.locale, "gameobjectcache.wdb");
                let items = file.entries.values().filter(|o| !o.quest_items.is_empty()).count();
                let chests = file.entries.values().filter(|o| o.loot.is_some()).count();
                println!(
                    "{name} {} objects: {total} records, {} read, {items} with quest items, {chests} chests",
                    file.locale,
                    file.entries.len()
                );
            }
        }
    }
}
