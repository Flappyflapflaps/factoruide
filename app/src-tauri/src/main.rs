// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fg_app::{CmdError, EditOpened, Host, Studio, now};
use fg_contrib::contribution::{Contribution, SendState};
use fg_library::{Comparison, DiskFiles, GuideSummary, GuideVersion, Library, Origin, VersionMeta};
use fg_route::Route;
use fg_route::job::{self, Options, Overrides, PlanOutcome, PlanRequest, RouteStep, StartState};
use fg_route::sync::LockExt;
use fg_route::xp::Edition;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};

/// Game database, overrides and addon, embedded at build time (see build.rs).
mod game;

/// Contributions to the contribution server.
mod contributions;
use contributions::{
    auto_contribute_loop, contribution_preview, need_help, read_send_record, send_contribution, send_issue,
    send_record_file, sent_lua, setup_report,
};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    wow_dir: PathBuf,
    /// Game version the app works on (forever, classic, tbc).
    #[serde(default)]
    edition: Edition,
    /// App language (also the default guide language); unset: the system language.
    #[serde(default)]
    language: Option<String>,
    /// Contribution server (its base address); unset: the one the app was built with.
    #[serde(default)]
    contribution_url: Option<String>,
    /// Random id of this install, sent with its contributions (tells how many different
    /// players back what they report; nothing else about the player).
    #[serde(default)]
    contributor: Option<String>,
    /// Send what is new to the contribution server on its own, once a day at most (off unless
    /// the player turns it on).
    #[serde(default)]
    auto_contribute: bool,
    /// Addon behaviour, written to the addon's Config.lua on install.
    #[serde(default)]
    addon: serde_json::Map<String, serde_json::Value>,
    /// What shows the guides in game on WoW Forever (see `guide_addon`).
    #[serde(default)]
    guide_addon: GuideAddon,
    /// Language the guides are written in in the addon (game locale); unset: each guide in the
    /// language it was made in.
    #[serde(default)]
    guide_locale: Option<String>,
    /// Offer every guide to every character, not only those of its race and class.
    #[serde(default)]
    all_guides: bool,
    /// What the addon in the game was last installed with (`addon_snapshot`): the settings page
    /// saves at once, the game gets them when asked.
    #[serde(default)]
    addon_installed: Option<String>,
    /// Whether the addon's setup questions were answered (or skipped): asked once, then on demand.
    #[serde(default)]
    addon_setup_done: bool,
    /// Whether the first launch's questions (where the game is) were answered or skipped.
    #[serde(default)]
    welcome_done: bool,
    /// Characters taken out of the Characters page ("<edition>/<Name-Realm>" -> the time of the
    /// profile then): deleted in game, but the game keeps their files and the addon their
    /// profile. One played again (a newer profile) is back.
    #[serde(default)]
    forgotten_characters: BTreeMap<String, i64>,
}

/// The settings the installed addon depends on (its options, the guides' language, what shows
/// the guides).
fn addon_snapshot(settings: &Settings) -> String {
    serde_json::to_string(&(
        &settings.addon,
        &settings.guide_locale,
        settings.guide_addon(settings.edition),
    ))
    .unwrap_or_default()
}

/// What shows the guides in game: the addon's guide part, or Polestar (WoW Forever only) with a
/// guide pack of them; the addon's profile part records in both cases.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GuideAddon {
    #[default]
    Factoruide,
    Polestar,
}

use game::{GUIDE_ADDON, PROFILE_ADDON, WowProblem, game_root};

impl Default for Settings {
    fn default() -> Self {
        let wow_dir = if cfg!(windows) {
            PathBuf::from(r"C:\Program Files (x86)\World of Warcraft")
        } else if cfg!(target_os = "macos") {
            PathBuf::from("/Applications/World of Warcraft")
        } else {
            // Under WSL, the Windows game; otherwise Battle.net in Lutris' default Wine prefix.
            let wsl = PathBuf::from("/mnt/c/Program Files (x86)/World of Warcraft");
            match std::env::var_os("HOME") {
                Some(home) if !wsl.is_dir() => {
                    PathBuf::from(home).join("Games/battlenet/drive_c/Program Files (x86)/World of Warcraft")
                }
                _ => wsl,
            }
        };
        Self {
            wow_dir,
            edition: Edition::Forever,
            language: None,
            contribution_url: None,
            contributor: None,
            auto_contribute: false,
            addon: serde_json::Map::new(),
            guide_addon: GuideAddon::Factoruide,
            guide_locale: None,
            all_guides: false,
            addon_installed: None,
            addon_setup_done: false,
            welcome_done: false,
            forgotten_characters: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// The addons folder of a game version's client.
    fn addons_dir(&self, edition: Edition) -> PathBuf {
        edition.client_dir(&self.wow_dir).join("Interface/AddOns")
    }
    /// What shows the guides in a game version: Polestar exists on WoW Forever only.
    fn guide_addon(&self, edition: Edition) -> GuideAddon {
        if edition == Edition::Forever {
            self.guide_addon
        } else {
            GuideAddon::Factoruide
        }
    }
}

struct AppState {
    settings: Mutex<Settings>,
    /// The last install added files the game reads only when it starts.
    restart_needed: Mutex<bool>,
    config_file: PathBuf,
    /// Where the embedded databases are unpacked, and those unpacked so far.
    db_dir: PathBuf,
    dbs: Mutex<HashMap<Edition, PathBuf>>,
    /// Saved guides and debug exports.
    data_dir: PathBuf,
    /// The same folder, for the guide library.
    files: DiskFiles,
    /// What the app remembers between commands: the generation under way, the guide in the
    /// editor, the walking memory, the names read.
    studio: Studio,
    /// The window's handle, for the events (set once the app is built).
    app: OnceLock<AppHandle>,
    /// Held while a contribution is read, sent and recorded: one send at a time, manual or
    /// automatic.
    send_lock: Mutex<()>,
    /// Wakes the automatic contributions up (turned on in the settings).
    auto_wake: std::sync::mpsc::Sender<()>,
}

type CmdResult<T> = Result<T, CmdError>;

fn err(e: impl std::fmt::Display) -> CmdError {
    CmdError::App(format!("{e:#}"))
}

/// Embedded overrides of an edition.
fn overrides(edition: Edition) -> CmdResult<Overrides> {
    Overrides::parse_edition(
        |name| {
            let (_, bytes) = embedded::OVERRIDES.iter().find(|(n, _)| *n == name)?;
            Some(String::from_utf8_lossy(bytes).into_owned())
        },
        edition,
    )
    .map_err(err)
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.locked().clone()
    }
    fn edition(&self) -> Edition {
        self.settings.locked().edition
    }
    /// The database of an edition, unpacked the first time.
    fn db_path(&self, edition: Edition) -> CmdResult<PathBuf> {
        if let Some(p) = self.dbs.locked().get(&edition) {
            return Ok(p.clone());
        }
        let path = unpack_database(&self.db_dir, edition).map_err(err)?;
        self.dbs.locked().insert(edition, path.clone());
        Ok(path)
    }
    fn open_db(&self) -> CmdResult<Connection> {
        open_db(&self.db_path(self.edition())?)
    }
    /// A guide written in `locale` (None: the language it was made in).
    fn render(&self, route: &Route, locale: Option<&str>) -> CmdResult<Route> {
        self.studio.render(self, self.edition(), route, locale)
    }
}

impl Host for AppState {
    type Files = DiskFiles;

