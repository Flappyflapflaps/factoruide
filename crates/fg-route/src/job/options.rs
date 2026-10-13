//! The choices the app offers: races, classes, zones, dungeons, professions.

use super::overrides::Overrides;
use crate::params::Params;
use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// Choices offered by the app.
#[derive(Debug, Clone, Serialize)]
pub struct Options {
    pub races: Vec<RaceOption>,
    pub classes: Vec<NamedId>,
    pub zones: Vec<ZoneOption>,
    pub dungeons: Vec<DungeonOption>,
    pub class_quests: HashMap<String, Vec<String>>,
    pub params: Params,
    /// Level cap of the game version.
    pub max_level: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaceOption {
    pub key: String,
    pub id: i64,
    pub name: String,
    pub faction: crate::faction::Faction,
    /// Classes this race can play (class keys), from the game's CharBaseInfo.
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NamedId {
    pub id: i64,
    pub key: String,
    pub name: String,
    /// Translated names by game locale (frFR...), when known.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
}

/// A dungeon, with the levels of its quests (from the quest log's dungeon category).
#[derive(Debug, Clone, Serialize)]
pub struct DungeonOption {
    pub id: i64,
    pub key: String,
    pub name: String,
    /// Translated names by game locale (frFR...), when known.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    /// Levels of most of its quests (the lowest and highest few left out), when it has some.
    pub min_level: Option<i64>,
    pub max_level: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZoneOption {
    pub id: i64,
    pub name: String,
    /// Translated names by game locale (frFR...), when known.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    pub continent: i64,
}

pub fn options(conn: &Connection, overrides: &Overrides) -> Result<Options> {
    let race_names: HashMap<i64, String> = {
        let mut stmt = conn.prepare("SELECT ID, Name_lang FROM client_chrraces")?;
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let races: Vec<RaceOption> = overrides
        .races
        .iter()
        .map(|(key, r)| {
            Ok(RaceOption {
                key: key.clone(),
                id: r.id,
                name: race_names.get(&r.id).cloned().unwrap_or_else(|| key.clone()),
                faction: r.faction,
                classes: race_classes(conn, r.id)?,
            })
        })
        .collect::<Result<_>>()?;
    // Classes some race can play.
    let classes = {
        let mut stmt = conn.prepare("SELECT ID, Filename, Name_male_lang FROM client_chrclasses ORDER BY ID")?;
        stmt.query_map([], |r| {
            Ok(NamedId {
                id: r.get(0)?,
                key: r.get::<_, String>(1)?.to_lowercase(),
                name: r.get(2)?,
                names: BTreeMap::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|c| races.iter().all(|r| r.classes.is_empty()) || races.iter().any(|r| r.classes.contains(&c.key)))
        .collect()
    };
    // Zone and dungeon names in the game's languages.
    let mut area_names: HashMap<i64, BTreeMap<String, String>> = HashMap::new();
    if let Ok(mut stmt) =
        conn.prepare("SELECT id, locale, name FROM l10n WHERE entity_type = 'zone' AND name IS NOT NULL")
    {
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        for row in rows {
            let (id, locale, name) = row?;
            area_names.entry(id).or_default().insert(locale, name);
        }
    }
    let names_of = |id: i64| area_names.get(&id).cloned().unwrap_or_default();
    let zones = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT a.AreaID, t.AreaName_lang, a.MapID FROM client_uimapassignment a
             JOIN client_areatable t ON t.ID = a.AreaID WHERE a.AreaID != 0 ORDER BY t.AreaName_lang",
        )?;
        stmt.query_map([], |r| {
            Ok(ZoneOption {
                id: r.get(0)?,
                name: r.get(1)?,
                names: names_of(r.get(0)?),
                continent: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?
    };
    // Dungeons with quests of this game version: something a quest needs lives inside (a mob or
    // object, or one dropping a quest item), plus those declared in dungeons.toml.
    let dungeons = {
        let mut stmt = conn.prepare(
            "WITH needed(entity_type, entity_id) AS (
               SELECT CASE o.type WHEN 'object' THEN 'object' ELSE 'npc' END, o.target_id
               FROM m_quest_objective o JOIN m_quest q ON q.id = o.quest_id
               WHERE q.status = 'available' AND q.level <= ?1 AND o.type != 'item'
               UNION SELECT i.from_type, i.from_id
               FROM m_quest_objective o JOIN m_quest q ON q.id = o.quest_id
               JOIN m_item_source i ON i.item_id = o.target_id AND i.from_type IN ('npc', 'object')
               WHERE q.status = 'available' AND q.level <= ?1 AND o.type = 'item'),
             -- The areas inside each dungeon, then the spawns read once (not once per dungeon).
             inside(dungeon, zone) AS (
               SELECT area, area FROM dungeon
               UNION SELECT d.area, j.value FROM dungeon d, json_each(coalesce(d.alt_areas, '[]')) j),
             found(dungeon) AS (
               SELECT DISTINCT i.dungeon FROM m_spawn s JOIN inside i ON i.zone = s.zone_id
               WHERE (s.entity_type, s.entity_id) IN needed)
             SELECT d.area, d.name, coalesce(d.alt_areas, '[]') FROM dungeon d JOIN client_areatable a ON a.ID = d.area
             JOIN client_map m ON m.InstanceType = 1 AND (m.ID = a.ContinentID OR m.AreaTableID = d.area)
             WHERE d.area IN found
             GROUP BY d.area",
        )?;
        let max_level = overrides.params.edition.rules().max_level + 2;
        // Quest levels by quest log category (a dungeon's quests are filed under it).
        let mut quest_levels: HashMap<i64, Vec<i64>> = HashMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT zone_or_sort, level FROM m_quest WHERE status = 'available' AND zone_or_sort > 0 AND level > 0",
            )?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (area, level) = row?;
                quest_levels.entry(area).or_default().push(level);
            }
        }
        // Its quests may be filed under one of its other areas (Gnomeregan's under the zone's).
        let dungeon = |area: i64, name: String, alt_areas: &[i64]| {
            let mut levels: Vec<i64> = std::iter::once(&area)
                .chain(alt_areas)
                .flat_map(|a| quest_levels.get(a).into_iter().flatten().copied())
                .collect();
            levels.sort_unstable();
            let n = levels.len();
            let def = overrides.dungeons.iter().find(|d| d.area == area);
            DungeonOption {
                id: area,
                key: area.to_string(),
                name,
                names: names_of(area),
                min_level: def
                    .and_then(|d| d.min_level)
                    .or_else(|| (n > 0).then(|| levels[n * 15 / 100])),
                max_level: def
                    .and_then(|d| d.max_level)
                    .or_else(|| (n > 0).then(|| levels[(n * 85).div_ceil(100).max(1) - 1])),
            }
        };
        let mut list: Vec<DungeonOption> = stmt
            .query_map([max_level], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .map(|r| {
                r.map(|(area, name, alt)| {
                    dungeon(area, name, &serde_json::from_str::<Vec<i64>>(&alt).unwrap_or_default())
                })
            })
            .collect::<rusqlite::Result<_>>()?;
        for def in &overrides.dungeons {
            if let Some(name) = &def.name
                && !list.iter().any(|d| d.id == def.area)
            {
                list.push(dungeon(def.area, name.clone(), &[]));
            }
        }
        // By level (those without, last), then name.
        list.sort_by(|a, b| {
            (a.min_level.unwrap_or(i64::MAX), a.max_level, &a.name).cmp(&(
                b.min_level.unwrap_or(i64::MAX),
                b.max_level,
                &b.name,
            ))
        });
        list
    };
    Ok(Options {
        races,
        classes,
        zones,
        dungeons,
        class_quests: overrides.class_quests.clone(),
        params: overrides.params.clone(),
        max_level: overrides.params.edition.rules().max_level,
    })
}

/// Classes a race can play (class keys: lowercase file names), from the game's CharBaseInfo.
pub fn race_classes(conn: &Connection, race_id: i64) -> Result<Vec<String>> {
    // Databases imported before CharBaseInfo (Classic Era comparisons) have no list: no restriction.
    let Ok(mut stmt) = conn.prepare(
        "SELECT lower(c.Filename) FROM client_charbaseinfo b JOIN client_chrclasses c ON c.ID = b.ClassID
         WHERE b.RaceID = ?1 ORDER BY c.ID",
    ) else {
        return Ok(Vec::new());
    };
    Ok(stmt
        .query_map([race_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
