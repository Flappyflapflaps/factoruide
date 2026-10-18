//! Routes saved as JSON, and the addon's data files.

use super::{Overrides, add_walk_paths, places, services_lua, walk_world};
use crate::export::{self, Route};
use crate::faction::Faction;
use crate::model::Names;
use crate::world::World;
use crate::xp::Edition;
use anyhow::Result;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Every saved route.
pub fn load_routes(routes_dir: &Path) -> Result<Vec<Route>> {
    let mut files: Vec<_> = match std::fs::read_dir(routes_dir) {
        Ok(dir) => dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    Ok(files
        .iter()
        .filter_map(|f| export::read_route(serde_json::from_str(&std::fs::read_to_string(f).ok()?).ok()?).ok())
        .collect())
}

pub fn save_route(routes_dir: &Path, route: &Route) -> Result<()> {
    std::fs::create_dir_all(routes_dir)?;
    let path = routes_dir.join(format!("{}.json", slug(&route.name)));
    std::fs::write(&path, serde_json::to_string_pretty(route)?)?;
    Ok(())
}

/// Regenerate the addon's Routes.lua from every saved route, each written in its own language,
/// with their walks, and Services.lua (`addon_walks`).
pub fn write_addon_routes(
    conn: &Connection,
    overrides: &Overrides,
    edition: Edition,
    routes_dir: &Path,
    addon_dir: &Path,
) -> Result<usize> {
    let mut routes = render_routes(conn, &load_routes(routes_dir)?, None)?;
    let locale = routes
        .first()
        .and_then(|r| r.locale.clone())
        .unwrap_or_else(|| "enUS".into());
    let services = addon_walks(conn, overrides, edition, &mut routes, &Names::load(conn, &locale)?)?;
    std::fs::write(addon_dir.join("Routes.lua"), export::to_lua(&routes))?;
    std::fs::write(addon_dir.join("Services.lua"), services)?;
    Ok(routes.len())
}

/// The walks of the guides (`add_walk_paths`, the world of each guide's faction), and the
/// addon's Services.lua with the names of `names`.
pub fn addon_walks(
    conn: &Connection,
    overrides: &Overrides,
    edition: Edition,
    routes: &mut [Route],
    names: &Names,
) -> Result<String> {
    let mut worlds: HashMap<Faction, World> = HashMap::new();
    for route in routes.iter_mut() {
        let world = match worlds.entry(route.faction) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => e.insert(walk_world(conn, overrides, route.faction)?),
        };
        add_walk_paths(world, route);
    }
    // Where a place is does not depend on the faction.
    let world = match worlds.into_values().next() {
        Some(w) => w,
        None => walk_world(conn, overrides, Faction::Alliance)?,
    };
    Ok(services_lua(&places(conn, &world, edition, names)?))
}

/// Write the guide pack for the Polestar addon (`export::polestar`) into `dir` from every saved
/// route, each written in its own language.
pub fn write_polestar_pack(conn: &Connection, routes_dir: &Path, dir: &Path, version: &str) -> Result<usize> {
    let routes = render_routes(conn, &load_routes(routes_dir)?, None)?;
    let guides: Vec<(String, Route)> = routes.into_iter().map(|r| (slug(&r.name), r)).collect();
    std::fs::create_dir_all(dir)?;
    for (name, content) in export::polestar::pack(&guides, version) {
        std::fs::write(dir.join(name), content)?;
    }
    Ok(guides.len())
}

/// Write a pack's folder for Polestar's own tools (`export::polestar::source`) into `dir` from
/// every saved route: its `pack.json` and a `.guide` file each, in English.
pub fn write_polestar_source(conn: &Connection, routes_dir: &Path, dir: &Path, version: &str) -> Result<usize> {
    let routes = render_routes(conn, &load_routes(routes_dir)?, Some("enUS"))?;
    add_polestar_source_pack(dir, version)?;
    for r in &routes {
        add_polestar_source(dir, &slug(&r.name), r, version)?;
    }
    Ok(routes.len())
}

/// The folder made for a pack for Polestar's own tools (not the in-game pack's addon folder,
/// `export::polestar::ADDON`).
pub const POLESTAR_SOURCE_FOLDER: &str = "Factoruide_Polestar_Pack";

/// The folder a pack for Polestar's own tools goes in when `picked` is chosen for it: `picked`
/// itself when it already holds a pack (its `pack.json`), else its subfolder
/// [`POLESTAR_SOURCE_FOLDER`].
pub fn polestar_source_dir(picked: &Path) -> PathBuf {
    if picked.join("pack.json").is_file() {
        picked.to_owned()
    } else {
        picked.join(POLESTAR_SOURCE_FOLDER)
    }
}

/// Add a guide (`id`, its route written in English) to the pack for Polestar's own tools in
/// `dir`, made when missing: its `.guide` file, replaced if there, the pack's other guides kept,
/// and the pack's `pack.json` written again. A folder holding another pack is left as it is.
/// Returns the guide's file.
pub fn add_polestar_source(dir: &Path, id: &str, route: &Route, version: &str) -> Result<PathBuf> {
    add_polestar_source_pack(dir, version)?;
    let file = dir.join(format!("{}.guide", export::polestar::guide_name(id)));
    std::fs::write(&file, export::polestar::source(id, route))?;
    Ok(file)
}

