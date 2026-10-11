//! Where an objective is done when its mobs or objects live in several places (`Objective::alts`):
//! the place that costs the least between where the character is and the quest's turn-in, counting
//! the trips, the work there (the level of its mobs, the wait for respawns) and the levels to grind
//! first when its mobs or the guards around are too strong. The turn-in, not the next stop of the
//! route: the choice stays the same wherever the stop is in the route (a route built stop by stop
//! is replayed as it was built).

use super::Planner;
use super::state::{Kind, State, Stop};
use crate::model::{Objective, Quest};

impl Planner<'_> {
    /// Objective `k` of `quest` at the place the character goes to from `s`.
    pub(crate) fn spot(&self, s: &State, quest: u32, k: u8) -> &Objective {
        let q = self.quest(quest);
        let o = &q.objectives[k as usize];
        if o.alts.is_empty() {
            return o;
        }
        let mut best = (f64::INFINITY, o);
        for place in std::iter::once(o).chain(&o.alts) {
            let there = self.spot_work(s, q, place);
            if there >= best.0 {
                continue;
            }
            let after = q
                .ends
                .iter()
                .map(|e| self.trips().leg(&place.loc.pos, &e.pos, s.level, s.known).0)
                .fold(f64::INFINITY, f64::min);
            let cost = there + after + self.trips().fastest(s, &place.loc.pos).0;
            if cost < best.0 {
                best = (cost, place);
            }
        }
        best.1
    }

    /// The objective at its place for an objective stop, None for the other stops.
    pub(crate) fn spot_of(&self, s: &State, stop: Stop) -> Option<&Objective> {
        match stop.kind {
            Kind::Objective(k) => Some(self.spot(s, stop.index, k)),
            _ => None,
        }
    }

    /// Time of the work at a place, with the grinding to the level it requires.
    fn spot_work(&self, s: &State, q: &Quest, o: &Objective) -> f64 {
        let gate = self.combat().level_at(q, o, s.bonus);
        let grind = if gate > s.level {
            self.growth().grind_time(s, gate)
        } else {
            0.0
        };
        grind + o.kills * self.combat().kill_time(s.level, s.bonus, o) + o.uses * self.params.object_time + o.extra
    }
}
