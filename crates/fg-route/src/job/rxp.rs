//! Guides written for RestedXP (the guide files of the RXPGuides addon, or the text of a guide
//! imported in it) read into a guide's route: the quests taken, done and turned in, the flight
//! paths, the inn, the hearthstone and the levels to grind to, in the guide's order. The steps
//! the guide gives this race and class are kept; the editor's replay then keeps those that hold
//! for the character in the database (`Editor::import`).
//!
//! RestedXP's guides are under CC BY-NC-SA 4.0 (RestedXP): nothing of them is shipped here, the
//! player's own copy is read.

use super::edit::{Editor, RouteStep};
use super::{Overrides, PlanOutcome, PlanRequest};
use crate::plan::{Edit, Outcome, Planner};
use crate::terrain::Walks;
use crate::world::Pos;
use crate::xp::Edition;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The license of RestedXP's guides, said with each guide made from them.
pub const LICENSE: &str = "RestedXP, CC BY-NC-SA 4.0: https://github.com/RestedXP/RXPGuides";

/// The guide files the RXPGuides addon (its folder) loads for a game version, in its order
/// (`Guides/GuideList-<game>.xml`); its folders read whole when that list is missing.
pub fn guide_files(addon: &Path, edition: Edition) -> Vec<PathBuf> {
    let list = addon.join(format!("Guides/GuideList-{}.xml", edition.key()));
    let Ok(xml) = std::fs::read_to_string(&list) else {
        return lua_files(&addon.join("Guides").join(edition.key()));
    };
    (xml.split("<Script file=\"").skip(1))
        .filter_map(|rest| rest.split('"').next())
        .filter(|f| !f.ends_with("Import.lua"))
        .filter_map(|f| find_path(addon, f))
        .collect()
}

/// The .lua files of a folder, by name.
pub fn lua_files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = (std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "lua"))
        .collect();
    found.sort();
    found
}

/// A path of the addon's lists, whatever the case of its folders (`Guides/forever/x.lua` may be
/// in `Guides/Forever` next to a `Guides/forever`: the game does not mind).
fn find_path(root: &Path, relative: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = relative.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    let Some((part, rest)) = parts.split_first() else {
        return Some(root.to_path_buf());
    };
    (std::fs::read_dir(root).ok()?.flatten())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(part))
        })
        .find_map(|p| {
            if rest.is_empty() {
                Some(p)
            } else {
                find_path(&p, &rest.join("/"))
            }
        })
}

/// What to make a guide of: the character, and the first RestedXP guide (default: its guide
/// from level 1).
#[derive(Debug, Clone)]
pub struct Import {
    pub race: String,
    pub class: String,
    pub first: Option<String>,
    /// The guide's name (default: "RestedXP <levels> <race> <class>").
    pub name: Option<String>,
    pub locale: Option<String>,
}

/// A guide made from RestedXP guides.
pub struct Imported {
    pub request: PlanRequest,
    pub outcome: PlanOutcome,
    /// The RestedXP guides followed, by name.
    pub guides: Vec<String>,
    pub report: ImportReport,
}

