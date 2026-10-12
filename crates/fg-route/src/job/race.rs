//! Choosing the best route, as a tournament counted in tries (the same guide on any machine, only
//! slower with fewer processors): candidate routes (each its own seed and construction settings)
//! are built and improved briefly, and the best `threads` go on, one search each. After each round
//! the better half goes on with twice the searches each, those of a route sharing their best
//! (`share_every`), down to the `final_routes` of the final. The best route wins. The app follows
//! it live (`@progress` and `@route` lines).

use super::Interrupt;
use crate::model::{Model, Profile};
use crate::params::{Construction, Params};
use crate::plan::{Effort, Exchange, Member, Planner, Stop, fmt_time};
use crate::world::World;
use rand::{Rng, SeedableRng};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

type Progress<'p> = &'p (dyn Fn(&str) + Sync);

/// Build routes and improve them in a tournament of `params.threads` searches (see the module).
/// The best wins.
pub fn optimize(planner: &Planner<'_>, progress: Progress<'_>) -> Vec<Stop> {
    optimize_interruptible(planner, progress, &Interrupt::default())
}

/// `optimize`, ended early by `interrupt`: no new candidate starts (but the first ones, so that
/// there is a route), the searches end at their next move and no new round begins. Stopped, the
/// best route so far is returned; cancelled, the caller throws it away.
pub fn optimize_interruptible(planner: &Planner<'_>, progress: Progress<'_>, interrupt: &Interrupt) -> Vec<Stop> {
    let plan = Tournament::new(planner.params);
    progress(&plan.announce());
    let live = Live::new(&plan, planner.params, progress);
    let searcher = Searcher {
        model: planner.model,
        world: planner.world,
        params: planner.params,
        profile: planner.profile,
        live: &live,
        progress,
        interrupt,
    };
    let ranked = std::thread::scope(|scope| {
        scope.spawn(|| live.watch(&searcher));
        // The watcher ends with the searches, also when one panics: the scope waits for it
        // before passing the panic on, the job would hang.
        let _done = Done(&live.done);
        let mut ranked = if plan.race {
            qualify(&searcher, &plan)
        } else {
            Vec::new()
        };
        for (stage, &width) in plan.stages.iter().enumerate() {
            let routes: Vec<(usize, Option<Vec<Stop>>)> = if stage == 0 && !plan.race {
                (0..width).map(|k| (k, None)).collect()
            } else {
                if interrupt.stopping() || ranked.is_empty() {
                    break;
                }
                ranked
                    .iter()
                    .take(width)
                    .map(|(_, k, r)| (*k, Some(r.clone())))
                    .collect()
            };
            ranked = round(&searcher, &plan, stage, &routes);
        }
        ranked
    });
    live.report();
    if !interrupt.cancelled() {
        progress(&summary(planner, &ranked));
    }
    ranked.into_iter().next().map(|(_, _, r)| r).unwrap_or_default()
}

/// Sets its flag when dropped, unwinding included.
struct Done<'a>(&'a AtomicBool);

impl Drop for Done<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// How the tries are shared: the race of candidates, then the routes of each round.
struct Tournament {
    threads: usize,
    candidates: usize,
    race: bool,
    /// Routes of each round: `threads`, then half as many each time, the last one the final.
    stages: Vec<usize>,
}

impl Tournament {
    fn new(params: &Params) -> Self {
        let threads = params.threads.max(1);
        // A low effort races fewer candidates: a preview does not build 64 routes.
        let candidates = ((params.candidates as f64 * params.effort.min(1.0)).round() as usize).max(threads);
        let race = candidates > threads;
        let last = params.final_routes.clamp(1, threads);
        let mut stages = vec![threads];
        while let Some(&n) = stages.last()
            && n > last
        {
            stages.push((n / 2).max(last));
        }
        // Rounds without tries would only rank the routes: straight to the final (without a race
        // the first round still builds them).
        if params.round_tries <= 0.0 {
            stages = if race || threads == last {
                vec![last]
            } else {
                vec![threads, last]
            };
        }
        Self {
            threads,
            candidates,
            race,
            stages,
        }
    }

