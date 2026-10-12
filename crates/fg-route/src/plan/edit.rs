//! Replaying a route edited by hand: each step checked in turn (the impossible ones are reported
//! and skipped), what each one did, and the time of the whole, as the optimizer counts it.

use super::Planner;
use super::state::{Breakdown, Event, Kind, State, Stop, Timed};
use super::step::recorder;
use std::ops::Range;

/// A step of a route edited by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// A stop of the route; `farm`: kill every normal mob met on the way to it, beyond
    /// `farm_cap`.
    Stop { stop: Stop, farm: bool },
    /// Fly from a flight path to another (taxi node indexes), both known.
    Fly { from: usize, to: usize },
    /// Kill this many mobs of the character's level where it stands.
    Grind { mobs: u32 },
}

/// What a step of an edited route did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    /// Nothing to do there (why): taken or turned in at hand earlier, objective done on the way,
    /// the hearthstone on cooldown, the target level reached before.
    Skipped(&'static str),
    /// Not possible there (why): the step is left out.
    Invalid(&'static str),
}

/// Mobs killed on the way to a step.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize)]
pub struct Farmed {
    pub kills: u32,
    pub xp: i64,
    pub seconds: f64,
}

/// A step of an edited route as replayed.
#[derive(Debug, Clone)]
pub struct EditedStep {
    pub outcome: Outcome,
    /// Its events in the trace.
    pub events: Range<usize>,
    /// Time before and after it, level after it, and the XP it gave.
    pub start: f64,
    pub end: f64,
    pub level: i64,
    pub xp: i64,
    /// Mobs farmed on the way to it.
    pub farm: Option<Farmed>,
    /// An objective done without `farm`: what killing every mob on the way to it would give.
    pub farm_all: Option<Farmed>,
}

/// An edited route as replayed.
pub struct Edited {
    pub steps: Vec<EditedStep>,
    /// The stops done, in order (for the guide).
    pub route: Vec<Stop>,
    pub trace: Vec<Timed>,
    /// Real play time in `breakdown.total`; `score` adds the optimizer's penalties.
    pub breakdown: Breakdown,
    pub score: f64,
    /// Grinding added at the end to reach the target level (seconds).
    pub final_grind: f64,
    /// Quests the route takes but never turns in.
    pub unfinished: Vec<u32>,
}

impl Edited {
    /// Steps that are not possible.
    pub fn errors(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.outcome, Outcome::Invalid(_)))
            .count()
    }
}