/// A guide of the editor from RestedXP guides (`texts`: their files): from the character's first
/// one, the guides its `#next` lead to, their steps for this race and class, the trips between
/// them as the optimizer makes them, and the steps not possible with the database left out.
pub fn import(
    conn: &Connection,
    overrides: &Overrides,
    texts: &[String],
    what: &Import,
    walks: Option<&Arc<Walks>>,
) -> Result<Imported> {
    let params = overrides.params.clone();
    let options = super::options(conn, overrides)?;
    let class_name = (options.classes.iter())
        .find(|c| c.key == what.class)
        .map_or_else(|| what.class.to_lowercase(), |c| c.name.to_lowercase());
    let race = (overrides.races.get(&what.race)).with_context(|| format!("unknown race {}", what.race))?;
    let reader = Reader::new(&what.race, &what.class, race.faction, params.edition);
    let guides: Vec<RxpGuide> = texts.iter().flat_map(|t| reader.parse(t)).collect();
    let first = match &what.first {
        Some(name) => name.clone(),
        None => first_guide(&reader, &guides)
            .and_then(|g| g.name.clone())
            .context("no RestedXP guide from level 1 for this race and class")?,
    };
    let chain = chain(&guides, &first);
    anyhow::ensure!(
        !chain.is_empty(),
        "no RestedXP guide named {first} for this race and class"
    );
    let names: Vec<String> = chain.iter().filter_map(|g| g.name.clone()).collect();
    let from_level = chain[0].levels().map_or(1, |l| l.0);
    let to_level = (chain.iter().filter_map(|g| g.levels()).map(|l| l.1).max())
        .unwrap_or(from_level + 1)
        .min(params.edition.rules().max_level);
    let name = what.name.clone().unwrap_or_else(|| {
        let race = options
            .races
            .iter()
            .find(|r| r.key == what.race)
            .map_or(what.race.as_str(), |r| r.name.as_str());
        let class = options
            .classes
            .iter()
            .find(|c| c.key == what.class)
            .map_or(what.class.as_str(), |c| c.name.as_str());
        format!("RestedXP {from_level}-{to_level} {race} {class}")
    });
    let request = PlanRequest {
        race: what.race.clone(),
        class: what.class.clone(),
        from_level,
        to_level,
        name: Some(name),
        params: Some(params),
        required_class_quests: Some(options.class_quests.get(&class_name).cloned().unwrap_or_default()),
        professions: vec![],
        locale: what.locale.clone(),
        mates: vec![],
        start: None,
    };
    let editor = Editor::open(conn, overrides, &request, walks)?;
    let mut report = ImportReport::default();
    let actions: Vec<Action> = chain.iter().flat_map(|g| g.actions.iter().cloned()).collect();
    let steps = editor.rxp_steps(&actions, &mut report);
    let steps = editor.import(&steps, &mut report);
    let notes = vec![format!(
        "From the RestedXP guides {} ({LICENSE}), replayed with this database: {} of their steps left out.",
        names.join(", "),
        report.dropped.len() + report.unknown_quests.len()
    )];
    let outcome = editor.save(&steps, notes)?;
    Ok(Imported {
        request,
        outcome,
        guides: names,
        report,
    })
}

/// Who a guide is read for: RestedXP's own tokens (`NightElf`, `Undead`, `WARRIOR`, `FOREVER`).
#[derive(Debug, Clone)]
pub struct Reader {
    pub race: String,
    pub class: String,
    pub faction: String,
    pub game: String,
}