    fn files(&self) -> &DiskFiles {
        &self.files
    }

    fn edition(&self) -> Edition {
        AppState::edition(self)
    }

    fn open_db(&self, edition: Edition) -> CmdResult<Connection> {
        open_db(&self.db_path(edition)?)
    }

    fn overrides(&self, edition: Edition) -> CmdResult<Overrides> {
        overrides(edition)
    }

    fn progress(&self, line: &str) {
        if let Some(app) = self.app.get() {
            let _ = app.emit("plan-progress", line.to_owned());
        }
    }
}

fn open_db(path: &Path) -> CmdResult<Connection> {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| CmdError::App(format!("{}: {e}", path.display())))
}

/// Unpack an edition's embedded database into `dir` once per build (the file name carries its
/// hash), removing the ones of previous builds.
fn unpack_database(dir: &Path, edition: Edition) -> anyhow::Result<PathBuf> {
    use std::hash::{Hash, Hasher};
    let Some((_, data)) = embedded::DATABASES.iter().find(|(e, _)| *e == edition.key()) else {
        anyhow::bail!("no data for {} in this build", edition.rules().edition_name);
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.hash(&mut hasher);
    let prefix = format!("{}-", edition.key());
    let name = format!("{prefix}{:016x}.sqlite", hasher.finish());
    let path = dir.join(&name);
    if !path.exists() {
        std::fs::create_dir_all(dir)?;
        for entry in std::fs::read_dir(dir)?.flatten() {
            let n = entry.file_name().to_string_lossy().into_owned();
            if (n.starts_with(&prefix) || n.starts_with("factoruide-")) && n.ends_with(".sqlite") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, zstd::decode_all(*data)?)?;
        std::fs::rename(&tmp, &path)?;
    }
    Ok(path)
}

/// An edition the app knows: whether its data is bundled and its game client installed.
#[derive(Serialize)]
struct EditionInfo {
    key: &'static str,
    name: &'static str,
    /// Data bundled in this build.
    available: bool,
    /// Game client folder found in the WoW folder.
    installed: bool,
    max_level: i64,
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn list_editions(state: State<'_, AppState>) -> Vec<EditionInfo> {
    let settings = state.settings();
    Edition::ALL
        .iter()
        .map(|e| EditionInfo {
            key: e.key(),
            name: e.rules().edition_name,
            available: embedded::DATABASES.iter().any(|(k, _)| *k == e.key()) && e.rules().known(),
            installed: e.installed_in(&settings.wow_dir),
            max_level: e.rules().max_level,
        })
        .collect()
}

/// Routes and results used to live in the Factoruide checkout (`workspace` setting): copy them
/// into the app data folder the first time.
fn migrate_workspace(config: &serde_json::Value, data_dir: &Path) {
    let Some(workspace) = config.get("workspace").and_then(|w| w.as_str()).map(PathBuf::from) else {
        return;
    };
    {
        let folder = "results";
        let (from, to) = (workspace.join(folder), data_dir.join(folder));
        if from.is_dir() && !to.exists() {
            let _ = copy_dir(&from, &to);
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_dir(&path, &to.join(entry.file_name()))?;
        } else {
            std::fs::copy(&path, to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Whether this build checks for updates: only the published ones (`FG_RELEASE` set by the
/// release workflow), not development or local builds, whose version is not a release's.
#[tauri::command]
fn updates_enabled() -> bool {
    option_env!("FG_RELEASE").is_some()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn set_settings(state: State<'_, AppState>, mut settings: Settings) -> CmdResult<()> {
    let before = state.settings();
    // Not edited by the settings page: kept.
    if settings.contributor.is_none() {
        settings.contributor = before.contributor;
    }
    if settings.addon_installed.is_none() {
        settings.addon_installed = before.addon_installed;
    }
    let wake = settings.auto_contribute && !before.auto_contribute;
    write_settings(&state, settings)?;
    if wake {
        let _ = state.auto_wake.send(());
    }
    Ok(())
}

fn write_settings(state: &AppState, mut settings: Settings) -> CmdResult<()> {
    settings.wow_dir = game_root(&settings.wow_dir);
    let mut current = state.settings.locked();
    // Created meanwhile by a contribution (see `contributor_id`): never lost to an older copy.
    if settings.contributor.is_none() {
        settings.contributor.clone_from(&current.contributor);
    }
    save_settings_file(&state.config_file, &settings)?;
    *current = settings;
    Ok(())
}

/// Turn the automatic contributions on or off (only that setting: the pages showing it hold
/// copies of the others).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn set_auto_contribute(state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    let mut settings = state.settings();
    let wake = on && !settings.auto_contribute;
    settings.auto_contribute = on;
    write_settings(&state, settings)?;
    if wake {
        let _ = state.auto_wake.send(());
    }
    Ok(())
}

fn save_settings_file(file: &Path, settings: &Settings) -> CmdResult<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(file, serde_json::to_string_pretty(settings).map_err(err)?).map_err(err)
}

/// Off the main thread: the window stays responsive while the database is unpacked and read.
#[tauri::command]
async fn get_options(app: AppHandle) -> CmdResult<Options> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        job::options(&state.open_db()?, &overrides(state.edition())?).map_err(err)
    })
    .await
    .map_err(err)?
}

/// Save a planning request for debugging (replay with `fg route --request <file>`).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn export_request(state: State<'_, AppState>, request: PlanRequest) -> CmdResult<String> {
    let dir = state.data_dir.join("debug");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let name = request
        .name
        .clone()
        .unwrap_or_else(|| format!("{}-{}", request.race, request.class));
    let path = dir.join(format!("request-{name}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&request).map_err(err)?).map_err(err)?;
    Ok(path.display().to_string())
}

/// The game's folder (`wow_dir`, else the saved one) and what is wrong with it for the game
/// version the app works on.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn wow_status(state: State<'_, AppState>, wow_dir: Option<PathBuf>) -> game::WowStatus {
    let settings = state.settings();
    game::status(wow_dir.as_deref().unwrap_or(&settings.wow_dir), settings.edition)
}

/// A World of Warcraft folder found on this computer and its game versions' folders.
#[derive(Debug, Serialize)]
struct FoundGame {
    root: PathBuf,
    clients: Vec<String>,
}

/// World of Warcraft folders found on this computer (Battle.net's, the usual places).
#[tauri::command(async)]
fn detect_wow() -> Vec<FoundGame> {
    game::detect()
        .into_iter()
        .map(|root| FoundGame {
            clients: game::client_folders(&root),
            root,
        })
        .collect()
}

/// What the game did with the addon of the game version the app works on: installed by which
/// app version, when it last ran, turned off for some characters, not loaded at all...
#[derive(Debug, Serialize)]
struct AddonState {
    #[serde(flatten)]
    health: game::AddonHealth,
    /// The last install added files the game reads only when it starts: a /reload is not enough.
    restart_needed: bool,
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn addon_health(state: State<'_, AppState>) -> AddonState {
    let settings = state.settings();
    AddonState {
        health: game::addon_health(&settings.wow_dir, settings.edition, ADDON_VERSION),
        restart_needed: *state.restart_needed.locked(),
    }
}

/// The .toc files of the addons the app installs and the files each loads ("<folder>" -> list).
fn loaded_files(addons: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for folder in [
        PROFILE_ADDON,
        GUIDE_ADDON,
        DATA_ADDON,
        fg_route::export::polestar::ADDON,
    ] {
        for entry in std::fs::read_dir(addons.join(folder)).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toc")
                && let Ok(toc) = std::fs::read_to_string(&path)
            {
                out.insert(
                    format!("{folder}/{}", entry.file_name().to_string_lossy()),
                    game::toc_files(&toc),
                );
            }
        }
    }
    out
}

// Guides ---------------------------------------------------------------------------
//
// A guide is a name and its versions: every computation (first one, or computed again with the
// same name) adds a version. The guides go to the game all together (the data addon) or one by
// one (the text of `/fg import`), each in the version its page says to follow; the player picks
// a character's guide in game.
//
//   guides/<slug>/guide.json          name, version to follow
//   guides/<slug>/v<N>/meta.json      summary of the version
//   guides/<slug>/v<N>/request.json   configuration
//   guides/<slug>/v<N>/outcome.json   computed guide

impl AppState {
    /// The guides of the current edition (the app switches edition only when no generation is
    /// running).
    fn library(&self) -> Library<'_, DiskFiles> {
        Library::new(&self.files, self.edition())
    }

    /// Every guide in the version its page says to follow (the latest unless one is chosen),
    /// written in the language it was made in, with when that version was computed and its
    /// number.
    fn followed_guides(&self) -> CmdResult<Vec<(Route, u64, u32)>> {
        self.studio.followed_guides(self, self.edition())
    }
}

/// Results of earlier versions of the app (`results/<id>/`): each becomes a version of the
/// guide of its name, oldest first.
fn migrate_results(state: &AppState) {
    #[derive(Deserialize)]
    struct OldMeta {
        created: u64,
        data: Option<String>,
        #[serde(default)]
        imported: bool,
    }
    let old = state.data_dir.join("results");
    let Ok(dir) = std::fs::read_dir(&old) else { return };
    let mut found: Vec<(OldMeta, PlanRequest, PlanOutcome)> = dir
        .flatten()
        .filter_map(|e| {
            let read = |f: &str| std::fs::read_to_string(e.path().join(f)).ok();
            Some((
                serde_json::from_str(&read("meta.json")?).ok()?,
                serde_json::from_str(&read("request.json")?).ok()?,
                PlanOutcome::from_json(&read("outcome.json")?).ok()?,
            ))
        })
        .collect();
    found.sort_by_key(|(m, _, _)| m.created);
    for (meta, request, outcome) in &found {
        let _ = state.library().add_version(
            request,
            outcome,
            meta.data.clone(),
            Origin {
                imported: meta.imported,
                edited: false,
            },
            meta.created,
        );
    }
    let _ = std::fs::rename(&old, state.data_dir.join("results.old"));
}

/// Runs a command on a worker thread (the planner takes seconds to minutes).
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || work(&app.state::<AppState>()))
        .await
        .map_err(err)?
}

