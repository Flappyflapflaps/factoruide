//! Running a planning job, from the request to the route: shared by the `fg` CLI and the desktop
//! app.

mod character;
mod edit;
mod notes;
mod options;
mod overrides;
mod race;
mod request;
mod routes;
mod start;

pub use edit::{
    CatalogDungeon, CatalogPlace, CatalogQuest, EditCatalog, EditReport, Editor, Line, QuestRef, RouteStep, StepReport,
};
pub use options::{NamedId, Options, RaceOption, ZoneOption, options, race_classes};
pub use overrides::{Overrides, RaceDef};
pub use race::{optimize, optimize_interruptible};
pub use request::{PlanOutcome, PlanRequest, Prepared};
pub use routes::{load_routes, render_routes, save_route, slug, write_addon_config, write_addon_routes};
pub use start::{Bind, LogObjective, LogQuest, MapPos, Skill, StartState};

use crate::export;
use crate::model::{self, Profile};
use crate::params::Params;
use crate::plan::Planner;
use crate::terrain::Walks;
use crate::world::World;
use anyhow::{Result, bail};
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// What the user asks a running job, from another thread (the app's buttons).
#[derive(Debug, Default)]
pub struct Interrupt {
    stop: AtomicBool,
    cancel: AtomicBool,
}

impl Interrupt {
    /// End the optimization now: the job goes on with the best route found so far.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Give the job up: it fails with `Cancelled`.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Ready for a new job.
    pub fn reset(&self) {
        self.stop.store(false, Ordering::Relaxed);
        self.cancel.store(false, Ordering::Relaxed);
    }

    /// The optimization must end (stopped or cancelled).
    pub(crate) fn stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || self.cancelled()
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// The error of a planning job the user cancelled (`Interrupt::cancel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// Plan a route. `progress` receives status lines (it may be called from several threads).
pub fn plan(
    conn: &Connection,
    overrides: &Overrides,
    req: &PlanRequest,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<PlanOutcome> {
    plan_interruptible(conn, overrides, req, progress, &Interrupt::default(), None)
}

/// `plan`, which `interrupt` can end early from another thread. Stopped, the optimization ends
/// at once and the route is the best found so far (the first routes are built if there are
/// none yet). Cancelled, it fails with `Cancelled` once the step under way ends (loading the
/// data is not cut). With `walks`, the walking memory kept between jobs, the paths it already
/// knows are not searched again, and the outcome holds those the route needed (to save with it).
pub fn plan_interruptible(
    conn: &Connection,
    overrides: &Overrides,
    req: &PlanRequest,
    progress: &(dyn Fn(&str) + Sync),
    interrupt: &Interrupt,
    walks: Option<&Arc<Walks>>,
) -> Result<PlanOutcome> {
    let mut prepared = prepare(conn, overrides, req, progress)?;
    if let Some(walks) = walks {
        prepared.world.terrain.share(Arc::clone(walks));
    }
    if interrupt.cancelled() {
        return Err(Cancelled.into());
    }
    let planner = Planner::new(&prepared.model, &prepared.world, &prepared.params, &prepared.profile);
    let best = optimize_interruptible(&planner, progress, interrupt);
    if interrupt.cancelled() {
        return Err(Cancelled.into());
    }
    // Objectives at the same place one after the other, done at once.
    let best = planner.route_together(best);
    let route = export::build(&planner, &prepared.profile, &best);
    for missing in planner.missing_class_quests(&best) {
        prepared.notes.push(format!("class quest not planned: {missing}"));
    }
    if !route.total_time.is_finite() {
        let why = planner
            .diagnose(&best)
            .map(|(n, w)| format!("stop {n}: {w}"))
            .unwrap_or_default();
        bail!("no valid route found ({why})");
    }
    progress(&format!(
        "{}: level {} -> {} in {} with {} quests",
        route.name, route.from_level, route.to_level, route.total_time_text, route.quests
    ));
    Ok(PlanOutcome {
        plan: edit::route_steps(&planner, &best),
        route,
        notes: prepared.notes,
        walks: walks.map(|_| prepared.world.terrain.memory()),
    })
}

/// Everything a route is planned from: the character, the settings, the world and the quests.
pub fn prepare(
    conn: &Connection,
    overrides: &Overrides,
    req: &PlanRequest,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<Prepared> {
    let (profile, mut params) = character::character(conn, overrides, req)?;
    progress("Loading the world");
    progress(r#"@progress {"phase":"prepare","step":"world"}"#);
    let world = load_world(conn, overrides, &profile, &params, progress)?;
    progress("Loading quests");
    progress(r#"@progress {"phase":"prepare","step":"quests"}"#);
    let mut model = model::load(conn, &world, &profile, &params)?;
    let mut start_notes = Vec::new();
    if let Some(start) = &req.start {
        model.initial = start.initial(&model, &world, &mut params, &mut start_notes);
        // Quests of the log the model does not have: named from the database, for their abandon.
        for (id, name) in model.initial.log.iter_mut().filter(|(_, name)| name.is_empty()) {
            *name = conn
                .query_row("SELECT name FROM m_quest WHERE id = ?1", [*id], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .ok()
                .flatten()
                .unwrap_or_else(|| format!("Quest {id}"));
        }
        progress(&format!(
            "Starting from {} (level {}): {} quests done, {} in the log",
            start.name,
            start.level,
            model.initial.turned.len(),
            model.initial.log.len()
        ));
    }
    let mut notes = notes::model_notes(&model, &world, progress);
    notes.extend(start_notes);
    Ok(Prepared {
        profile,
        params,
        world,
        model,
        notes,
    })
}

/// Zones and flights of the character's faction, with the walking passages between zones and
/// the boats, zeppelins and trams.
fn load_world(
    conn: &Connection,
    overrides: &Overrides,
    profile: &Profile,
    params: &Params,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<World> {
    let mut world = World::load(conn, profile.faction, params.flight_speed)?;
    let passes: Vec<_> = overrides
        .passes
        .iter()
        .filter(|p| params.shortcuts || !p.shortcut)
        .cloned()
        .collect();
    for warning in world.add_passes(conn, &passes)? {
        progress(&warning);
    }
    world.add_links(
        &overrides.links,
        profile.faction,
        params.run_speed / params.detour,
        params.link_wait,
    );
    Ok(world)
}