impl Reader {
    /// The reader for a race key of `races.toml` (night_elf, skyborne_horde...), a class
    /// (warrior, Mage...), a faction and an edition.
    pub fn new(race: &str, class: &str, faction: crate::faction::Faction, edition: Edition) -> Self {
        let race = race.trim_end_matches("_horde").trim_end_matches("_alliance");
        let race = race
            .split('_')
            .map(|w| {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect::<String>())
                    .unwrap_or_default()
            })
            .collect::<String>();
        let game = match edition {
            Edition::Forever => "FOREVER",
            Edition::Classic => "CLASSIC",
            Edition::Tbc => "TBC",
        };
        Self {
            race,
            class: class.to_uppercase(),
            faction: faction.name().to_owned(),
            game: game.to_owned(),
        }
    }

    /// Whether a condition (`<< Alliance/!Shaman`, `<< Horde Shaman`) holds: alternatives split
    /// by `/`, each a list of words all true, `!` a word that must not be, `( )` groups.
    pub fn applies(&self, tags: &str) -> bool {
        // A comment after the tags (`<< Rogue -- Quickblade's Dagger`) is not a tag.
        let tags = tags.split_once("--").map_or(tags, |(tags, _)| tags);
        let mut text = tags.to_owned();
        // The innermost groups first, each replaced by a word true or false.
        while let Some(open) = text.rfind('(') {
            let Some(len) = text[open..].find(')') else { break };
            let inner = text[open + 1..open + len].to_owned();
            let not = text[..open].ends_with('!');
            let start = if not { open - 1 } else { open };
            let word = if self.applies(&inner) == not { "FALSE" } else { "TRUE" };
            text.replace_range(start..=open + len, word);
        }
        text.split('/').any(|alt| {
            alt.split(|c: char| !(c.is_alphanumeric() || c == '!'))
                .filter(|w| !w.is_empty() && *w != "!")
                .all(|word| {
                    let (not, word) = match word.strip_prefix('!') {
                        Some(w) => (true, w),
                        None => (false, word),
                    };
                    self.word(word) != not
                })
        })
    }

    fn word(&self, word: &str) -> bool {
        let upper = word.to_uppercase();
        let race = match word {
            "Scourge" => "Undead",
            "Haranir" => "Harronir",
            w => w,
        };
        // A level ("<< 10"): not known while reading, the replay keeps what holds.
        word.parse::<i64>().is_ok()
            || word == "TRUE"
            || upper == self.class
            || (upper == "DK" && self.class == "DEATHKNIGHT")
            || race.eq_ignore_ascii_case(&self.race)
            || word == self.faction
            || upper == self.game
    }

    /// The guides of a guide file (`RXPGuides.RegisterGuide([[...]])` each), or of a guide's
    /// text alone.
    pub fn parse(&self, text: &str) -> Vec<RxpGuide> {
        const OPEN: &str = "RegisterGuide(";
        let mut bodies = Vec::new();
        let mut rest = text;
        while let Some(at) = rest.find(OPEN) {
            rest = &rest[at + OPEN.len()..];
            let Some(start) = rest.find("[[") else { break };
            let body = &rest[start + 2..];
            let end = body.find("]]").unwrap_or(body.len());
            bodies.push(&body[..end]);
            rest = &body[end..];
        }
        if bodies.is_empty() && text.contains("\nstep") {
            bodies.push(text);
        }
        bodies.into_iter().filter_map(|b| self.guide(b)).collect()
    }

    /// One guide's text: its header (`#name`, `#next`...) then its steps.
    fn guide(&self, body: &str) -> Option<RxpGuide> {
        let mut guide = RxpGuide::default();
        let mut step: Option<Step> = None;
        let mut steps = Vec::new();
        let mut skipping = false;
        for raw in body.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with("--") {
                continue;
            }
            if line == "step" || line.starts_with("step ") || line.starts_with("step<<") {
                steps.extend(step.take());
                skipping = (line.split_once("<<")).is_some_and(|(_, tags)| !self.applies(tags.trim()));
                if !skipping {
                    step = Some(Step::default());
                }
                continue;
            }
            if skipping {
                continue;
            }
            // The rest of a line after `<<` says for whom: the line is left out for the others.
            let code = match line.split_once("<<") {
                Some((code, tags)) => {
                    if !self.applies(tags.trim()) {
                        if code.trim().is_empty() && step.is_none() {
                            guide.available = false;
                        }
                        continue;
                    }
                    code.trim()
                }
                None => line,
            };
            match &mut step {
                None => guide.header(code),
                Some(step) => step.line(code),
            }
        }
        steps.extend(step);
        guide.name.as_ref()?;
        let mut actions = Vec::new();
        for step in steps.into_iter().filter(Step::applies) {
            actions.extend(step.actions.into_iter().map(|a| match a {
                Action::Complete { quest, slot, .. } => Action::Complete {
                    quest,
                    slot,
                    partial: step.partial,
                },
                Action::FlightPath { name, at: None } => Action::FlightPath { name, at: step.at },
                Action::Bind { npc, name, at: None } => Action::Bind {
                    npc: npc.or(step.target.as_ref().and_then(|t| t.1)),
                    name: name.or(step.target.as_ref().map(|t| t.0.clone())),
                    at: step.at,
                },
                a => a,
            }));
        }
        guide.actions = actions;
        Some(guide)
    }
}

/// A guide of the file: its name, the guides it leads to, and what it does.
#[derive(Debug, Clone)]
pub struct RxpGuide {
    pub name: Option<String>,
    /// The guides that follow, the first one found taken (`#next a;b`, with their group
    /// `Group\name` left out).
    pub next: Vec<String>,
    /// Races and classes it is the default guide of (`#defaultfor`).
    pub default_for: Option<String>,
    /// False when the guide is for others (a header line `<< Horde` alone).
    pub available: bool,
    pub actions: Vec<Action>,
}

impl Default for RxpGuide {
    fn default() -> Self {
        Self {
            name: None,
            next: Vec::new(),
            default_for: None,
            available: true,
            actions: Vec::new(),
        }
    }
}

