//! `fg travel`: how we walk between two points (distance, passages), to check travel costs. The
//! points: `Zone,x,y` (map percent) or `w:continent,x,y` (world yards).

use anyhow::{Context, Result};
use fg_route::job::Overrides;
use fg_route::world::World;
use rusqlite::Connection;
use std::path::Path;

fn point(world: &World, spec: &str) -> Result<fg_route::world::Pos> {
    // World coordinates: w:continent,x,y.
    if let Some(rest) = spec.strip_prefix("w:") {
        let v: Vec<f64> = rest.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "expected w:continent,x,y: {spec}");
        let mut p = fg_route::world::Pos {
            continent: v[0] as i64,
            x: v[1],
            y: v[2],
            zone: 0,
        };
        p.zone = world.zone_at(&p).unwrap_or(0);
        return Ok(p);
    }
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    anyhow::ensure!(parts.len() == 3, "expected Zone,x,y: {spec}");
    let zone = match parts[0].parse::<i64>() {
        Ok(id) => id,
        Err(_) => *world
            .zone_names
            .iter()
            .filter(|(_, n)| n.eq_ignore_ascii_case(parts[0]))
            .map(|(id, _)| id)
            .min()
            .with_context(|| format!("unknown zone {}", parts[0]))?,
    };
    world
        .to_world(zone, parts[1].parse()?, parts[2].parse()?)
        .with_context(|| format!("no map for zone {zone}"))
}

pub fn run(
    conn: &Connection,
    overrides_dir: &Path,
    faction: &str,
    from: &str,
    to: &str,
    level: Option<i64>,
    cells: Option<&Path>,
) -> Result<()> {
    let overrides = Overrides::load_edition(overrides_dir, crate::edition())?;
    let params = &overrides.params;
    let faction =
        fg_route::faction::Faction::parse(faction).ok_or_else(|| anyhow::anyhow!("unknown faction {faction}"))?;
    let mut world = World::load(conn, faction, params.flight_speed)?;
    for w in world.close(&overrides.closed) {
        eprintln!("{w}");
    }
    for w in world.enclose(&overrides.enclosed) {
        eprintln!("{w}");
    }
    for w in world.add_caves(&overrides.caves) {
        eprintln!("{w}");
    }
    world.raise(&overrides.raised);
    for w in world.add_passes(conn, &overrides.passes)? {
        eprintln!("{w}");
    }
    let (a, b) = (point(&world, from)?, point(&world, to)?);
    let (d, passes) = world.walk_route(&a, &b);
    println!(
        "straight {:.0} yd, walking {:.0} yd = {:.1} min on foot",
        a.dist(&b),
        d,
        d * params.detour / params.run_speed / 60.0
    );
    let planned = world.walk_distance(&a, &b);
    if (planned - d).abs() > 1e-6 && !(planned.is_infinite() && d.is_infinite()) {
        println!("the planner walks {planned:.0} yd (straight line where the walk is unknown)");
    }
    if let (Some(ta), Some(tb)) = (world.terrain.area_of(&a), world.terrain.area_of(&b)) {
        let how = match world.terrain.distance(&a, &b, 0) {
            None | Some(f64::INFINITY) if world.bridge(&a, &b, 0).is_some() => "areas joined by passages",
            None if world.terrain.disconnected(&a, &b) => "way unknown (straight line, longer)",
            None => "way unknown (straight line assumed)",
            Some(w) if w.is_infinite() => "an enclosed area and another (no walk)",
            Some(w) if (w - a.dist(&b)).abs() < 1e-6 => "straight line clear",
            Some(_) => "path around the steep ground",
        };
        println!(
            "terrain: area {} ({} cells) to area {} ({} cells), {how}",
            ta.0, ta.1, tb.0, tb.1
        );
    }
    if let Some((i, j)) = passes {
        let p = |k: usize| {
            format!(
                "{} ({:.0},{:.0})",
                world.passes[k].name, world.passes[k].pos.x, world.passes[k].pos.y
            )
        };
        println!(
            "leaves {} by {}, enters {} by {}",
            world.zone_name(a.zone),
            p(i),
            world.zone_name(b.zone),
            p(j)
        );
    }
    // The way the planner draws it: the passages and the turns around the steep terrain.
    let path = world.walk_path(&a, &b, 0);
    // Every cell walked over, for `fgd terrain-walks-map` (a line: name, continent, the drawn
    // way, the cells).
    if let Some(out) = cells {
        let mut detailed: Vec<String> = vec![format!("{:.1},{:.1}", a.x, a.y)];
        for w in path.windows(2) {
            for (p, f) in world.terrain.cell_path(&w[0], &w[1], 0) {
                detailed.push(if f == 0 { format!("{:.1},{:.1}", p.x, p.y) } else { format!("{:.1},{:.1},{f}", p.x, p.y) });
            }
            detailed.push(format!("{:.1},{:.1}", w[1].x, w[1].y));
        }
        let drawn: Vec<String> = path.iter().map(|p| format!("{:.1},{:.1}", p.x, p.y)).collect();
        std::fs::write(
            out,
            format!("travel\t{}\t{}\t{}\t0\t0\n", a.continent, drawn.join(";"), detailed.join(";")),
        )?;
    }
    if path.len() > 2 {
        let stops: Vec<String> = path[1..path.len() - 1]
            .iter()
            .map(|p| {
                let pass = world.passes.iter().find(|q| q.pos.dist(p) < 1.0);
                let zone = world.zone_at(p).unwrap_or(p.zone);
                let at = world.to_map(zone, p).map_or_else(
                    || format!("{:.0},{:.0}", p.x, p.y),
                    |m| format!("{} {:.1},{:.1}", world.zone_name(zone), m.x, m.y),
                );
                match pass {
                    Some(q) => format!("[{}] {at}", q.name),
                    None => at,
                }
            })
            .collect();
        println!("way: {}", stops.join(" > "));
    }
    if let Some(level) = level {
        flights(&world, params, &a, &b, level);
    }
    Ok(())
}

