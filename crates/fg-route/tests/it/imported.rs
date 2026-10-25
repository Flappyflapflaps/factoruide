//! A guide written for Polestar, replayed on the Forever database under our time model and
//! saved as a guide of ours.

use crate::common::{database_of, repo};
use fg_route::import::polestar::{self, Character};
use fg_route::job::{Editor, ImportStep, Overrides, PlanRequest};
use fg_route::xp::Edition;

const NORTHSHIRE: &str = r"guide test/northshire
title Northshire Valley
faction Alliance
race Human
levels 1-3
zone Elwynn Forest

step Deputy Willem
  accept 783
  turnin 783
  accept 7
  accept 5261 only Warrior   -- not a mage's

step Kobold camp
  kill 6 x10 quest 7.1

step Back to the abbey
  turnin 7
  turnin 99999999
  level 3
";

fn editor() -> Editor {
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Forever).unwrap();
    let request = PlanRequest {
        race: "human".into(),
        class: "mage".into(),
        from_level: 1,
        to_level: 3,
        name: Some("polestar-test".into()),
        params: None,
        required_class_quests: None,
        professions: vec![],
        locale: None,
        mates: vec![],
        start: None,
    };
    Editor::open(&database_of("forever"), &overrides, &request, None).unwrap()
}

/// The guide's quests replay in its order, a quest our data lacks is reported, and the guide
/// saved as ours keeps the replay's time.
#[test]
fn polestar_guide_replays_and_saves() {
    let editor = editor();
    let who = Character::of(editor.profile()).unwrap();
    assert_eq!((who.race.as_str(), who.faction.as_str()), ("Human", "Alliance"));
    let guide = polestar::parse(NORTHSHIRE).unwrap();
    let steps = polestar::steps(&[&guide], &who);
    assert!(!steps.contains(&ImportStep::Accept(5261)), "not a warrior's step");
    let report = editor.follow(&steps);
    assert_eq!(report.quests, 2, "two quests handed in: {:?}", report.dropped);
    assert_eq!(report.dropped.len(), 1, "{:?}", report.dropped);
    assert_eq!(report.dropped[0].0, "quest 99999999");
    assert_eq!(report.sections.len(), 2, "the guide, then the end");
    assert!(report.levels[3].is_some_and(|t| t > 0.0), "level 3 reached");
    let saved = editor.save_import(&report.steps, vec!["credit".into()]).unwrap();
    let (ours, theirs) = (saved.route.total_time, report.total_time);
    assert!(
        (ours - theirs).abs() <= theirs * 0.05,
        "saved {ours} s, replayed {theirs} s"
    );
    assert_eq!(saved.notes, ["credit"]);
}