impl RxpGuide {
    fn header(&mut self, line: &str) {
        let Some(line) = line.strip_prefix('#') else { return };
        let (tag, value) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let value = value.trim();
        // The first line of a tag that holds for the character is its value.
        match tag {
            "name" if self.name.is_none() => self.name = Some(value.to_owned()),
            "next" if self.next.is_empty() => {
                self.next = value
                    .split(';')
                    .map(|n| n.rsplit('\\').next().unwrap_or(n).trim().to_owned())
                    .filter(|n| !n.is_empty())
                    .collect();
            }
            "defaultfor" if self.default_for.is_none() => self.default_for = Some(value.to_owned()),
            _ => {}
        }
    }

    /// The levels in its name ("6-11 Elwynn Forest": 6 and 11).
    pub fn levels(&self) -> Option<(i64, i64)> {
        let name = self.name.as_deref()?;
        let range = name.split_whitespace().next()?;
        let (from, to) = range.split_once('-')?;
        Some((from.parse().ok()?, to.parse().ok()?))
    }
}

/// What a guide's step does that a route keeps (its `.collect` lines name a quest but not the
/// objective: the objectives the guide does not name are done before the turn-in).
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Accept(i64),
    TurnIn(i64),
    /// An objective by its number in the quest (from 1); `partial`: done in part while going to
    /// a later step (`#completewith`), finished where the guide says so later.
    Complete {
        quest: i64,
        slot: u8,
        partial: bool,
    },
    /// Learn a flight path: its name in the guide and where it stands when the guide says.
    FlightPath {
        name: String,
        at: Option<Pos>,
    },
    /// Bind the hearthstone: the innkeeper targeted (name, ID when given) and where.
    Bind {
        npc: Option<i64>,
        name: Option<String>,
        at: Option<Pos>,
    },
    Hearth,
    GrindTo(i64),
    Abandon(i64),
}

/// A step being read.
#[derive(Debug, Default)]
struct Step {
    actions: Vec<Action>,
    partial: bool,
    /// Left out: for hardcore realms, for a season, for an XP rate other than the normal one.
    skip: bool,
    at: Option<Pos>,
    target: Option<(String, Option<i64>)>,
}

impl Step {
    fn applies(&self) -> bool {
        !self.skip
    }

    fn line(&mut self, line: &str) {
        if let Some(tag) = line.strip_prefix('#') {
            let (tag, value) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
            match tag {
                "completewith" => self.partial = true,
                "hardcore" | "hardcoreserver" | "som" | "ssf" => self.skip = true,
                "season" => self.skip |= !value.split([',', ';', ' ']).any(|s| s.trim() == "0"),
                "xprate" => self.skip |= !xp_rate_one(value.trim()),
                _ => {}
            }
            return;
        }
        let Some(cmd) = line.strip_prefix('.') else { return };
        // The text shown (`>>`) and comments (`--`) are not the command's.
        let cmd = cmd.split(">>").next().unwrap_or(cmd);
        let cmd = cmd.split(" --").next().unwrap_or(cmd).trim();
        let (name, args) = cmd.split_once(char::is_whitespace).unwrap_or((cmd, ""));
        let args = args.trim();
        let ids: Vec<&str> = args.split(',').map(str::trim).collect();
        let id = |i: usize| ids.get(i).and_then(|s| s.parse::<i64>().ok());
        let action = match name {
            "accept" => id(0).map(Action::Accept),
            "turnin" => id(0).map(Action::TurnIn),
            "complete" => id(0).zip(id(1)).map(|(quest, slot)| Action::Complete {
                quest,
                slot: slot as u8,
                partial: false,
            }),
            "fp" => Some(Action::FlightPath {
                name: args.to_owned(),
                at: None,
            }),
            "home" => Some(Action::Bind {
                npc: None,
                name: None,
                at: None,
            }),
            "hs" => Some(Action::Hearth),
            // `.xp 12`, `.xp 2+480`: grind to that level; `.xp <12` is a condition.
            "xp" if !args.starts_with(['<', '>']) => {
                let level: String = args.chars().take_while(char::is_ascii_digit).collect();
                level.parse().ok().map(Action::GrindTo)
            }
            "abandon" => id(0).map(Action::Abandon),
            "goto" => {
                self.at = self.at.or_else(|| world_pos(&ids));
                None
            }
            "target" if self.target.is_none() => {
                let first = args.split(',').next().unwrap_or(args);
                let (name, npc) = match first.split_once("::") {
                    Some((name, id)) => (name, id.trim().parse().ok()),
                    None => (first, None),
                };
                self.target = Some((name.trim().to_owned(), npc));
                None
            }
            _ => None,
        };
        self.actions.extend(action);
    }
}