/// The walk at `level`'s speed against the 3 best flights with every flight path known: its real
/// time and the weighted time the planner compares (flight weight, gold).
fn flights(
    world: &World,
    params: &fg_route::params::Params,
    a: &fg_route::world::Pos,
    b: &fg_route::world::Pos,
    level: i64,
) {
    let speed = params.speed(level);
    let walk = |p: &fg_route::world::Pos, q: &fg_route::world::Pos| world.walk_distance(p, q) * params.detour / speed;
    let n = world.taxi_nodes.len();
    let all = if n >= 128 { u128::MAX } else { (1u128 << n) - 1 };
    let (time, price) = world.flights(all);
    let direct = walk(a, b);
    println!("level {level}: walking {direct:.0} s");
    let mut found: Vec<(f64, f64, usize, usize, f64)> = Vec::new();
    for i in 0..n {
        let wa = walk(a, &world.taxi_nodes[i].pos);
        if !wa.is_finite() || wa >= direct {
            continue;
        }
        for j in (0..n).filter(|&j| j != i && time[i][j].is_finite()) {
            let wb = walk(&world.taxi_nodes[j].pos, b);
            let t = wa + time[i][j] + params.flight_overhead + wb;
            let w = t
                + (params.flight_weight - 1.0) * (time[i][j] + params.flight_overhead)
                + params.gold_weight_at(level) * price[i][j] / 10_000.0;
            found.push((t, w, i, j, price[i][j]));
        }
    }
    found.sort_by(|x, y| x.1.total_cmp(&y.1));
    for &(t, w, i, j, price) in found.iter().take(3) {
        println!(
            "flight {} -> {}: {t:.0} s ({:.0} s walking to and from), {price:.0} copper, weighted {w:.0} s",
            world.taxi_nodes[i].name,
            world.taxi_nodes[j].name,
            t - time[i][j] - params.flight_overhead,
        );
    }
}

/// `fg travel --overrides`: what the walkable floors make of the hand-made terrain overrides
/// (`passes.toml`): the areas declared enclosed the floors join to the mainland, the caves they
/// hold, the passages whose sides they join.
pub fn overrides(conn: &Connection, overrides_dir: &Path, faction: &str) -> Result<()> {
    let overrides = Overrides::load_edition(overrides_dir, crate::edition())?;
    let params = &overrides.params;
    let faction =
        fg_route::faction::Faction::parse(faction).ok_or_else(|| anyhow::anyhow!("unknown faction {faction}"))?;
    let mut world = World::load(conn, faction, params.flight_speed)?;
    for w in world.close(&overrides.closed) {
        println!("{w}");
    }
    let area = |world: &World, zone: i64, x: f64, y: f64| {
        let p = world.to_world(zone, x, y)?;
        let (a, size) = world.terrain.area_of(&p)?;
        Some((p.continent, a, size, world.terrain.main_area(p.continent) == Some(a)))
    };
    for d in &overrides.enclosed {
        match area(&world, d.at.zone, d.at.x, d.at.y) {
            Some((_, _, _, true)) => println!("enclosed {}: joined to the mainland by the floors (redundant, or a way the game closes)", d.name),
            Some((_, a, size, false)) => println!("enclosed {}: its own area {a} ({size} floors): kept", d.name),
            None => println!("enclosed {}: no terrain", d.name),
        }
    }
    for d in &overrides.caves {
        let (mouth, inside) = (area(&world, d.at.zone, d.at.x, d.at.y), area(&world, d.inside.zone, d.inside.x, d.inside.y));
        match (mouth, inside) {
            (Some((_, m, _, _)), Some((_, i, size, _))) if m == i && size >= fg_route::terrain::LARGE_AREA => {
                println!("cave {}: the floors hold it, inside in the mouth's area (redundant)", d.name);
            }
            (m, i) => println!("cave {}: mouth area {:?}, inside area {:?}: kept", d.name, m.map(|x| x.1), i.map(|x| (x.1, x.2))),
        }
    }
    for w in world.enclose(&overrides.enclosed) {
        println!("{w}");
    }
    world.add_caves(&overrides.caves);
    world.raise(&overrides.raised);
    for w in world.add_passes(conn, &overrides.passes)? {
        println!("{w}");
    }
    for p in &world.passes {
        // Its sides: the areas around, and in one area the points the floors walk to only by a
        // long way around (`World::pass_areas`).
        let how = match p.areas.len() {
            0 => "no walkable area around".to_owned(),
            1 => "the floors walk across it directly (a waypoint between zones only)".to_owned(),
            n => {
                let mut areas: Vec<u32> = p.areas.iter().map(|a| a.0).collect();
                areas.sort_unstable();
                areas.dedup();
                format!("{n} sides the floors join only by a long way round, or not ({} areas): needed", areas.len())
            }
        };
        println!("pass {}: {how}", p.name);
    }
    for d in &overrides.raised {
        println!("raised {}: kept until places stand on a floor by their height", d.name);
    }
    Ok(())
}
