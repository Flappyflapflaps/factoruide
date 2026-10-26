//! "Re-plan from here" (the addon's Replan.lua): a character asks in game for its route to be
//! computed again from where it stands, and the game reloads so that its SavedVariables hold the
//! request (`FactoruideDB.replans`) and the character as it is now. While the app runs, it looks at
//! those files every few seconds: a new request is planned in the background with the settings of
//! the guide the character followed, saved as a new version of that character's own guide and
//! installed; the character follows it at its next /reload (the route's `replan` says for whom).
//! The window is told when a re-plan starts and ends (`replan` events).

use super::{
    AppState, CmdResult, GuideInfo, VersionMeta, characters, data_version, err, now, overrides, write_atomic,
    write_data,
};
use fg_contrib::profile::ReplanAsk;
use fg_route::job::{self, PlanRequest, StartState};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// How often the SavedVariables are looked at.
const EVERY: std::time::Duration = std::time::Duration::from_secs(5);
/// Requests older than this are not answered (the addon forgets them after a day too).
const TOO_OLD: i64 = 86_400;
/// The event telling the window.
pub const EVENT: &str = "replan";

/// What the window is told: a re-plan for `character` started (`planning`), is installed (`ready`,
/// `guide` its name) or failed (`failed`, `message` why).
#[derive(Debug, Clone, Serialize)]
pub struct ReplanEvent {
    pub character: String,
    pub state: &'static str,
    pub guide: Option<String>,
    pub message: Option<String>,
}

/// The requests answered (or tried) for each character of an edition: the time of the last one.
fn answered_file(data_dir: &Path, edition: fg_route::xp::Edition) -> PathBuf {
    data_dir.join(format!("replans-{}.json", edition.key()))
}

fn read_answered(file: &Path) -> BTreeMap<String, i64> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// The requests to answer now, the oldest first: newer than the last one answered for their
/// character, and not too old (`now`, unix seconds).
pub fn due(asks: Vec<ReplanAsk>, answered: &BTreeMap<String, i64>, now: i64) -> Vec<ReplanAsk> {
    let mut latest: BTreeMap<String, ReplanAsk> = BTreeMap::new();
    for ask in asks {
        if answered.get(&ask.character).is_some_and(|&t| t >= ask.asked) || now - ask.asked > TOO_OLD {
            continue;
        }
        // Several from one character (copies of the SavedVariables): its last one.
        if latest.get(&ask.character).is_none_or(|a| a.asked < ask.asked) {
            latest.insert(ask.character.clone(), ask);
        }
    }
    let mut out: Vec<ReplanAsk> = latest.into_values().collect();
    out.sort_by_key(|a| a.asked);
    out
}

/// Look at the SavedVariables while the app runs (a thread of its own).
pub fn watch_loop(app: &AppHandle) {
    let mut seen = None;
    loop {
        std::thread::sleep(EVERY);
        let state = app.state::<AppState>();
        let settings = state.settings();
        let files = fg_contrib::collector::find_saved_variables(&settings.edition.client_dir(&settings.wow_dir));
        // The game writes them at a reload or a logout: read again only then.
        let stamp = files.iter().filter_map(|f| f.metadata().ok()?.modified().ok()).max();
        if stamp.is_none() || stamp == seen {
            continue;
        }
        seen = stamp;
        if let Err(e) = answer_all(app, &state, &files) {
            eprintln!("re-plan: {e}");
        }
    }
}

fn answer_all(app: &AppHandle, state: &AppState, files: &[PathBuf]) -> CmdResult<()> {
    let asks = fg_contrib::profile::replans(files).map_err(err)?;
    let file = answered_file(&state.data_dir, state.edition());
    let mut answered = read_answered(&file);
    let now = i64::try_from(now()?).map_err(err)?;
    for ask in due(asks, &answered, now) {
        // Recorded first: a request that fails is not tried again at every save of the files.
        answered.insert(ask.character.clone(), ask.asked);
        write_atomic(&file, &serde_json::to_vec_pretty(&answered).map_err(err)?).map_err(err)?;
        let tell = |state: &'static str, guide: Option<String>, message: Option<String>| {
            let _ = app.emit(
                EVENT,
                ReplanEvent {
                    character: ask.character.clone(),
                    state,
                    guide,
                    message,
                },
            );
        };
        tell("planning", ask.route.clone(), None);
        match answer(state, &ask) {
            Ok(meta) => tell("ready", Some(meta.name), None),
            Err(e) => tell("failed", ask.route.clone(), Some(e)),
        }
    }
    Ok(())
}

