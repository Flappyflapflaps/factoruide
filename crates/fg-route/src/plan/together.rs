//! The last pass on a route, computed or edited by hand: objective stops done at the same place
//! brought next to each other when that does not make the route worse, so that the guide shows
//! them together (the addon lists the steps at one place as one) and the player does them at
//! once, the objects gathered among the mobs (see `state::Together`).

use super::Planner;
use super::state::{Kind, Stop};
use crate::world::Pos;

/// How many items later an objective at the same place is still brought next to the first.
const AHEAD: usize = 30;

impl Planner<'_> {
    /// `items` (stops of a route, or steps of an edited guide) with each objective stop done
    /// within `gather_radius` of an earlier one, up to `AHEAD` items later, moved right after it
    /// (and the others already there), each move kept when `score` (lower is better, None:
    /// invalid) is no worse. `stop`: the route stop of an item; `places`: where each item was
    /// done (None: not an objective, or not done).
    pub(crate) fn bring_together<T: Clone>(
        &self,
        items: Vec<T>,
        stop: impl Fn(&T) -> Option<Stop>,
        places: Vec<Option<Pos>>,
        mut score: impl FnMut(&[T]) -> Option<f64>,
    ) -> Vec<T> {
        let radius = self.params.gather_radius;
        let objective = |item: &T| stop(item).is_some_and(|s| matches!(s.kind, Kind::Objective(_)));
        let Some(mut best) = score(&items) else {
            return items;
        };
        let mut items: Vec<(T, Option<Pos>)> = items.into_iter().zip(places).collect();
        let mut i = 0;
        while i < items.len() {
            let here = match &items[i] {
                (item, Some(p)) if objective(item) => *p,
                _ => {
                    i += 1;
                    continue;
                }
            };
            // `end`: the last of the objectives at this place, next to each other.
            let mut end = i;
            let mut j = i + 1;
            while j < items.len() && j <= end + AHEAD {
                let near = objective(&items[j].0)
                    && items[j]
                        .1
                        .is_some_and(|p| p.continent == here.continent && p.dist(&here) <= radius);
                if near {
                    if j == end + 1 {
                        end = j;
                    } else {
                        let mut candidate = items.clone();
                        let moved = candidate.remove(j);
                        candidate.insert(end + 1, moved);
                        let route: Vec<T> = candidate.iter().map(|(item, _)| item.clone()).collect();
                        if let Some(t) = score(&route)
                            && t <= best + 1e-6
                        {
                            (items, best) = (candidate, t);
                            end += 1;
                        }
                    }
                }
                j += 1;
            }
            i = end + 1;
        }
        items.into_iter().map(|(item, _)| item).collect()
    }

    /// `bring_together` on a route of the optimizer, scored as the search does.
    pub fn route_together(&self, route: Vec<Stop>) -> Vec<Stop> {
        let Some(run) = self.run(&route) else {
            return route;
        };
        // Where each stop was done: the character's place after it.
        let places = (0..route.len())
            .map(|k| {
                matches!(route[k].kind, Kind::Objective(_))
                    .then(|| run.states.get(k + 1).map(|s| s.pos))
                    .flatten()
            })
            .collect();
        drop(run);
        self.bring_together(route, |s| Some(*s), places, |r| self.simulate(r, None))
    }
}
