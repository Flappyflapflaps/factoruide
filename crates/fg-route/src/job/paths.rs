//! The walks of a guide for the addon: where each walk from one step to the next turns, so that
//! the compass shows which way to go around the steep ground.

use super::Overrides;
use crate::export::{Route, Step, StepKind};
use crate::faction::Faction;
use crate::world::{Pos, World};
use anyhow::Result;
use rusqlite::Connection;

/// Walks shorter than this (yards) are not worked out: the step is in sight.
const SHORT: f64 = 40.0;

/// The world as the planner walks it for a faction: the terrain, the areas left only by boat and
/// the passages between zones.
pub fn walk_world(conn: &Connection, overrides: &Overrides, faction: Faction) -> Result<World> {
    let params = &overrides.params;
    let mut world = World::load(conn, faction, params.flight_speed)?;
    world.enclose(&overrides.enclosed);
    let passes: Vec<_> = (overrides.passes.iter())
        .filter(|p| params.shortcuts || !p.shortcut)
        .cloned()
        .collect();
    world.add_passes(conn, &passes)?;
    Ok(world)
}

/// Fill each step's `path`: the start and turns of the walk to it from where the step before
/// leaves the character (a flight's landing, the inn of a hearthstone), when it does not go
/// straight. None after a boat, a teleport or a step without a place, for the steps done along
/// the way and inside dungeons.
pub fn add_walk_paths(world: &World, route: &mut Route) {
    let mut from: Option<Pos> = None;
    for step in &mut route.steps {
        step.path.clear();
        if step.bg || step.inside {
            continue;
        }
        let here = position(world, step);
        if let (Some(a), Some(b)) = (&from, &here)
            && a.continent == b.continent
            && a.dist(b) >= SHORT
        {
            let walk = world.walk_path(a, b, u8::try_from(step.level.clamp(0, 255)).unwrap_or(0));
            if walk.len() > 2 {
                step.path = walk[..walk.len() - 1]
                    .iter()
                    .map(|p| [p.x.round(), p.y.round()])
                    .collect();
            }
        }
        from = match step.kind {
            // Where the flight lands.
            StepKind::Fly => step.target.as_ref().and_then(|t| {
                let node = world.taxi_nodes.iter().find(|n| n.id == t.id)?;
                Some(Pos {
                    zone: world.zone_at(&node.pos).unwrap_or(0),
                    ..node.pos
                })
            }),
            StepKind::Link | StepKind::Teleport => None,
            _ => here.or(from),
        };
    }
}

/// Where a step is: its world position, else (guides made before they had it) its map's.
fn position(world: &World, step: &Step) -> Option<Pos> {
    let p = if let Some([continent, x, y]) = step.world {
        Pos {
            continent: continent as i64,
            x,
            y,
            zone: step.zone_id.unwrap_or(0),
        }
    } else {
        let map = step.map.as_ref()?;
        let zone = world.zone_of_ui_map(map.ui_map)?;
        world.to_world(zone, map.x, map.y)?
    };
    Some(Pos {
        zone: if p.zone == 0 {
            world.zone_at(&p).unwrap_or(0)
        } else {
            p.zone
        },
        ..p
    })
}