    /// Searches improving each route of round `stage` (as even as can be).
    fn searches(&self, stage: usize, route: usize) -> usize {
        let width = self.stages[stage];
        self.threads / width + usize::from(route < self.threads % width)
    }

    fn is_final(&self, stage: usize) -> bool {
        stage + 1 == self.stages.len()
    }

    /// Tries of the race and of the whole tournament, per stop of the routes (times `effort`).
    fn tries_per_stop(&self, params: &Params) -> (f64, f64) {
        let race = if self.race {
            self.candidates as f64 * params.qualify_tries
        } else {
            0.0
        };
        let rounds = (self.stages.len() - 1) as f64;
        let after = self.threads as f64 * (rounds * params.round_tries + params.final_tries);
        (race * params.effort, (race + after) * params.effort)
    }

    fn announce(&self) -> String {
        let rounds: Vec<String> = self.stages.iter().map(ToString::to_string).collect();
        let rounds = format!("{} routes, {} searches", rounds.join(" -> "), self.threads);
        if self.race {
            format!("Optimizing (a race of {} candidates, then {rounds})", self.candidates)
        } else {
            format!("Optimizing ({rounds})")
        }
    }
}

/// Construction settings of race candidate `k`: its seed, and (with `construction_random`)
/// varied greedy weights.
fn candidate_params(params: &Params, k: usize) -> Params {
    let mut p = params.clone();
    p.seed = params.seed.wrapping_add(k as u64);
    if params.construction == Construction::Mixed {
        p.construction = if k % 2 == 1 {
            Construction::Regions
        } else {
            Construction::Greedy
        };
    }
    if params.construction_random && k > 0 && p.construction == Construction::Greedy {
        let mut rng = rand::rngs::SmallRng::seed_from_u64(p.seed ^ 0x9e37_79b9_7f4a_7c15);
        p.zone_stay = rng.random_range(1.5..=3.5);
        p.continent_stay = rng.random_range(1.0..=4.0);
        p.dungeon_quest_weight = rng.random_range(0.25..=1.0);
        p.fit_penalty = rng.random_range(0.0..=0.4);
        p.fit_above_penalty = rng.random_range(0.0..=0.6);
        p.efficiency_power = rng.random_range(0.0..=0.6);
    }
    p
}

/// What the app sees while the tournament runs. Routes are ranked by score, which weighs
/// grinding, flights and zone changes on top of the play time.
struct Live<'p> {
    progress: Progress<'p>,
    race: bool,
    candidates: usize,
    threads: usize,
    stages: Vec<usize>,
    /// Tries of the race and of the whole tournament per stop (see `Tournament::tries_per_stop`).
    tries_per_stop: (f64, f64),
    /// Time spent building candidate routes, how many were built and their stops.
    built: Mutex<(Duration, u32, usize)>,
    /// Tries of all the searches so far.
    tried: AtomicU64,
    /// Best (score, play time) of each candidate's route so far.
    routes: Mutex<Vec<Option<(f64, f64)>>>,
    /// The round under way (none during the race) and its routes.
    round: Mutex<Option<(usize, Vec<usize>)>>,
    started: OnceLock<Instant>,
    /// (best start score, its play time, best score, its play time)
    scores: Mutex<(f64, f64, f64, f64)>,
    /// Best route so far, and whether it changed since last drawn.
    best_route: Mutex<(Vec<Stop>, bool)>,
    done: AtomicBool,
    qualified: AtomicUsize,
}

