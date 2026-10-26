//! `fg ready`: the ready-made guides of the published Factoruide_Ready addon. A guide for every
//! race and class of the edition, planned as the app's "New guide" page plans one (its settings,
//! the class quests it lists, no professions, no character), so that a player follows one with
//! no app at all; then the addon's pack of this edition (`job::ready_pack_lua`).

use anyhow::Result;
use fg_route::job::{self, Overrides, PlanRequest};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub struct ReadyOptions {
    pub effort: Option<f64>,
    /// Only these, as race-class keys (human-warrior); empty: all.
    pub only: Vec<String>,
    /// Pack the guides of `routes_dir` as they are, planning none.
    pub pack_only: bool,
    /// Planner parameter overrides (key=value).
    pub params: Vec<String>,
    pub overrides: PathBuf,
    pub routes_dir: PathBuf,
    pub out: PathBuf,
}

/// The name of the ready-made guide of a race and class (its file in `routes_dir` is its slug).
fn guide_name(race: &str, class: &str, to_level: i64) -> String {
    format!("Ready-made {race} {class} 1-{to_level}")
}

pub fn run(conn: &Connection, opts: &ReadyOptions) -> Result<()> {
    let edition = crate::edition();
    let overrides = Overrides::load_edition(&opts.overrides, edition)?;
    let mut params = overrides.params.with_overrides(&opts.params)?;
    if let Some(e) = opts.effort {
        params.effort = e;
    }
    let options = job::options(conn, &overrides)?;
    let to_level = edition.rules().max_level;
    let mut names = Vec::new();
    for race in &options.races {
        for key in &race.classes {
            let Some(class) = options.classes.iter().find(|c| &c.key == key) else {
                continue;
            };
            if !opts.only.is_empty() && !opts.only.contains(&format!("{}-{}", race.key, class.key)) {
                continue;
            }
            let name = guide_name(&race.name, &class.name, to_level);
            names.push(job::slug(&name));
            let path = opts.routes_dir.join(format!("{}.json", job::slug(&name)));
            if opts.pack_only || path.exists() {
                continue;
            }
            eprintln!("planning {name}");
            let began = std::time::Instant::now();
            let request = PlanRequest {
                race: race.key.clone().into(),
                class: class.key.clone().into(),
                from_level: 1,
                to_level,
                name: Some(name.clone()),
                params: Some(params.clone()),
                required_class_quests: Some(
                    options
                        .class_quests
                        .get(&class.name.to_lowercase())
                        .cloned()
                        .unwrap_or_default(),
                ),
                professions: vec![],
                locale: Some("enUS".into()),
                mates: vec![],
                start: None,
            };
            let outcome = job::plan(conn, &overrides, &request, &|m| {
                if !m.starts_with('@') && !m.starts_with("  ") {
                    eprintln!("  {m}");
                }
            })?;
            let route = &outcome.route;
            job::save_route(&opts.routes_dir, route)?;
            eprintln!(
                "  {}: {} quests, {} ({:.0} s)",
                route.name,
                route.quests,
                route.total_time_text,
                began.elapsed().as_secs_f64()
            );
        }
    }
    let routes: Vec<_> = job::load_routes(&opts.routes_dir)?
        .into_iter()
        .filter(|r| names.contains(&job::slug(&r.name)))
        .collect();
    let n = routes.len();
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let lua = job::ready_pack_lua(conn, &overrides, edition, &routes, time)?;
    let (interface, version) = guide_toc(Path::new("addon"));
    job::write_ready_addon(&opts.out, edition, &lua, &interface, &version)?;
    eprintln!(
        "wrote {} ({n} guides, {} KB)",
        opts.out.join(format!("Ready_{}.lua", edition.key())).display(),
        lua.len() / 1024
    );
    Ok(())
}

/// The "## Interface:" line of the guide addon of this checkout (`addon_root`, the folder of the
/// addons) and its version: the pack is made for that addon.
fn guide_toc(addon_root: &Path) -> (String, String) {
    let toc = std::fs::read_to_string(addon_root.join("Factoruide_Guide/Factoruide_Guide.toc")).unwrap_or_default();
    let interface = toc
        .lines()
        .find(|l| l.starts_with("## Interface:"))
        .map_or_else(|| "## Interface: 11508".to_owned(), str::to_owned);
    let version = toc
        .lines()
        .find_map(|l| l.strip_prefix("## Version:"))
        .map_or_else(|| env!("CARGO_PKG_VERSION").to_owned(), |v| v.trim().to_owned());
    (interface, version)
}
