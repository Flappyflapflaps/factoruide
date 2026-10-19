//! The app's calls, as the desktop app's backend answers them (app/src-tauri/src/main.rs), on
//! the files of `store` instead of the app's folders and with the addon reached through texts:
//! the guide import the player pastes in game (`addon_export`), the character export pasted in
//! the app (`import_profile`).

use crate::{embedded, fgstring, host, store};
use fg_route::Route;
use fg_route::export::diff::{self, Change, RouteDiff};
use fg_route::job::{self, EditCatalog, Overrides, PlanOutcome, PlanRequest, RouteStep, StartState};
use fg_route::model::Names;
use fg_route::terrain::{WalkMemory, Walks};
use fg_route::xp::Edition;
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::{Arc, LazyLock, Mutex};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

/// The app's version: the release's (`FG_VERSION` at build time), else the crate's.
const APP_VERSION: &str = match option_env!("FG_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

// Settings -----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    /// Kept for the pages written for the desktop app: the browser has no game folder.
    #[serde(default)]
    wow_dir: String,
    #[serde(default)]
    edition: Edition,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    all_guides: bool,
    /// What the last text for the game was made with (`guides_stamp`).
    #[serde(default)]
    addon_installed: Option<String>,
    #[serde(default)]
    welcome_done: bool,
    #[serde(default)]
    forgotten_characters: BTreeMap<String, i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wow_dir: String::new(),
            edition: Edition::Forever,
            language: None,
            all_guides: false,
            addon_installed: None,
            welcome_done: false,
            forgotten_characters: BTreeMap::new(),
        }
    }
}

const SETTINGS_FILE: &str = "settings.json";

static SETTINGS: LazyLock<Mutex<Settings>> = LazyLock::new(|| {
    Mutex::new(
        store::read(SETTINGS_FILE)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default(),
    )
});

fn settings() -> Settings {
    SETTINGS.lock().unwrap().clone()
}

fn edition() -> Edition {
    SETTINGS.lock().unwrap().edition
}

fn write_settings(settings: Settings) -> CmdResult<()> {
    let mut current = SETTINGS.lock().unwrap();
    store::write(SETTINGS_FILE, serde_json::to_vec_pretty(&settings).map_err(err)?);
    *current = settings;
    Ok(())
}

/// What the text for the game depends on: who follows which guide version.
fn addon_snapshot(edition: Edition) -> String {
    serde_json::to_string(&guides_stamp(edition)).unwrap_or_default()
}

// Game data ----------------------------------------------------------------------------

/// The databases fetched so far, unpacked (kept for good: each connection reads them in place).
static DATABASES: LazyLock<Mutex<HashMap<Edition, &'static [u8]>>> = LazyLock::new(Mutex::default);

/// An edition's database, fetched and unpacked the first time (`data/<edition>.sqlite.zst`).
fn database(edition: Edition) -> CmdResult<&'static [u8]> {
    let mut dbs = DATABASES.lock().unwrap();
    if let Some(db) = dbs.get(&edition) {
        return Ok(db);
    }
    let url = format!("data/{}.sqlite.zst", edition.key());
    let answer = host::fetch("GET", &url, None, &[])?;
    if answer.status != 200 {
        return Err(format!(
            "no data for {} ({url}: {})",
            edition.rules().edition_name,
            answer.status
        ));
    }
    let bytes = zstd::decode_all(answer.body.as_slice()).map_err(err)?;
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    dbs.insert(edition, bytes);
    Ok(bytes)
}

fn open_db(edition: Edition) -> CmdResult<Connection> {
    let bytes = database(edition)?;
    let mut conn = Connection::open_in_memory().map_err(err)?;
    conn.deserialize_bytes("main", bytes).map_err(err)?;
    // No folder for temporary files in the browser: sorts and temporary tables in memory.
    conn.pragma_update(None, "temp_store", "MEMORY").map_err(err)?;
    Ok(conn)
}

fn overrides(edition: Edition) -> CmdResult<Overrides> {
    Overrides::parse_edition(override_file, edition).map_err(err)
}

fn override_file(name: &str) -> Option<String> {
    let (_, bytes) = embedded::OVERRIDES.iter().find(|(n, _)| *n == name)?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// Names of an edition in a game language.
type NamesCache = HashMap<(Edition, String), Arc<Names>>;
static NAMES: LazyLock<Mutex<NamesCache>> = LazyLock::new(Mutex::default);

fn names(edition: Edition, locale: &str) -> CmdResult<Arc<Names>> {
    let key = (edition, locale.to_owned());
    if let Some(n) = NAMES.lock().unwrap().get(&key) {
        return Ok(n.clone());
    }
    let names = Arc::new(Names::load(&open_db(edition)?, locale).map_err(err)?);
    NAMES.lock().unwrap().insert(key, names.clone());
    Ok(names)
}

fn render(edition: Edition, route: &Route, locale: Option<&str>) -> CmdResult<Route> {
    let locale = locale.or(route.locale.as_deref()).unwrap_or("enUS");
    Ok(fg_route::export::render(route, &*names(edition, locale)?))
}

fn data_version(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT detail FROM import_run WHERE source = 'questie'", [], |r| {
        r.get::<_, Option<String>>(0)
    })
    .ok()
    .flatten()
}

fn now() -> CmdResult<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_secs())
}