/// Plan a guide on a worker thread, streaming progress as `plan-progress` events. The result
/// is saved as a new version of the guide of the request's name. `stop_plan` ends it early with
/// the best guide found so far; `cancel_plan` makes it fail with "cancelled" (nothing saved).
#[tauri::command]
async fn plan_route(app: AppHandle, request: PlanRequest) -> CmdResult<VersionMeta> {
    blocking(app, move |state| state.studio.plan_route(state, &request)).await
}

/// The flight paths a character probably knows from the quests it did in their towns
/// (`job::inferred_flights`), besides those its addon recorded.
#[tauri::command]
async fn inferred_flights(app: AppHandle, start: StartState) -> CmdResult<Vec<i64>> {
    blocking(app, move |state| state.studio.inferred_flights(state, &start)).await
}

/// The starts the character can choose from before the guide is computed (`job::preview_starts`),
/// streaming `plan-progress` events (`@preview <done> <total>`). Fewer than two: nothing to choose.
/// `cancel_plan` ends it with "cancelled".
#[tauri::command]
async fn preview_starts(app: AppHandle, request: PlanRequest) -> CmdResult<job::StartPreview> {
    blocking(app, move |state| state.studio.preview_starts(state, &request)).await
}

/// A new guide of no steps from the request's character and settings, saved as a version
/// edited by hand: its steps are then added in the editor.
#[tauri::command]
async fn manual_route(app: AppHandle, request: PlanRequest) -> CmdResult<VersionMeta> {
    blocking(app, move |state| state.studio.manual_route(state, &request)).await
}