impl Planner<'_> {
    /// Replay an edited route as the optimizer simulates its own (`simulate_full`: the same
    /// route gives the same time), each step checked: an impossible one is reported and left out.
    pub fn edit(&self, edits: &[Edit]) -> Edited {
        let stops: Vec<Stop> = edits
            .iter()
            .filter_map(|e| match e {
                Edit::Stop { stop, .. } => Some(*stop),
                _ => None,
            })
            .collect();
        let mut s = self.initial_state();
        s.planned = self.planned(&stops);
        let mut trace = Vec::new();
        let mut route = Vec::new();
        let mut steps = Vec::with_capacity(edits.len());
        let mut at = 0;
        // The next stop of each step (an objective is done at its place on the way to it).
        let nexts: Vec<Option<Stop>> = (0..edits.len())
            .map(|e| {
                edits[e + 1..].iter().find_map(|x| match x {
                    Edit::Stop { stop, .. } => Some(*stop),
                    _ => None,
                })
            })
            .collect();
        for (e, edit) in edits.iter().enumerate() {
            let (start, xp, first) = (s.time, self.total_xp(&s), trace.len());
            let farm_xp = s.spent.farm_xp;
            let mut farm_all = None;
            let outcome = if s.level >= self.profile.to_level {
                Outcome::Skipped("target level reached")
            } else {
                match *edit {
                    Edit::Stop { stop, farm } => {
                        s.at = at;
                        s.next = nexts[e].map(|next| self.places().pos(&s, next));
                        if !farm && let Kind::Objective(_) = stop.kind {
                            farm_all = self.farm_all_estimate(&s, stop);
                        }
                        let outcome = self.edit_stop(&mut s, stop, farm, &mut trace);
                        if !matches!(outcome, Outcome::Invalid(_)) {
                            route.push(stop);
                        }
                        outcome
                    }
                    Edit::Fly { from, to } => self.edit_fly(&mut s, from, to, &mut trace),
                    Edit::Grind { mobs } => {
                        if mobs == 0 {
                            Outcome::Invalid("no mob to kill")
                        } else {
                            let mut trace = Some(&mut trace);
                            let mut record = recorder(&mut trace);
                            let (near, zone) = (s.pos, s.zone);
                            let seconds = self.growth().grind_mobs(&mut s, mobs);
                            record(
                                &s,
                                Event::GrindMobs {
                                    mobs,
                                    seconds,
                                    near,
                                    zone,
                                },
                            );
                            self.train(&mut s, &mut record);
                            Outcome::Done
                        }
                    }
                }
            };
            if let Edit::Stop { .. } = edit {
                at += 1;
            }
            steps.push(EditedStep {
                outcome,
                events: first..trace.len(),
                start,
                end: s.time,
                level: s.level,
                xp: (self.total_xp(&s) - xp) as i64,
                farm: farmed(&trace[first..], s.spent.farm_xp - farm_xp),
                farm_all,
            });
        }
        let grinding = s.spent.grinding;
        let score = self.finish(&mut s, Some(&mut trace));
        // Taken and never turned in by the route, left in the log or given up (those it turns in
        // after the target level are not: the guide ends there).
        let has = |i: usize, kind: Kind| stops.iter().any(|t| t.index as usize == i && t.kind == kind);
        let unfinished = (0..self.model.quests.len())
            .filter(|&i| {
                !s.rewarded(i) && (s.accepted[i] || s.turned[i]) && has(i, Kind::Accept) && !has(i, Kind::TurnIn)
            })
            .map(|i| i as u32)
            .collect();
        Edited {
            steps,
            route,
            trace,
            final_grind: s.spent.grinding - grinding,
            breakdown: s.spent,
            score,
            unfinished,
        }
    }

    /// A stop of an edited route, checked first.
    fn edit_stop(&self, s: &mut State, stop: Stop, farm: bool, trace: &mut Vec<Timed>) -> Outcome {
        // Nothing to do rather than impossible: a flight path known, the hearthstone bound there.
        let i = stop.index as usize;
        match stop.kind {
            Kind::LearnFlight if s.known & (1u128 << i) != 0 => return Outcome::Skipped("flight path already known"),
            Kind::Bind if s.bind == i => return Outcome::Skipped("hearthstone already bound there"),
            _ => {}
        }
        if !self.availability().valid(s, stop) {
            return Outcome::Invalid(self.why_invalid(s, stop));
        }
        let before = Self::already_done(s, stop);
        let (saved, mark) = (s.clone(), trace.len());
        self.farm_all.set(farm);
        self.apply(s, stop, Some(trace));
        self.farm_all.set(false);
        if !s.time.is_finite() {
            *s = saved;
            trace.truncate(mark);
            return Outcome::Invalid("unreachable");
        }
        let hearth = || trace[mark..].iter().any(|t| matches!(t.event, Event::Hearth { .. }));
        if before {
            Outcome::Skipped("done before")
        } else if stop.kind == Kind::Hearth && !hearth() {
            Outcome::Skipped("not ready")
        } else {
            Outcome::Done
        }
    }

    /// What killing every normal mob on the way to `stop` would give.
    fn farm_all_estimate(&self, s: &State, stop: Stop) -> Option<Farmed> {
        if !self.availability().valid(s, stop) || Self::already_done(s, stop) {
            return None;
        }
        let mut s = s.clone();
        let mut trace = Vec::new();
        let farm_xp = s.spent.farm_xp;
        self.farm_all.set(true);
        self.apply(&mut s, stop, Some(&mut trace));
        self.farm_all.set(false);
        farmed(&trace, s.spent.farm_xp - farm_xp)
    }

    /// A flight of an edited route: to the flight master first.
    fn edit_fly(&self, s: &mut State, from: usize, to: usize, trace: &mut Vec<Timed>) -> Outcome {
        let known = |n: usize| s.known & (1u128 << n) != 0;
        if from == to {
            return Outcome::Invalid("same flight path");
        }
        if !known(from) {
            return Outcome::Invalid("flight path not known");
        }
        if !known(to) {
            return Outcome::Invalid("destination not known");
        }
        let table = self.trips().flight_table(s.known);
        let (time, price) = (table.0[from][to], table.1[from][to]);
        if !time.is_finite() {
            return Outcome::Invalid("no flight between them");
        }
        let nodes = &self.world.taxi_nodes;
        let mut trace = Some(trace);
        let mut record = recorder(&mut trace);
        // As a trip taking the flight by itself: the walk to the flight master and the flight
        // count as flying, the walk on to the next stop ends it (`State::landed`).
        let (start, end) = (nodes[from].pos, nodes[to].pos);
        let walk = self.trips().walk(&s.pos, &start, s.level);
        s.time += walk;
        s.spent.flights += walk;
        record(s, Event::Fly { from, to });
        let seconds = time + self.params.flight_overhead;
        s.time += seconds;
        s.spent.flights += seconds;
        if price.is_finite() {
            s.spent.flight_cost += price as i64;
        }
        s.pos = end;
        s.landed = true;
        Outcome::Done
    }
}

/// The mobs farmed among `events`, which gave `xp`.
fn farmed(events: &[Timed], xp: i64) -> Option<Farmed> {
    let (kills, seconds) = events
        .iter()
        .filter_map(|t| match t.event {
            Event::Farm { kills, seconds } => Some((kills, seconds)),
            _ => None,
        })
        .fold((0, 0.0), |(k, s), (kills, seconds)| (k + kills, s + seconds));
    (kills > 0).then_some(Farmed { kills, xp, seconds })
}