impl<'p> Live<'p> {
    fn new(plan: &Tournament, params: &Params, progress: Progress<'p>) -> Self {
        Self {
            progress,
            race: plan.race,
            candidates: plan.candidates,
            threads: plan.threads,
            stages: plan.stages.clone(),
            tries_per_stop: plan.tries_per_stop(params),
            built: Mutex::new((Duration::ZERO, 0, 0)),
            tried: AtomicU64::new(0),
            routes: Mutex::new(vec![None; plan.candidates.max(plan.threads)]),
            round: Mutex::new(None),
            started: OnceLock::new(),
            scores: Mutex::new((f64::INFINITY, f64::INFINITY, f64::INFINITY, f64::INFINITY)),
            best_route: Mutex::new((Vec::new(), false)),
            done: AtomicBool::new(false),
            qualified: AtomicUsize::new(0),
        }
    }

    /// Report every 300 ms until the tournament is over.
    fn watch(&self, searcher: &Searcher<'_>) {
        let viewer = Planner::new(searcher.model, searcher.world, searcher.params, searcher.profile);
        let mut last_route = Instant::now();
        while !self.done.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(300));
            self.report();
            self.report_route(&mut last_route, &viewer);
        }
    }

    /// Round `stage` begins with these routes.
    fn enter(&self, stage: usize, ids: Vec<usize>) {
        *self.round.lock().unwrap() = Some((stage, ids));
    }

    /// The routes followed: during the race the best `threads` candidates so far, then the
    /// routes of the round. Each as {id, time (play), score}; `selected` is the best score.
    fn followed(&self) -> (Vec<serde_json::Value>, Option<usize>) {
        let routes = self.routes.lock().unwrap();
        let ids: Vec<usize> = if let Some((_, ids)) = &*self.round.lock().unwrap() {
            ids.clone()
        } else {
            let mut ranked: Vec<usize> = (0..routes.len()).filter(|&k| routes[k].is_some()).collect();
            ranked.sort_by(|&a, &b| routes[a].unwrap().0.total_cmp(&routes[b].unwrap().0));
            ranked.truncate(self.threads);
            ranked
        };
        let shown: Vec<(usize, f64, f64)> = ids
            .into_iter()
            .filter_map(|k| routes.get(k).copied().flatten().map(|(score, play)| (k, score, play)))
            .filter(|(_, score, _)| score.is_finite())
            .collect();
        let selected = shown.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|r| r.0);
        let list = shown
            .iter()
            .map(|&(id, score, play)| serde_json::json!({ "id": id, "time": play, "score": score }))
            .collect();
        (list, selected)
    }

    /// Share of the tries done (0 to 1), and where the race ends, once a route gives the size of
    /// the routes (the tries are per stop).
    fn progress(&self) -> Option<(f64, f64)> {
        let (_, n, stops) = *self.built.lock().unwrap();
        let (race, total) = self.tries_per_stop;
        if n == 0 || total <= 0.0 {
            return None;
        }
        let tries = total * stops as f64 / f64::from(n);
        let done = self.tried.load(Ordering::Relaxed) as f64;
        Some(((done / tries.max(1.0)).min(1.0), race / total))
    }

    /// `@progress {json}`: phase, elapsed seconds, share of the tries done, play time of the best
    /// start and best route, the stage (race, round, final) and the routes followed.
    fn report(&self) {
        let (_, start, _, best) = *self.scores.lock().unwrap();
        let Some(t) = self.started.get() else {
            (self.progress)(&format!("@progress {}", serde_json::json!({ "phase": "construct" })));
            return;
        };
        let (routes, selected) = self.followed();
        let mut msg = serde_json::json!({
            "phase": "optimize",
            "elapsed": t.elapsed().as_secs_f64(),
            "start": if start.is_finite() { Some(start) } else { None },
            "best": if best.is_finite() { Some(best) } else { None },
            "routes": routes,
            "selected": selected,
            "candidates": if self.race { self.candidates } else { 0 },
            "qualified": self.qualified.load(Ordering::Relaxed),
            "rounds": self.stages.len(),
        });
        if let Some((done, race_end)) = self.progress() {
            msg["progress"] = done.into();
            msg["race_end"] = race_end.into();
        }
        match &*self.round.lock().unwrap() {
            None => msg["stage"] = "qualify".into(),
            Some((stage, ids)) => {
                let last = stage + 1 == self.stages.len();
                msg["stage"] = if last { "final" } else { "round" }.into();
                msg["round"] = (stage + 1).into();
                msg["width"] = ids.len().into();
                msg["searches"] = (self.threads / ids.len().max(1)).into();
            }
        }
        (self.progress)(&format!("@progress {msg}"));
    }

    /// `@route {points}` for live drawing, when the best route changed (at most every 1.5 s).
    fn report_route(&self, last: &mut Instant, planner: &Planner<'_>) {
        if last.elapsed() < Duration::from_millis(1500) {
            return;
        }
        let route = {
            let mut b = self.best_route.lock().unwrap();
            if !b.1 {
                return;
            }
            b.1 = false;
            b.0.clone()
        };
        *last = Instant::now();
        let points = planner.places().polyline(&route, 800);
        (self.progress)(&format!("@route {}", serde_json::json!({ "points": points })));
    }

    fn on_best(&self, k: usize, first: &Cell<bool>, score: f64, play: f64, route: &[Stop]) {
        // Several searches may improve the same route: its best of them.
        if let Some(r) = self.routes.lock().unwrap().get_mut(k)
            && r.is_none_or(|(s, _)| score < s)
        {
            *r = Some((score, play));
        }
        let mut s = self.scores.lock().unwrap();
        if first.replace(false) && score < s.0 {
            (s.0, s.1) = (score, play);
        }
        if score < s.2 {
            (s.2, s.3) = (score, play);
            *self.best_route.lock().unwrap() = (route.to_vec(), true);
        }
    }
}