/// A version replayed to be watched: when each step starts and ends, the character's moves and
/// what it has after each step (`locale`: language of the names).
#[tauri::command]
async fn guide_timeline(app: AppHandle, guide: String, version: u32, locale: String) -> CmdResult<job::Timeline> {
    blocking(app, move |state| {
        state.studio.guide_timeline(state, &guide, version, &locale)
    })
    .await
}

// Guide editor -----------------------------------------------------------------------

/// Open a version in the editor (`locale`: language of the lines and names shown).
#[tauri::command]
async fn edit_open(app: AppHandle, guide: String, version: u32, locale: String) -> CmdResult<EditOpened> {
    blocking(app, move |state| {
        state.studio.edit_open(state, &guide, version, &locale)
    })
    .await
}

/// Replay the steps of the guide open in the editor: its report, its lines and their timeline.
/// The replay under way gives up for this one (it then fails with "superseded").
#[tauri::command]
async fn edit_simulate(app: AppHandle, steps: Vec<RouteStep>, locale: String) -> CmdResult<job::EditView> {
    blocking(app, move |state| state.studio.edit_simulate(state, &steps, &locale)).await
}

/// What could be added to the steps of the guide open in the editor after its step `after`
/// (none: before the first): quests not taken, objectives and turn-ins the steps forget. Gives
/// up for a newer replay or suggestion (failing with "superseded").
#[tauri::command]
async fn edit_suggest(
    app: AppHandle,
    steps: Vec<RouteStep>,
    after: Option<usize>,
    locale: String,
) -> CmdResult<job::Suggestions> {
    blocking(app, move |state| {
        state.studio.edit_suggest(state, &steps, after, &locale)
    })
    .await
}

/// Save the edited steps as a new version of the guide (refused while a step is not possible)
/// and close the editor.
#[tauri::command]
async fn edit_save(app: AppHandle, steps: Vec<RouteStep>) -> CmdResult<VersionMeta> {
    blocking(app, move |state| state.studio.edit_save(state, &steps)).await
}

/// Leave the editor without saving.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn edit_close(state: State<'_, AppState>) {
    state.studio.edit_close();
}

/// End the optimization under way now: `plan_route` saves and returns the best guide so far.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn stop_plan(state: State<'_, AppState>) {
    state.studio.stop_plan();
}

/// Give the generation under way up: `plan_route` fails with "cancelled" within a moment.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn cancel_plan(state: State<'_, AppState>) {
    state.studio.cancel_plan();
}

/// Every guide, most recently computed first.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn list_guides(state: State<'_, AppState>) -> Vec<GuideSummary> {
    state.library().list()
}

#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn get_guide(state: State<'_, AppState>, id: String) -> CmdResult<GuideSummary> {
    Ok(state.library().summary(&id)?)
}

/// Versions `a` and `b` of a guide compared, written in `locale` (game locale).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn compare_versions(
    state: State<'_, AppState>,
    guide: String,
    a: u32,
    b: u32,
    locale: Option<String>,
) -> CmdResult<Comparison> {
    // Compared as saved (zones by their English names), shown in `locale`.
    state.studio.compare_versions(&*state, &guide, a, b, locale.as_deref())
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn get_version(
    state: State<'_, AppState>,
    guide: String,
    version: u32,
    locale: Option<String>,
) -> CmdResult<GuideVersion> {
    state.studio.get_version(&*state, &guide, version, locale.as_deref())
}

/// The version of a guide its characters follow (`None`: the latest), written to the addon.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn set_installed(state: State<'_, AppState>, guide: String, version: Option<u32>) -> CmdResult<()> {
    Ok(state.library().set_installed(&guide, version)?)
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn delete_version(state: State<'_, AppState>, guide: String, version: u32) -> CmdResult<()> {
    Ok(state.library().delete_version(&guide, version)?)
}

#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn delete_guide(state: State<'_, AppState>, guide: String) -> CmdResult<()> {
    Ok(state.library().delete_guide(&guide)?)
}

use job::DATA_ADDON;

/// The oldest addon this app works with (its .toc version): older ones are to be updated.
const ADDON_VERSION: &str = embedded::ADDON_VERSION;

/// Write the guides assigned to a character (in the version they follow) and who follows which,
/// the places of the trainers, vendors... and what the contribution server has (the addon drops
/// those recordings) into the data addon; on WoW Forever with Polestar, its guide pack too. The
/// addon itself comes from an addon site and is set up in game. Returns the number of guides.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn install_addon(state: State<'_, AppState>) -> CmdResult<usize> {
    write_data(&state)
}