/// Whether a step's `#xprate` (`<1.5`, `>1.49`, `1.5-2`) holds at the normal XP rate.
fn xp_rate_one(value: &str) -> bool {
    let num = |s: &str| s.trim().parse::<f64>().ok();
    if let Some(v) = value.strip_prefix('<') {
        return num(v).is_none_or(|v| 1.0 < v);
    }
    if let Some(v) = value.strip_prefix('>') {
        return num(v).is_none_or(|v| 1.0 > v);
    }
    match value.split_once('-') {
        Some((a, b)) => num(a).is_none_or(|a| a <= 1.0) && num(b).is_none_or(|b| 1.0 <= b),
        None => num(value).is_none_or(|a| a <= 1.0),
    }
}

/// A `.goto` in world coordinates (`1453/0,484.2,-8832.5`: map/continent, then y and x).
fn world_pos(args: &[&str]) -> Option<Pos> {
    let (_, continent) = args.first()?.split_once('/')?;
    Some(Pos {
        continent: continent.parse().ok()?,
        x: args.get(2)?.parse().ok()?,
        y: args.get(1)?.parse().ok()?,
        zone: 0,
        z: None,
    })
}

/// The guides to follow from a first one, by their `#next`: the first of each list found among
/// the guides available.
pub fn chain<'a>(guides: &'a [RxpGuide], first: &str) -> Vec<&'a RxpGuide> {
    let find = |name: &str| {
        guides
            .iter()
            .find(|g| g.available && g.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name)))
    };
    let mut out: Vec<&RxpGuide> = Vec::new();
    let mut next = find(first);
    while let Some(guide) = next {
        if out.iter().any(|g| std::ptr::eq(*g, guide)) {
            break;
        }
        out.push(guide);
        next = guide.next.iter().find_map(|n| find(n));
    }
    out
}

/// The first guide of the character: available, the default for it, from level 1.
pub fn first_guide<'a>(reader: &Reader, guides: &'a [RxpGuide]) -> Option<&'a RxpGuide> {
    guides.iter().find(|g| {
        g.available
            && g.levels().is_some_and(|(from, _)| from == 1)
            && g.default_for.as_deref().is_some_and(|d| reader.applies(d))
    })
}