/// Make `dir` a pack for Polestar's own tools: its `pack.json`, unless it holds another pack.
fn add_polestar_source_pack(dir: &Path, version: &str) -> Result<()> {
    let manifest = dir.join("pack.json");
    if let Ok(text) = std::fs::read_to_string(&manifest) {
        let id = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}'))
            .ok()
            .and_then(|v| v.get("id")?.as_str().map(str::to_owned));
        if id.as_deref() != Some(export::polestar::PACK_ID) {
            anyhow::bail!(
                "{} holds another Polestar pack ({}): choose another folder",
                dir.display(),
                id.as_deref().unwrap_or("no id")
            );
        }
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(manifest, export::polestar::pack_json(version))?;
    Ok(())
}

/// Routes written in `locale` (None: each in the language it was made in), the names of each
/// language read once from the database.
pub fn render_routes(conn: &Connection, routes: &[Route], locale: Option<&str>) -> Result<Vec<Route>> {
    let mut names: HashMap<String, Names> = HashMap::new();
    routes
        .iter()
        .map(|r| {
            let locale = locale.or(r.locale.as_deref()).unwrap_or("enUS").to_owned();
            if !names.contains_key(&locale) {
                names.insert(locale.clone(), Names::load(conn, &locale)?);
            }
            Ok(export::render(r, &names[&locale]))
        })
        .collect()
}

/// Write the addon's Config.lua from the app's addon settings (flat key -> bool/number/string).
pub fn write_addon_config(addon_dir: &Path, config: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let mut out =
        String::from("-- Generated by the Factoruide app. Do not edit.\nlocal FG = Factoruide\nFG.Config = {\n");
    let mut keys: Vec<_> = config.keys().collect();
    keys.sort();
    for key in keys {
        let value = match &config[key] {
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => format!("{s:?}"),
            _ => continue,
        };
        if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            let _ = writeln!(out, "    {key} = {value},");
        }
    }
    out.push_str("}\n");
    std::fs::write(addon_dir.join("Config.lua"), out)?;
    Ok(())
}

/// Lowercase ASCII identifier of a name, for file names ("Mon guide Nain !" -> "mon-guide-nain").
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() { "route".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn route(name: &str) -> Route {
        serde_json::from_value(json!({
            "name": name, "race_id": 3, "class_id": 1, "class": "Warrior",
            "faction": "Alliance", "from_level": 1, "to_level": 60, "total_time": 0.0,
            "total_time_text": "", "quests": 0, "breakdown": crate::plan::Breakdown::default(),
            "locale": "enUS", "steps": [
                { "kind": "accept", "quest": 179, "quest_name": "Dwarven Outfitters", "min_level": 1,
                  "level": 1, "time": 0.0, "target": { "kind": "npc", "id": 658, "name": "Sten Stoutarm" },
                  "map": { "ui_map": 1426, "x": 29.93, "y": 50.0 } },
            ],
        }))
        .unwrap()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fg-route-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_picked_folder_gets_its_own_pack_folder_unless_it_is_the_pack() {
        let picked = temp_dir("polestar-picked");
        let pack = polestar_source_dir(&picked);
        assert_eq!(pack, picked.join(POLESTAR_SOURCE_FOLDER));
        add_polestar_source(&pack, "dwarf-warrior", &route("Dwarf warrior"), "1.0.0").unwrap();
        // The pack chosen again: its own folder.
        assert_eq!(polestar_source_dir(&pack), pack);
        std::fs::remove_dir_all(&picked).unwrap();
    }

    #[test]
    fn guides_are_added_to_the_pack_and_replaced_the_others_kept() {
        let dir = temp_dir("polestar-pack").join("pack");
        let file = add_polestar_source(&dir, "dwarf-warrior", &route("Dwarf warrior"), "1.0.0").unwrap();
        assert_eq!(file, dir.join("dwarf-warrior.guide"));
        add_polestar_source(&dir, "troll-mage", &route("Troll mage"), "1.0.0").unwrap();
        add_polestar_source(&dir, "dwarf-warrior", &route("Dwarf warrior again"), "1.1.0").unwrap();
        assert_eq!(files(&dir), ["dwarf-warrior.guide", "pack.json", "troll-mage.guide"]);
        let guide = std::fs::read_to_string(dir.join("dwarf-warrior.guide")).unwrap();
        assert!(guide.contains("guide factoruide/dwarf-warrior\n"));
        assert!(guide.contains("title \"Dwarf warrior again\"\n"));
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("pack.json")).unwrap()).unwrap();
        assert_eq!(manifest["id"], "factoruide");
        assert_eq!(manifest["version"], "1.1.0");
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn another_pack_is_left_as_it_is() {
        let dir = temp_dir("polestar-other");
        let theirs = "{ \"id\": \"joe-routes\" }\n";
        std::fs::write(dir.join("pack.json"), theirs).unwrap();
        let err = add_polestar_source(&dir, "dwarf-warrior", &route("Dwarf warrior"), "1.0.0").unwrap_err();
        assert!(err.to_string().contains("joe-routes"));
        assert_eq!(files(&dir), ["pack.json"]);
        assert_eq!(std::fs::read_to_string(dir.join("pack.json")).unwrap(), theirs);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
