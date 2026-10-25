//! Polestar's leveling guides (github.com/fitzforge/polestar, GPL-3.0): the `.guide` sources of a
//! pack (Polestar's own: its repository's `guides/` folder), read in their guide format v1
//! (docs/GUIDE_FORMAT.md there), and the chain of guides a character follows from its start
//! (`next`, the author's default each time) as quest steps by game IDs.
//!
//! Only what our time model replays is read: quests taken, their objectives, their turn-ins, and
//! the levels the guide grinds to. Travel, training, vendors and the rest are the replay's own
//! choices (as the optimizer makes them), so the comparison is of the quest order. Steps and goals
//! for other classes, races or factions (`only`) are left out; of the steps applying only in some
//! state (`when`), those for a dungeon or a group quest the player chose are left out (Polestar
//! runs neither unless asked), the others are kept and the replay drops what is not possible.

use crate::export::polestar::{class_file, race_file};
use crate::job::ImportStep;
use crate::model::Profile;
use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A guide of a pack: its header, then its steps.
#[derive(Debug, Clone, Default)]
pub struct Guide {
    pub id: String,
    pub title: String,
    pub faction: Option<String>,
    pub races: Vec<String>,
    pub classes: Vec<String>,
    pub levels: (f64, f64),
    /// The guides that may follow (`next`), the author's default first, each with whether it
    /// holds only in some state (`when`).
    pub next: Vec<(String, bool)>,
    /// `Leveling` unless the header names another.
    pub category: String,
    pub steps: Vec<GuideStep>,
}

#[derive(Debug, Clone, Default)]
pub struct GuideStep {
    pub title: Option<String>,
    pub only: Vec<String>,
    pub when: Vec<String>,
    pub goals: Vec<Goal>,
}

/// A goal: its verb, the integer after it (a quest, NPC or item ID), the quest objective it
/// counts (`quest Q` or `quest Q.O`, O from 1 in the game's order), `level`'s target, `only`.
#[derive(Debug, Clone, Default)]
pub struct Goal {
    pub verb: String,
    pub id: Option<i64>,
    pub quest: Option<(i64, Option<u8>)>,
    pub level: Option<f64>,
    pub only: Vec<String>,
    pub direct: bool,
}

/// Who follows the guides: Polestar's names (`Human`, `NightElf`, `Mage`, `Alliance`).
#[derive(Debug, Clone)]
pub struct Character {
    pub race: String,
    pub class: String,
    pub faction: String,
}

impl Character {
    /// The character of a guide of ours, in Polestar's names.
    pub fn of(profile: &Profile) -> Option<Self> {
        Some(Self {
            race: race_file(profile.race_id)?.to_owned(),
            class: class_file(profile.class_id)?.to_owned(),
            faction: profile.faction.name().to_owned(),
        })
    }
}

/// The tokens of a line: comments and player notes cut, quoted strings whole (unquoted).
fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut in_quotes = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_quotes {
            match c {
                '\\' if i + 1 < chars.len() => {
                    cur.push(chars[i + 1]);
                    i += 1;
                }
                '"' => in_quotes = false,
                c => cur.push(c),
            }
        } else if c == '"' {
            in_quotes = true;
            quoted = true;
        } else if c.is_whitespace() {
            // A player note: ` -- ` to the end of the line.
            if chars[i..].starts_with(&[' ', '-', '-', ' ']) || chars[i..].iter().collect::<String>() == " --" {
                break;
            }
            if !cur.is_empty() || quoted {
                out.push(std::mem::take(&mut cur));
                quoted = false;
            }
        } else if c == '#' && cur.is_empty() && !quoted {
            // An author comment: first on the line or after a space.
            break;
        } else {
            cur.push(c);
        }
        i += 1;
    }
    if !cur.is_empty() || quoted {
        out.push(cur);
    }
    out
}