/// What the import kept and left out.
#[derive(Debug, Default)]
pub struct ImportReport {
    /// The guide's actions read.
    pub read: usize,
    /// Quests the character's data does not have (other version, other race or class).
    pub unknown_quests: Vec<i64>,
    /// Flight paths and inns not found.
    pub unknown_places: Vec<String>,
    /// Steps the replay left out, with why.
    pub dropped: Vec<(RouteStep, &'static str)>,
    /// Prerequisites the guide does not name, added before the quests needing them.
    pub added: Vec<i64>,
}

impl Editor {
    /// The route steps of a guide's actions: quests by their IDs and objectives by their number,
    /// flight paths and inns found by place or name. An objective the guide leaves to a later
    /// step (`#completewith`) is done where the guide finishes it; one the guide never names is
    /// done right before the quest's turn-in.
    pub fn rxp_steps(&self, actions: &[Action], report: &mut ImportReport) -> Vec<RouteStep> {
        let (model, world) = (&self.prepared.model, &self.prepared.world);
        report.read += actions.len();
        let objective = |quest: i64, slot: u8| {
            let q = &model.quests[*model.index.get(&quest)?];
            q.objectives.iter().position(|o| o.slot == Some(slot)).map(|i| i as u8)
        };
        // Objectives finished by a later step: the parts done before it are left to it.
        let finished: HashSet<(i64, u8)> = (actions.iter())
            .filter_map(|a| match *a {
                Action::Complete {
                    quest,
                    slot,
                    partial: false,
                } => Some((quest, slot)),
                _ => None,
            })
            .collect();
        // The quests the guide names: a prerequisite it never names is added before the quest
        // needing it (the database knows of one the guide's author did not need to write).
        let named: HashSet<i64> = (actions.iter())
            .filter_map(|a| match *a {
                Action::Accept(q) | Action::TurnIn(q) | Action::Abandon(q) | Action::Complete { quest: q, .. } => {
                    Some(q)
                }
                _ => None,
            })
            .collect();
        let mut out = Out {
            model,
            named,
            ..Out::default_for(model)
        };
        let unknown = |id: i64, report: &mut ImportReport| {
            if !report.unknown_quests.contains(&id) {
                report.unknown_quests.push(id);
            }
        };
        for action in actions {
            match *action {
                Action::Accept(quest) | Action::TurnIn(quest) | Action::Abandon(quest)
                    if !model.index.contains_key(&quest) =>
                {
                    unknown(quest, report);
                }
                Action::Accept(quest) => out.accept(quest, 0, report),
                Action::TurnIn(quest) => out.turn_in(quest),
                Action::Abandon(quest) => out.steps.push(RouteStep::Abandon { quest }),
                Action::Complete { quest, slot, partial } => {
                    if partial && finished.contains(&(quest, slot)) {
                        continue;
                    }
                    if !model.index.contains_key(&quest) {
                        unknown(quest, report);
                        continue;
                    }
                    // Items got before the quest is taken: done again before its turn-in.
                    if !out.taken.contains(&quest) {
                        continue;
                    }
                    if let Some(k) = objective(quest, slot)
                        && out.done.insert((quest, k))
                    {
                        out.steps.push(objective_step(quest, k));
                    }
                }
                Action::FlightPath { ref name, at } => {
                    let near = at.and_then(|p| world.taxi_near(&p, 150.0));
                    let named = || {
                        let name = name.to_lowercase();
                        (world.taxi_nodes.iter()).position(|n| n.name.to_lowercase().starts_with(&name))
                    };
                    match near.or_else(named) {
                        Some(n) => out.steps.push(RouteStep::FlightMaster {
                            node: world.taxi_nodes[n].id,
                        }),
                        None => report.unknown_places.push(format!("flight path {name}")),
                    }
                }
                Action::Bind { npc, ref name, at } => {
                    let inns = || model.inns.iter().enumerate().filter(|(i, _)| *i != model.start_inn);
                    let by_id = npc.and_then(|id| inns().find(|(_, l)| l.id == id));
                    let by_name = || {
                        let name = name.as_deref()?.trim_start_matches("Innkeeper ").to_lowercase();
                        inns().find(|(_, l)| l.name.to_lowercase().ends_with(&name))
                    };
                    let by_place = || {
                        let at = at?;
                        inns()
                            .filter(|(_, l)| l.pos.continent == at.continent)
                            .map(|(i, l)| (i, l, (l.pos.x - at.x).hypot(l.pos.y - at.y)))
                            .filter(|(_, _, d)| *d < 60.0)
                            .min_by(|a, b| a.2.total_cmp(&b.2))
                            .map(|(i, l, _)| (i, l))
                    };
                    match by_id.or_else(by_name).or_else(by_place) {
                        Some((_, inn)) => out.steps.push(RouteStep::Bind { inn: inn.id }),
                        None => report
                            .unknown_places
                            .push(format!("inn {}", name.as_deref().unwrap_or("?"))),
                    }
                }
                Action::Hearth => out.steps.push(RouteStep::Hearth { recall: false }),
                Action::GrindTo(level) => out.steps.push(RouteStep::GrindTo { level }),
            }
        }
        out.steps
    }

