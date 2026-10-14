//! Mobs anyone can farm on the way: the spawns of normal hostile or neutral mobs, by cell, to
//! count those met along a walk.

use crate::world::{Pos, World};
use anyhow::Result;
use rusqlite::Connection;
use rustc_hash::FxHashMap;
use std::collections::HashSet;

/// Spawns of a cell (continent, x, y): position and level.
type Cells = FxHashMap<(i64, i64, i64), Vec<(f32, f32, u8)>>;

/// Spawns of normal mobs by cell of `FarmMobs::CELL` yards.
#[derive(Debug, Default)]
pub struct FarmMobs {
    cells: Cells,
}

impl FarmMobs {
    const CELL: f64 = 100.0;
    /// Spawns closer than this are one mob (the sources list some twice).
    const SAME: f64 = 10.0;

    /// Normal mobs (not elite or rare, no gossip, vendor or quest flag) that are not of a faction:
    /// hostile to both, or neutral beasts. None in the capitals (the game's flag 0x100): what the
    /// sources know nothing of there is a passer-by (Theramore Emissary in Stormwind), a pet
    /// (the warlock trainers' demons) or a dungeon's mob put at its door (the Stockade's).
    pub(super) fn load(conn: &Connection, world: &World) -> Result<Self> {
        let mut stmt = conn.prepare(
            "SELECT s.zone_id, s.x, s.y, coalesce(n.min_level, 1), coalesce(n.max_level, n.min_level, 1)
             FROM m_spawn s JOIN m_npc n ON n.id = s.entity_id
             WHERE s.entity_type = 'npc' AND s.x >= 0 AND coalesce(n.rank, 0) = 0
               AND coalesce(n.npc_flags, 0) = 0 AND coalesce(n.friendly_to, '') IN ('', 'AH')
               AND s.zone_id NOT IN (SELECT ID FROM client_areatable WHERE (Flags_0 & 256) != 0)",
        )?;
        let mut cells = Cells::default();
        let mut seen = HashSet::new();
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        for row in rows {
            let (zone, x, y, min, max) = row?;
            let Some(p) = world.to_world(zone, x, y) else { continue };
            let level = i64::midpoint(min, max).clamp(1, 255) as u8;
            let same = (
                p.continent,
                (p.x / Self::SAME).floor() as i64,
                (p.y / Self::SAME).floor() as i64,
                level,
            );
            if seen.insert(same) {
                cells
                    .entry(Self::cell(&p))
                    .or_default()
                    .push((p.x as f32, p.y as f32, level));
            }
        }
        Ok(Self { cells })
    }

    fn cell(p: &Pos) -> (i64, i64, i64) {
        (
            p.continent,
            (p.x / Self::CELL).floor() as i64,
            (p.y / Self::CELL).floor() as i64,
        )
    }

    /// Mobs within `radius` of `p` whose level is in `levels`.
    pub fn around(&self, p: &Pos, radius: f64, levels: std::ops::RangeInclusive<i64>) -> usize {
        let (lo, hi) = (
            Self::cell(&Pos {
                x: p.x - radius,
                y: p.y - radius,
                ..*p
            }),
            Self::cell(&Pos {
                x: p.x + radius,
                y: p.y + radius,
                ..*p
            }),
        );
        let mut n = 0;
        for cx in lo.1..=hi.1 {
            for cy in lo.2..=hi.2 {
                n += self
                    .cells
                    .get(&(p.continent, cx, cy))
                    .into_iter()
                    .flatten()
                    .filter(|&&(x, y, level)| {
                        levels.contains(&i64::from(level)) && (f64::from(x) - p.x).hypot(f64::from(y) - p.y) <= radius
                    })
                    .count();
            }
        }
        n
    }

    /// Mobs within `corridor` of the walk through `path` (its points in order, on the same
    /// continent), each once, by level: (level, count).
    pub fn along(&self, path: &[Pos], corridor: f64) -> Vec<(u8, u16)> {
        let mut counts = [0u16; 256];
        let Some(first) = path.first() else {
            return Vec::new();
        };
        let legs: Vec<(Pos, Pos)> = match path {
            [p] => vec![(*p, *p)],
            _ => path.windows(2).map(|w| (w[0], w[1])).collect(),
        };
        let near = |p: &Pos, reach: f64| legs.iter().any(|(a, b)| crate::plan::segment_dist(p, a, b) <= reach);
        // The cells of the box of each leg grown by the corridor, once each, in order.
        let mut cells: Vec<(i64, i64)> = Vec::new();
        for (a, b) in &legs {
            let (lo, hi) = (
                Self::cell(&Pos {
                    x: a.x.min(b.x) - corridor,
                    y: a.y.min(b.y) - corridor,
                    ..*a
                }),
                Self::cell(&Pos {
                    x: a.x.max(b.x) + corridor,
                    y: a.y.max(b.y) + corridor,
                    ..*a
                }),
            );
            cells.extend((lo.1..=hi.1).flat_map(|cx| (lo.2..=hi.2).map(move |cy| (cx, cy))));
        }
        cells.sort_unstable();
        cells.dedup();
        for (cx, cy) in cells {
            // Cells farther than the corridor from the walk hold nothing to count.
            let center = Pos {
                x: (cx as f64 + 0.5) * Self::CELL,
                y: (cy as f64 + 0.5) * Self::CELL,
                ..*first
            };
            if !near(&center, corridor + Self::CELL * 0.75) {
                continue;
            }
            for &(x, y, level) in self.cells.get(&(first.continent, cx, cy)).into_iter().flatten() {
                let p = Pos {
                    x: f64::from(x),
                    y: f64::from(y),
                    ..*first
                };
                if near(&p, corridor) {
                    counts[level as usize] = counts[level as usize].saturating_add(1);
                }
            }
        }
        counts
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(level, n)| (level as u8, *n))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64) -> Pos {
        Pos {
            continent: 1,
            x,
            y,
            zone: 0,
        }
    }

    /// A walk around a corner meets the mobs along its two legs, each once (the one at the
    /// corner too), not those on the straight line it does not take.
    #[test]
    fn mobs_met_along_the_walk_taken() {
        let mut farm = FarmMobs::default();
        for (x, y, level) in [
            (0.0, 500.0, 10),
            (250.0, 250.0, 11),
            (500.0, 500.0, 12),
            (500.0, 250.0, 13),
        ] {
            farm.cells
                .entry(FarmMobs::cell(&at(x, y)))
                .or_default()
                .push((x as f32, y as f32, level));
        }
        let (a, corner, b) = (at(0.0, 0.0), at(0.0, 500.0), at(500.0, 500.0));
        assert_eq!(farm.along(&[a, corner, b], 25.0), vec![(10, 1), (12, 1)]);
        assert_eq!(farm.along(&[a, b], 25.0), vec![(11, 1), (12, 1)]);
    }
}
