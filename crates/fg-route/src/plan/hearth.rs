//! Where the hearthstone is bound. The route is cut into bands of levels (fewer levels each
//! higher up, where each takes longer); for each, the inns closest to where its time is spent
//! are tried, bound during the band, and the best route is kept (one band after the other).
//! The local search then moves and changes them (`moves`).

use super::Planner;
use super::evaluate::Run;
use super::state::{Kind, Stop};
use crate::world::Pos;

/// Inns tried for a band, or drawn from by the local search: the closest ones.
pub(crate) const INNS_TRIED: usize = 3;

impl Planner<'_> {
    /// Levels a band starting at `level` covers.
    fn band_width(level: i64) -> i64 {
        match level {
            ..20 => 5,
            20..40 => 4,
            _ => 3,
        }
    }

    /// The hearthstone bound anew for each band of levels at one of the `INNS_TRIED` inns closest
    /// to where the band's time is spent, its other binds removed, or nothing changed: whatever
    /// makes the route best.
    pub(crate) fn rebind(&self, mut route: Vec<Stop>) -> Vec<Stop> {
        let Some(mut run) = self.run(&route) else {
            return route;
        };
        let mut low = self.profile.from_level;
        while low < self.profile.to_level && !self.aborted() {
            let high = low + Self::band_width(low);
            let band: Vec<usize> = (0..run.states.len() - 1)
                .filter(|&k| (low..high).contains(&run.states[k].level))
                .collect();
            low = high;
            let (Some(&a), Some(&z)) = (band.first(), band.last()) else {
                continue;
            };
            let Some(center) = Self::activity_center(&run, a, z) else {
                continue;
            };
            let mut best: Option<(f64, Vec<Stop>)> = None;
            let mut consider = |candidate: Vec<Stop>| {
                if let Some(t) = self.simulate(&candidate, None)
                    && t < best.as_ref().map_or(run.total - 0.5, |b| b.0)
                {
                    best = Some((t, candidate));
                }
            };
            let others: Vec<Stop> = route[a..=z].iter().filter(|s| s.kind != Kind::Bind).copied().collect();
            let with = |at: Option<(usize, Stop)>| {
                let mut out = route[..a].to_vec();
                let mut middle = others.clone();
                if let Some((at, bind)) = at {
                    middle.insert(at, bind);
                }
                out.extend(middle);
                out.extend_from_slice(&route[z + 1..]);
                out
            };
            if others.len() <= z - a {
                consider(with(None));
            }
            for inn in self.inns_near(&center, INNS_TRIED) {
                let bind = Stop {
                    index: inn as u32,
                    kind: Kind::Bind,
                };
                let pos = self.model.inns[inn].pos;
                // After the stop closest to the inn, in the whole band or early in it.
                let s0 = self.initial_state();
                let nearest = |n: usize| {
                    (0..n.min(others.len()))
                        .min_by(|&x, &y| {
                            let dx = self.places().pos(&s0, others[x]).dist(&pos);
                            let dy = self.places().pos(&s0, others[y]).dist(&pos);
                            dx.total_cmp(&dy)
                        })
                        .map_or(0, |k| k + 1)
                };
                let (anywhere, early) = (nearest(others.len()), nearest(others.len().div_ceil(3)));
                consider(with(Some((anywhere, bind))));
                if early != anywhere {
                    consider(with(Some((early, bind))));
                }
            }
            if let Some((_, better)) = best
                && let Some(r) = self.run(&better)
            {
                (route, run) = (better, r);
            }
        }
        route
    }

    /// Where the time of stops `a..=z` is spent: the mean place of the stops on the continent
    /// most of it is spent on, weighed by their time.
    fn activity_center(run: &Run, a: usize, z: usize) -> Option<Pos> {
        let spent: Vec<(Pos, f64)> = (a..=z)
            .map(|k| (run.states[k + 1].pos, run.states[k + 1].time - run.states[k].time))
            .filter(|(_, t)| t.is_finite() && *t > 0.0)
            .collect();
        let mut by_continent: Vec<(i64, f64)> = Vec::new();
        for (p, t) in &spent {
            match by_continent.iter_mut().find(|(c, _)| *c == p.continent) {
                Some(c) => c.1 += t,
                None => by_continent.push((p.continent, *t)),
            }
        }
        let continent = by_continent.iter().max_by(|x, y| x.1.total_cmp(&y.1))?.0;
        let here: Vec<&(Pos, f64)> = spent.iter().filter(|(p, _)| p.continent == continent).collect();
        let total: f64 = here.iter().map(|(_, t)| t).sum();
        let first = here.first()?.0;
        Some(Pos {
            x: here.iter().map(|(p, t)| p.x * t).sum::<f64>() / total,
            y: here.iter().map(|(p, t)| p.y * t).sum::<f64>() / total,
            ..first
        })
    }

    /// The `n` inns closest to `pos` on its continent the hearthstone can be bound to.
    pub(crate) fn inns_near(&self, pos: &Pos, n: usize) -> Vec<usize> {
        let mut inns: Vec<(usize, f64)> = self
            .model
            .inns
            .iter()
            .enumerate()
            .filter(|(i, l)| *i != self.model.start_inn && l.pos.continent == pos.continent)
            .map(|(i, l)| (i, l.pos.dist(pos)))
            .collect();
        inns.sort_by(|x, y| x.1.total_cmp(&y.1));
        inns.into_iter().take(n).map(|(i, _)| i).collect()
    }
}
