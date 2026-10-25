//! `fg polestar-import`: Polestar's leveling guides (github.com/fitzforge/polestar, GPL-3.0) for
//! a race and class, replayed under our time model and compared with a guide of ours; kept as a
//! guide of ours with `--save`.

use anyhow::{Context, Result};
use fg_route::Route;
use fg_route::import::polestar::{self, Character};
use fg_route::job::{self, Editor, Overrides, PlanRequest};
use fg_route::plan::fmt_time;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct ImportOptions {
    pub guides: PathBuf,
    pub race: String,
    pub class: String,
    pub from_level: i64,
    pub to_level: i64,
    pub against: Option<String>,
    pub save: Option<String>,
    pub params: Vec<String>,
    pub overrides: PathBuf,
    pub routes_dir: PathBuf,
    pub addon_dir: PathBuf,
}

/// Credit kept with a saved guide.
const CREDIT: &str =
    "Quest order from Polestar's leveling guides (github.com/fitzforge/polestar, GPL-3.0), replayed by Factoruide";

pub fn run(conn: &Connection, opts: &ImportOptions) -> Result<()> {
    let overrides = Overrides::load_edition(&opts.overrides, crate::edition())?;
    let params = overrides.params.with_overrides(&opts.params)?;
    let request = PlanRequest {
        race: opts.race.clone(),
        class: opts.class.clone(),
        from_level: opts.from_level,
        to_level: opts.to_level,
        name: Some(
            opts.save
                .clone()
                .unwrap_or_else(|| format!("polestar-{}-{}", opts.race, opts.class)),
        ),
        params: Some(params),
        required_class_quests: None,
        professions: vec![],
        locale: None,
        start: None,
        mates: vec![],
    };
    eprintln!("loading the world and the quests");
    let editor = Editor::open(conn, &overrides, &request, None)?;
    let who = Character::of(editor.profile()).context("a race or class Polestar has no name for")?;
    let (steps, guides) = polestar::import(&opts.guides, &who, opts.from_level, opts.to_level)?;
    eprintln!("replaying {} steps of {} guides", steps.len(), guides.len());
    let report = editor.follow(&steps);

    println!(
        "Polestar for a {} {} ({}), levels {} to {}, under Factoruide's time model:",
        who.race, who.class, who.faction, opts.from_level, opts.to_level
    );
    println!("  {}", guides.join(" > "));
    for (name, time, level, _) in &report.sections {
        let label = if name == "end" { "end of its steps" } else { name };
        println!("  {:>8}  level {level:>2}  {label}", fmt_time(*time));
    }
    let b = &report.breakdown;
    println!(
        "  total {} ({} quests): travel {} | flights {} | fighting {} | grinding {} | overhead {}",
        report.total_time_text,
        report.quests,
        fmt_time(b.travel),
        fmt_time(b.flights),
        fmt_time(b.fighting),
        fmt_time(b.grinding),
        fmt_time(b.overhead)
    );
    if !report.dropped.is_empty() {
        let mut why: Vec<(&str, usize)> = Vec::new();
        for (_, reason) in &report.dropped {
            match why.iter_mut().find(|(r, _)| r == reason) {
                Some((_, n)) => *n += 1,
                None => why.push((reason, 1)),
            }
        }
        why.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let text: Vec<String> = why.iter().map(|(r, n)| format!("{n} {r}")).collect();
        println!("  {} steps not replayed: {}", report.dropped.len(), text.join(", "));
        // Each once, with how many of its steps.
        let mut each: Vec<(&str, &str, usize)> = Vec::new();
        for (what, reason) in &report.dropped {
            match each.iter_mut().find(|(w, r, _)| w == what && r == reason) {
                Some((_, _, n)) => *n += 1,
                None => each.push((what, reason, 1)),
            }
        }
        for (what, reason, n) in each.iter().take(20) {
            let times = if *n > 1 { format!(" ({n} steps)") } else { String::new() };
            println!("    {what}{times}: {reason}");
        }
        if each.len() > 20 {
            println!("    and {} more", each.len() - 20);
        }
    }

    // Ours: the route named, else <race>-<class> when there.
    let ours = opts
        .against
        .clone()
        .unwrap_or_else(|| format!("{}-{}", opts.race, opts.class));
    let path = opts.routes_dir.join(format!("{}.json", job::slug(&ours)));
    if let Ok(text) = std::fs::read_to_string(&path) {
        let route: Route = fg_route::export::read_route(serde_json::from_str(&text)?)?;
        println!("Compared with {} ({}):", route.name, route.total_time_text);
        println!("  {:>5}  {:>8}  {:>8}  {:>8}", "level", "Polestar", "ours", "ahead");
        let top = opts.to_level.min(route.to_level);
        let mut level = opts.from_level + 5 - opts.from_level % 5;
        while level <= top {
            let theirs = report.levels.get(level as usize).copied().flatten();
            let mine = route.steps.iter().find(|s| s.level >= level).map(|s| s.time);
            if let (Some(t), Some(m)) = (theirs, mine) {
                let ahead = if m <= t {
                    format!("ours {}", fmt_time(t - m))
                } else {
                    format!("Polestar {}", fmt_time(m - t))
                };
                println!("  {level:>5}  {:>8}  {:>8}  {ahead}", fmt_time(t), fmt_time(m));
            }
            level += 5;
        }
    } else if opts.against.is_some() {
        eprintln!("no route {} to compare with", path.display());
    }

    if opts.save.is_some() {
        let outcome = editor.save_import(&report.steps, vec![CREDIT.to_owned()])?;
        job::save_route(&opts.routes_dir, &outcome.route)?;
        eprintln!(
            "saved {} ({})",
            opts.routes_dir
                .join(format!("{}.json", job::slug(&outcome.route.name)))
                .display(),
            outcome.route.total_time_text
        );
        crate::route::write_addon_routes(conn, &opts.overrides, &opts.routes_dir, &opts.addon_dir)?;
    }
    Ok(())
}
