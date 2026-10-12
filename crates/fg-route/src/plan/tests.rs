//! Small routes built on the Classic database: the incremental evaluation of a changed route
//! gives the cost of its full replay, objectives done partly on the way.

use super::{Event, Kind, Planner, Stop};
use crate::job::{self, Overrides, PlanRequest, Prepared};
use crate::model::{Guard, Initial, Objective, Spots};
use crate::world::Pos;
use crate::xp::Edition;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The Classic database, unpacked next to the packed one when missing or older.
fn database() -> Connection {
    static UNPACK: Mutex<()> = Mutex::new(());
    let _guard = UNPACK.lock().unwrap();
    let (path, packed) = (
        repo().join("data/classic.sqlite"),
        repo().join("data/classic.sqlite.zst"),
    );
    let modified = |p: &PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    if !path.exists() || modified(&packed) > modified(&path) {
        let data = zstd::decode_all(std::fs::File::open(&packed).unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();
    }
    Connection::open(path).unwrap()
}

/// Level 1 human warrior with only `n` quests of its own: taken and turned in at the start NPC,
/// nothing to do, no XP (each test sets what it checks). Nothing done on the way.
fn quests_at_start(n: usize) -> Prepared {
    let overrides = Overrides::load_edition(&repo().join("overrides"), Edition::Classic).unwrap();
    let request = PlanRequest {
        race: "human".into(),
        class: "warrior".into(),
        from_level: 1,
        to_level: 2,
        name: None,
        params: Some(overrides.params.clone()),
        required_class_quests: None,
        professions: vec![],
        locale: None,
        group: vec![],
        start: None,
    };
    let mut p = job::prepare(&database(), &overrides, &request, &|_| {}).unwrap();
    let here = p.model.start.clone();
    let template = p.model.quests[0].clone();
    p.model.quests = (0..n)
        .map(|i| {
            let mut q = template.clone();
            (q.id, q.name, q.level, q.min_level, q.xp, q.xp_observed) =
                (900_000 + i as i64, format!("quest {i}"), 1, 1, 0, true);
            (q.starts, q.ends) = (vec![here.clone()], vec![here.clone()]);
            q.objectives.clear();
            (q.pre_all, q.pre_any, q.exclusive, q.breadcrumb_for) = (vec![], vec![], vec![], None);
            (q.start_kills, q.start_uses, q.mandatory, q.skill, q.power) = (0.0, 0.0, false, None, None);
            (q.start_guard, q.end_guard) = (Guard::default(), Guard::default());
            q
        })
        .collect();
    p.model.index = p.model.quests.iter().enumerate().map(|(i, q)| (q.id, i)).collect();
    (p.model.chain_top, p.model.unlocks, p.model.rewards) = (vec![1; n], vec![0.0; n], vec![(vec![], vec![]); n]);
    p.model.initial = Initial::default();
    p.model.power = None;
    p.model.trainers.clear();
    p.model.explore.clear();
    p.model.explore_by_zone.clear();
    (
        p.params.camp_radius,
        p.params.along_corridor,
        p.params.farm_on_way,
        p.params.flight_learn_radius,
    ) = (0.0, 0.0, false, 0.0);
    p
}

fn planner(p: &Prepared) -> Planner<'_> {
    Planner::new(&p.model, &p.world, &p.params, &p.profile)
}

/// Swapping two accepts leaves the same state after them but for a timed quest's deadline: the
/// rest of the route is replayed (the quest is turned in in time now, it expired before).
#[test]
fn timed_quest_deadline_is_part_of_the_state() {
    let mut p = quests_at_start(2);
    p.model.quests[0].time_limit = Some(400.0);
    p.model.quests[0].xp = 400;
    // Taking quest 1 takes time.
    p.model.quests[1].start_uses = 100.0;
    let planner = planner(&p);
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
        Stop::quest(0, Kind::TurnIn),
    ];
    let swapped = [route[1], route[0], route[2], route[3]];
    let run = planner.run(&route).unwrap();
    let incremental = planner.evaluate(&run, &swapped, 0, 1, true).unwrap();
    let full = planner.simulate(&swapped, None).unwrap();
    assert!(
        (incremental - full).abs() < 1e-6,
        "incremental {incremental} != full {full}"
    );
}

/// An objective done at another of its places, where a quest of the route is taken and turned
/// in at hand: removing that quest replays the objective's stop, whose place is the one really
/// visited (not the objective's nearest place).
#[test]
fn quests_at_hand_are_looked_for_where_the_stop_went() {
    let mut p = quests_at_start(2);
    p.params.camp_radius = 10.0;
    p.world.terrain = crate::terrain::Terrain::default();
    let (mut near, mut far) = (p.model.start.clone(), p.model.start.clone());
    near.pos.x += 100.0;
    far.pos.x += 300.0;
    // The nearest place takes 20 minutes more than the other one.
    let mut objective = Objective {
        loc: near,
        text: "Gather object".into(),
        kills: 0.0,
        mob_level: 0,
        uses: 1.0,
        extra: 1200.0,
        dungeon: None,
        elite: false,
        count: 1.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 1,
        pack: 1.0,
        spots: None,
        guard: Guard::default(),
        alts: vec![],
    };
    let mut other = objective.clone();
    (other.loc, other.extra) = (far.clone(), 0.0);
    objective.alts = vec![other];
    p.model.quests[0].objectives = vec![objective];
    (p.model.quests[1].starts, p.model.quests[1].ends) = (vec![far.clone()], vec![far]);
    p.model.quests[1].xp = 100;
    let planner = planner(&p);
    let route = [
        Stop::quest(0, Kind::Accept),
        Stop::quest(0, Kind::Objective(0)),
        Stop::quest(1, Kind::Accept),
        Stop::quest(1, Kind::TurnIn),
        Stop::quest(0, Kind::TurnIn),
    ];
    let without = [route[0], route[1], route[4]];
    let run = planner.run(&route).unwrap();
    let incremental = planner.evaluate(&run, &without, 2, route.len(), false).unwrap();
    let full = planner.simulate(&without, None).unwrap();
    assert!(
        (incremental - full).abs() < 1e-6,
        "incremental {incremental} != full {full}"
    );
}

