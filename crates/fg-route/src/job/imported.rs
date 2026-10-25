//! A guide written for another addon (`crate::import`), as quest steps by game IDs: replayed as
//! the optimizer replays an imposed route (`Planner::follow`: its quests in their order, the
//! travel, training and grinding between them as the optimizer chooses them), timed, and saved
//! as a guide of ours to follow or edit.

use super::edit::{RouteStep, route_steps};
use super::{Editor, PlanOutcome};
use crate::model::Profile;
use crate::plan::{Breakdown, Kind, Planner, Stop, Want, fmt_time};
use anyhow::Result;
use serde::Serialize;

/// A step of another addon's guide, by game IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportStep {
    /// The start of one of its guides (its name).
    Section(String),
    Accept(i64),
    /// A quest's objective, from 1 in the game's order (its quest log's).
    Objective {
        quest: i64,
        slot: u8,
    },
    /// Every objective of a quest.
    Complete(i64),
    TurnIn(i64),
    /// Grind to this level when below.
    Level(i64),
}

/// An imported guide as replayed.
#[derive(Debug, Clone, Serialize)]
pub struct ReplayReport {
    /// Real play time to the target level, the grinding after the guide's steps included.
    pub total_time: f64,
    pub total_time_text: String,
    pub breakdown: Breakdown,
    /// Each of its guides: name, time, level and XP into it at its start; the last, "end": where
    /// its steps leave the character.
    pub sections: Vec<(String, f64, i64, i64)>,
    /// Play time on reaching each level (index: the level; none before the start).
    pub levels: Vec<Option<f64>>,
    pub quests: usize,
    /// Steps not done: what, and why (a quest our data does not have for the character, or not
    /// possible when the guide gets there).
    pub dropped: Vec<(String, &'static str)>,
    /// The replayed route, each thing done a step: what the guide editor and `Editor::save` take.
    #[serde(skip)]
    pub steps: Vec<RouteStep>,
}

const END: &str = "end";

impl Editor {
    /// An imported guide's replayed steps (`ReplayReport::steps`) saved as a guide of ours, with
    /// `notes` (its credits): replayed as imposed (gray quests and the steps our data would refuse
    /// a level early kept).
    pub fn save_import(&self, steps: &[RouteStep], notes: Vec<String>) -> Result<PlanOutcome> {
        let p = &self.prepared;
        let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile)
            .replaying()
            .explicit();
        self.save_by(&planner, steps, notes)
    }

    /// The character the guide is for.
    pub fn profile(&self) -> &Profile {
        &self.prepared.profile
    }

    /// Replay another addon's guide for the character.
    pub fn follow(&self, steps: &[ImportStep]) -> ReplayReport {
        let p = &self.prepared;
        let names = &p.model.names;
        let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile);
        let model = &p.model;
        let quest_name = |id: i64| match model.index.get(&id) {
            Some(&i) => names.get("quest", id, &model.quests[i].name).to_owned(),
            None => format!("quest {id}"),
        };
        let mut dropped = Vec::new();
        let mut wants = Vec::new();
        for step in steps {
            let quest = |id: i64| model.index.get(&id).map(|&i| i as u32);
            let stop = |index: u32, kind: Kind| Want::Stop(Stop { index, kind });
            match *step {
                ImportStep::Section(ref name) => wants.push(Want::Section(name.clone())),
                ImportStep::Level(level) => wants.push(Want::Grind(level)),
                ImportStep::Accept(id) | ImportStep::TurnIn(id) | ImportStep::Complete(id) => {
                    let Some(i) = quest(id) else {
                        dropped.push((quest_name(id), "quest not in our data for this character"));
                        continue;
                    };
                    match *step {
                        ImportStep::Accept(_) => wants.push(stop(i, Kind::Accept)),
                        ImportStep::TurnIn(_) => wants.push(stop(i, Kind::TurnIn)),
                        _ => {
                            for k in 0..model.quests[i as usize].objectives.len() {
                                wants.push(stop(i, Kind::Objective(k as u8)));
                            }
                        }
                    }
                }
                ImportStep::Objective { quest: id, slot } => {
                    let Some(i) = quest(id) else {
                        dropped.push((quest_name(id), "quest not in our data for this character"));
                        continue;
                    };
                    let objectives = &model.quests[i as usize].objectives;
                    // Its place among ours, by its slot in the game's order (else the slot's rank).
                    let k = objectives
                        .iter()
                        .position(|o| o.slot == Some(slot))
                        .or_else(|| (usize::from(slot) <= objectives.len()).then(|| usize::from(slot) - 1));
                    match k {
                        Some(k) => wants.push(stop(i, Kind::Objective(k as u8))),
                        None => dropped.push((format!("{} ({slot})", quest_name(id)), "objective not in our data")),
                    }
                }
            }
        }
        wants.push(Want::Section(END.into()));
        let followed = planner.follow(&wants);
        for (stop, why) in &followed.dropped {
            let q = &model.quests[stop.index as usize];
            let what = match stop.kind {
                Kind::Accept => "accept",
                Kind::TurnIn => "turn in",
                Kind::Objective(_) => "objective",
                _ => "step",
            };
            dropped.push((format!("{} ({what})", names.get("quest", q.id, &q.name)), why));
        }
        let mut levels = vec![None; (p.profile.to_level + 1).max(1) as usize];
        for t in &followed.trace {
            if let Some(slot) = levels.get_mut(t.level as usize)
                && slot.is_none()
            {
                *slot = Some(t.time);
            }
        }
        if let Some(slot) = levels.get_mut(p.profile.from_level as usize) {
            *slot = Some(0.0);
        }
        let quests = (followed.route.iter()).filter(|s| s.kind == Kind::TurnIn).count();
        let total = followed.breakdown.total;
        ReplayReport {
            total_time: total.round(),
            total_time_text: fmt_time(total),
            breakdown: followed.breakdown,
            sections: followed.sections,
            levels,
            quests,
            dropped,
            steps: route_steps(&planner.replaying(), &followed.route),
        }
    }
}