/// Where a search stands in the tournament.
#[derive(Clone, Copy)]
struct Slot {
    /// Round and search of the route (None in the race: the candidate's own seed).
    round: Option<(usize, usize)>,
    /// Tries per stop of the route (times `effort`).
    tries_per_stop: f64,
    /// Its progress is logged (the first search of the best route of a round).
    logs: bool,
}

/// Builds and improves the route of one candidate.
struct Searcher<'a> {
    model: &'a Model,
    world: &'a World,
    params: &'a Params,
    profile: &'a Profile,
    live: &'a Live<'a>,
    progress: Progress<'a>,
    /// The user stopping or cancelling the generation.
    interrupt: &'a Interrupt,
}

impl Searcher<'_> {
    /// Improve `route` (built when None) with candidate `k`'s settings for the tries of `slot`,
    /// meeting the other searches of the route at `member`'s exchange. Returns its score under
    /// the request's own settings; None when the generation is cancelled, or stopped while
    /// building a route that is not `needed` (the first ones are: without them there would be no
    /// guide to keep).
    fn search(
        &self,
        k: usize,
        route: Option<Vec<Stop>>,
        slot: Slot,
        needed: bool,
        member: Option<&Member<'_>>,
    ) -> Option<(f64, Vec<Stop>)> {
        if self.interrupt.cancelled() {
            return None;
        }
        let mut p = candidate_params(self.params, k);
        if let Some((stage, search)) = slot.round {
            p.seed = round_seed(p.seed, stage, search);
        }
        let give_up = || self.interrupt.cancelled() || (!needed && self.interrupt.stopping());
        let planner = Planner::new(self.model, self.world, &p, self.profile)
            .abortable(&give_up)
            .sharing(member);
        let log = |m: &str| {
            if slot.logs {
                (self.progress)(m);
            }
        };
        let first = Cell::new(route.is_none());
        let start = if let Some(route) = route {
            route
        } else {
            let begun = Instant::now();
            let route = planner.construct(log);
            if planner.aborted() {
                return None;
            }
            let mut built = self.live.built.lock().unwrap();
            built.0 += begun.elapsed();
            built.1 += 1;
            built.2 += route.len();
            route
        };
        self.live.started.get_or_init(Instant::now);
        let tries = (slot.tries_per_stop * self.params.effort * start.len() as f64).round() as u64;
        let route = planner.improve_while(
            start,
            Effort::Tries(tries),
            || {
                self.live.tried.fetch_add(1, Ordering::Relaxed);
                !self.interrupt.stopping()
            },
            log,
            |score, play, route| {
                self.live.on_best(k, &first, score, play, route);
            },
        );
        if self.interrupt.cancelled() {
            return None;
        }
        let base = Planner::new(self.model, self.world, self.params, self.profile);
        Some((base.simulate(&route, None).unwrap_or(f64::INFINITY), route))
    }
}