/// An objective whose places need different kills (drop rates): what was killed on the way at
/// one counts as its share of the objective at the other. 10 items: 2 kills where it takes 20
/// (50 %) leave 90 % of the objective, 9 kills where it takes 10.
#[test]
fn progress_on_the_way_is_a_share_of_the_objective() {
    let mut p = quests_at_start(1);
    p.profile.to_level = 10;
    p.params.along_corridor = 10.0;
    p.world.terrain = crate::terrain::Terrain::default();
    let mut loc = p.model.start.clone();
    loc.pos.x += 100.0;
    let (x, y) = (loc.pos.x, loc.pos.y);
    let mut objective = Objective {
        loc: loc.clone(),
        text: "Gather 10 items".into(),
        kills: 20.0,
        mob_level: 1,
        uses: 0.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 10.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 20,
        pack: 1.0,
        spots: Some(Spots {
            continent: loc.pos.continent,
            min: (x - 20.0, y),
            max: (x + 20.0, y),
            points: vec![(x - 20.0, y), (x + 20.0, y)],
        }),
        guard: Guard::default(),
        alts: vec![],
    };
    let mut other = objective.clone();
    (other.loc.pos.x, other.kills, other.spots) = (x + 200.0, 10.0, None);
    objective.alts = vec![other];
    p.model.quests[0].objectives = vec![objective];
    p.model.initial.accepted = vec![0];
    let planner = planner(&p);
    let mut s = planner.initial_state();
    // A walk past both spawns of the first place.
    let start = s.pos;
    planner.kill_along(
        &mut s,
        &start,
        &Pos {
            x: start.x + 140.0,
            ..start
        },
        &mut |_, _| {},
    );
    assert!((s.along[0].2 - 0.1).abs() < 1e-9, "share done {}", s.along[0].2);
    // At the other place.
    let xp = s.spent.mob_xp;
    s.pos.x += 300.0;
    planner.apply(&mut s, Stop::quest(0, Kind::Objective(0)), None);
    let each = planner.combat().mob_xp(1, &p.model.quests[0].objectives[0].alts[0]);
    let kills = (s.spent.mob_xp - xp) as f64 / each;
    assert!((kills - 9.0).abs() < 1e-6, "{kills} kills at the other place");
    assert_eq!(s.objectives[0], 1);
}

/// Objects met on the way at a place guarded by mobs far above the character are left there, as
/// on their own stop: the place's guards count, not those of the objective's main place (safe,
/// far away).
#[test]
fn objects_met_on_the_way_keep_their_place_guards() {
    let mut p = quests_at_start(2);
    p.params.along_corridor = 10.0;
    p.world.terrain = crate::terrain::Terrain::default();
    let at = |dx: f64| {
        let mut l = p.model.start.clone();
        l.pos.x += dx;
        l
    };
    let (safe, guarded, giver) = (at(10_000.0), at(50.0), at(100.0));
    let mut objective = Objective {
        loc: safe.clone(),
        text: "Gather one object".into(),
        kills: 0.0,
        mob_level: 0,
        uses: 1.0,
        extra: 0.0,
        dungeon: None,
        elite: false,
        count: 1.0,
        mobs: vec![],
        pull: 1.0,
        spawns: 2,
        pack: 1.0,
        spots: Some(Spots {
            continent: safe.pos.continent,
            min: (guarded.pos.x, safe.pos.y),
            max: (safe.pos.x, safe.pos.y),
            points: vec![(safe.pos.x, safe.pos.y), (guarded.pos.x, guarded.pos.y)],
        }),
        guard: Guard::default(),
        alts: vec![],
    };
    let mut other = objective.clone();
    other.loc = guarded;
    other.guard = Guard {
        level: 25,
        count: 3,
        elite: false,
    };
    objective.alts = vec![other];
    p.model.quests[0].objectives = vec![objective];
    // On the way to the giver of quest 1, past the guarded place.
    p.model.quests[1].starts = vec![giver];
    p.model.initial.accepted = vec![0];
    let planner = planner(&p);
    let mut trace = Vec::new();
    planner
        .simulate(&[Stop::quest(1, Kind::Accept)], Some(&mut trace))
        .unwrap();
    assert!(
        !trace.iter().any(|t| matches!(t.event, Event::Along { quest: 0, .. })),
        "objects taken at the guarded place"
    );
}