fn list(rest: &[String]) -> Vec<String> {
    rest.join(" ")
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn levels(text: &str) -> Option<(f64, f64)> {
    let (a, b) = text.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// A `level` goal's target: `12`, `12.5` (half way through 12) or `12+2500` (2500 XP into it,
/// kept as level 12).
fn level_target(text: &str) -> Option<f64> {
    let base = text.split('+').next()?;
    base.parse().ok()
}

/// `Q` or `Q.O`.
fn quest_ref(text: &str) -> Option<(i64, Option<u8>)> {
    match text.split_once('.') {
        Some((q, o)) => Some((q.parse().ok()?, Some(o.parse().ok()?))),
        None => Some((text.parse().ok()?, None)),
    }
}

/// One guide's source.
pub fn parse(text: &str) -> Result<Guide> {
    let mut guide = Guide {
        category: "Leveling".into(),
        ..Guide::default()
    };
    let mut step: Option<GuideStep> = None;
    for (n, raw) in text.lines().enumerate() {
        let toks = tokens(raw);
        let Some(first) = toks.first() else { continue };
        let key = first.to_ascii_lowercase();
        let indented = raw.starts_with([' ', '\t']);
        if !indented && key == "step" {
            guide.steps.extend(step.take());
            step = Some(GuideStep {
                title: (toks.len() > 1).then(|| toks[1..].join(" ")),
                ..GuideStep::default()
            });
            continue;
        }
        let Some(current) = step.as_mut() else {
            let rest = toks[1..].join(" ");
            match key.as_str() {
                "guide" => guide.id = rest,
                "title" => guide.title = rest,
                "faction" => guide.faction = Some(rest),
                "race" => guide.races = list(&toks[1..]),
                "class" => guide.classes = list(&toks[1..]),
                "levels" => {
                    guide.levels = levels(&rest).with_context(|| format!("line {}: levels {rest}", n + 1))?;
                }
                "category" => guide.category = rest,
                "next" => {
                    // `next a, b` or `next a when <predicate>`.
                    let when = toks.iter().position(|t| t.eq_ignore_ascii_case("when"));
                    let ids = list(&toks[1..when.unwrap_or(toks.len())]);
                    guide.next.extend(ids.into_iter().map(|id| (id, when.is_some())));
                }
                _ => {}
            }
            continue;
        };
        match key.as_str() {
            "only" => current.only.extend(list(&toks[1..])),
            "when" => current.when.push(toks[1..].join(" ")),
            "zone" | "along" | "label" | "skipto" | "tip" | "image" | "loop" => {}
            verb => {
                let mut goal = Goal {
                    verb: verb.to_owned(),
                    ..Goal::default()
                };
                goal.id = toks.get(1).and_then(|t| t.parse().ok());
                if verb == "level" {
                    goal.level = toks.get(1).and_then(|t| level_target(t));
                }
                let mut i = 2;
                while i < toks.len() {
                    match toks[i].to_ascii_lowercase().as_str() {
                        "quest" => {
                            goal.quest = toks.get(i + 1).and_then(|t| quest_ref(t));
                            i += 1;
                        }
                        "only" => {
                            goal.only = toks
                                .get(i + 1)
                                .map(|t| list(std::slice::from_ref(t)))
                                .unwrap_or_default();
                            i += 1;
                        }
                        "direct" => goal.direct = true,
                        _ => {}
                    }
                    i += 1;
                }
                current.goals.push(goal);
            }
        }
    }
    guide.steps.extend(step);
    if guide.id.is_empty() {
        bail!("no `guide` line");
    }
    Ok(guide)
}

/// Every guide under `dir` (its `.guide` files, in sub-folders too), by ID.
pub fn read_dir(dir: &Path) -> Result<HashMap<String, Guide>> {
    let mut out = HashMap::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).with_context(|| format!("reading {}", folder.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|e| e == "guide") {
                let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
                let guide = parse(&text).with_context(|| format!("{}", path.display()))?;
                out.insert(guide.id.clone(), guide);
            }
        }
    }
    if out.is_empty() {
        bail!("no .guide file in {}", dir.display());
    }
    Ok(out)
}

fn same(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        let s = s.to_ascii_lowercase().replace([' ', '_'], "");
        if s == "scourge" { "undead".to_owned() } else { s }
    };
    norm(a) == norm(b)
}

const CLASSES: [&str; 9] = [
    "warrior", "paladin", "hunter", "rogue", "priest", "shaman", "mage", "warlock", "druid",
];

/// Whether conditions (`only`) hold for `who`: classes (`!` turns one around), races and factions
/// each OR together, the three AND. An unknown name is a race (new Forever races).
pub fn applies(conds: &[String], who: &Character) -> bool {
    let (mut classes, mut races, mut factions) = (Vec::new(), Vec::new(), Vec::new());
    for c in conds {
        let name = c.trim_start_matches('!');
        if CLASSES.iter().any(|k| same(k, name)) {
            classes.push(c.as_str());
        } else if same(name, "alliance") || same(name, "horde") {
            factions.push(name);
        } else {
            races.push(name);
        }
    }
    let class_ok = {
        let (neg, pos): (Vec<&str>, Vec<&str>) = classes.iter().partition(|c| c.starts_with('!'));
        neg.iter().all(|c| !same(&c[1..], &who.class)) && (pos.is_empty() || pos.iter().any(|c| same(c, &who.class)))
    };
    class_ok
        && (races.is_empty() || races.iter().any(|r| same(r, &who.race)))
        && (factions.is_empty() || factions.iter().any(|f| same(f, &who.faction)))
}

