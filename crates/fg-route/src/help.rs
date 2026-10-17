//! Checks the players can make in game where the quest data cannot settle a question (the app's
//! "Need help" page): does an NPC offer a quest before another one is turned in? Listed in
//! `overrides/<edition>/need_help.toml`, answered from the app with a problem report.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// A check as the file lists it.
#[derive(Debug, Clone, Deserialize)]
pub struct CheckDef {
    /// The quest asked about.
    pub quest: i64,
    /// Quests it may come after: is it offered before they are turned in?
    pub before: Vec<i64>,
}

/// A thing with its id and its name in the player's language.
#[derive(Debug, Clone, Serialize)]
pub struct Named {
    pub id: i64,
    pub name: String,
}

/// Where the quest's giver stands: zone and percent coordinates on its map.
#[derive(Debug, Clone, Serialize)]
pub struct Place {
    pub zone: String,
    pub x: f64,
    pub y: f64,
}

/// A check ready to show.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// Stable id, in the answers sent (`<edition>/offered-before/<quest>`).
    pub id: String,
    pub quest: Named,
    pub giver: Option<Named>,
    pub place: Option<Place>,
    pub before: Vec<Named>,
    pub min_level: i64,
    /// `alliance`, `horde` or `both`.
    pub side: &'static str,
}

/// The checks of an edition, from its overrides (`file` reads `<edition>/need_help.toml` as the
/// overrides' other files are read: from the repository or the app).
pub fn load(file: impl Fn(&str) -> Option<String>, edition: crate::xp::Edition) -> Result<Vec<CheckDef>> {
    #[derive(Deserialize, Default)]
    struct File {
        #[serde(default)]
        check: Vec<CheckDef>,
    }
    let name = format!("{}/need_help.toml", edition.overrides_subdir());
    let Some(text) = file(&name) else {
        return Ok(Vec::new());
    };
    let parsed: File = toml::from_str(&text).with_context(|| format!("parsing {name}"))?;
    Ok(parsed.check)
}

/// The checks with their names in `locale` (the game's, else English), the quest's giver and
/// its place; checks of quests the data no longer has are left out.
pub fn resolve(conn: &Connection, defs: &[CheckDef], edition: crate::xp::Edition, locale: &str) -> Result<Vec<Check>> {
    let name = |kind: &str, id: i64, english: String| -> Result<String> {
        let local: Option<String> = conn
            .query_row(
                "SELECT name FROM l10n WHERE entity_type = ?1 AND id = ?2 AND locale = ?3",
                rusqlite::params![kind, id, locale],
                |r| r.get(0),
            )
            .optional()
            .unwrap_or(None);
        Ok(local.filter(|n| !n.is_empty()).unwrap_or(english))
    };
    let quest = |id: i64| -> Result<Option<(Named, i64, i64)>> {
        let row: Option<(String, Option<i64>, Option<i64>)> = conn
            .query_row(
                "SELECT name, min_level, races_mask FROM m_quest WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(match row {
            Some((english, min, races)) => Some((
                Named {
                    id,
                    name: name("quest", id, english)?,
                },
                min.unwrap_or(1),
                races.unwrap_or(0),
            )),
            None => None,
        })
    };
    let mut out = Vec::new();
    for d in defs {
        let Some((named, min_level, races)) = quest(d.quest)? else {
            continue;
        };
        let giver: Option<(i64, String)> = conn
            .query_row(
                "SELECT n.id, n.name FROM m_quest_relation r JOIN m_npc n ON n.id = r.entity_id
                 WHERE r.quest_id = ?1 AND r.role = 'start' AND r.entity_type = 'npc' ORDER BY n.id LIMIT 1",
                [d.quest],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let spot: Option<(i64, String, f64, f64)> = match &giver {
            Some((id, _)) => conn
                .query_row(
                    "SELECT s.zone_id, coalesce(a.AreaName_lang, ''), s.x, s.y FROM m_spawn s
                     LEFT JOIN client_areatable a ON a.ID = s.zone_id
                     WHERE s.entity_type = 'npc' AND s.entity_id = ?1 AND s.x >= 0 LIMIT 1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?,
            None => None,
        };
        let place = match spot {
            Some((zone, english, x, y)) => Some(Place {
                zone: name("zone", zone, english)?,
                x,
                y,
            }),
            None => None,
        };
        let mut before = Vec::new();
        for &b in &d.before {
            if let Some((n, _, _)) = quest(b)? {
                before.push(n);
            }
        }
        // Playable races of each side (Skyborne: bits 32 and 33).
        let (alliance, horde) = (races & 0x1_0000_004D != 0, races & 0x2_0000_00B2 != 0);
        out.push(Check {
            id: format!("{}/offered-before/{}", edition.key(), d.quest),
            quest: named,
            giver: match giver {
                Some((id, english)) => Some(Named {
                    id,
                    name: name("npc", id, english)?,
                }),
                None => None,
            },
            place,
            before,
            min_level,
            side: match (alliance, horde) {
                (true, false) => "alliance",
                (false, true) => "horde",
                _ => "both",
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's checks read, each with the quests it may come after.
    #[test]
    fn the_checks_of_the_repository_read() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../overrides");
        let defs = load(
            |name| std::fs::read_to_string(dir.join(name)).ok(),
            crate::xp::Edition::Forever,
        )
        .unwrap();
        assert!(!defs.is_empty());
        assert!(
            defs.iter().all(|d| !d.before.is_empty()),
            "each check names the quests it may come after"
        );
    }
}
