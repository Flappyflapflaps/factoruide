//! Items sold in limited supply (the vendor may be out of them): the step at the vendor the route
//! goes to does not hold the guide (it stays listed along the way until the item is got: the
//! turn-in waits for it), and the other vendors of the item the route passes by between the
//! quest's accept and its turn-in offer it too, along the way.

use super::phrase::{Arg, Phrase};
use super::types::{Step, StepKind};
use super::writer::StepWriter;
use crate::world::Pos;

/// A vendor is passed by when a step of the route is this close (yards).
const NEAR: f64 = 150.0;
/// Vendors offered on the way for one item, at most.
const MAX_TRIES: usize = 2;

/// Add the steps offering the item at the vendors passed by, with the XP the route has after
/// the step they follow.
pub(super) fn add_tries(writer: &StepWriter<'_>, steps: &mut Vec<Step>, xps: &mut Vec<i64>) {
    let model = writer.planner.model;
    let mut tries: Vec<(usize, Step)> = Vec::new();
    let mut limited = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if step.kind != StepKind::Objective || step.bg {
            continue;
        }
        let (Some(q), Some(k)) = (step.quest, step.objective) else {
            continue;
        };
        let Some(o) = (model.index.get(&q)).and_then(|&i| model.quests[i].objectives.get(usize::from(k))) else {
            continue;
        };
        if o.vendors.first().is_none_or(|v| v.stock.is_none()) {
            continue;
        }
        let Some(item) = step.target.as_ref().map(|t| Arg::Item(t.id, t.name.clone())) else {
            continue;
        };
        limited.push(i);
        // From the accept on (a quest of the character's log: from the start), to the turn-in.
        let start = (steps[..i].iter())
            .rposition(|s| s.kind == StepKind::Accept && s.quest == Some(q))
            .map_or(0, |a| a + 1);
        let end = (steps[i..].iter())
            .position(|s| s.kind == StepKind::TurnIn && s.quest == Some(q))
            .map_or(steps.len(), |t| i + t);
        for (j, v) in passed_by(steps, start..end, i, &o.vendors[1..]) {
            let at = &steps[j];
            let mut offer = Step::bare(StepKind::Objective, at.level, at.time);
            offer.quest = Some(q);
            offer.quest_name.clone_from(&step.quest_name);
            offer.objective = Some(k);
            offer.target.clone_from(&step.target);
            offer.bg = true;
            offer.try_buy = true;
            offer.mobs = vec![v.name.clone()];
            offer.mob_ids = vec![v.id];
            offer.say.push(
                Phrase::new("objective.buy.try")
                    .arg("who", item.clone())
                    .arg("vendor", Arg::Npc(v.id, v.name.clone()))
                    .arg("n", Arg::Num(o.count.round().max(1.0) as i64)),
            );
            if let Some((n, restock)) = v.stock {
                offer.say.push(
                    Phrase::new("suffix.stock")
                        .arg("n", Arg::Text(n.to_string()))
                        .arg("hours", Arg::Num((restock / 3600.0).round().max(1.0) as i64)),
                );
            }
            writer.put_at(&mut offer, &v.pos, v.zone);
            tries.push((j + 1, offer));
        }
    }
    for i in limited {
        steps[i].bg = true;
    }
    // From the last, so that the places stay right; those after the same step in route order.
    tries.sort_by_key(|t| t.0);
    for (at, offer) in tries.into_iter().rev() {
        let xp = xps[at - 1];
        steps.insert(at, offer);
        xps.insert(at, xp);
    }
}

/// The vendors the steps of `window` (but the buy step `own`) pass by, each with the first step
/// passing by it, `MAX_TRIES` at most, in the order of `vendors`.
fn passed_by<'v>(
    steps: &[Step],
    window: std::ops::Range<usize>,
    own: usize,
    vendors: &'v [crate::model::Vendor],
) -> Vec<(usize, &'v crate::model::Vendor)> {
    (vendors.iter())
        .filter_map(|v| {
            let j = window
                .clone()
                .find(|&j| j != own && steps[j].world.is_some_and(|w| near(w, &v.pos)))?;
            Some((j, v))
        })
        .take(MAX_TRIES)
        .collect()
}

/// Whether a step at `world` ([continent, x, y]) passes by `pos`.
fn near(world: [f64; 3], pos: &Pos) -> bool {
    world[0] as i64 == pos.continent && (world[1] - pos.x).hypot(world[2] - pos.y) <= NEAR
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Vendor;

    fn at(x: f64, y: f64) -> Step {
        let mut s = Step::bare(StepKind::Objective, 20, 0.0);
        s.world = Some([0.0, x, y]);
        s
    }

    fn vendor(id: i64, x: f64, y: f64) -> Vendor {
        let pos = Pos {
            continent: 0,
            x,
            y,
            zone: 0,
        };
        Vendor {
            id,
            name: String::new(),
            zone: 0,
            zone_name: String::new(),
            pos,
            stock: Some((2, 10800.0)),
        }
    }

    /// The vendors passed by between the accept and the turn-in, the first step near each, two
    /// at most; not the buy step itself, nor the other continent, nor far away.
    #[test]
    fn vendors_passed_by_on_the_way() {
        let steps = vec![
            at(0.0, 0.0),
            at(1000.0, 0.0),
            at(2000.0, 0.0),
            at(2050.0, 0.0),
            at(5000.0, 0.0),
        ];
        let vendors = [
            vendor(1, 2060.0, 30.0),
            vendor(2, 5000.0, 0.0),
            vendor(3, 990.0, 0.0),
            vendor(4, 9000.0, 0.0),
        ];
        let found: Vec<(usize, i64)> = passed_by(&steps, 1..5, 4, &vendors)
            .iter()
            .map(|(j, v)| (*j, v.id))
            .collect();
        assert_eq!(found, vec![(2, 1), (1, 3)]);
    }
}