/// Seed of search `search` of a route in round `stage`, from the candidate's seed.
fn round_seed(seed: u64, stage: usize, search: usize) -> u64 {
    let mix = ((stage as u64 + 1) << 32) | search as u64;
    rand::rngs::SmallRng::seed_from_u64(seed ^ mix.wrapping_mul(0x9e37_79b9_7f4a_7c15)).random()
}

/// Routes ranked by score, then by candidate (the same tries give the same order).
fn rank(mut routes: Vec<(f64, usize, Vec<Stop>)>) -> Vec<(f64, usize, Vec<Stop>)> {
    routes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    routes
}

/// The race: every candidate is built and improved for `qualify_tries`, `threads` at a time.
/// Stopped, no new candidate starts but the first ones (there must be a route).
fn qualify(searcher: &Searcher<'_>, plan: &Tournament) -> Vec<(f64, usize, Vec<Stop>)> {
    let next = AtomicUsize::new(0);
    let qualified: Mutex<Vec<(f64, usize, Vec<Stop>)>> = Mutex::new(Vec::new());
    let slot = Slot {
        round: None,
        tries_per_stop: searcher.params.qualify_tries,
        logs: false,
    };
    std::thread::scope(|scope| {
        for _ in 0..plan.threads {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let stopping = searcher.interrupt.stopping() && k >= plan.threads;
                    if k >= plan.candidates || stopping || searcher.interrupt.cancelled() {
                        break;
                    }
                    if let Some((score, route)) = searcher.search(k, None, slot, k < plan.threads, None) {
                        qualified.lock().unwrap().push((score, k, route));
                        searcher.live.qualified.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    let ranked = rank(qualified.into_inner().unwrap());
    if std::env::var_os("FG_RACE_LOG").is_some() {
        for (score, k, _) in &ranked {
            (searcher.progress)(&format!("candidate {k}: {}", fmt_time(*score)));
        }
    }
    ranked
}

/// Round `stage`: each route improved by its searches (`Tournament::searches`), which share their
/// best (`share_every`). Each route's best, ranked.
fn round(
    searcher: &Searcher<'_>,
    plan: &Tournament,
    stage: usize,
    routes: &[(usize, Option<Vec<Stop>>)],
) -> Vec<(f64, usize, Vec<Stop>)> {
    searcher.live.enter(stage, routes.iter().map(|(k, _)| *k).collect());
    let tries_per_stop = if plan.is_final(stage) {
        searcher.params.final_tries
    } else {
        searcher.params.round_tries
    };
    let exchanges: Vec<Exchange> = (0..routes.len())
        .map(|i| Exchange::new(plan.searches(stage, i)))
        .collect();
    let found: Vec<(f64, usize, Vec<Stop>)> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (i, (k, route)) in routes.iter().enumerate() {
            let searches = plan.searches(stage, i);
            let exchange = &exchanges[i];
            for search in 0..searches {
                let slot = Slot {
                    round: Some((stage, search)),
                    tries_per_stop,
                    logs: i == 0 && search == 0,
                };
                handles.push(scope.spawn(move || {
                    // Dropped at the end, panic included: the others stop waiting for it.
                    let member = exchange.member();
                    let seat = (searcher.params.share_every > 0 && searches > 1).then_some(&member);
                    let result = searcher.search(*k, route.clone(), slot, true, seat);
                    result.map(|(score, r)| (score, *k, r))
                }));
            }
        }
        handles.into_iter().filter_map(|h| h.join().unwrap()).collect()
    });
    // Each route's best search (the first one on a tie).
    let mut best: Vec<(f64, usize, Vec<Stop>)> = Vec::new();
    for (score, k, route) in found {
        match best.iter_mut().find(|b| b.1 == k) {
            Some(b) if score < b.0 => *b = (score, k, route),
            Some(_) => {}
            None => best.push((score, k, route)),
        }
    }
    let ranked = rank(best);
    if std::env::var_os("FG_RACE_LOG").is_some() {
        for (score, k, _) in &ranked {
            (searcher.progress)(&format!("round {} route {k}: {}", stage + 1, fmt_time(*score)));
        }
    }
    ranked
}

/// Play time of the routes of the last round (ranked by score).
fn summary(planner: &Planner<'_>, ranked: &[(f64, usize, Vec<Stop>)]) -> String {
    let times: Vec<String> = ranked
        .iter()
        .map(|(_, _, r)| {
            if let Some((_, b)) = planner.simulate_full(r, None) {
                return fmt_time(b.total);
            }
            if std::env::var_os("FG_RACE_LOG").is_some()
                && let Some((n, why)) = planner.diagnose(r)
            {
                eprintln!("invalid route ({} stops) at stop {n}: {why}", r.len());
            }
            "-".into()
        })
        .collect();
    format!("{} routes: {}", ranked.len(), times.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tournament(threads: usize, candidates: usize, final_routes: usize, effort: f64) -> Tournament {
        Tournament::new(&Params {
            candidates,
            round_tries: 20.0,
            final_routes,
            effort,
            threads,
            ..Params::default()
        })
    }

    /// Without tries in the rounds, the race goes straight to the final; without a race the
    /// routes are built by a first round of their own.
    #[test]
    fn no_rounds_without_tries() {
        let params = |effort| Params {
            candidates: 32,
            round_tries: 0.0,
            final_routes: 1,
            effort,
            threads: 8,
            ..Params::default()
        };
        assert_eq!(Tournament::new(&params(1.0)).stages, vec![1]);
        let preview = Tournament::new(&params(0.1));
        assert!(!preview.race);
        assert_eq!(preview.stages, vec![8, 1]);
        assert_eq!(preview.searches(1, 0), 8);
    }

    /// Each round keeps half the routes, with twice the searches each, down to the final.
    #[test]
    fn rounds_halve_the_routes() {
        let plan = tournament(8, 64, 2, 1.0);
        assert!(plan.race);
        assert_eq!(plan.stages, vec![8, 4, 2]);
        let searches = |stage| {
            (0..plan.stages[stage])
                .map(|i| plan.searches(stage, i))
                .collect::<Vec<_>>()
        };
        assert_eq!(searches(0), vec![1; 8]);
        assert_eq!(searches(1), vec![2; 4]);
        assert_eq!(searches(2), vec![4; 2]);
        assert_eq!(tournament(6, 64, 2, 1.0).stages, vec![6, 3, 2]);
        assert_eq!(tournament(1, 64, 2, 1.0).stages, vec![1]);
    }

    /// A low effort races fewer candidates, down to none (one route per search).
    #[test]
    fn a_preview_races_fewer_candidates() {
        assert_eq!(tournament(8, 64, 2, 0.5).candidates, 32);
        let preview = tournament(8, 64, 2, 0.1);
        assert_eq!(preview.candidates, 8);
        assert!(!preview.race);
    }

    /// The tries are counted per stop: the race's share of them, then every search's.
    #[test]
    fn progress_counts_the_tries_per_stop() {
        let params = Params {
            candidates: 64,
            qualify_tries: 2.0,
            round_tries: 10.0,
            final_routes: 2,
            final_tries: 50.0,
            effort: 1.0,
            threads: 8,
            ..Params::default()
        };
        let (race, total) = Tournament::new(&params).tries_per_stop(&params);
        assert!((race - 128.0).abs() < 1e-9);
        assert!((total - (128.0 + 8.0 * (2.0 * 10.0 + 50.0))).abs() < 1e-9);
    }
}
