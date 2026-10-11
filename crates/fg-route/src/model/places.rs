//! The NPCs a route visits besides quests: class trainers, innkeepers; and the class spells
//! that travel: teleports to the capitals, a second hearthstone.

use super::entities::Entities;
use super::types::{EntityKind, Loc, Profile, Teleport};
use crate::world::World;
use anyhow::Result;
use rusqlite::Connection;

pub(super) struct Places {
    pub(super) trainers: Vec<Loc>,
    pub(super) inns: Vec<Loc>,
    /// Where the hearthstone of a new character takes it: its starting place (not an inn, it
    /// cannot be bound there).
    pub(super) start_inn: usize,
    pub(super) teleports: Vec<Teleport>,
    pub(super) recall: Option<i64>,
}

impl Places {
    pub(super) fn load(
        conn: &Connection,
        world: &World,
        entities: &Entities,
        profile: &Profile,
        start: &Loc,
    ) -> Result<Self> {
        let finder = NpcFinder {
            conn,
            world,
            entities,
            faction: profile.faction.letter(),
        };
        let mut inns = finder.with_subname("Innkeeper")?;
        // A new character's hearthstone takes it back to its starting place (the Valley of
        // Trials, not the closest inn as the crow flies) until it is bound at an inn.
        inns.push(start.clone());
        let start_inn = inns.len() - 1;
        let trainers = finder.with_subname(&format!("{} Trainer", profile.class_name))?;
        let spells = class_spells(conn, profile.class_id)?;
        // Where a teleport lands: the class trainer of that city, else its inn (the character's
        // capitals only: the others have none friendly to it).
        let in_city = |locs: &[Loc], city: &str| {
            locs.iter()
                .find(|l| {
                    let zone = world.zone_name(l.zone);
                    zone == city || zone.strip_prefix(city).is_some_and(|rest| rest.starts_with(' '))
                })
                .cloned()
        };
        let teleports = spells
            .iter()
            .filter_map(|(name, level)| {
                let city = name.strip_prefix("Teleport: ")?;
                let to = in_city(&trainers, city).or_else(|| in_city(&inns[..start_inn], city))?;
                Some(Teleport {
                    name: name.clone(),
                    level: *level,
                    to,
                })
            })
            .collect();
        let recall = spells
            .iter()
            .find(|(name, _)| name == "Astral Recall")
            .map(|(_, level)| *level);
        Ok(Self {
            trainers,
            inns,
            start_inn,
            teleports,
            recall,
        })
    }
}

/// The class's spells that travel ("Teleport: Stormwind", "Astral Recall") and the level they
/// are learned at; none without the class spell data.
fn class_spells(conn: &Connection, class_id: i64) -> Result<Vec<(String, i64)>> {
    let known: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE name = 'm_class_spell'",
        [],
        |r| r.get(0),
    )?;
    if known == 0 {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT name, min(level) FROM m_class_spell
         WHERE class_id = ?1 AND (name LIKE 'Teleport: %' OR name = 'Astral Recall')
         GROUP BY name",
    )?;
    let rows = stmt.query_map([class_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

struct NpcFinder<'a> {
    conn: &'a Connection,
    world: &'a World,
    entities: &'a Entities,
    faction: &'static str,
}

impl NpcFinder<'_> {
    fn place(&self, id: i64) -> Option<Loc> {
        let (pos, zone) = *self.entities.points(self.world, EntityKind::Npc, id).first()?;
        Some(Loc {
            pos,
            zone,
            kind: EntityKind::Npc,
            id,
            name: self.entities.name(EntityKind::Npc, id),
        })
    }

    /// NPCs of a title ("Innkeeper", "Warrior Trainer") friendly to the character.
    fn with_subname(&self, subname: &str) -> Result<Vec<Loc>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM m_npc WHERE subname = ?1 AND (friendly_to IS NULL OR instr(friendly_to, ?2) > 0)",
        )?;
        let ids: Vec<i64> = stmt
            .query_map(rusqlite::params![subname, self.faction], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids.into_iter().filter_map(|id| self.place(id)).collect())
    }
}