    /// The steps of a guide written elsewhere, made to hold for this character: the trips
    /// between them as the optimizer makes them (flights, boats, training, selling), then the
    /// steps that are not possible here left out until every one holds.
    pub fn import(&self, steps: &[RouteStep], report: &mut ImportReport) -> Vec<RouteStep> {
        let p = &self.prepared;
        // Replaying the guide's order: no grinding to the level the optimizer would wait for.
        let planner = Planner::new(&p.model, &p.world, &p.params, &p.profile).replaying();
        let mut edits: Vec<(RouteStep, Edit)> = Vec::new();
        for step in steps {
            match step.resolve(&planner) {
                Ok(edit) => edits.push((step.clone(), edit)),
                Err(why) => report.dropped.push((step.clone(), why)),
            }
        }
        // What the optimizer's replay cannot do is left out of its own expansion.
        let plain: Vec<Edit> = edits.iter().map(|(_, e)| *e).collect();
        let edited = planner.edit(&plain);
        for ((step, _), s) in edits.iter().zip(&edited.steps) {
            if let Outcome::Invalid(why) = s.outcome {
                report.dropped.push((step.clone(), why));
            }
        }
        let mut steps: Vec<RouteStep> = (planner.expand(&plain).into_iter())
            .map(|edit| RouteStep::of(edit, &planner))
            .collect();
        // The explicit replay: a step left out can make later ones impossible, again until none.
        let explicit = self.planner();
        for _ in 0..50 {
            let edits: Vec<Option<Edit>> = steps.iter().map(|s| s.resolve(&explicit).ok()).collect();
            let plain: Vec<Edit> = edits.iter().flatten().copied().collect();
            let edited = explicit.edit(&plain);
            let mut outcomes = edited.steps.iter();
            let mut keep = Vec::with_capacity(steps.len());
            let mut dropped = 0;
            for (step, edit) in steps.into_iter().zip(&edits) {
                let why = match edit {
                    None => Some("not in this character's data"),
                    Some(_) => match outcomes.next().map(|s| s.outcome) {
                        Some(Outcome::Invalid(why)) => Some(why),
                        _ => None,
                    },
                };
                match why {
                    Some(why) => {
                        dropped += 1;
                        report.dropped.push((step, why));
                    }
                    None => keep.push(step),
                }
            }
            steps = keep;
            if dropped == 0 {
                break;
            }
        }
        steps
    }
}

/// The steps being written from a guide's actions.
struct Out<'a> {
    model: &'a crate::model::Model,
    /// Quests the guide names anywhere.
    named: HashSet<i64>,
    steps: Vec<RouteStep>,
    /// Objectives written (quest, index), quests taken and turned in.
    done: HashSet<(i64, u8)>,
    taken: HashSet<i64>,
    turned: HashSet<i64>,
}

impl<'a> Out<'a> {
    fn default_for(model: &'a crate::model::Model) -> Self {
        Self {
            model,
            named: HashSet::new(),
            steps: Vec::new(),
            done: HashSet::new(),
            taken: HashSet::new(),
            turned: HashSet::new(),
        }
    }

    /// Take a quest, after the prerequisites the guide never names (taken, done and turned in
    /// first, theirs before them, a few deep).
    fn accept(&mut self, quest: i64, depth: u8, report: &mut ImportReport) {
        let model = self.model;
        let q = &model.quests[model.index[&quest]];
        let missing = |p: &&i64| model.index.contains_key(*p) && !self.named.contains(*p) && !self.turned.contains(*p);
        let mut add: Vec<i64> = q.pre_all.iter().filter(missing).copied().collect();
        if !q
            .pre_any
            .iter()
            .any(|p| self.turned.contains(p) || self.named.contains(p))
        {
            add.extend(q.pre_any.iter().find(missing).copied());
        }
        if depth < 4 {
            for p in add {
                report.added.push(p);
                self.accept(p, depth + 1, report);
                self.turn_in(p);
            }
        }
        self.taken.insert(quest);
        self.steps.push(RouteStep::Accept { quest });
    }

    /// Turn a quest in, its objectives not written yet done first.
    fn turn_in(&mut self, quest: i64) {
        let q = &self.model.quests[self.model.index[&quest]];
        for k in 0..q.objectives.len() as u8 {
            if self.done.insert((quest, k)) {
                self.steps.push(objective_step(quest, k));
            }
        }
        self.turned.insert(quest);
        self.steps.push(RouteStep::TurnIn { quest });
    }
}

