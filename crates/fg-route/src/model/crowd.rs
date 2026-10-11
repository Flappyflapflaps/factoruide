//! The spot where an objective's normal mobs (or objects) are farmed: how many live there, how
//! many come in each pull, and the wait when the objective asks for more than the spot holds,
//! shared with the other players of a crowded server. Measured once, when the model is built.

use super::builder::QuestBuilder;
use super::guard::Guards;
use super::sources::QuestRow;
use super::types::{EntityKind, Objective};
use crate::world::Pos;

/// Spawns this close to an objective's place are its spot (yards).
const SPOT: f64 = 200.0;

impl QuestBuilder<'_> {
    pub(super) fn spots_and_crowd(&self, r: &QuestRow, objectives: &mut [Objective]) {
        let p = self.params;
        let level = r.level.unwrap_or(1).max(1);
        let crowd = p.crowd_at(level);
        for o in objectives.iter_mut().filter(|o| o.dungeon.is_none()) {
            if o.loc.kind == EntityKind::Area && is_event(r, &o.text) {
                o.extra += crowd * p.crowd_event_time;
                continue;
            }
            let (work, each) = if o.kills > 0.0 && !o.elite && !o.mobs.is_empty() {
                let points = self.spot_points(o, EntityKind::Npc);
                o.spawns = points.len() as u32;
                if !points.is_empty() {
                    let pulls: u32 = points.iter().map(|q| self.sources.guards.pull_at(q)).sum();
                    o.pack = f64::from(pulls) / points.len() as f64;
                }
                (o.kills, p.kill_time(level, 0.0, o.mob_level))
            } else if o.uses > 0.0 && o.loc.kind == EntityKind::Object {
                o.spawns = self.spot_points(o, EntityKind::Object).len() as u32;
                (o.uses, p.object_time)
            } else {
                continue;
            };
            o.extra += respawn_wait(work, f64::from(o.spawns), each, p.respawn_time, crowd * p.crowd_players);
        }
    }

    /// Spawn points of the objective's mobs (or its object) at its spot, those at one point
    /// counted once.
    fn spot_points(&self, o: &Objective, kind: EntityKind) -> Vec<Pos> {
        let ids: Vec<i64> = if kind == EntityKind::Npc {
            o.mobs.iter().map(|(id, _)| *id).collect()
        } else {
            vec![o.loc.id]
        };
        let mut points: Vec<Pos> = Vec::new();
        for id in ids {
            for (q, _) in self.sources.entities.points(self.world, kind, id) {
                if q.dist(&o.loc.pos) <= SPOT && !points.iter().any(|x| x.dist(&q) < Guards::ONE_POINT) {
                    points.push(q);
                }
            }
        }
        points
    }
}

/// Time waiting for respawns: `work` kills (or uses) of `each` seconds at a spot of `spawns`,
/// shared with `others` players. Once the spot is cleared, one comes back every `respawn` /
/// spawns seconds; the wait is what that leaves beyond the kill itself. Nothing when the spot
/// is unknown.
fn respawn_wait(work: f64, spawns: f64, each: f64, respawn: f64, others: f64) -> f64 {
    if spawns <= 0.0 {
        return 0.0;
    }
    let mine = spawns / (1.0 + others);
    if work <= mine {
        return 0.0;
    }
    (respawn / mine - each).max(0.0) * (work - mine)
}

/// An escort or an event (defend someone, overhear a talk): the quest is flagged as one, or its
/// objective says so.
fn is_event(r: &QuestRow, text: &str) -> bool {
    r.flags.is_some_and(|f| f & 2 != 0) || ["Escort", "Protect", "Defend"].iter().any(|w| text.starts_with(w))
}

#[cfg(test)]
mod tests {
    use super::respawn_wait;

    /// Enough mobs: no wait; a small spot: the respawns are waited for, longer when shared.
    #[test]
    fn small_spots_wait_for_respawns() {
        assert!(respawn_wait(10.0, 30.0, 25.0, 300.0, 0.0).abs() < 1e-9);
        let alone = respawn_wait(22.0, 7.0, 25.0, 300.0, 0.0);
        let shared = respawn_wait(22.0, 7.0, 25.0, 300.0, 3.0);
        assert!(alone > 0.0 && shared > 3.0 * alone, "{alone} {shared}");
        assert!(respawn_wait(22.0, 0.0, 25.0, 300.0, 3.0).abs() < 1e-9, "unknown spot");
    }
}