/// Whether the guide is for `who` (its faction, races and classes).
fn for_character(g: &Guide, who: &Character) -> bool {
    g.faction.as_deref().is_none_or(|f| same(f, &who.faction))
        && (g.races.is_empty() || g.races.iter().any(|r| same(r, &who.race)))
        && (g.classes.is_empty() || g.classes.iter().any(|c| same(c, &who.class)))
}

/// The leveling guides `who` follows from level `from` to `to`: the one for its race starting
/// lowest (else for its faction), then each guide's first `next` that is for it and holds in
/// any state, else its first that is for it.
pub fn chain<'a, S: std::hash::BuildHasher>(
    guides: &'a HashMap<String, Guide, S>,
    who: &Character,
    from: i64,
    to: i64,
) -> Vec<&'a Guide> {
    let leveling = |g: &&Guide| g.category.eq_ignore_ascii_case("leveling") && for_character(g, who);
    let mut starts: Vec<&Guide> = guides
        .values()
        .filter(leveling)
        .filter(|g| g.levels.1 > from as f64)
        .collect();
    // Its own race's guides first, then the lowest, then by ID (the same chain each time).
    starts.sort_by(|a, b| {
        (a.races.is_empty(), a.levels.0, &a.id)
            .partial_cmp(&(b.races.is_empty(), b.levels.0, &b.id))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let start = starts
        .iter()
        .find(|g| g.levels.0 <= from as f64 + 1.0)
        .or(starts.first());
    let mut out: Vec<&Guide> = Vec::new();
    let mut seen = HashSet::new();
    let mut at = start.copied();
    while let Some(g) = at {
        if !seen.insert(g.id.clone()) || g.levels.0 >= to as f64 {
            break;
        }
        out.push(g);
        let options: Vec<(&Guide, bool)> = g
            .next
            .iter()
            .filter_map(|(id, when)| guides.get(id).map(|n| (n, *when)))
            .filter(|(n, _)| for_character(n, who))
            .collect();
        at = options
            .iter()
            .find(|(_, when)| !when)
            .or(options.first())
            .map(|(n, _)| *n);
    }
    out
}

/// Whether a step's `when` lines may hold for a player following the guide alone: not the steps
/// of a dungeon or a group quest they would have to choose.
fn solo(when: &[String]) -> bool {
    when.iter().all(|w| {
        let w = w.trim().to_ascii_lowercase();
        !(w.starts_with("dungeon") || w.starts_with("group"))
    })
}

/// The quest steps of guides in their order, for `who`: each guide a section, then its quests
/// taken, their objectives, their turn-ins, and the levels to reach.
pub fn steps(guides: &[&Guide], who: &Character) -> Vec<ImportStep> {
    let mut out = Vec::new();
    for g in guides {
        out.push(ImportStep::Section(format!("{} ({})", g.title, g.id)));
        for step in g.steps.iter().filter(|s| applies(&s.only, who) && solo(&s.when)) {
            for goal in step.goals.iter().filter(|g| applies(&g.only, who)) {
                let quest = goal.quest;
                match (goal.verb.as_str(), goal.id) {
                    ("accept", Some(q)) => out.push(ImportStep::Accept(q)),
                    ("turnin", Some(q)) => {
                        if goal.direct {
                            out.push(ImportStep::Accept(q));
                        }
                        out.push(ImportStep::TurnIn(q));
                    }
                    ("complete", Some(q)) => out.push(ImportStep::Complete(q)),
                    ("level", _) => {
                        if let Some(level) = goal.level {
                            out.push(ImportStep::Level(level.floor() as i64));
                        }
                    }
                    _ => match quest {
                        Some((q, Some(slot))) => out.push(ImportStep::Objective { quest: q, slot }),
                        Some((q, None)) => out.push(ImportStep::Complete(q)),
                        None => {}
                    },
                }
            }
        }
    }
    out
}

/// The guides `who` follows from `from` to `to` in the pack folder `dir`, as quest steps; with
/// the guides' titles and level ranges.
pub fn import(dir: &Path, who: &Character, from: i64, to: i64) -> Result<(Vec<ImportStep>, Vec<String>)> {
    let guides = read_dir(dir)?;
    let chain = chain(&guides, who, from, to);
    if chain.is_empty() {
        bail!(
            "no Polestar leveling guide for a {} {} ({}) in {}",
            who.race,
            who.class,
            who.faction,
            dir.display()
        );
    }
    let names = chain
        .iter()
        .map(|g| format!("{} {}-{}", g.title, g.levels.0, g.levels.1))
        .collect();
    Ok((steps(&chain, who), names))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human_mage() -> Character {
        Character {
            race: "Human".into(),
            class: "Mage".into(),
            faction: "Alliance".into(),
        }
    }

    const ELWYNN: &str = r#"guide forever/alliance/human/01-elwynn
title Elwynn Forest
faction Alliance
race Human
levels 1-12
zone Elwynn Forest
next forever/alliance/02-westfall
# an author comment

step Deputy Willem  # a comment after a space
  accept 783                          -- He stands at the abbey gate
  turnin 783
  accept 7
  accept 33 only Mage,Warrior

step Kobolds
  kill 6 x10 quest 7.1
  collect 750 x8 quest 33 optional

step Warriors only
  only Warrior
  accept 9999

step Not mages
  only !Mage
  accept 9998

step Ragefire
  when dungeon "Deadmines"
  accept 9997

step Back
  when onquest 7
  turnin 7
  level 4.5
"#;

    const WESTFALL: &str = "guide forever/alliance/02-westfall\ntitle \"Westfall\"\nfaction Alliance\nlevels 12-17\nnext forever/alliance/03-darkshore\n\nstep Hill\n  accept 64\n";

    #[test]
    fn reads_a_guide() {
        let g = parse(ELWYNN).unwrap();
        assert_eq!(g.id, "forever/alliance/human/01-elwynn");
        assert_eq!(g.levels, (1.0, 12.0));
        assert_eq!(g.races, ["Human"]);
        assert_eq!(g.steps.len(), 6);
        assert_eq!(g.steps[0].title.as_deref(), Some("Deputy Willem"));
        assert_eq!(g.steps[0].goals.len(), 4);
        assert_eq!(g.steps[1].goals[0].quest, Some((7, Some(1))));
        assert_eq!(g.steps[1].goals[1].quest, Some((33, None)));
        assert_eq!(g.steps[5].goals[1].level, Some(4.5));
        assert_eq!(
            tokens(r#"  goto 42.1,65.9 "Gold \"shire\"" -- a note"#),
            ["goto", "42.1,65.9", "Gold \"shire\""]
        );
    }

    #[test]
    fn steps_for_the_character() {
        use ImportStep::*;
        let g = parse(ELWYNN).unwrap();
        let s = steps(&[&g], &human_mage());
        assert_eq!(
            s,
            [
                Section("Elwynn Forest (forever/alliance/human/01-elwynn)".into()),
                Accept(783),
                TurnIn(783),
                Accept(7),
                Accept(33),
                Objective { quest: 7, slot: 1 },
                Complete(33),
                TurnIn(7),
                Level(4),
            ]
        );
    }

    #[test]
    fn conditions() {
        let mage = human_mage();
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert!(applies(&s(&["Mage", "Warrior"]), &mage));
        assert!(!applies(&s(&["Warrior", "Dwarf"]), &mage));
        assert!(applies(&s(&["Mage", "Human"]), &mage));
        assert!(!applies(&s(&["!Mage"]), &mage));
        assert!(!applies(&s(&["Horde"]), &mage));
        let undead = Character {
            race: "Scourge".into(),
            class: "Mage".into(),
            faction: "Horde".into(),
        };
        assert!(applies(&s(&["Undead"]), &undead));
    }

    #[test]
    fn follows_next() {
        let mut guides = HashMap::new();
        for text in [ELWYNN, WESTFALL] {
            let g = parse(text).unwrap();
            guides.insert(g.id.clone(), g);
        }
        let ids: Vec<&str> = chain(&guides, &human_mage(), 1, 60)
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["forever/alliance/human/01-elwynn", "forever/alliance/02-westfall"]
        );
        let ids: Vec<&str> = chain(&guides, &human_mage(), 1, 12)
            .iter()
            .map(|g| g.id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["forever/alliance/human/01-elwynn"],
            "Westfall starts at the target level"
        );
        let dwarf = Character {
            race: "Dwarf".into(),
            ..human_mage()
        };
        let ids: Vec<&str> = chain(&guides, &dwarf, 1, 60).iter().map(|g| g.id.as_str()).collect();
        assert_eq!(ids, ["forever/alliance/02-westfall"], "not the Human start");
    }
}