#[derive(Serialize)]
struct EditionInfo {
    key: &'static str,
    name: &'static str,
    available: bool,
    /// The browser cannot tell which games are installed: every one.
    installed: bool,
    max_level: i64,
}

fn list_editions() -> Vec<EditionInfo> {
    Edition::ALL
        .iter()
        .map(|e| EditionInfo {
            key: e.key(),
            name: e.rules().edition_name,
            available: e.rules().known(),
            installed: true,
            max_level: e.rules().max_level,
        })
        .collect()
}

// Guides -------------------------------------------------------------------------------
//
//   guides/<edition>/characters.json          "Name-Realm" -> guide (slug)
//   guides/<edition>/<slug>/guide.json        name, version the characters follow
//   guides/<edition>/<slug>/v<N>/meta.json    summary of the version
//   guides/<edition>/<slug>/v<N>/request.json configuration
//   guides/<edition>/<slug>/v<N>/outcome.json computed guide

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GuideInfo {
    name: String,
    #[serde(default)]
    installed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VersionMeta {
    guide: String,
    version: u32,
    name: String,
    created: u64,
    race: String,
    class: String,
    #[serde(default)]
    group: Vec<String>,
    from_level: i64,
    to_level: i64,
    total_time: f64,
    total_time_text: String,
    quests: usize,
    data: Option<String>,
    #[serde(default)]
    imported: bool,
    #[serde(default)]
    edited: bool,
    #[serde(default)]
    character: Option<String>,
}

#[derive(Serialize)]
struct GuideSummary {
    id: String,
    name: String,
    installed: Option<u32>,
    versions: Vec<VersionMeta>,
}

#[derive(Serialize, Deserialize)]
struct GuideVersion {
    meta: VersionMeta,
    request: PlanRequest,
    outcome: PlanOutcome,
}

fn guides_dir(edition: Edition) -> String {
    format!("guides/{}", edition.key())
}

fn guide_info(edition: Edition, id: &str) -> CmdResult<GuideInfo> {
    serde_json::from_str(&store::read_string(&format!(
        "{}/{id}/guide.json",
        guides_dir(edition)
    ))?)
    .map_err(err)
}

fn write_guide_info(edition: Edition, id: &str, info: &GuideInfo) -> CmdResult<()> {
    store::write(
        &format!("{}/{id}/guide.json", guides_dir(edition)),
        serde_json::to_vec_pretty(info).map_err(err)?,
    );
    Ok(())
}

fn versions(edition: Edition, id: &str) -> Vec<VersionMeta> {
    let dir = format!("{}/{id}", guides_dir(edition));
    let mut out: Vec<VersionMeta> = store::list(&dir)
        .into_iter()
        .filter(|n| n.starts_with('v'))
        .filter_map(|v| serde_json::from_slice(&store::read(&format!("{dir}/{v}/meta.json"))?).ok())
        .collect();
    out.sort_by_key(|v| std::cmp::Reverse(v.version));
    out
}

fn version(edition: Edition, id: &str, version: u32) -> CmdResult<GuideVersion> {
    let folder = format!("{}/{id}/v{version}", guides_dir(edition));
    let read = |f: &str| store::read_string(&format!("{folder}/{f}"));
    Ok(GuideVersion {
        meta: serde_json::from_str(&read("meta.json")?).map_err(err)?,
        request: serde_json::from_str(&read("request.json")?).map_err(err)?,
        outcome: PlanOutcome::from_json(&read("outcome.json")?).map_err(err)?,
    })
}

/// The edition a version was made for (the current one when its request does not say).
fn request_edition(request: &PlanRequest) -> Edition {
    request.params.as_ref().map_or_else(edition, |p| p.edition)
}

static WALKS: LazyLock<Mutex<HashMap<Edition, Arc<Walks>>>> = LazyLock::new(Mutex::default);

/// The walking memory of an edition: the first time, the memories of all its guides.
fn walks(edition: Edition) -> Arc<Walks> {
    #[derive(Deserialize)]
    struct Saved {
        #[serde(default)]
        walks: Option<WalkMemory>,
    }
    if let Some(w) = WALKS.lock().unwrap().get(&edition) {
        return Arc::clone(w);
    }
    let walks = Arc::new(Walks::default());
    let dir = guides_dir(edition);
    for guide in store::list(&dir) {
        for v in store::list(&format!("{dir}/{guide}")) {
            let memory = store::read(&format!("{dir}/{guide}/{v}/outcome.json"))
                .and_then(|json| serde_json::from_slice::<Saved>(&json).ok())
                .and_then(|saved| saved.walks);
            if let Some(memory) = memory {
                walks.add(memory);
            }
        }
    }
    Arc::clone(WALKS.lock().unwrap().entry(edition).or_insert(walks))
}

fn add_version(
    edition: Edition,
    request: &PlanRequest,
    outcome: &PlanOutcome,
    data: Option<String>,
    (imported, edited): (bool, bool),
    created: u64,
) -> CmdResult<VersionMeta> {
    let route = &outcome.route;
    let id = job::slug(&route.name);
    let info = guide_info(edition, &id).unwrap_or(GuideInfo {
        name: route.name.clone(),
        installed: None,
    });
    write_guide_info(
        edition,
        &id,
        &GuideInfo {
            name: route.name.clone(),
            ..info
        },
    )?;
    let version = versions(edition, &id).first().map_or(1, |v| v.version + 1);
    let meta = VersionMeta {
        guide: id.clone(),
        version,
        name: route.name.clone(),
        created,
        race: request.race.clone(),
        class: request.class.clone(),
        group: request.group.clone(),
        from_level: route.from_level,
        to_level: route.to_level,
        total_time: route.total_time,
        total_time_text: route.total_time_text.clone(),
        quests: route.quests,
        data,
        imported,
        edited,
        character: request.start.as_ref().map(|s| s.name.clone()),
    };
    let folder = format!("{}/{id}/v{version}", guides_dir(edition));
    store::write(
        &format!("{folder}/meta.json"),
        serde_json::to_vec_pretty(&meta).map_err(err)?,
    );
    store::write(
        &format!("{folder}/request.json"),
        serde_json::to_vec_pretty(request).map_err(err)?,
    );
    store::write(
        &format!("{folder}/outcome.json"),
        serde_json::to_vec(outcome).map_err(err)?,
    );
    Ok(meta)
}

fn assignments(edition: Edition) -> BTreeMap<String, String> {
    store::read(&format!("{}/characters.json", guides_dir(edition)))
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_assignments(edition: Edition, assignments: &BTreeMap<String, String>) -> CmdResult<()> {
    store::write(
        &format!("{}/characters.json", guides_dir(edition)),
        serde_json::to_vec_pretty(assignments).map_err(err)?,
    );
    Ok(())
}

fn followed_version(edition: Edition, id: &str) -> Option<u32> {
    let chosen = guide_info(edition, id).ok()?.installed;
    chosen.or_else(|| versions(edition, id).first().map(|v| v.version))
}

/// The guides assigned to a character, in the version they follow, by guide.
fn assigned_routes(edition: Edition) -> BTreeMap<String, Route> {
    let mut routes = BTreeMap::new();
    for id in assignments(edition).into_values() {
        if routes.contains_key(&id) {
            continue;
        }
        if let Some(route) = followed_version(edition, &id)
            .and_then(|v| version(edition, &id, v).ok())
            .map(|v| v.outcome.route)
        {
            routes.insert(id, route);
        }
    }
    routes
}

/// What the guides in the import depend on: who follows which version (made when it was).
fn guides_stamp(edition: Edition) -> Vec<(String, String, u32, u64)> {
    assignments(edition)
        .into_iter()
        .filter_map(|(character, id)| {
            let v = followed_version(edition, &id)?;
            let created = versions(edition, &id).into_iter().find(|m| m.version == v)?.created;
            Some((character, id, v, created))
        })
        .collect()
}

fn summary(edition: Edition, id: &str) -> CmdResult<GuideSummary> {
    let info = guide_info(edition, id)?;
    Ok(GuideSummary {
        id: id.to_owned(),
        name: info.name,
        installed: info.installed,
        versions: versions(edition, id),
    })
}

fn list_guides() -> Vec<GuideSummary> {
    let edition = edition();
    let mut out: Vec<GuideSummary> = store::list(&guides_dir(edition))
        .into_iter()
        .filter_map(|id| summary(edition, &id).ok())
        .filter(|g| !g.versions.is_empty())
        .collect();
    out.sort_by(|a, b| b.versions[0].created.cmp(&a.versions[0].created));
    out
}

#[derive(Serialize)]
struct Comparison {
    a: GuideVersion,
    b: GuideVersion,
    diff: RouteDiff,
    settings: Vec<Change>,
    text: String,
}

fn compare_versions(guide: &str, a: u32, b: u32, locale: Option<&str>) -> CmdResult<Comparison> {
    let edition = edition();
    let load = |n: u32| -> CmdResult<GuideVersion> {
        let mut v = version(edition, guide, n)?;
        v.outcome.walks = None;
        v.outcome.plan.clear();
        Ok(v)
    };
    let (mut a, mut b) = (load(a)?, load(b)?);
    let settings = |v: &GuideVersion| -> CmdResult<serde_json::Value> {
        let mut value = serde_json::to_value(&v.request).map_err(err)?;
        if let serde_json::Value::Object(map) = &mut value {
            map.insert("data".into(), v.meta.data.clone().into());
        }
        Ok(value)
    };
    let settings = diff::value_changes(&settings(&a)?, &settings(&b)?);
    let d = diff::diff(&a.outcome.route, &b.outcome.route);
    let labels = [format!("v{}", a.meta.version), format!("v{}", b.meta.version)];
    let text = diff::to_text(
        &d,
        &a.outcome.route,
        &b.outcome.route,
        [&labels[0], &labels[1]],
        &settings,
    );
    if let Some(locale) = locale {
        a.outcome.route = render(edition, &a.outcome.route, Some(locale))?;
        b.outcome.route = render(edition, &b.outcome.route, Some(locale))?;
    }
    Ok(Comparison {
        a,
        b,
        diff: d,
        settings,
        text,
    })
}

fn get_version(guide: &str, n: u32, locale: Option<&str>) -> CmdResult<GuideVersion> {
    let edition = edition();
    let mut v = version(edition, guide, n)?;
    v.outcome.walks = None;
    v.outcome.plan.clear();
    if let Some(locale) = locale {
        v.outcome.route = render(edition, &v.outcome.route, Some(locale))?;
    }
    Ok(v)
}

fn set_installed(guide: &str, v: Option<u32>) -> CmdResult<usize> {
    let edition = edition();
    let info = guide_info(edition, guide)?;
    write_guide_info(edition, guide, &GuideInfo { installed: v, ..info })?;
    install_addon()
}

fn delete_version(guide: &str, v: u32) -> CmdResult<()> {
    let edition = edition();
    store::remove_dir(&format!("{}/{guide}/v{v}", guides_dir(edition)));
    let info = guide_info(edition, guide)?;
    if versions(edition, guide).is_empty() {
        delete_guide(guide)
    } else {
        if info.installed == Some(v) {
            write_guide_info(
                edition,
                guide,
                &GuideInfo {
                    installed: None,
                    ..info
                },
            )?;
        }
        if assignments(edition).values().any(|g| g == guide) {
            install_addon()?;
        }
        Ok(())
    }
}

fn delete_guide(guide: &str) -> CmdResult<()> {
    let edition = edition();
    if !store::remove_dir(&format!("{}/{guide}", guides_dir(edition))) {
        return Err(format!("no guide {guide}"));
    }
    let mut assigned = assignments(edition);
    let before = assigned.len();
    assigned.retain(|_, g| g != guide);
    if assigned.len() != before {
        write_assignments(edition, &assigned)?;
        install_addon()?;
    }
    Ok(())
}

fn assign_guide(character: String, guide: Option<String>) -> CmdResult<usize> {
    let edition = edition();
    let mut assigned = assignments(edition);
    match guide {
        Some(guide) => {
            guide_info(edition, &guide)?;
            assigned.insert(character, guide);
        }
        None => {
            assigned.remove(&character);
        }
    }
    write_assignments(edition, &assigned)?;
    install_addon()
}

// The text for the game -------------------------------------------------------------------

/// What the player pastes in game with `/fg import`: the guides their characters follow (each
/// in the version it follows, in the language it was made in), who follows which, and the
/// places of the trainers, vendors... (the Lua the desktop app writes in Factoruide_Data).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AddonExport {
    text: String,
    guides: usize,
}

fn addon_export_file(edition: Edition) -> String {
    format!("addon-export-{}.json", edition.key())
}

/// A Lua string literal.
fn lua_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if c.is_ascii_control() => {
                let _ = write!(out, "\\{:03}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The text for the game, made again and saved; the settings note it was made.
fn install_addon() -> CmdResult<usize> {
    static MAKING: Mutex<()> = Mutex::new(());
    let _one = MAKING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let settings = settings();
    let edition = settings.edition;
    let assigned = assigned_routes(edition);
    let mut routes = assigned
        .values()
        .map(|r| render(edition, r, None))
        .collect::<CmdResult<Vec<_>>>()?;
    routes.sort_by(|a, b| a.name.cmp(&b.name));
    let locale = routes
        .first()
        .and_then(|r| r.locale.clone())
        .unwrap_or_else(|| "enUS".into());
    let services = job::addon_walks(
        &open_db(edition)?,
        &overrides(edition)?,
        edition,
        &mut routes,
        &*names(edition, &locale)?,
    )
    .map_err(err)?;
    let mut lua = fg_route::export::to_lua(&routes);
    lua.push_str("\n-- Which guide each character follows (assigned in the app).\nFG.Assignments = {\n");
    for (character, guide) in assignments(edition) {
        if let Some(route) = assigned.get(&guide) {
            let _ = writeln!(lua, "  [{}] = {},", lua_string(&character), lua_string(&route.name));
        }
    }
    lua.push_str("}\n");
    lua.push_str(&services);
    let export = AddonExport {
        text: fgstring::encode(fgstring::GUIDE_PREFIX, lua.as_bytes()),
        guides: routes.len(),
    };
    store::write(&addon_export_file(edition), serde_json::to_vec(&export).map_err(err)?);
    write_settings(Settings {
        addon_installed: Some(addon_snapshot(edition)),
        ..settings
    })?;
    Ok(export.guides)
}

/// The text for the game made last (made now when there is none, or it is out of date).
fn addon_export() -> CmdResult<AddonExport> {
    let edition = edition();
    if !addon_synced() {
        install_addon()?;
    }
    let bytes = store::read(&addon_export_file(edition)).ok_or("no text for the game made yet")?;
    serde_json::from_slice(&bytes).map_err(err)
}

/// Whether the last text for the game has the current guides.
fn addon_synced() -> bool {
    let settings = settings();
    let edition = settings.edition;
    store::exists(&addon_export_file(edition))
        && settings.addon_installed.as_deref() == Some(addon_snapshot(edition).as_str())
}

/// WoW Forever: the guides the characters follow as a guide pack for Polestar, a zip of its
/// addon folder to unzip in the game's AddOns folder.
fn polestar_pack() -> CmdResult<serde_json::Value> {
    if edition() != Edition::Forever {
        return Err("Polestar is for WoW Forever only".into());
    }
    let guides = assigned_routes(Edition::Forever)
        .iter()
        .map(|(id, r)| Ok((id.clone(), render(Edition::Forever, r, None)?)))
        .collect::<CmdResult<Vec<_>>>()?;
    let folder = fg_route::export::polestar::ADDON;
    let files: Vec<(String, Vec<u8>)> = fg_route::export::polestar::pack(&guides, APP_VERSION)
        .into_iter()
        .map(|(name, content)| (format!("{folder}/{name}"), content.into_bytes()))
        .collect();
    Ok(serde_json::json!({ "file": format!("{folder}.zip"), "base64": base64(&crate::zip::zip(&files)) }))
}

fn base64(bytes: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let v = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(CHARS[(v >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A guide version written for Polestar's own tools: a zip of a pack folder holding its
/// `.guide` file (in English). Polestar exists on WoW Forever only.
fn export_polestar(guide: &str, v: u32) -> CmdResult<serde_json::Value> {
    if edition() != Edition::Forever {
        return Err("Polestar is for WoW Forever only".into());
    }
    let route = render(
        Edition::Forever,
        &version(Edition::Forever, guide, v)?.outcome.route,
        Some("enUS"),
    )?;
    let folder = job::POLESTAR_SOURCE_FOLDER;
    let files = vec![
        (
            format!("{folder}/pack.json"),
            fg_route::export::polestar::pack_json(APP_VERSION).into_bytes(),
        ),
        (
            format!("{folder}/{}.guide", fg_route::export::polestar::guide_name(guide)),
            fg_route::export::polestar::source(guide, &route).into_bytes(),
        ),
    ];
    Ok(serde_json::json!({ "file": format!("{folder}.zip"), "base64": base64(&crate::zip::zip(&files)) }))
}

// Shared guides --------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct SharedGuide {
    format: String,
    version: u32,
    meta: serde_json::Value,
    request: PlanRequest,
    outcome: PlanOutcome,
}

const SHARE_FORMAT: &str = "factoruide-guide";
const LEGACY_SHARE_FORMAT: &str = "brouzquest-result";

/// A version as a .fgguide file to download.
fn export_version(guide: &str, v: u32) -> CmdResult<serde_json::Value> {
    let gv = version(edition(), guide, v)?;
    let shared = SharedGuide {
        format: SHARE_FORMAT.into(),
        version: 2,
        meta: serde_json::to_value(&gv.meta).map_err(err)?,
        request: gv.request,
        outcome: gv.outcome,
    };
    Ok(serde_json::json!({
        "file": format!("{guide}-v{v}.fgguide"),
        "text": serde_json::to_string(&shared).map_err(err)?,
    }))
}

fn import_guide(content: &str) -> CmdResult<VersionMeta> {
    let mut shared: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("not a Factoruide guide file: {e}"))?;
    if let Some(outcome) = shared.get_mut("outcome") {
        let o = PlanOutcome::from_value(outcome.take()).map_err(err)?;
        *outcome = serde_json::to_value(o).map_err(err)?;
    }
    let shared: SharedGuide =
        serde_json::from_value(shared).map_err(|e| format!("not a Factoruide guide file: {e}"))?;
    if shared.format != SHARE_FORMAT && shared.format != LEGACY_SHARE_FORMAT {
        return Err("not a Factoruide guide file".into());
    }
    let data = shared.meta.get("data").and_then(|d| d.as_str()).map(str::to_owned);
    let edition = edition();
    if let (Some(memory), Some(w)) = (&shared.outcome.walks, WALKS.lock().unwrap().get(&edition)) {
        w.add(memory.clone());
    }
    add_version(edition, &shared.request, &shared.outcome, data, (true, false), now()?)
}

// Planning and the editor ----------------------------------------------------------------

static PLAN_INTERRUPT: LazyLock<job::Interrupt> = LazyLock::new(job::Interrupt::default);
static EDIT_REQUESTS: LazyLock<job::Requests> = LazyLock::new(job::Requests::default);

fn plan_route(request: &PlanRequest) -> CmdResult<VersionMeta> {
    PLAN_INTERRUPT.reset();
    let edition = request_edition(request);
    let conn = open_db(edition)?;
    let overrides = overrides(edition)?;
    let walks = walks(edition);
    let progress = |m: &str| host::event("plan-progress", &serde_json::Value::String(m.to_owned()));
    let outcome = job::plan_interruptible(&conn, &overrides, request, &progress, &PLAN_INTERRUPT, Some(&walks))
        .map_err(|e| {
            if e.is::<job::Cancelled>() {
                job::Cancelled.to_string()
            } else {
                err(e)
            }
        })?;
    add_version(edition, request, &outcome, data_version(&conn), (false, false), now()?)
}

struct EditSession {
    edition: Edition,
    request: PlanRequest,
    notes: Vec<String>,
    editor: job::Editor,
}

static EDITOR: Mutex<Option<EditSession>> = Mutex::new(None);

#[derive(Serialize)]
struct EditOpened {
    steps: Vec<RouteStep>,
    rebuilt: bool,
    view: job::EditView,
    catalog: EditCatalog,
}

type TimelineKey = (Edition, String, u32, u64, String);
static TIMELINE: Mutex<Option<(TimelineKey, job::Timeline)>> = Mutex::new(None);

const SUPERSEDED: &str = "superseded";

fn guide_timeline(guide: &str, n: u32, locale: &str) -> CmdResult<job::Timeline> {
    let v = version(edition(), guide, n)?;
    let edition = request_edition(&v.request);
    let key: TimelineKey = (edition, guide.to_owned(), n, v.meta.created, locale.to_owned());
    if let Some((k, timeline)) = TIMELINE.lock().unwrap().as_ref()
        && *k == key
    {
        return Ok(timeline.clone());
    }
    let conn = open_db(edition)?;
    let editor = job::Editor::open(&conn, &overrides(edition)?, &v.request, Some(&walks(edition))).map_err(err)?;
    let timeline = editor.timeline(&v.outcome, &*names(edition, locale)?);
    *TIMELINE.lock().unwrap() = Some((key, timeline.clone()));
    Ok(timeline)
}

fn edit_open(guide: &str, n: u32, locale: &str) -> CmdResult<EditOpened> {
    let v = version(edition(), guide, n)?;
    let edition = request_edition(&v.request);
    let conn = open_db(edition)?;
    let editor = job::Editor::open(&conn, &overrides(edition)?, &v.request, Some(&walks(edition))).map_err(err)?;
    let names = names(edition, locale)?;
    let (steps, rebuilt) = editor.steps(&v.outcome);
    let request = EDIT_REQUESTS.next();
    let opened = EditOpened {
        view: editor.view(&steps, &names, &request).ok_or(SUPERSEDED)?,
        catalog: editor.catalog(&names),
        steps,
        rebuilt,
    };
    *EDITOR.lock().unwrap() = Some(EditSession {
        edition,
        request: v.request,
        notes: v.outcome.notes,
        editor,
    });
    Ok(opened)
}

fn edit_simulate(steps: &[RouteStep], locale: &str) -> CmdResult<job::EditView> {
    let request = EDIT_REQUESTS.next();
    let session = EDITOR.lock().unwrap();
    let session = session.as_ref().ok_or("no guide open in the editor")?;
    let names = names(session.edition, locale)?;
    session
        .editor
        .view(steps, &names, &request)
        .ok_or_else(|| SUPERSEDED.to_owned())
}

fn edit_suggest(steps: &[RouteStep], after: Option<usize>, locale: &str) -> CmdResult<job::Suggestions> {
    let request = EDIT_REQUESTS.next();
    let session = EDITOR.lock().unwrap();
    let session = session.as_ref().ok_or("no guide open in the editor")?;
    let names = names(session.edition, locale)?;
    session
        .editor
        .suggest(steps, after, &names, &request)
        .ok_or_else(|| SUPERSEDED.to_owned())
}

fn edit_save(steps: &[RouteStep]) -> CmdResult<VersionMeta> {
    let mut session = EDITOR.lock().unwrap();
    let open = session.as_ref().ok_or("no guide open in the editor")?;
    let outcome = open.editor.save(steps, open.notes.clone()).map_err(err)?;
    let data = data_version(&open_db(open.edition)?);
    let meta = add_version(open.edition, &open.request, &outcome, data, (false, true), now()?)?;
    *session = None;
    Ok(meta)
}

// Characters ---------------------------------------------------------------------------
//
//   characters/<edition>/<Name-Realm>.json   the profile of the last export pasted

#[derive(Serialize)]
struct CharacterInfo {
    full_name: Option<String>,
    race: Option<String>,
    class: String,
    professions: Vec<(String, i64)>,
    profile: StartState,
}

fn rested_now(p: &StartState, now: i64) -> i64 {
    if p.xp_max <= 0 || p.time <= 0 {
        return p.rested;
    }
    let hours = (now - p.time).max(0) as f64 / 3600.0;
    let per_hour = 0.05 / 8.0 * p.xp_max as f64 * if p.resting { 1.0 } else { 0.25 };
    let cap = p.xp_max * 3 / 2;
    (p.rested + (hours * per_hour) as i64).min(cap.max(p.rested))
}

fn forgotten_key(edition: Edition, character: &str) -> String {
    format!("{}/{character}", edition.key())
}

fn is_forgotten(forgotten: &BTreeMap<String, i64>, edition: Edition, profile: &StartState) -> bool {
    forgotten
        .get(&forgotten_key(edition, &profile.name))
        .is_some_and(|&time| profile.time <= time)
}

/// A file name for a character's key ("Name-Realm" holds no slash, but a realm could).
fn character_file(dir: &str, edition: Edition, key: &str) -> String {
    format!("{dir}/{}/{}.json", edition.key(), key.replace('/', "_"))
}

fn profiles(edition: Edition) -> Vec<serde_json::Value> {
    let dir = format!("characters/{}", edition.key());
    let mut out: Vec<serde_json::Value> = store::list(&dir)
        .into_iter()
        .filter_map(|f| serde_json::from_slice(&store::read(&format!("{dir}/{f}"))?).ok())
        .collect();
    out.sort_by_key(|p: &serde_json::Value| std::cmp::Reverse(p["time"].as_i64().unwrap_or(0)));
    out
}

fn characters() -> CmdResult<Vec<CharacterInfo>> {
    let settings = settings();
    let edition = settings.edition;
    let races = overrides(edition)?.races;
    let now = i64::try_from(now()?).map_err(err)?;
    Ok(profiles(edition)
        .into_iter()
        .filter_map(|json| serde_json::from_value::<StartState>(json).ok())
        .filter(|profile| !is_forgotten(&settings.forgotten_characters, edition, profile))
        .map(|mut profile| {
            profile.rested = rested_now(&profile, now);
            CharacterInfo {
                full_name: None,
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
                profile,
            }
        })
        .collect())
}

fn forget_character(character: &str) -> CmdResult<()> {
    let time = characters()?
        .into_iter()
        .find(|c| c.profile.name == character)
        .map_or(i64::MAX, |c| c.profile.time);
    let mut settings = settings();
    let edition = settings.edition;
    settings
        .forgotten_characters
        .insert(forgotten_key(edition, character), time);
    write_settings(settings)?;
    let mut assigned = assignments(edition);
    if assigned.remove(character).is_some() {
        write_assignments(edition, &assigned)?;
        install_addon()?;
    }
    Ok(())
}

/// What the addon's export holds (`/fg export`, see the addon's Profile.lua).
#[derive(Deserialize)]
struct ProfileExport {
    format: String,
    version: u32,
    edition: Option<String>,
    #[serde(default)]
    addon: Option<String>,
    character: String,
    profile: serde_json::Value,
}

/// A character's export pasted: what it was read as.
#[derive(Serialize)]
struct ProfileImported {
    character: String,
    edition: &'static str,
    level: i64,
}

/// When the addon was last seen (the newest export pasted), for the setup pages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LastExport {
    time: i64,
    version: String,
}

fn last_export_file(edition: Edition) -> String {
    format!("last-export-{}.json", edition.key())
}

/// Keep a character's export (its text as the addon gives it): its profile replaces the one
/// kept.
fn import_profile(content: &str) -> CmdResult<ProfileImported> {
    let json = fgstring::decode(fgstring::PROFILE_PREFIX, content).map_err(|e| match e.as_str() {
        "not_this_kind" => "not_a_profile".to_owned(),
        _ => "corrupt_profile".to_owned(),
    })?;
    let export: ProfileExport = serde_json::from_slice(&json).map_err(|_| "corrupt_profile".to_owned())?;
    if export.format != "factoruide-profile" || export.version != 1 {
        return Err("not_a_profile".into());
    }
    let edition = match export.edition.as_deref() {
        Some(key) => Edition::ALL
            .iter()
            .copied()
            .find(|e| e.key() == key)
            .ok_or_else(|| format!("unknown game version {key}"))?,
        None => edition(),
    };
    let key = export.character.trim().to_owned();
    if key.is_empty() || key.len() > 100 {
        return Err("corrupt_profile".into());
    }
    let mut profile = export.profile;
    if profile["name"].as_str().is_none_or(str::is_empty) {
        profile["name"] = key.clone().into();
    }
    let parsed: StartState = serde_json::from_value(profile.clone()).map_err(|e| format!("corrupt_profile: {e}"))?;
    store::write(
        &character_file("characters", edition, &key),
        serde_json::to_vec(&profile).map_err(err)?,
    );
    let last = LastExport {
        time: i64::try_from(now()?).map_err(err)?,
        version: export.addon.unwrap_or_default(),
    };
    store::write(&last_export_file(edition), serde_json::to_vec(&last).map_err(err)?);
    // Pasted again after being taken out: back.
    let mut settings = settings();
    if settings
        .forgotten_characters
        .remove(&forgotten_key(edition, &key))
        .is_some()
    {
        write_settings(settings)?;
    } else {
        drop(settings);
    }
    Ok(ProfileImported {
        character: key,
        edition: edition.key(),
        level: parsed.level,
    })
}

// The addon, as the exports show it -----------------------------------------------------------

#[derive(Serialize)]
struct AddonState {
    app_version: &'static str,
    /// The version of the addon of the last export pasted.
    installed_version: Option<String>,
    seen: Option<LastExport>,
}

fn last_export(edition: Edition) -> Option<LastExport> {
    serde_json::from_slice(&store::read(&last_export_file(edition))?).ok()
}

fn addon_health() -> AddonState {
    let seen = last_export(edition());
    AddonState {
        app_version: APP_VERSION,
        installed_version: seen.as_ref().map(|s| s.version.clone()),
        seen,
    }
}

// Maps ------------------------------------------------------------------------------------

fn map_index() -> CmdResult<serde_json::Value> {
    let db = open_db(edition())?;
    let text: String = db
        .query_row("SELECT value FROM app_meta WHERE key = 'map_index'", [], |r| r.get(0))
        .map_err(err)?;
    let mut index: serde_json::Value = serde_json::from_str(&text).map_err(err)?;
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

// Calls -----------------------------------------------------------------------------------

#[derive(Deserialize)]
struct Call {
    cmd: String,
    #[serde(default)]
    args: serde_json::Value,
}

/// The arguments of a call, named as the page names them (camelCase, as Tauri takes them).
fn args<T: DeserializeOwned>(args: serde_json::Value) -> CmdResult<T> {
    serde_json::from_value(if args.is_null() { serde_json::json!({}) } else { args }).map_err(err)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuideArgs {
    #[serde(alias = "id")]
    guide: String,
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    locale: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StepsArgs {
    steps: Vec<RouteStep>,
    #[serde(default)]
    after: Option<usize>,
    #[serde(default)]
    locale: String,
}

fn to_json<T: Serialize>(result: CmdResult<T>) -> CmdResult<serde_json::Value> {
    result.and_then(|v| serde_json::to_value(v).map_err(err))
}

fn version_of(a: &GuideArgs) -> CmdResult<u32> {
    a.version.ok_or_else(|| "version missing".to_owned())
}

/// Answer a call from the page: `{"ok": value}` or `{"err": message}`.
pub fn dispatch(request: &[u8]) -> String {
    let result = serde_json::from_slice::<Call>(request)
        .map_err(err)
        .and_then(|call| run(&call.cmd, call.args));
    match result {
        Ok(value) => serde_json::json!({ "ok": value }).to_string(),
        Err(e) => serde_json::json!({ "err": e }).to_string(),
    }
}

fn run(cmd: &str, a: serde_json::Value) -> CmdResult<serde_json::Value> {
    use serde_json::{Value, json};
    match cmd {
        "updates_enabled" => Ok(Value::Bool(false)),
        "get_settings" => to_json(Ok(settings())),
        "set_settings" => {
            #[derive(Deserialize)]
            struct A {
                settings: Settings,
            }
            let mut s = args::<A>(a)?.settings;
            let before = settings();
            if s.addon_installed.is_none() {
                s.addon_installed = before.addon_installed;
            }
            to_json(write_settings(s))
        }
        "set_edition" => {
            #[derive(Deserialize)]
            struct A {
                edition: Edition,
            }
            let mut s = settings();
            s.edition = args::<A>(a)?.edition;
            to_json(write_settings(s))
        }
        "list_editions" => to_json(Ok(list_editions())),
        "get_options" => {
            let edition = edition();
            to_json(job::options(&open_db(edition)?, &overrides(edition)?).map_err(err))
        }
        "plan_route" => {
            #[derive(Deserialize)]
            struct A {
                request: PlanRequest,
            }
            to_json(plan_route(&args::<A>(a)?.request))
        }
        "stop_plan" => {
            PLAN_INTERRUPT.stop();
            Ok(Value::Null)
        }
        "cancel_plan" => {
            PLAN_INTERRUPT.cancel();
            Ok(Value::Null)
        }
        "export_request" => {
            #[derive(Deserialize)]
            struct A {
                request: PlanRequest,
            }
            let request = args::<A>(a)?.request;
            let name = request
                .name
                .clone()
                .unwrap_or_else(|| format!("{}-{}", request.race, request.class));
            Ok(json!({
                "file": format!("request-{name}.json"),
                "text": serde_json::to_string_pretty(&request).map_err(err)?,
            }))
        }
        "list_guides" => to_json(Ok(list_guides())),
        "get_guide" => to_json(summary(edition(), &args::<GuideArgs>(a)?.guide)),
        "get_version" => {
            let g = args::<GuideArgs>(a)?;
            to_json(get_version(&g.guide, version_of(&g)?, g.locale.as_deref()))
        }
        "compare_versions" => {
            #[derive(Deserialize)]
            struct Versions {
                guide: String,
                a: u32,
                b: u32,
                #[serde(default)]
                locale: Option<String>,
            }
            let x = args::<Versions>(a)?;
            to_json(compare_versions(&x.guide, x.a, x.b, x.locale.as_deref()))
        }
        "set_installed" => {
            let g = args::<GuideArgs>(a)?;
            to_json(set_installed(&g.guide, g.version))
        }
        "delete_version" => {
            let g = args::<GuideArgs>(a)?;
            to_json(delete_version(&g.guide, version_of(&g)?))
        }
        "delete_guide" => to_json(delete_guide(&args::<GuideArgs>(a)?.guide)),
        "list_assignments" => to_json(Ok(assignments(edition()))),
        "assign_guide" => {
            #[derive(Deserialize)]
            struct A {
                character: String,
                guide: Option<String>,
            }
            let x = args::<A>(a)?;
            to_json(assign_guide(x.character, x.guide))
        }
        "forget_character" => {
            #[derive(Deserialize)]
            struct A {
                character: String,
            }
            to_json(forget_character(&args::<A>(a)?.character))
        }
        "export_version" => {
            let g = args::<GuideArgs>(a)?;
            export_version(&g.guide, version_of(&g)?)
        }
        "import_guide" => {
            #[derive(Deserialize)]
            struct A {
                content: String,
            }
            to_json(import_guide(&args::<A>(a)?.content))
        }
        "import_profile" => {
            #[derive(Deserialize)]
            struct A {
                content: String,
            }
            to_json(import_profile(&args::<A>(a)?.content))
        }
        "list_characters" => to_json(characters()),
        "install_addon" => to_json(install_addon()),
        "addon_export" => to_json(addon_export()),
        "polestar_pack" => polestar_pack(),
        "addon_synced" => Ok(Value::Bool(addon_synced())),
        "addon_health" => to_json(Ok(addon_health())),
        // Polestar is installed in the game: the browser cannot tell, the player says.
        "polestar_installed" => Ok(Value::Bool(true)),
        "export_polestar" => {
            let g = args::<GuideArgs>(a)?;
            export_polestar(&g.guide, version_of(&g)?)
        }
        "wow_status" => Ok(json!({ "root": "", "clients": [], "problem": null })),
        "detect_wow" => Ok(json!([])),
        "map_index" => map_index(),
        "guide_timeline" => {
            let g = args::<GuideArgs>(a)?;
            to_json(guide_timeline(
                &g.guide,
                version_of(&g)?,
                g.locale.as_deref().unwrap_or("enUS"),
            ))
        }
        "edit_open" => {
            let g = args::<GuideArgs>(a)?;
            to_json(edit_open(
                &g.guide,
                version_of(&g)?,
                g.locale.as_deref().unwrap_or("enUS"),
            ))
        }
        "edit_simulate" => {
            let s = args::<StepsArgs>(a)?;
            to_json(edit_simulate(&s.steps, &s.locale))
        }
        "edit_suggest" => {
            let s = args::<StepsArgs>(a)?;
            to_json(edit_suggest(&s.steps, s.after, &s.locale))
        }
        "edit_save" => to_json(edit_save(&args::<StepsArgs>(a)?.steps)),
        "edit_close" => {
            *EDITOR.lock().unwrap() = None;
            Ok(Value::Null)
        }
        _ => Err(format!("unknown call {cmd}")),
    }
}

#[cfg(test)]
mod tests;
