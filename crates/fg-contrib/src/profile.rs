//! Characters' profiles the addon saves in its SavedVariables (`FactoruideDB.profiles`, see
//! the addon's Profile.lua): level, quests done and in the log, position, flight paths... for
//! the app to plan a guide from where a character stands. Never part of a contribution.

use crate::lua::{self, Val};
use anyhow::Result;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// C_TaxiMap state of a flight path the character does not know.
const UNKNOWN_FLIGHT: i64 = 2;

/// The profiles of SavedVariables files (see `collector::find_saved_variables`), as JSON objects
/// (the planner's `StartState`), most recent first; those received from other players carry
/// `"shared": true`. Flight paths also come from the flight maps
/// the collector recorded before the addon kept them in the profile.
pub fn read(files: &[PathBuf]) -> Result<Vec<serde_json::Value>> {
    let mut out = Vec::new();
    for file in files {
        // The renamed addon's file has no profiles.
        if file.ends_with("BrouzQuest.lua") {
            continue;
        }
        let lua = lua::new_lua();
        let Some(db) = lua::load_globals(&lua, file, &["FactoruideDB"])?.pop() else {
            continue;
        };
        for (key, p) in db.field("profiles").str_entries() {
            if p.field("level").int().is_none() {
                continue;
            }
            let mut json = p.to_json();
            let mut flights: BTreeSet<i64> = p
                .field("flights")
                .entries()
                .into_iter()
                .filter(|(_, v)| !matches!(v, Val::Nil | Val::Bool(false)))
                .map(|(id, _)| id)
                .collect();
            flights.extend(recorded_flights(db.field("characters").field(key)));
            json["flights"] = flights.into_iter().collect();
            if json["name"].as_str().is_none_or(str::is_empty) {
                json["name"] = key.into();
            }
            out.push(json);
        }
        // Other players' profiles received in a party (`/fg share`, the addon's Share.lua), for
        // group guides: marked `shared`, under the name of who sent them.
        for (key, p) in db.field("shared").str_entries() {
            if p.field("level").int().is_none() {
                continue;
            }
            let mut json = p.to_json();
            json["name"] = key.into();
            json["shared"] = true.into();
            out.push(json);
        }
    }
    out.sort_by_key(|p| std::cmp::Reverse(p["time"].as_i64().unwrap_or(0)));
    Ok(out)
}

/// A character's request to compute its route again from where it stands ("re-plan from here",
/// the addon's Replan.lua): `FactoruideDB.replans["Name-Realm"]`, saved at the reload it asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplanAsk {
    /// "Name-Realm".
    pub character: String,
    /// When it was asked (unix seconds): the request's id, given back in the route.
    pub asked: i64,
    /// The guide the character followed (its name in game), if any.
    pub route: Option<String>,
    pub level: i64,
}

/// The re-plan requests of SavedVariables files, the oldest first.
pub fn replans(files: &[PathBuf]) -> Result<Vec<ReplanAsk>> {
    let mut out = Vec::new();
    for file in files {
        if file.ends_with("BrouzQuest.lua") {
            continue;
        }
        let lua = lua::new_lua();
        let Some(db) = lua::load_globals(&lua, file, &["FactoruideDB"])?.pop() else {
            continue;
        };
        for (key, ask) in db.field("replans").str_entries() {
            let Some(asked) = ask.field("asked").int() else {
                continue;
            };
            out.push(ReplanAsk {
                character: key.to_owned(),
                asked,
                route: ask.field("route").str().map(str::to_owned),
                level: ask.field("level").int().unwrap_or(0),
            });
        }
    }
    out.sort_by_key(|a| a.asked);
    Ok(out)
}

/// Flight paths shown known on the flight maps the collector recorded (`taxi` events).
fn recorded_flights(character: &Val) -> Vec<i64> {
    character
        .field("events")
        .list()
        .into_iter()
        .filter(|e| e.get(3).str() == Some("taxi"))
        .flat_map(|e| e.get(11).field("nodes").list())
        .filter(|n| n.field("state").int().unwrap_or(UNKNOWN_FLIGHT) != UNKNOWN_FLIGHT)
        .filter_map(|n| n.field("id").int())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replan_requests() {
        let dir = std::env::temp_dir().join(format!("fg-replans-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Factoruide.lua");
        std::fs::write(
            &file,
            r#"FactoruideDB = {
                ["replans"] = {
                    ["Brouz-Realm"] = { ["asked"] = 300, ["route"] = "Gnome Mage", ["level"] = 14, ["step"] = 120 },
                    ["Pata-Realm"] = { ["asked"] = 200, ["level"] = 9 },
                    ["Broken-Realm"] = { ["route"] = "x" },
                },
            }"#,
        )
        .unwrap();
        let asks = replans(&[file]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            asks,
            [
                ReplanAsk {
                    character: "Pata-Realm".into(),
                    asked: 200,
                    route: None,
                    level: 9
                },
                ReplanAsk {
                    character: "Brouz-Realm".into(),
                    asked: 300,
                    route: Some("Gnome Mage".into()),
                    level: 14
                },
            ],
            "the oldest first, those without a time left out"
        );
    }

    #[test]
    fn profiles_with_flights_from_the_profile_and_the_flight_maps() {
        let dir = std::env::temp_dir().join(format!("fg-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Factoruide.lua");
        std::fs::write(
            &file,
            r#"FactoruideDB = {
                ["characters"] = { ["Brouz-Realm"] = { ["seq"] = 2, ["events"] = {
                    { 1, 100, "taxi", false, false, false, 1411, 50, 50, 12,
                      { ["map"] = 1411, ["nodes"] = { { ["id"] = 25, ["state"] = 0 }, { ["id"] = 22, ["state"] = 2 } } } },
                    { 2, 101, "accept", 788 },
                } } },
                ["profiles"] = {
                    ["Brouz-Realm"] = { ["level"] = 12, ["xp"] = 3400, ["time"] = 200,
                        ["completed"] = { 4641, 788 }, ["log"] = {}, ["flights"] = { [23] = true } },
                    ["Old-Realm"] = { ["level"] = 5, ["time"] = 300 },
                    ["Empty-Realm"] = {},
                },
                ["shared"] = {
                    ["Pata-Realm"] = { ["level"] = 13, ["name"] = "Someone-Else", ["time"] = 100 },
                },
            }"#,
        )
        .unwrap();
        let profiles = read(&[file]).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let names: Vec<&str> = profiles.iter().map(|p| p["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["Old-Realm", "Brouz-Realm", "Pata-Realm"], "most recent first");
        assert_eq!(
            profiles[2]["shared"], true,
            "received from another player, named by its sender"
        );
        assert!(profiles[1].get("shared").is_none());
        let brouz = &profiles[1];
        assert_eq!(brouz["flights"], serde_json::json!([23, 25]));
        assert_eq!(brouz["completed"], serde_json::json!([4641, 788]));
        assert_eq!(brouz["log"], serde_json::json!([]));
    }
}
