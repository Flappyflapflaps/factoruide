//! The fastest way between two places: walking, boats and zeppelins, flights between known
//! flight paths, class teleports.

use super::state::{Leg, State, Via};
use crate::model::Model;
use crate::params::Params;
use crate::world::{Pos, World};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::rc::Rc;

/// Flight times (seconds) and prices (copper) between flight paths.
pub(crate) type FlightTable = (Vec<Vec<f64>>, Vec<Vec<f64>>);

/// Flight tables by set of known flight paths, computed once each.
pub(crate) type FlightTables = RefCell<FxHashMap<u128, Rc<FlightTable>>>;

/// A point as a key: continent, coordinates (bits), zone.
type PosKey = (i64, u64, u64, i64);

/// Trips already computed (from, to, mounted, known flight paths): a replay asks the same trips
/// again and again. Emptied when it grows past `LEGS_MAX`.
pub(crate) type Legs = RefCell<FxHashMap<(PosKey, PosKey, bool, u128), (f64, Leg)>>;
const LEGS_MAX: usize = 1 << 18;

#[derive(Clone, Copy)]
pub(crate) struct Trips<'t> {
    pub(crate) world: &'t World,
    pub(crate) params: &'t Params,
    pub(crate) model: &'t Model,
    pub(crate) flights: &'t FlightTables,
    pub(crate) legs: &'t Legs,
}

impl Trips<'_> {
    pub(crate) fn walk(&self, a: &Pos, b: &Pos, level: i64) -> f64 {
        self.world.walk_distance(a, b) * self.params.detour / self.params.speed(level)
    }

    /// Fastest trip from `from` to `b` without the hearthstone.
    pub(crate) fn leg(&self, from: &Pos, b: &Pos, level: i64, known: u128) -> (f64, Leg) {
        let key = |p: &Pos| (p.continent, p.x.to_bits(), p.y.to_bits(), p.zone);
        // The level only matters through the speed: mounted or not.
        let key = (key(from), key(b), level >= self.params.mount_level, known);
        if let Some(&leg) = self.legs.borrow().get(&key) {
            return leg;
        }
        let leg = self.compute_leg(from, b, level, known);
        let mut legs = self.legs.borrow_mut();
        if legs.len() >= LEGS_MAX {
            legs.clear();
        }
        legs.insert(key, leg);
        leg
    }

    fn compute_leg(&self, from: &Pos, b: &Pos, level: i64, known: u128) -> (f64, Leg) {
        let walk = self.walk(from, b, level);
        let mut best = (walk, Leg::Walk);
        let links = &self.world.links;
        if !links.is_empty() && (from.continent != b.continent || walk > 240.0) {
            // The walks from each link to `b`, once (when a link is worth boarding).
            let mut to_b: Vec<f64> = Vec::new();
            for (i, li) in links.iter().enumerate() {
                let wa = self.walk(from, &li.from, level);
                if !wa.is_finite() || wa >= best.0 {
                    continue;
                }
                if to_b.is_empty() {
                    to_b = links.iter().map(|lj| self.walk(&lj.to, b, level)).collect();
                }
                for (j, wb) in to_b.iter().enumerate() {
                    let t = wa + self.world.link_chain[i][j] + wb;
                    if t < best.0 {
                        best = (t, Leg::Link(i, j));
                    }
                }
            }
        }
        if known.count_ones() < 2 {
            return best;
        }
        let flights = self.flight_table(known);
        let (flights, prices) = (&flights.0, &flights.1);
        // Flights weigh more than their time (flight_weight) and cost gold (gold_weight): the
        // trip is chosen on that weighted time, its real time is returned.
        let mut weighted = best.0;
        // The 3 known flight paths nearest on foot (ties: the first). A walk is never shorter
        // than the straight line: the paths are walked to nearest first as the crow flies, until
        // the next one is farther in a straight line than the third nearest walk.
        let nearest = |p: &Pos| {
            let line = |q: &Pos| p.dist(q) * self.params.detour / self.params.speed(level);
            let mut by_line: Vec<(usize, f64)> = (0..self.world.taxi_nodes.len())
                .filter(|i| known & (1u128 << i) != 0)
                .map(|i| (i, line(&self.world.taxi_nodes[i].pos)))
                .filter(|(_, d)| d.is_finite())
                .collect();
            by_line.sort_by(|x, y| x.1.total_cmp(&y.1));
            let mut v: Vec<(usize, f64)> = Vec::with_capacity(4);
            for (i, line) in by_line {
                if v.len() == 3 && line * (1.0 - 1e-9) > v[2].1 {
                    break;
                }
                let walk = self.walk(p, &self.world.taxi_nodes[i].pos, level);
                if walk.is_finite() {
                    let at = v.partition_point(|x| x.1.total_cmp(&walk).then(x.0.cmp(&i)).is_lt());
                    v.insert(at, (i, walk));
                    v.truncate(3);
                }
            }
            v
        };
        let near_b = nearest(b);
        for (i, wa) in nearest(from) {
            for &(j, wb) in &near_b {
                if i == j {
                    continue;
                }
                let t = wa + flights[i][j] + wb + self.params.flight_overhead;
                let w = t
                    + (self.params.flight_weight - 1.0) * (flights[i][j] + self.params.flight_overhead)
                    + self.params.gold_weight * prices[i][j] / 10_000.0;
                if w < weighted {
                    weighted = w;
                    best = (t, Leg::Fly(i, j));
                }
            }
        }
        best
    }

    /// Fastest trip from the current position, a class teleport first when learned (trained at
    /// its level) and faster. The hearthstone is a stop of its own (`Kind::Hearth`).
    pub(crate) fn fastest(&self, s: &State, b: &Pos) -> (f64, Via) {
        let (t, leg) = self.leg(&s.pos, b, s.level, s.known);
        let mut best = (t, Via { teleport: None, leg });
        for (i, tp) in self.model.teleports.iter().enumerate() {
            // Not before it is learned, nor when it cannot be faster: landing on another
            // continent, or farther than the trip as the crow flies at flight speed.
            if tp.level > s.trained
                || tp.to.pos.continent != b.continent
                || self.params.teleport_cast + tp.to.pos.dist(b) / self.params.flight_speed >= best.0
            {
                continue;
            }
            let (t, leg) = self.leg(&tp.to.pos, b, s.level, s.known);
            let t = t + self.params.teleport_cast;
            if t < best.0 {
                best = (t, Via { teleport: Some(i), leg });
            }
        }
        best
    }

    pub(crate) fn flight_table(&self, known: u128) -> Rc<FlightTable> {
        self.flights
            .borrow_mut()
            .entry(known)
            .or_insert_with(|| Rc::new(self.world.flights(known)))
            .clone()
    }
}
