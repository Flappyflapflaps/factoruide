mod audit;
mod route;
mod talents;
mod travel;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fg_route::xp::Edition;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "fg",
    about = "Factoruide planner: plan leveling routes on the app's quest databases"
)]
struct Cli {
    /// Game version: forever (default), classic, tbc. Sets the database
    /// (data/<edition>.sqlite.zst) and the overrides (overrides/<edition>/ over overrides/).
    #[arg(long, env = "FG_EDITION", default_value = "forever", global = true)]
    edition: String,
    /// Normalized database (default: the edition's, unpacked from data/<edition>.sqlite.zst)
    #[arg(long, env = "FG_DB", global = true)]
    db: Option<PathBuf>,
    /// Directory of the planned routes (JSON)
    #[arg(long, env = "FG_ROUTES", default_value = "routes", global = true)]
    routes: PathBuf,
    /// Addon directory receiving Routes.lua
    #[arg(long, env = "FG_ADDON_DIR", default_value = "addon/Factoruide", global = true)]
    addon_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Plan a leveling route and export it (routes/*.json and the addon's Routes.lua)
    Route {
        /// Race key from overrides/races.toml (gnome, human, orc...)
        #[arg(long)]
        race: String,
        /// Class (warrior, mage...)
        #[arg(long)]
        class: String,
        #[arg(long, default_value_t = 1)]
        from_level: i64,
        /// Default: the edition's level cap
        #[arg(long)]
        to_level: Option<i64>,
        /// How much the optimization tries (1 = normal, 0.1 = preview, 3 = thorough; 0 = the first
        /// routes only): the same guide on any machine, only slower with fewer processors
        #[arg(long)]
        effort: Option<f64>,
        /// Initial construction: regions (quest groups) or greedy
        #[arg(long)]
        construction: Option<String>,
        /// Route name (default: <race>-<class>)
        #[arg(long)]
        name: Option<String>,
        /// Allow quests with elite objectives
        #[arg(long)]
        allow_elite: bool,
        /// Do not run dungeons
        #[arg(long)]
        no_dungeons: bool,
        /// Battleground (PvP) quests: marks of honor and battleground objectives
        #[arg(long)]
        pvp_quests: bool,
        /// Progression profile: cautious, normal or risky (how far above its power the character goes)
        #[arg(long, value_parser = ["cautious", "normal", "risky"])]
        progression: Option<String>,
        /// Professions leveled along the route (their quests are planned, optional), as key[:target] (tailoring:300, fishing, cooking:225)
        #[arg(long, value_delimiter = ',')]
        profession: Vec<String>,
        /// Planner parameters, as key=value (repeatable)
        #[arg(long = "param")]
        params: Vec<String>,
        /// Language of the guide texts (frFR, deDE...; English where not translated)
        #[arg(long)]
        locale: Option<String>,
        /// Classes of the other players leveling together (group mode), e.g. priest,mage
        #[arg(long, value_delimiter = ',')]
        group: Vec<String>,
    },
    /// Summarize a planned route (zone changes, or every step with --full)
    Show {
        name: String,
        #[arg(long)]
        full: bool,
    },
    /// Regenerate the addon's Routes.lua from routes/*.json
    Addon,
    /// Replay a request exported by the app (debug/request-*.json)
    Replay {
        file: PathBuf,
        /// Override how much the optimization tries (see `route --effort`)
        #[arg(long)]
        effort: Option<f64>,
    },
    /// Walking distance and passages between two points (Zone,x,y)
    Travel {
        from: String,
        to: String,
        #[arg(long, default_value = "Alliance")]
        faction: String,
    },
    /// Look for each known kind of data problem in all the edition's data, for every race
    /// (accepted findings: overrides/audit.toml); fails when some are left to fix
    Audit {
        /// Checks to run (default: all): islands, source-items
        #[arg(long = "check")]
        checks: Vec<String>,
        /// Also list the accepted findings
        #[arg(long)]
        all: bool,
    },
    /// Write the leveling talent plans of the addon (TalentBuilds_<edition>.lua) from the game's talent trees
    Talents {
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// The edition's normalized database: `data/<edition>.sqlite`, unpacked from
/// `data/<edition>.sqlite.zst` when missing or older.
fn database(edition: Edition) -> Result<PathBuf> {
    let packed = PathBuf::from(format!("data/{}.sqlite.zst", edition.key()));
    let path = PathBuf::from(format!("data/{}.sqlite", edition.key()));
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if packed.exists() && (!path.exists() || modified(&packed) > modified(&path)) {
        eprintln!("unpacking {}", packed.display());
        let data = zstd::decode_all(std::fs::File::open(&packed)?)?;
        std::fs::write(&path, data)?;
    }
    anyhow::ensure!(
        path.exists(),
        "{} is missing: the quest databases are versioned in data/ (git checkout -- data/)",
        packed.display()
    );
    Ok(path)
}

static EDITION: std::sync::OnceLock<Edition> = std::sync::OnceLock::new();

/// Game version given on the command line (`--edition`).
pub fn edition() -> Edition {
    EDITION.get().copied().unwrap_or_default()
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let edition = Edition::from_key(&cli.edition)
        .with_context(|| format!("unknown edition {} (forever, classic, tbc)", cli.edition))?;
    let _ = EDITION.set(edition);
    let overrides = PathBuf::from("overrides");
    let open = || -> Result<Connection> {
        let path = match &cli.db {
            Some(p) => p.clone(),
            None => database(edition)?,
        };
        Connection::open(&path).with_context(|| format!("opening {}", path.display()))
    };

    match cli.command {
        Command::Route {
            race,
            class,
            from_level,
            to_level,
            effort,
            construction,
            name,
            profession,
            allow_elite,
            no_dungeons,
            pvp_quests,
            progression,
            params,
            locale,
            group,
        } => route::run(
            &open()?,
            &route::RouteOptions {
                race,
                class,
                from_level,
                to_level: to_level.unwrap_or(edition.rules().max_level),
                effort,
                construction,
                out_name: name,
                allow_elite,
                no_dungeons,
                pvp_quests,
                progression,
                params,
                locale,
                group,
                professions: profession
                    .iter()
                    .map(|p| {
                        let (key, target) = p.split_once(':').unwrap_or((p, "300"));
                        fg_route::profession::ProfessionGoal {
                            key: key.to_owned(),
                            target: target.parse().unwrap_or(300),
                            start_level: None,
                            milestones: vec![],
                        }
                    })
                    .collect(),
                overrides,
                routes_dir: cli.routes.clone(),
                addon_dir: cli.addon_dir.clone(),
            },
        )?,
        Command::Show { name, full } => route::show(&cli.routes, &name, full)?,
        Command::Addon => route::write_addon_routes(&open()?, &cli.routes, &cli.addon_dir)?,
        Command::Replay { file, effort } => route::replay(&open()?, &overrides, &file, effort, &cli.routes)?,
        Command::Travel { from, to, faction } => travel::run(&open()?, &overrides, &faction, &from, &to)?,
        Command::Audit { checks, all } => audit::run(&open()?, &overrides, &checks, all)?,
        Command::Talents { out } => talents::run(
            &open()?,
            &out.unwrap_or_else(|| PathBuf::from(format!("addon/Factoruide/TalentBuilds_{}.lua", edition.key()))),
            edition,
        )?,
    }
    Ok(())
}