fn objective_step(quest: i64, objective: u8) -> RouteStep {
    RouteStep::Objective {
        quest,
        objective,
        farm: false,
        spot: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faction::Faction;
    use crate::xp::Edition;

    fn human_warrior() -> Reader {
        Reader::new("human", "warrior", Faction::Alliance, Edition::Forever)
    }

    #[test]
    fn conditions() {
        let r = human_warrior();
        assert!(r.applies("Alliance"));
        assert!(!r.applies("Horde"));
        assert!(r.applies("Human Warrior"));
        assert!(!r.applies("Human !Warrior"));
        assert!(r.applies("Dwarf/Human"));
        assert!(r.applies("!NightElf !Hunter/!Dwarf !Hunter"));
        assert!(!r.applies("none Human"));
        assert!(r.applies("!(Dwarf Hunter)"));
        assert!(r.applies("Warrior -- Quickblade's Dagger"));
        assert!(r.applies("FOREVER"));
        assert!(!r.applies("SoD"));
        let undead = Reader::new("undead", "mage", Faction::Horde, Edition::Forever);
        assert!(undead.applies("Scourge"));
        assert!(Reader::new("night_elf", "druid", Faction::Alliance, Edition::Forever).applies("NightElf"));
        assert!(Reader::new("skyborne_horde", "shaman", Faction::Horde, Edition::Forever).applies("Horde Skyborne"));
    }

    const GUIDE: &str = r"
RXPGuides.RegisterGuide([[
#forever
#name 1-6 Northshire
#next 6-11 Elwynn Forest << Warrior
#next 6-11 Other
#defaultfor Human

step
    .goto 1429,48.1,42.9
    .accept 783 >>Accept A Threat Within
step << Mage
    .accept 5261
step
    #completewith next
    .complete 7,1 --Kobold Vermin slain
step
    .turnin 783 >>Turn in A Threat Within
    .accept 7 << Warrior
    .accept 9 << Mage
step
    .complete 7,1
    .complete 33,1 --Tough Wolf Meat
    .collect 769,10,2178,1,0x20,cooking
step
    #hardcore
    .accept 18
step
    #xprate <1.5
    .xp 4+300 >>Grind
    .xp <5,1
step
    .goto 1453/0,484.200,-8832.500
    .fp Stormwind >> Get the flight path
    .target Dungar Longdrink
step
    .home >> Set your hearthstone
    .target Innkeeper Farley::295
    .hs >> Hearth
]])
RXPGuides.RegisterGuide([[
#name 1-6 Durotar
<< Horde
step
    .accept 4641
]])
";

    #[test]
    fn reads_a_guide() {
        let guides = human_warrior().parse(GUIDE);
        assert_eq!(guides.len(), 2);
        let g = &guides[0];
        assert_eq!(g.name.as_deref(), Some("1-6 Northshire"));
        assert_eq!(g.next, vec!["6-11 Elwynn Forest"]);
        assert_eq!(g.levels(), Some((1, 6)));
        assert!(!guides[1].available);
        let a = &g.actions;
        assert_eq!(a[0], Action::Accept(783));
        assert_eq!(
            a[1],
            Action::Complete {
                quest: 7,
                slot: 1,
                partial: true
            }
        );
        assert_eq!(a[2], Action::TurnIn(783));
        assert_eq!(a[3], Action::Accept(7));
        assert_eq!(
            a[5],
            Action::Complete {
                quest: 33,
                slot: 1,
                partial: false
            }
        );
        assert_eq!(a[6], Action::GrindTo(4));
        let Action::FlightPath { name, at: Some(at) } = &a[7] else {
            panic!("{:?}", a[7])
        };
        assert_eq!(name, "Stormwind");
        assert!((at.x + 8832.5).abs() < 0.01 && (at.y - 484.2).abs() < 0.01 && at.continent == 0);
        assert_eq!(
            a[8],
            Action::Bind {
                npc: Some(295),
                name: Some("Innkeeper Farley".into()),
                at: None
            }
        );
        assert_eq!(a[9], Action::Hearth);
        assert_eq!(a.len(), 10);
    }

    #[test]
    fn follows_next() {
        let reader = human_warrior();
        let mut text = GUIDE.to_owned();
        text.push_str(
            "RXPGuides.RegisterGuide([[\n#name 6-11 Elwynn Forest\n#next 1-6 Northshire\nstep\n.accept 1\n]])",
        );
        let guides = reader.parse(&text);
        let first = first_guide(&reader, &guides).expect("a first guide");
        let names: Vec<_> = chain(&guides, first.name.as_deref().unwrap_or_default())
            .iter()
            .filter_map(|g| g.name.clone())
            .collect();
        assert_eq!(names, vec!["1-6 Northshire", "6-11 Elwynn Forest"]);
    }
}