/// What `install_addon` does.
fn write_data(state: &AppState) -> CmdResult<usize> {
    // Off the main thread, two writes could touch the same files at once: one at a time.
    static INSTALLING: Mutex<()> = Mutex::new(());
    let _one = INSTALLING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let settings = state.settings();
    match game::status(&settings.wow_dir, settings.edition).problem {
        None => {}
        Some(WowProblem::NotWritable) => {
            return Err(CmdError::App(format!(
                "the app may not write in {}: give your Windows user the right to modify the World of \
                 Warcraft folder (its Properties, Security tab), or start the app once as administrator",
                settings.addons_dir(settings.edition).display()
            )));
        }
        Some(_) => {
            return Err(CmdError::App(format!(
                "{} not found in {}: choose the World of Warcraft folder in Settings",
                settings.edition.rules().edition_name,
                settings.wow_dir.display()
            )));
        }
    }
    let addons = settings.addons_dir(settings.edition);
    let before = loaded_files(&addons);
    migrate_legacy_addon(&addons);
    let guides = state.followed_guides()?;
    let count = guides.len();
    let mut lua = job::guides_lua(
        &state.open_db()?,
        &overrides(settings.edition)?,
        settings.edition,
        guides,
    )
    .map_err(err)?;
    lua.push('\n');
    let sent = read_send_record(&send_record_file(&state.data_dir, settings.edition)).unwrap_or_default();
    lua.push_str(&sent_lua(&sent.state));
    let guide_toc = addons.join(GUIDE_ADDON).join(format!("{GUIDE_ADDON}.toc"));
    let interface = std::fs::read_to_string(&guide_toc)
        .ok()
        .and_then(|toc| toc.lines().find(|l| l.starts_with("## Interface:")).map(str::to_owned))
        .unwrap_or_else(|| embedded::ADDON_INTERFACE.to_owned());
    job::write_data_addon(
        &addons.join(DATA_ADDON),
        &lua,
        &interface,
        env!("CARGO_PKG_VERSION"),
        now()?,
    )
    .map_err(err)?;
    // A folder or a file the game does not load yet: it reads the .toc files when it starts.
    let after = loaded_files(&addons);
    let restart = after.iter().any(|(toc, files)| before.get(toc) != Some(files));
    *state.restart_needed.locked() = restart;
    write_settings(
        state,
        Settings {
            addon_installed: Some(addon_snapshot(&settings)),
            ..settings
        },
    )?;
    Ok(count)
}

/// WoW Forever: every guide (see `followed_guides`) as a guide pack for Polestar, written into
/// the game (Polestar picks each character's guide by its race, class and level). Returns the
/// number of guides.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn install_polestar(state: State<'_, AppState>) -> CmdResult<usize> {
    let settings = state.settings();
    if settings.edition != Edition::Forever {
        return Err("Polestar is for WoW Forever only".into());
    }
    let guides = state
        .followed_guides()?
        .into_iter()
        .map(|(route, _, _)| (job::slug(&route.name), route))
        .collect::<Vec<_>>();
    let pack = settings
        .addons_dir(Edition::Forever)
        .join(fg_route::export::polestar::ADDON);
    let files = fg_route::export::polestar::pack(&guides, env!("CARGO_PKG_VERSION"));
    let _ = std::fs::remove_dir_all(&pack);
    std::fs::create_dir_all(&pack).map_err(err)?;
    for (name, content) in files {
        std::fs::write(pack.join(name), content).map_err(err)?;
    }
    *state.restart_needed.locked() = true;
    Ok(guides.len())
}

/// What the player pastes in game with `/fg import`: one guide version (the addon adds it to
/// the guides it has, or replaces the one of its name).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn guide_text(state: State<'_, AppState>, guide: String, version: u32) -> CmdResult<String> {
    let v = state.library().version(&guide, version)?;
    let edition = v.request.params.as_ref().map_or_else(|| state.edition(), |p| p.edition);
    let route = state.render(&v.outcome.route, None)?;
    let lua = job::guides_lua(
        &state.open_db()?,
        &overrides(edition)?,
        edition,
        vec![(route, v.meta.created, version)],
    )
    .map_err(err)?;
    Ok(fg_route::export::fgstring::encode(
        fg_route::export::fgstring::GUIDE_PREFIX,
        lua.as_bytes(),
    ))
}

/// What the player pastes on Polestar's Import page (`/polestar import`): one guide version, in
/// the game's language. Polestar exists on WoW Forever only.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn polestar_string(
    state: State<'_, AppState>,
    guide: String,
    version: u32,
) -> CmdResult<fg_route::export::polestar::ImportString> {
    if state.edition() != Edition::Forever {
        return Err("Polestar is for WoW Forever only".into());
    }
    let route = state.render(&state.library().version(&guide, version)?.outcome.route, None)?;
    Ok(fg_route::export::polestar::import_string(
        &job::slug(&route.name),
        &route,
        version,
    ))
}

/// Whether Polestar is installed in WoW Forever (it shows guides there only).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn polestar_installed(state: State<'_, AppState>) -> bool {
    polestar_installed_in(&state.settings())
}

fn polestar_installed_in(settings: &Settings) -> bool {
    settings
        .addons_dir(Edition::Forever)
        .join("Polestar/Polestar.toc")
        .exists()
}

/// Export a guide version for Polestar's own tools (route planner, playground): its `.guide`
/// file (in English) added to the pack folder in `dir` (the folder chosen, or a pack folder made
/// in it), the pack's other guides kept. Returns the pack's folder. Polestar exists on WoW
/// Forever only.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn export_polestar(state: State<'_, AppState>, guide: String, version: u32, dir: String) -> CmdResult<String> {
    if state.edition() != Edition::Forever {
        return Err("Polestar is for WoW Forever only".into());
    }
    let route = state.render(&state.library().version(&guide, version)?.outcome.route, Some("enUS"))?;
    let pack = job::polestar_source_dir(Path::new(&dir));
    job::add_polestar_source(&pack, &guide, &route, env!("CARGO_PKG_VERSION")).map_err(err)?;
    Ok(pack.display().to_string())
}

/// Whether the data addon in the game has the current guides (else it is to be written again).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn addon_synced(state: State<'_, AppState>) -> bool {
    let settings = state.settings();
    let addons = settings.addons_dir(settings.edition);
    addons.join(DATA_ADDON).join("Data.lua").exists()
        && settings.addon_installed.as_deref() == Some(addon_snapshot(&settings).as_str())
}

/// Pages the app opens in the browser: the project's (repository, license, notice) and the
/// community tools the contribution page points to; nothing else.
const LINKS: &[&str] = &[
    "https://github.com/brouznouf/factoruide/",
    "https://www.curseforge.com/wow/",
    "https://github.com/Questie/QuestieTrace",
    "https://questie.dev/trace",
    "https://www.wowhead.com/",
];

/// Open one of the `LINKS` pages in the browser.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn open_link(url: String) -> CmdResult<()> {
    if !LINKS.iter().any(|l| url.starts_with(l)) {
        return Err(CmdError::App(format!("not a link the app opens: {url}")));
    }
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program).arg(&url).spawn().map_err(err)?;
    Ok(())
}

