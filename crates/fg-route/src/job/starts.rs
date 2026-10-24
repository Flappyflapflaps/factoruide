//! The starts of a guide to choose from: for a character who can begin in several places (a
//! hearthstone, flight paths, a zone rich in quests next door), a few routes built and lightly
//! improved, each beginning in another zone, to show before the guide is computed.

use super::{Interrupt, Overrides, PlanRequest, optimize_interruptible, prepare};
use crate::export::{self, StepKind};
use crate::plan::Planner;
use crate::terrain::Walks;
use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;
use std::sync::Arc;

/// How much a start is improved (`Params::effort`): enough to rank it fairly, not to finish it.
const PREVIEW_EFFORT: f64 = 0.06;
/// Starts shown at most, the planner's own included.
const MAX_STARTS: usize = 8;
/// A start whose score is this much above the best one is not worth showing.
const WORTH_SHOWING: f64 = 1.25;

/// A way to begin the guide.
#[derive(Debug, Clone, Serialize)]
pub struct StartOption {
    /// The zone it begins in (AreaTable ID), and its map (for its picture).
    pub zone: i64,
    pub ui_map: i64,
    /// What to set as `Params::start_zone` to get it (0: the planner's own choice).
    pub start_zone: i64,
    /// Play time of the guide it leads to as it was improved here (seconds), and its score (the
    /// time the planner ranks guides by, which weighs flights, grinding and zone changes).
    pub time: f64,
    pub score: f64,
    /// Steps and quests of that guide.
    pub steps: usize,
    pub quests: usize,
    /// How it begins: its first steps.
    pub first: Vec<String>,
}

/// The starts offered, and how many were tried (those that begin in the same zone count once).
#[derive(Debug, Clone, Serialize)]
pub struct StartPreview {
    pub tried: usize,
    pub options: Vec<StartOption>,
    /// What was tried, for whoever wonders why so few starts: the flight paths the character
    /// has, and for each start the zone forced, where it really begins, its time and score.
    pub details: Vec<String>,
}

/// The starts the character can choose from, the best score first; fewer than two when there
/// is nothing to choose (a guide with zones imposed, or no other zone worth starting in).
/// `progress` gets `@preview <done> <total>` lines.
pub fn preview_starts(
    conn: &Connection,
    overrides: &Overrides,
    req: &PlanRequest,
    progress: &(dyn Fn(&str) + Sync),
    interrupt: &Interrupt,
    walks: Option<&Arc<Walks>>,
) -> Result<StartPreview> {
    let mut prepared = prepare(conn, overrides, req, &|_| {})?;
    if let Some(walks) = walks {
        prepared.world.terrain.share(Arc::clone(walks));
    }
    let params = &prepared.params;
    if interrupt.cancelled() {
        return Err(super::Cancelled.into());
    }
    if !params.preferred_zones.is_empty() || !params.forced_zones.is_empty() || params.start_zone != 0 {
        return Ok(StartPreview { tried: 0, options: Vec::new(), details: Vec::new() });
    }
    let give_up = || interrupt.cancelled();
    let base = Planner::new(&prepared.model, &prepared.world, params, &prepared.profile).abortable(&give_up);
    let mut starts = vec![0];
    starts.extend(base.start_zones().into_iter().take(MAX_STARTS - 1));
    if starts.len() < 2 {
        return Ok(StartPreview { tried: 0, options: Vec::new(), details: Vec::new() });
    }
    let mut found: Vec<StartOption> = Vec::new();
    let mut details = vec![format!(
        "{} flight paths known, {} starts to try",
        prepared.model.initial.known.count_ones(),
        starts.len()
    )];
    for (i, &start_zone) in starts.iter().enumerate() {
        progress(&format!("@preview {i} {}", starts.len()));
        let mut p = params.clone();
        p.start_zone = start_zone;
        p.effort = PREVIEW_EFFORT;
        p.construction_random = false;
        let planner = Planner::new(&prepared.model, &prepared.world, &p, &prepared.profile).abortable(&give_up);
        let best = optimize_interruptible(&planner, &|_| {}, interrupt);
        if interrupt.cancelled() {
            return Err(super::Cancelled.into());
        }
        let best = planner.route_together(best);
        let route = export::build(&planner, &prepared.profile, &best);
        let Some((score, _)) = base.simulate_full(&best, None) else {
            continue;
        };
        // Where the guide begins its quests: the zone of the first step that is not a flight, a
        // training or an abandoned quest.
        let Some(first) = route.steps.iter().find(|s| {
            s.zone_id.is_some()
                && matches!(
                    s.kind,
                    StepKind::Accept | StepKind::TurnIn | StepKind::Objective | StepKind::Grind
                )
        }) else {
            continue;
        };
        let (Some(zone), Some(map)) = (first.zone_id, first.map.as_ref()) else {
            continue;
        };
        let line = format!(
            "start in zone {start_zone}: begins in zone {zone}, {:.1} h played, score {score:.0}",
            route.total_time / 3600.0
        );
        progress(&line);
        details.push(line);
        found.push(StartOption {
            zone,
            ui_map: map.ui_map,
            start_zone,
            time: route.total_time,
            score,
            steps: route.steps.len(),
            quests: route.quests,
            first: (route.steps.iter())
                .filter(|s| s.kind != StepKind::Abandon)
                .take(4)
                .map(|s| s.text.clone())
                .collect(),
        });
    }
    progress(&format!("@preview {} {}", starts.len(), starts.len()));
    // One per zone (two starts that begin in the same zone are the same start), the best first,
    // those far behind the best left out.
    let tried = found.len();
    found.sort_by(|a, b| a.score.total_cmp(&b.score));
    let mut out: Vec<StartOption> = Vec::new();
    for option in found {
        if out.iter().any(|o| o.zone == option.zone) {
            continue;
        }
        if out.first().is_some_and(|best| option.score > best.score * WORTH_SHOWING) {
            continue;
        }
        out.push(option);
    }
    Ok(StartPreview { tried, options: out, details })
}