impl AppState {
    /// The request of the guide named `name` (in game), in the version its characters follow, and
    /// the character it started from.
    fn followed_request(&self, name: &str) -> Option<(PlanRequest, Option<String>)> {
        for entry in std::fs::read_dir(self.guides()).into_iter().flatten().flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            let Some(n) = self.followed_version(&id) else { continue };
            let Ok(v) = self.version(&id, n) else { continue };
            if v.meta.name == name {
                return Some((v.request, v.meta.character));
            }
        }
        None
    }
}

/// Compute the route again for a request and install it: the guide the character followed, its
/// settings, from the character as the game saved it; a ready-made or imported guide (not the
/// app's), the "New guide" page's settings for its race and class.
pub fn answer(state: &AppState, ask: &ReplanAsk) -> CmdResult<VersionMeta> {
    let edition = state.edition();
    let all = characters(state)?;
    let character = all
        .iter()
        .find(|c| !c.shared && c.profile.name == ask.character)
        .ok_or_else(|| format!("{}: no profile saved in game", ask.character))?;
    let profiles: Vec<StartState> = all.iter().map(|c| c.profile.clone()).collect();
    let start = character.profile.clone();
    let level = start.level;
    let conn = state.open_db()?;
    let overrides = overrides(edition)?;
    let followed = ask
        .route
        .as_deref()
        .and_then(|name| Some((name, state.followed_request(name)?)));
    let request = if let Some((name, (base, base_start))) = followed {
        let name = job::replan_name(name, base_start.as_deref(), &ask.character);
        job::replan_request(&base, name, start, &profiles)
    } else {
        let race = character
            .race
            .clone()
            .ok_or_else(|| format!("{}: a race this game version does not know", ask.character))?;
        let options = job::options(&conn, &overrides).map_err(err)?;
        let base = ask
            .route
            .clone()
            .unwrap_or_else(|| format!("{race} {}", character.class));
        let name = job::replan_name(&base, None, &ask.character);
        job::default_request(&options, &race, &character.class, name, start)
    };
    let walks = state.walks(edition);
    let mut outcome = job::plan_interruptible(
        &conn,
        &overrides,
        &request,
        &|_| {},
        &job::Interrupt::default(),
        Some(&walks),
    )
    .map_err(err)?;
    outcome.route.replan = Some(job::replanned(&ask.character, ask.asked, level));
    let meta = state.add_version(&request, &outcome, data_version(&conn), (false, false), now()?)?;
    // Followed in this version, even where an older one was chosen.
    let info = state.guide_info(&meta.guide)?;
    if info.installed.is_some() {
        state.write_guide_info(
            &meta.guide,
            &GuideInfo {
                installed: None,
                ..info
            },
        )?;
    }
    write_data(state)?;
    Ok(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(character: &str, asked: i64) -> ReplanAsk {
        ReplanAsk {
            character: character.into(),
            asked,
            route: None,
            level: 10,
        }
    }

    #[test]
    fn the_last_new_request_of_each_character() {
        let answered = BTreeMap::from([("Brouz-Realm".to_owned(), 1_000)]);
        let now = 100_000;
        let due = due(
            vec![
                ask("Brouz-Realm", 1_000), // answered already
                ask("Pata-Realm", 2_000),  // too old
                ask("Pata-Realm", 99_000),
                ask("Pata-Realm", 99_500), // its last one
                ask("Brouz-Realm", 99_200),
            ],
            &answered,
            now,
        );
        let got: Vec<(&str, i64)> = due.iter().map(|a| (a.character.as_str(), a.asked)).collect();
        assert_eq!(got, [("Brouz-Realm", 99_200), ("Pata-Realm", 99_500)]);
    }
}