/// A Lua string literal.
fn lua_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            // Other control characters as decimal escapes, always 3 digits (a digit may follow).
            c if c.is_ascii_control() => {
                let _ = write!(out, "\\{:03}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The addon was called BrouzQuest, then was a single addon, Factoruide, before its profile and
/// guide parts: drop their folders (the game would load them too) and carry their SavedVariables
/// over to the profile part. `addons`: the client's Interface/AddOns folder.
fn migrate_legacy_addon(addons: &Path) {
    let _ = std::fs::remove_dir_all(addons.join("BrouzQuest"));
    let _ = std::fs::remove_dir_all(addons.join("Factoruide"));
    if let Some(client) = addons.parent().and_then(Path::parent) {
        migrate_saved_variables(client);
    }
}

/// The SavedVariables of the former addons, carried over to the profile part, unless that one
/// has newer ones (a game still running the old addon writes its file until it restarts: carried
/// over again then). The old files are kept as `.migrated`.
fn migrate_saved_variables(client: &Path) {
    let Ok(accounts) = std::fs::read_dir(client.join("WTF/Account")) else {
        return;
    };
    let modified = |f: &Path| std::fs::metadata(f).and_then(|m| m.modified()).ok();
    for account in accounts.flatten() {
        let dir = account.path().join("SavedVariables");
        let (brouzquest, single, profile) = (
            dir.join("BrouzQuest.lua"),
            dir.join("Factoruide.lua"),
            dir.join(format!("{PROFILE_ADDON}.lua")),
        );
        if !single.exists()
            && !profile.exists()
            && let Ok(text) = std::fs::read_to_string(&brouzquest)
            && std::fs::write(&single, text.replace("BrouzQuestDB", "FactoruideDB")).is_ok()
        {
            let _ = std::fs::rename(&brouzquest, dir.join("BrouzQuest.lua.migrated"));
        }
        if single.exists()
            && (!profile.exists() || modified(&single) > modified(&profile))
            && std::fs::copy(&single, &profile).is_ok()
        {
            let _ = std::fs::rename(&single, dir.join("Factoruide.lua.migrated"));
        }
    }
}

/// Write a version as a .fgguide file in the Downloads folder; returns its path.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn export_version(app: AppHandle, state: State<'_, AppState>, guide: String, version: u32) -> CmdResult<String> {
    let shared = state.library().share(&guide, version)?;
    let dir = app.path().download_dir().map_err(err)?;
    let path = dir.join(format!("{guide}-v{version}.fgguide"));
    std::fs::write(&path, shared.to_json()?).map_err(err)?;
    Ok(path.display().to_string())
}

/// Add a guide received as a .fgguide file (its content): a new version of the guide of its name.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn import_guide(state: State<'_, AppState>, content: String) -> CmdResult<VersionMeta> {
    state.studio.import_guide(&*state, &content)
}

// Characters -------------------------------------------------------------------------

/// A character the addon recorded (`/fg profile`, or at each logout), to start a guide from.
#[derive(Serialize)]
struct CharacterInfo {
    /// Full name when the game gives characters one (WoW Forever: first and last name), from the
    /// client's per-character folders (`WTF/Account/<account>/<realm>/<First>-<Last>/`).
    full_name: Option<String>,
    /// Race key of `races.toml` (None: a race this game version does not know).
    race: Option<String>,
    class: String,
    /// Professions it has, by key, with their skill.
    professions: Vec<(String, i64)>,
    /// Another player's, received in a party (`/fg share`): for group guides, not played here.
    shared: bool,
    profile: StartState,
}

/// Rested XP gained while logged out since the profile was saved: 5% of a level every 8 hours
/// in an inn or a city, a quarter of it elsewhere, up to a level and a half.
fn rested_now(p: &StartState, now: i64) -> i64 {
    if p.xp_max <= 0 || p.time <= 0 {
        return p.rested;
    }
    let hours = (now - p.time).max(0) as f64 / 3600.0;
    let per_hour = 0.05 / 8.0 * p.xp_max as f64 * if p.resting { 1.0 } else { 0.25 };
    let cap = p.xp_max * 3 / 2;
    (p.rested + (hours * per_hour) as i64).min(cap.max(p.rested))
}

/// The characters of the game client of the current edition, most recently played first.
// Off the main thread: reading the SavedVariables can take a while.
#[tauri::command]
async fn list_characters(app: AppHandle) -> CmdResult<Vec<CharacterInfo>> {
    tauri::async_runtime::spawn_blocking(move || characters(&app.state::<AppState>()))
        .await
        .map_err(err)?
}

/// Full names of the characters by first name, from the per-character folders of the accounts
/// of these SavedVariables files (`<account>/<realm>/<First>-<Last>/`).
fn full_names(files: &[PathBuf]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for account in files.iter().filter_map(|f| f.parent()?.parent()) {
        for realm in std::fs::read_dir(account).into_iter().flatten().flatten() {
            for character in std::fs::read_dir(realm.path()).into_iter().flatten().flatten() {
                let folder = character.file_name().to_string_lossy().into_owned();
                if let Some((first, last)) = folder.split_once('-')
                    && character.path().is_dir()
                {
                    let full = format!("{first} {last}");
                    let names = out.entry(first.to_owned()).or_default();
                    if !names.contains(&full) {
                        names.push(full);
                    }
                }
            }
        }
    }
    out
}

/// Key of a character in `Settings::forgotten_characters`.
fn forgotten_key(edition: Edition, character: &str) -> String {
    format!("{}/{character}", edition.key())
}

/// Whether a character's profile was taken out of the list (and not played since).
fn is_forgotten(forgotten: &BTreeMap<String, i64>, edition: Edition, profile: &StartState) -> bool {
    forgotten
        .get(&forgotten_key(edition, &profile.name))
        .is_some_and(|&time| profile.time <= time)
}

/// Take a character out of the Characters page (deleted in game): it no longer follows a guide.
/// Played again, it is back.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn forget_character(state: State<'_, AppState>, character: String) -> CmdResult<()> {
    let time = characters(&state)?
        .into_iter()
        .find(|c| c.profile.name == character)
        .map_or(i64::MAX, |c| c.profile.time);
    let mut settings = state.settings();
    settings
        .forgotten_characters
        .insert(forgotten_key(settings.edition, &character), time);
    write_settings(&state, settings)
}

