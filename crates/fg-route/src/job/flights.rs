//! Flight paths a character probably knows from its quests: the game only tells an addon the
//! ones of the continent where a flight master's map is open, so those of the other continent
//! are deduced from the quests done in their towns.

use crate::faction::Faction;
use crate::world::World;
use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

/// A quest giver or turn-in this close to a flight master (yards) is in its town.
const TOWN: f64 = 300.0;
/// Quests done in a town before its flight path counts as learned: one may have been done on a
/// walk through.
const MIN_QUESTS: usize = 2;

/// The flight paths (TaxiNodes IDs, usable by `faction`) of the towns where at least
/// `MIN_QUESTS` of the `completed` quests were given or turned in.
pub fn inferred_flights(conn: &Connection, faction: Faction, completed: &[i64]) -> Result<Vec<i64>> {
    let world = World::load(conn, faction, 1.0)?;
    let done: HashSet<i64> = completed.iter().copied().collect();
    let mut stmt = conn.prepare(
        "SELECT r.quest_id, s.zone_id, s.x, s.y FROM m_quest_relation r
         JOIN m_spawn s ON s.entity_type = r.entity_type AND s.entity_id = r.entity_id
         WHERE s.x >= 0",
    )?;
    // Quests with a giver or a turn-in in the town of each flight master.
    let mut towns: HashMap<usize, HashSet<i64>> = HashMap::new();
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, f64>(2)?,
            r.get::<_, f64>(3)?,
        ))
    })? {
        let (quest, zone, x, y) = row?;
        if !done.contains(&quest) {
            continue;
        }
        let Some(p) = world.to_world(zone, x, y) else {
            continue;
        };
        for (n, node) in world.taxi_nodes.iter().enumerate() {
            if node.pos.continent == p.continent && node.pos.dist(&p) <= TOWN {
                towns.entry(n).or_default().insert(quest);
            }
        }
    }
    let mut found: Vec<i64> = towns
        .into_iter()
        .filter(|(_, quests)| quests.len() >= MIN_QUESTS)
        .map(|(n, _)| world.taxi_nodes[n].id)
        .collect();
    found.sort_unstable();
    Ok(found)
}
