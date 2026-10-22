//! `fg walks`: the walks of a guide from step to step as the planner walks them (the danger of
//! the mobs counted, the passages between zones), to compare the walks of two versions.

use anyhow::{Context, Result};
use fg_route::export::Route;
use fg_route::job::{Overrides, route_walks};
use fg_route::world::World;
use rusqlite::Connection;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Writes a line a walk (tab-separated): the step, the continent, the start (x, y, zone), the
/// end, the level, the straight line and the walk (yards), then its points (`x,y` by `;`; with
/// `cells`, every cell walked over, `x,y,f` on a floor `f` other than the cell's first).
pub fn run(conn: &Connection, overrides_dir: &Path, route: &Path, out: &Path, cells: bool) -> Result<()> {
    let overrides = Overrides::load_edition(overrides_dir, crate::edition())?;
    let params = &overrides.params;
    let route: Route = serde_json::from_str(
        &std::fs::read_to_string(route).with_context(|| format!("reading {}", route.display()))?,
    )?;
    let mut world = World::load(conn, route.faction, params.flight_speed)?;
    if params.danger_weight > 0.0 {
        world.add_danger(conn, params.danger_weight)?;
    }
    world.close(&overrides.closed);
    world.enclose(&overrides.enclosed);
    world.add_caves(&overrides.caves);
    world.raise(&overrides.raised);
    let passes: Vec<_> = (overrides.passes.iter())
        .filter(|p| params.shortcuts || !p.shortcut)
        .cloned()
        .collect();
    world.add_passes(conn, &passes)?;
    let walks = route_walks(&world, &route);
    let next = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(1, usize::from).min(8);
    let mut lines: Vec<(usize, String)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut lines = Vec::new();
                    while let Some(&(k, a, b, level)) = walks.get(next.fetch_add(1, Ordering::Relaxed)) {
                        let d = world.walk_distance_at(&a, &b, level);
                        let mut path = world.walk_path(&a, &b, level);
                        // Every cell walked over, between the turns (floors other than the
                        // cells' first: as `x,y,f`).
                        let mut floors: Vec<u8> = vec![0; path.len()];
                        if cells && path.len() >= 2 {
                            let (mut detailed, mut over) = (vec![path[0]], vec![0u8]);
                            for w in path.windows(2) {
                                let leg = world.terrain.cell_path(&w[0], &w[1], level);
                                if leg.len() >= 2 {
                                    for &(p, f) in &leg[1..leg.len() - 1] {
                                        detailed.push(p);
                                        over.push(f);
                                    }
                                }
                                detailed.push(w[1]);
                                over.push(0);
                            }
                            (path, floors) = (detailed, over);
                        }
                        let mut line = format!(
                            "{k}\t{}\t{:.1}\t{:.1}\t{}\t{:.1}\t{:.1}\t{}\t{level}\t{:.1}\t{:.1}\t",
                            a.continent,
                            a.x,
                            a.y,
                            a.zone,
                            b.x,
                            b.y,
                            b.zone,
                            a.dist(&b),
                            d
                        );
                        for (i, (p, f)) in path.iter().zip(&floors).enumerate() {
                            let _ = write!(line, "{}{:.1},{:.1}", if i == 0 { "" } else { ";" }, p.x, p.y);
                            if *f != 0 {
                                let _ = write!(line, ",{f}");
                            }
                        }
                        lines.push((k, line));
                    }
                    lines
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap()).collect()
    });
    lines.sort_by_key(|l| l.0);
    let text: String = lines.into_iter().map(|(_, l)| l + "\n").collect();
    std::fs::write(out, text)?;
    Ok(())
}