fn characters(state: &AppState) -> CmdResult<Vec<CharacterInfo>> {
    let settings = state.settings();
    let client_dir = settings.edition.client_dir(&settings.wow_dir);
    let files = fg_contrib::collector::find_saved_variables(&client_dir);
    let full_names = full_names(&files);
    let races = overrides(settings.edition)?.races;
    let now = i64::try_from(now()?).map_err(err)?;
    let profiles: Vec<(bool, StartState)> = fg_contrib::profile::read(&files)
        .map_err(err)?
        .into_iter()
        .filter_map(|json| {
            let shared = json["shared"].as_bool().unwrap_or(false);
            Some((shared, serde_json::from_value::<StartState>(json).ok()?))
        })
        .collect();
    // A character of this computer is not also listed as received from another player.
    let own: std::collections::HashSet<String> = (profiles.iter())
        .filter(|(shared, _)| !shared)
        .map(|(_, p)| p.name.clone())
        .collect();
    Ok(profiles
        .into_iter()
        .filter(|(shared, p)| !shared || !own.contains(&p.name))
        .filter(|(_, profile)| !is_forgotten(&settings.forgotten_characters, settings.edition, profile))
        .map(|(shared, mut profile)| {
            profile.rested = rested_now(&profile, now);
            // Only when the first name is not shared (two characters with the same one).
            let first = profile.name.split('-').next().unwrap_or_default();
            let full_name = match full_names.get(first).map(Vec::as_slice) {
                Some([one]) => Some(one.clone()),
                _ => None,
            };
            CharacterInfo {
                full_name,
                race: races
                    .iter()
                    .find(|(_, r)| r.id == profile.race)
                    .map(|(key, _)| key.clone()),
                class: profile.class.to_lowercase(),
                professions: profile
                    .professions
                    .iter()
                    .filter_map(|s| {
                        let def = fg_route::profession::PROFESSIONS
                            .iter()
                            .find(|d| d.skill_id == s.line)?;
                        Some((def.key.to_owned(), s.rank))
                    })
                    .collect(),
                shared,
                profile,
            }
        })
        .collect())
}

// Maps -----------------------------------------------------------------------------

/// Map index written by `fg maps` (ids, names, sizes, world bounds).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn map_index(state: State<'_, AppState>) -> CmdResult<serde_json::Value> {
    let db = state.open_db()?;
    let text: String = db
        .query_row("SELECT value FROM app_meta WHERE key = 'map_index'", [], |r| r.get(0))
        .map_err(err)?;
    let mut index: serde_json::Value = serde_json::from_str(&text).map_err(err)?;
    // Zone names in the game's languages (`names`: locale -> name), when the data has them.
    let mut names: HashMap<i64, serde_json::Map<String, serde_json::Value>> = HashMap::new();
    if let Ok(mut stmt) =
        db.prepare("SELECT id, locale, name FROM l10n WHERE entity_type = 'zone' AND name IS NOT NULL")
    {
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })
            .map_err(err)?;
        for (id, locale, name) in rows.flatten() {
            names.entry(id).or_default().insert(locale, name.into());
        }
    }
    for map in index.as_array_mut().into_iter().flatten() {
        if let Some(n) = map["area"].as_i64().and_then(|a| names.get(&a)) {
            map["names"] = serde_json::Value::Object(n.clone());
        }
    }
    Ok(index)
}

/// A map image as a data URL.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn map_image(state: State<'_, AppState>, id: i64) -> CmdResult<tauri::ipc::Response> {
    let name = format!("{}/{id}.webp", state.edition().key());
    let (_, bytes) = embedded::MAPS
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("no map image {name}"))?;
    Ok(tauri::ipc::Response::new(bytes.to_vec()))
}

/// An edition's minimap pack (`fg minimap`): its index, `(map, level, x, y)` -> (offset, length)
/// in the images after it.
struct Minimap {
    tiles: HashMap<(i64, u32, u32, u32), (usize, usize)>,
    images: &'static [u8],
}

fn minimap(edition: Edition) -> Option<&'static Minimap> {
    static PACKS: OnceLock<HashMap<&'static str, Minimap>> = OnceLock::new();
    let packs = PACKS.get_or_init(|| {
        embedded::MINIMAPS
            .iter()
            .filter_map(|&(edition, pack)| {
                let len = u32::from_le_bytes(pack.get(4..8)?.try_into().ok()?) as usize;
                if pack.get(..4)? != b"FGMM" {
                    return None;
                }
                let index: Vec<(i64, u32, u32, u32, usize, usize)> =
                    serde_json::from_slice(pack.get(8..8 + len)?).ok()?;
                let tiles = index
                    .into_iter()
                    .map(|(m, l, x, y, o, n)| ((m, l, x, y), (o, n)))
                    .collect();
                Some((
                    edition,
                    Minimap {
                        tiles,
                        images: &pack[8 + len..],
                    },
                ))
            })
            .collect()
    });
    packs.get(edition.key())
}

/// The minimap's images of the current game version: `[map, level, x, y]` of each (none
/// without a minimap).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn minimap_index(state: State<'_, AppState>) -> Vec<(i64, u32, u32, u32)> {
    let mut tiles: Vec<_> = minimap(state.edition())
        .map(|m| m.tiles.keys().copied().collect())
        .unwrap_or_default();
    tiles.sort_unstable();
    tiles
}

/// A minimap image (WebP): of game map `map`, at `level` (2^level ADT tiles a side), the
/// `x`-th from the west and `y`-th from the north.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "tauri commands take their arguments by value"
)]
fn minimap_image(state: State<'_, AppState>, map: i64, level: u32, x: u32, y: u32) -> CmdResult<tauri::ipc::Response> {
    let m = minimap(state.edition()).ok_or("no minimap")?;
    let &(offset, len) = m
        .tiles
        .get(&(map, level, x, y))
        .ok_or_else(|| format!("no minimap image {map}/{level}/{x}/{y}"))?;
    let bytes = m.images.get(offset..offset + len).ok_or("minimap pack truncated")?;
    Ok(tauri::ipc::Response::new(bytes.to_vec()))
}

