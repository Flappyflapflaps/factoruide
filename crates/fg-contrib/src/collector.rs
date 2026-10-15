//! What the Factoruide addon recorded in game (its `Factoruide.lua` SavedVariables): quest
//! accepts and turn-ins with the NPC and position, objectives progress, NPCs seen...

#[cfg(feature = "local")]
use crate::lua::{self, Val};
#[cfg(feature = "local")]
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(feature = "local")]
use std::path::{Path, PathBuf};

/// One recorded event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: Option<i64>,
    pub ts: Option<i64>,
    pub event: Option<String>,
    pub quest_id: Option<i64>,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub ui_map_id: Option<i64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub level: Option<i64>,
    /// Extra data of the event (objectives, rewards...).
    pub data: Option<serde_json::Value>,
}

/// The events of one character.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// "Name-Realm" when read locally; an opaque id in contributions.
    pub character: String,
    pub race: Option<String>,
    pub class: Option<String>,
    /// In contributions: day of the first event (days since 1970-01-01), event times being
    /// seconds since that event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<u32>,
    /// Account folder the character was read from (local only: never serialized).
    #[serde(skip)]
    pub account: Option<String>,
    pub events: Vec<Event>,
}

#[cfg(feature = "local")]
/// Every `Factoruide.lua` SavedVariables file under the client's WTF folder (or, for an account
/// that has not run the renamed addon yet, its former `BrouzQuest.lua`).
pub fn find_saved_variables(client_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(accounts) = std::fs::read_dir(client_dir.join("WTF/Account")) else {
        return found;
    };
    for account in accounts.flatten() {
        let dir = account.path().join("SavedVariables");
        if let Some(file) = ["Factoruide.lua", "BrouzQuest.lua"]
            .iter()
            .map(|n| dir.join(n))
            .find(|f| f.exists())
        {
            found.push(file);
        }
    }
    found
}

/// The characters and events of SavedVariables files.
#[cfg(feature = "local")]
pub fn read(files: &[PathBuf]) -> Result<Vec<Character>> {
    let mut out = Vec::new();
    for file in files {
        // WTF/Account/<account>/SavedVariables/<file>.
        let account = file
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned());
        let lua = lua::new_lua();
        let global = if file.ends_with("BrouzQuest.lua") {
            "BrouzQuestDB"
        } else {
            "FactoruideDB"
        };
        let db = lua::load_globals(&lua, file, &[global])?
            .pop()
            .with_context(|| format!("{global} missing"))?;
        for (character, c) in db.field("characters").str_entries() {
            let events = c
                .field("events")
                .list()
                .into_iter()
                .map(|e| match e {
                    Val::Str(row) => parse_row(row),
                    _ => table_row(e),
                })
                .collect();
            out.push(Character {
                character: character.to_owned(),
                race: c.field("race").str().map(str::to_owned),
                class: c.field("class").str().map(str::to_owned),
                day: None,
                account: account.clone(),
                events,
            });
        }
    }
    Ok(out)
}

/// A row as the addon first recorded them: a table of 11 fields.
#[cfg(feature = "local")]
fn table_row(e: &Val) -> Event {
    Event {
        seq: e.get(1).int(),
        ts: e.get(2).int(),
        event: e.get(3).str().map(str::to_owned),
        quest_id: e.get(4).int(),
        entity_type: e.get(5).str().map(str::to_owned),
        entity_id: e.get(6).int(),
        ui_map_id: e.get(7).int(),
        x: e.get(8).num(),
        y: e.get(9).num(),
        level: e.get(10).int(),
        // An empty Lua table is no data (it would read as an empty JSON array).
        data: match e.get(11) {
            Val::Table(_) => Some(e.get(11).to_json()).filter(|d| {
                !(d.as_array().is_some_and(Vec::is_empty) || d.as_object().is_some_and(serde_json::Map::is_empty))
            }),
            _ => None,
        },
    }
}

/// A compact row (what the addon records now, cheaper to save and load): one string
/// `seq|time|event|quest|entityType|entityID|map|x|y|level|data`, empty fields for none, the data
/// as `key=value` pairs joined by `;` (see `validate::decode_field`).
pub fn parse_row(row: &str) -> Event {
    let mut f = row.splitn(11, '|');
    let mut next = || f.next().filter(|v| !v.is_empty());
    let int = |v: Option<&str>| v.and_then(|v| v.parse::<i64>().ok());
    let num = |v: Option<&str>| v.and_then(|v| v.parse::<f64>().ok()).filter(|n| n.is_finite());
    let seq = int(next());
    let ts = int(next());
    let event = next().map(str::to_owned);
    let quest_id = int(next());
    let entity_type = next().map(str::to_owned);
    let entity_id = int(next());
    let ui_map_id = int(next());
    let x = num(next());
    let y = num(next());
    let level = int(next());
    let data = next().map(|d| {
        let ev = event.as_deref().unwrap_or_default();
        serde_json::Value::Object(
            d.split(';')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let (k, v) = p.split_once('=').unwrap_or((p, ""));
                    (k.to_owned(), crate::validate::decode_field(ev, k, v))
                })
                .collect(),
        )
    });
    Event {
        seq,
        ts,
        event,
        quest_id,
        entity_type,
        entity_id,
        ui_map_id,
        x,
        y,
        level,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_compact_row() {
        let e = parse_row("1234|1791911435|loot||npc|1553|1429|45.12|60.33|12|items=2589:2,1468:1");
        assert_eq!(
            (e.seq, e.ts, e.quest_id, e.entity_id),
            (Some(1234), Some(1791911435), None, Some(1553))
        );
        assert_eq!(e.event.as_deref(), Some("loot"));
        assert_eq!(e.entity_type.as_deref(), Some("npc"));
        assert_eq!((e.x, e.y, e.level), (Some(45.12), Some(60.33), Some(12)));
        assert_eq!(e.data, Some(serde_json::json!({"items": [[2589, 2], [1468, 1]]})));
        crate::validate::check_local_event(&e, 1_800_000_000).unwrap();
    }

    #[test]
    fn data_is_typed_by_the_event_schema() {
        let e = parse_row("7|1791911435|accept|155|npc|234|1436|56.3|47.5|14|title=Wanted%3A 100%25 %7C done;level=14");
        assert_eq!(
            e.data,
            Some(serde_json::json!({"title": "Wanted: 100% | done", "level": 14}))
        );
        let e = parse_row("8|1791911435|seen||npc|6|1429|40|60|5|elite=1;age=300");
        assert_eq!(e.data, Some(serde_json::json!({"elite": true, "age": 300})));
        // No data, no position.
        let e = parse_row("9|1791911435|removed|155||||||14|");
        assert_eq!((&e.data, e.x, e.ui_map_id), (&None, None, None));
        crate::validate::check_local_event(&e, 1_800_000_000).unwrap();
    }

    #[test]
    fn an_odd_row_fails_its_check_instead_of_passing_altered() {
        let now = 1_800_000_000;
        for row in [
            "10|1791911435|loot||npc|1553|1429|45|60|12|items=2589:x",
            "11|1791911435|seen||npc|6|1429|40|60|5|note=hi",
            "12|1791911435|seen||npc|6|1429|40|60|5|name=bad%ZZ",
            "garbage",
        ] {
            assert!(
                crate::validate::check_local_event(&parse_row(row), now).is_err(),
                "{row}"
            );
        }
    }
}