/// WebKitGTK renders a blank window under WSLg (no usable GPU for DMA-BUF/EGL),
/// and its DMA-BUF renderer dies with "Error 71 (Protocol error)" on Wayland with
/// the NVIDIA driver: fall back to its software paths in those cases.
#[expect(
    unsafe_code,
    reason = "set_var is unsafe in edition 2024; this runs before any thread is spawned"
)]
fn linux_rendering_workaround() {
    let in_wsl = std::fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"));
    let vars: &[&str] = if in_wsl {
        &["WEBKIT_DISABLE_DMABUF_RENDERER", "WEBKIT_DISABLE_COMPOSITING_MODE"]
    } else if Path::new("/proc/driver/nvidia").exists() {
        &["WEBKIT_DISABLE_DMABUF_RENDERER"]
    } else {
        &[]
    };
    for var in vars {
        if std::env::var_os(var).is_none() {
            // SAFETY: called at startup, before any other thread exists.
            unsafe { std::env::set_var(var, "1") };
        }
    }
}

fn main() {
    if cfg!(target_os = "linux") {
        linux_rendering_workaround();
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let menu = tauri::menu::Menu::default(app.handle())?;
                // Tauri's default macOS menu begins with the application submenu.
                if let Some(app_menu) = menu.items()?.first().and_then(tauri::menu::MenuItemKind::as_submenu) {
                    let check = tauri::menu::MenuItem::with_id(
                        app,
                        "check-for-updates",
                        "Check for Updates…",
                        true,
                        None::<&str>,
                    )?;
                    app_menu.insert(&check, 1)?;
                }
                app.set_menu(menu)?;
                app.on_menu_event(|app, event| {
                    if event.id().as_ref() == "check-for-updates"
                        && let Some(window) = app.get_webview_window("main")
                    {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        let _ = window.emit("check-for-updates", ());
                    }
                });
            }
            migrate_app_dirs(app.handle());
            let config_file = app.path().app_config_dir()?.join("settings.json");
            let data_dir = app.path().app_data_dir()?;
            let config: Option<serde_json::Value> = std::fs::read_to_string(&config_file)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());
            if let Some(config) = &config {
                migrate_workspace(config, &data_dir);
            }
            let first_launch = config.is_none();
            let mut settings: Settings = config.and_then(|c| serde_json::from_value(c).ok()).unwrap_or_default();
            // A version's folder chosen by older versions of the app: the game's folder above.
            settings.wow_dir = game_root(&settings.wow_dir);
            if first_launch {
                find_game(&mut settings);
                // Found once: not looked for again at every launch.
                let _ = save_settings_file(&config_file, &settings);
            }
            let db_dir = app.path().app_local_data_dir()?;
            let (auto_wake, wake) = std::sync::mpsc::channel();
            app.manage(AppState {
                settings: Mutex::new(settings),
                restart_needed: Mutex::new(false),
                config_file,
                db_dir,
                dbs: Mutex::new(HashMap::default()),
                files: DiskFiles::new(data_dir.clone()),
                data_dir,
                studio: Studio::default(),
                app: OnceLock::new(),
                send_lock: Mutex::new(()),
                auto_wake,
            });
            // The events of a generation go to this window.
            let _ = app.state::<AppState>().app.set(app.handle().clone());
            migrate_guides(&app.state::<AppState>().data_dir);
            migrate_results(&app.state::<AppState>());
            let handle = app.handle().clone();
            std::thread::spawn(move || auto_contribute_loop(&handle, &wake));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            updates_enabled,
            get_settings,
            set_settings,
            get_options,
            plan_route,
            inferred_flights,
            preview_starts,
            manual_route,
            stop_plan,
            cancel_plan,
            export_request,
            install_addon,
            addon_synced,
            polestar_installed,
            export_polestar,
            polestar_string,
            open_link,
            wow_status,
            detect_wow,
            setup_report,
            addon_health,
            list_editions,
            set_edition,
            contribution_preview,
            send_contribution,
            send_issue,
            need_help,
            set_auto_contribute,
            list_guides,
            get_guide,
            get_version,
            compare_versions,
            set_installed,
            delete_version,
            delete_guide,
            guide_text,
            install_polestar,
            forget_character,
            export_version,
            import_guide,
            map_index,
            map_image,
            minimap_index,
            minimap_image,
            list_characters,
            guide_timeline,
            edit_open,
            edit_simulate,
            edit_suggest,
            edit_save,
            edit_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Factoruide");
}

/// At the first launch: the game where it was found, and a game version installed there (WoW
/// Forever first) when the default one is not.
fn find_game(settings: &mut Settings) {
    if matches!(
        game::status(&settings.wow_dir, settings.edition).problem,
        Some(WowProblem::Missing | WowProblem::NoGame)
    ) && let Some(found) = game::detect().into_iter().next()
    {
        settings.wow_dir = found;
    }
    let installed = |e: Edition| e.installed_in(&settings.wow_dir);
    if !installed(settings.edition)
        && let Some(e) = Edition::ALL.iter().copied().find(|e| installed(*e))
    {
        settings.edition = e;
    }
}

/// Switch the edition the app works on (saved with the settings).
#[tauri::command]
fn set_edition(state: State<'_, AppState>, edition: Edition) -> CmdResult<()> {
    let mut settings = state.settings();
    settings.edition = edition;
    set_settings(state, settings)
}

/// Guides made before editions existed (`guides/<slug>/`) are WoW Forever guides.
/// The app was called BrouzQuest (identifier fr.brouz.brouzquest): move its settings, guides and
/// databases to the new identifier's folders, unless these already exist.
fn migrate_app_dirs(app: &AppHandle) {
    let path = app.path();
    for dir in [path.app_config_dir(), path.app_data_dir(), path.app_local_data_dir()]
        .into_iter()
        .flatten()
    {
        let Some(old) = dir.parent().map(|p| p.join("fr.brouz.brouzquest")) else {
            continue;
        };
        if old.is_dir() && !dir.exists() {
            let _ = std::fs::rename(&old, &dir);
        }
    }
}

fn migrate_guides(data_dir: &Path) {
    let root = data_dir.join("guides");
    let Ok(dir) = std::fs::read_dir(&root) else { return };
    for entry in dir.flatten() {
        if entry.path().join("guide.json").is_file() {
            let dest = root.join(Edition::Forever.key());
            let _ = std::fs::create_dir_all(&dest);
            let _ = std::fs::rename(entry.path(), dest.join(entry.file_name()));
        }
    }
}
