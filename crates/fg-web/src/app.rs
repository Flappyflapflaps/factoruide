//! The app's calls, as the desktop app's backend answers them (app/src-tauri/src/main.rs), on
//! the files of `store` instead of the app's folders and with the addon reached through texts:
//! the guide import the player pastes in game (`addon_export`), the character export pasted in
//! the app (`import_profile`).

use crate::{embedded, fgstring, host, store};
use fg_contrib::collector::Character;
use fg_contrib::contribution::{Contribution, SendState};
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
const PROFILE_ADDON: &str = "Factoruide_Profile";
const GUIDE_ADDON: &str = "Factoruide_Guide";

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
    /// Contribution server (its base address); unset: the one the page is served by.
    #[serde(default)]
    contribution_url: Option<String>,
    #[serde(default)]
    contributor: Option<String>,
    #[serde(default)]
    auto_contribute: bool,
    #[serde(default)]
    addon: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    guide_addon: GuideAddon,
    #[serde(default)]
    guide_locale: Option<String>,
    #[serde(default)]
    all_guides: bool,
    /// What the last guide import was made with (`addon_snapshot`).
    #[serde(default)]
    addon_installed: Option<String>,
    #[serde(default)]
    addon_setup_done: bool,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum GuideAddon {
    #[default]
    Factoruide,
    Polestar,
}

impl Settings {
    fn guide_addon(&self, edition: Edition) -> GuideAddon {
        if edition == Edition::Forever {
            self.guide_addon
        } else {
            GuideAddon::Factoruide
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

fn write_settings(mut settings: Settings) -> CmdResult<()> {
    let mut current = SETTINGS.lock().unwrap();
    if settings.contributor.is_none() {
        settings.contributor.clone_from(&current.contributor);
    }
    store::write(SETTINGS_FILE, serde_json::to_vec_pretty(&settings).map_err(err)?);
    *current = settings;
    Ok(())
}

/// The settings the guide import depends on.
fn addon_snapshot(settings: &Settings, edition: Edition) -> String {
    serde_json::to_string(&(
        &settings.addon,
        &settings.guide_locale,
        settings.guide_addon(edition),
        guides_stamp(edition),
    ))
    .unwrap_or_default()
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

// The guide import ---------------------------------------------------------------------

/// What the player takes to the game: a text to paste (`/fg import`), or for Polestar a guide
/// pack to unzip in the game's AddOns folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum AddonExport {
    Text {
        text: String,
        guides: usize,
    },
    Zip {
        file: String,
        base64: String,
        guides: usize,
    },
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

/// What the addon gets: the Lua of Routes.lua (the guides assigned to a character, in the
/// version they follow, and who follows which), Services.lua and Config.lua, and the events
/// the contribution server accepted (Sent.lua). Saved, and the settings note it was made.
fn install_addon() -> CmdResult<usize> {
    static MAKING: Mutex<()> = Mutex::new(());
    let _one = MAKING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let settings = settings();
    let edition = settings.edition;
    let assigned = assigned_routes(edition);
    let render_one = |r: &Route| render(edition, r, settings.guide_locale.as_deref());
    let export = match settings.guide_addon(edition) {
        GuideAddon::Factoruide => {
            let mut routes = assigned.values().map(render_one).collect::<CmdResult<Vec<_>>>()?;
            routes.sort_by(|a, b| a.name.cmp(&b.name));
            let locale = (settings.guide_locale.clone())
                .or_else(|| routes.first().and_then(|r| r.locale.clone()))
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
            lua.push('\n');
            lua.push_str(&job::addon_config_lua(
                &settings.addon,
                settings.guide_locale.as_deref(),
            ));
            lua.push('\n');
            lua.push_str(&sent_lua(&read_send_record(edition)?.state));
            AddonExport::Text {
                text: fgstring::encode(fgstring::GUIDE_PREFIX, lua.as_bytes()),
                guides: routes.len(),
            }
        }
        GuideAddon::Polestar => {
            let guides = assigned
                .iter()
                .map(|(id, r)| Ok((id.clone(), render_one(r)?)))
                .collect::<CmdResult<Vec<_>>>()?;
            let folder = fg_route::export::polestar::ADDON;
            let files: Vec<(String, Vec<u8>)> = fg_route::export::polestar::pack(&guides, APP_VERSION)
                .into_iter()
                .map(|(name, content)| (format!("{folder}/{name}"), content.into_bytes()))
                .collect();
            AddonExport::Zip {
                file: format!("{folder}.zip"),
                base64: base64(&crate::zip::zip(&files)),
                guides: guides.len(),
            }
        }
    };
    let count = match &export {
        AddonExport::Text { guides, .. } | AddonExport::Zip { guides, .. } => *guides,
    };
    store::write(&addon_export_file(edition), serde_json::to_vec(&export).map_err(err)?);
    write_settings(Settings {
        addon_installed: Some(addon_snapshot(&settings, edition)),
        ..settings
    })?;
    Ok(count)
}

/// The guide import made last (made now when there is none, or it is out of date).
fn addon_export() -> CmdResult<AddonExport> {
    let edition = edition();
    if !addon_synced()
        && let Err(e) = install_addon()
    {
        return Err(e);
    }
    let bytes = store::read(&addon_export_file(edition)).ok_or("no guide import made yet")?;
    serde_json::from_slice(&bytes).map_err(err)
}

/// Whether the last guide import has the current guides and settings.
fn addon_synced() -> bool {
    let settings = settings();
    let edition = settings.edition;
    store::exists(&addon_export_file(edition))
        && settings.addon_installed.as_deref() == Some(addon_snapshot(&settings, edition).as_str())
}

/// The addon to install by hand (when it is not taken from an addon site): its two folders, as
/// they are without guides, in a zip.
fn addon_package() -> serde_json::Value {
    let mut files = Vec::new();
    for (folder, list) in [
        (PROFILE_ADDON, embedded::PROFILE_ADDON),
        (GUIDE_ADDON, embedded::GUIDE_ADDON),
    ] {
        for (name, bytes) in list {
            let content = if std::path::Path::new(name)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("toc"))
            {
                with_app_version(bytes)
            } else {
                bytes.to_vec()
            };
            files.push((format!("{folder}/{name}"), content));
        }
    }
    // The files the app used to write, empty: the guides come with `/fg import`.
    for name in ["Routes.lua", "Services.lua", "Config.lua"] {
        files.push((
            format!("{GUIDE_ADDON}/{name}"),
            b"-- Filled by `/fg import` (the Factoruide web app's guide import).\n".to_vec(),
        ));
    }
    files.push((
        format!("{PROFILE_ADDON}/Sent.lua"),
        b"-- Filled by `/fg import`.\n".to_vec(),
    ));
    serde_json::json!({ "file": "Factoruide-addon.zip", "base64": base64(&crate::zip::zip(&files)) })
}

/// An addon's .toc with the app's version.
fn with_app_version(toc: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(toc);
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if line.starts_with("## Version:") {
            let end = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            let _ = write!(out, "## Version: {APP_VERSION}{end}");
        } else {
            out.push_str(line);
        }
    }
    out.into_bytes()
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
//   collector/<edition>/<Name-Realm>.json    the recordings not sent yet

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
    #[serde(default)]
    collector: Option<CollectorExport>,
}

#[derive(Deserialize, Serialize, Default)]
struct CollectorExport {
    #[serde(default)]
    race: Option<String>,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    events: Vec<String>,
}

/// A character's export pasted: what it was read as.
#[derive(Serialize)]
struct ProfileImported {
    character: String,
    edition: &'static str,
    level: i64,
    /// New recordings kept for the contribution server.
    events: usize,
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
/// kept, its recordings join those not sent yet.
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
    let mut events = 0;
    if let Some(collector) = export.collector {
        let file = character_file("collector", edition, &key);
        let mut kept: CollectorExport = store::read(&file)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let sent = read_send_record(edition)?
            .state
            .characters
            .get(&key)
            .map_or(0, |s| s.last_seq);
        let seq = |row: &str| row.split('|').next().and_then(|s| s.parse::<i64>().ok());
        let mut rows: BTreeMap<i64, String> = kept
            .events
            .drain(..)
            .chain(collector.events)
            .filter_map(|r| Some((seq(&r)?, r)))
            .filter(|(s, _)| *s > sent)
            .collect();
        // As many as the addon keeps.
        while rows.len() > 20_000 {
            rows.pop_first();
        }
        events = rows.len();
        kept.events = rows.into_values().collect();
        kept.race = collector.race.or(kept.race);
        kept.class = collector.class.or(kept.class);
        store::write(&file, serde_json::to_vec(&kept).map_err(err)?);
    }
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
        events,
    })
}

// What the setup pages show ---------------------------------------------------------------

#[derive(Serialize)]
struct AddonState {
    app_version: &'static str,
    installed_version: Option<String>,
    installed_at: Option<i64>,
    complete: bool,
    seen: Option<LastExport>,
    not_loaded: bool,
    disabled_for: Vec<String>,
    saved_variables: usize,
    restart_needed: bool,
    polestar: bool,
}

fn last_export(edition: Edition) -> Option<LastExport> {
    serde_json::from_slice(&store::read(&last_export_file(edition))?).ok()
}

/// The addon as the last export pasted shows it.
fn addon_health() -> AddonState {
    let settings = settings();
    let edition = settings.edition;
    let seen = last_export(edition);
    AddonState {
        app_version: APP_VERSION,
        installed_version: seen.as_ref().map(|s| s.version.clone()),
        installed_at: None,
        complete: true,
        seen,
        not_loaded: false,
        disabled_for: Vec::new(),
        saved_variables: profiles(edition).len(),
        restart_needed: false,
        polestar: settings.guide_addon(edition) == GuideAddon::Polestar,
    }
}

fn version_like(v: &str) -> bool {
    !v.is_empty() && v.len() <= 32 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
}

fn setup_of() -> fg_contrib::issue::Setup {
    let settings = settings();
    let edition = settings.edition;
    let seen = last_export(edition);
    let today = now().unwrap_or_default().cast_signed();
    let polestar = settings.guide_addon(edition) == GuideAddon::Polestar;
    fg_contrib::issue::Setup {
        wow_dir: "web".into(),
        clients: Vec::new(),
        problem: None,
        guide_addon: if polestar { "polestar" } else { "factoruide" }.into(),
        addon_version: seen.as_ref().map(|s| s.version.clone()).filter(|v| version_like(v)),
        addon_complete: true,
        seen_days_ago: seen
            .as_ref()
            .and_then(|s| u32::try_from((today - s.time).max(0) / 86_400).ok()),
        seen_version: seen.map(|s| s.version).filter(|v| version_like(v)),
        not_loaded: false,
        disabled: 0,
        saved_variables: u32::try_from(profiles(edition).len()).unwrap_or(u32::MAX),
        restart_needed: false,
    }
}

// Contributions --------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct SendRecord {
    #[serde(default)]
    last_sent: Option<u64>,
    #[serde(default)]
    retry_after: Option<u64>,
    #[serde(default)]
    last_auto: Option<AutoSend>,
    #[serde(default)]
    more: bool,
    #[serde(default)]
    state: SendState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AutoSend {
    time: u64,
    status: String,
    #[serde(default)]
    message: Option<String>,
}

fn send_record_file(edition: Edition) -> String {
    format!("contribution-state-{}.json", edition.key())
}

fn read_send_record(edition: Edition) -> CmdResult<SendRecord> {
    let file = send_record_file(edition);
    match store::read(&file) {
        Some(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{file}: {e}")),
        None => Ok(SendRecord::default()),
    }
}

fn write_send_record(edition: Edition, record: &SendRecord) -> CmdResult<()> {
    store::write(
        &send_record_file(edition),
        serde_json::to_vec_pretty(record).map_err(err)?,
    );
    Ok(())
}

/// Sent.lua: the last event of each character the server accepted (joined to the import).
fn sent_lua(state: &SendState) -> String {
    let mut lua = String::from(
        "-- Last event of each character the contribution server accepted: the addon drops the events up to it.\n\
         FactoruideSent = {\n",
    );
    for (character, last_seq) in state.sent_by_name() {
        let _ = writeln!(lua, "  [{}] = {last_seq},", lua_string(&character));
    }
    lua.push_str("}\n");
    lua
}

#[derive(Serialize)]
struct ContributionPreview {
    edition: &'static str,
    client_dir: String,
    found: bool,
    quests: usize,
    entities: usize,
    events: usize,
    characters: usize,
    locales: Vec<String>,
    size: usize,
    more: bool,
    url: Option<String>,
    last_sent: Option<u64>,
    retry_after: Option<u64>,
    automatic: Vec<AutoStatus>,
}

#[derive(Serialize)]
struct AutoStatus {
    edition: &'static str,
    name: &'static str,
    #[serde(flatten)]
    send: AutoSend,
}

/// `/v1/<path>` on the contribution server: the one configured, else the one built in
/// (`FG_CONTRIBUTION_SERVER`), else the server the page comes from.
fn server_endpoint(settings: &Settings, path: &str) -> String {
    let base = settings
        .contribution_url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .or(option_env!("FG_CONTRIBUTION_SERVER"))
        .unwrap_or("");
    let base = base
        .trim_end_matches('/')
        .trim_end_matches("/v1/contributions")
        .trim_end_matches("/contributions");
    if base.is_empty() {
        format!("v1/{path}")
    } else {
        format!("{base}/v1/{path}")
    }
}

static SEND_LOCK: Mutex<()> = Mutex::new(());

fn contributor_id() -> CmdResult<String> {
    let mut settings = SETTINGS.lock().unwrap();
    if let Some(id) = settings
        .contributor
        .clone()
        .filter(|id| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Ok(id);
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(err)?;
    let id = bytes.iter().fold(String::with_capacity(32), |mut id, b| {
        let _ = write!(id, "{b:02x}");
        id
    });
    settings.contributor = Some(id.clone());
    store::write(SETTINGS_FILE, serde_json::to_vec_pretty(&*settings).map_err(err)?);
    Ok(id)
}

/// The recordings of an edition's characters kept from their exports.
fn recorded(edition: Edition) -> Vec<Character> {
    let dir = format!("collector/{}", edition.key());
    store::list(&dir)
        .into_iter()
        .filter_map(|f| {
            let kept: CollectorExport = serde_json::from_slice(&store::read(&format!("{dir}/{f}"))?).ok()?;
            Some(Character {
                character: f.strip_suffix(".json")?.to_owned(),
                race: kept.race,
                class: kept.class,
                day: None,
                account: None,
                events: kept
                    .events
                    .iter()
                    .map(|r| fg_contrib::collector::parse_row(r))
                    .collect(),
            })
        })
        .collect()
}

/// What is new since `sent` (one document of it), the state to keep once the server accepted
/// it and whether more is left. The browser cannot read the game's caches: only recordings.
fn collect_contribution(edition: Edition, sent: &SendState) -> CmdResult<(Contribution, SendState, bool)> {
    let contributor = contributor_id()?;
    let now = now()?;
    let (collector, next, more) = fg_contrib::contribution::anonymize(
        recorded(edition),
        sent,
        now,
        fg_contrib::contribution::MAX_DOCUMENT_EVENTS,
    )
    .map_err(err)?;
    let contribution = Contribution {
        format: fg_contrib::contribution::FORMAT.into(),
        version: fg_contrib::contribution::VERSION,
        edition: edition.key().into(),
        app_version: APP_VERSION.into(),
        contributor,
        day: (now / 86_400) as u32,
        quest_cache: Vec::new(),
        creature_cache: Vec::new(),
        object_cache: Vec::new(),
        collector,
    };
    fg_contrib::validate::validate(&contribution, now).map_err(err)?;
    if serde_json::to_vec(&contribution).map_err(err)?.len() > fg_contrib::validate::MAX_BYTES / 2 {
        return Err("contribution too large".into());
    }
    Ok((contribution, next, more))
}

fn preview() -> CmdResult<ContributionPreview> {
    let settings = settings();
    let edition = settings.edition;
    let record = read_send_record(edition)?;
    let (c, _, more) = collect_contribution(edition, &record.state)?;
    let automatic = Edition::ALL
        .iter()
        .filter_map(|&e| {
            let send = read_send_record(e).ok()?.last_auto?;
            Some(AutoStatus {
                edition: e.key(),
                name: e.rules().edition_name,
                send,
            })
        })
        .collect();
    Ok(ContributionPreview {
        edition: edition.key(),
        client_dir: String::new(),
        found: !profiles(edition).is_empty(),
        quests: c.quests(),
        entities: c.entities(),
        events: c.events(),
        characters: c.collector.len(),
        locales: Vec::new(),
        size: serde_json::to_vec(&c).map_err(err)?.len(),
        more,
        url: Some(server_endpoint(&settings, "contributions")),
        last_sent: record.last_sent,
        retry_after: record.retry_after.filter(|&t| now().is_ok_and(|now| now < t)),
        automatic,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum Posted {
    Accepted,
    AlreadyReceived,
    RateLimited(Option<u64>),
    Failed(String),
}

impl Posted {
    fn accepted(&self) -> bool {
        matches!(self, Posted::Accepted | Posted::AlreadyReceived)
    }
    fn status(&self) -> &'static str {
        match self {
            Posted::Accepted => "accepted",
            Posted::AlreadyReceived => "already_received",
            Posted::RateLimited(_) => "rate_limited",
            Posted::Failed(_) => "error",
        }
    }
}

const MAX_RETRY_AFTER: u64 = 7 * 86_400;

fn post(url: &str, body: &[u8]) -> Posted {
    let r = match host::fetch("POST", url, Some("application/json"), body) {
        Ok(r) => r,
        Err(e) => return Posted::Failed(e),
    };
    match r.status {
        201 => Posted::Accepted,
        200 => Posted::AlreadyReceived,
        429 => Posted::RateLimited(
            r.retry_after
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|s| s.min(MAX_RETRY_AFTER)),
        ),
        code @ 400.. => {
            let json: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            let message = json["error"].as_str().or(json["status"].as_str()).unwrap_or("");
            Posted::Failed(format!("{code}: {message}"))
        }
        code => Posted::Failed(format!("{code}: unexpected answer")),
    }
}

/// Post one document and record the answer: the new state is kept only once accepted. Sent
/// recordings are dropped from the ones kept.
fn send_one(
    edition: Edition,
    url: &str,
    record: &SendRecord,
    automatic: bool,
) -> CmdResult<Option<(Posted, SendRecord, bool)>> {
    let (contribution, next, more) = collect_contribution(edition, &record.state)?;
    if contribution.is_empty() {
        return Ok(None);
    }
    let now = now()?;
    let posted = post(url, &serde_json::to_vec(&contribution).map_err(err)?);
    let mut record = record.clone();
    match &posted {
        Posted::Accepted | Posted::AlreadyReceived => {
            record.state = next;
            record.last_sent = Some(now);
            record.retry_after = None;
            record.more = more;
        }
        Posted::RateLimited(wait) => record.retry_after = wait.map(|w| now + w),
        Posted::Failed(_) => {}
    }
    if automatic {
        record.last_auto = Some(AutoSend {
            time: now,
            status: posted.status().to_owned(),
            message: match &posted {
                Posted::Failed(m) => Some(m.clone()),
                _ => None,
            },
        });
    }
    write_send_record(edition, &record)?;
    if posted.accepted() {
        drop_sent(edition, &record.state);
    }
    Ok(Some((posted, record, more)))
}

/// Drop the recordings the server has from those kept.
fn drop_sent(edition: Edition, state: &SendState) {
    let dir = format!("collector/{}", edition.key());
    for f in store::list(&dir) {
        let path = format!("{dir}/{f}");
        let Some(key) = f.strip_suffix(".json") else { continue };
        let Some(sent) = state.characters.get(key) else {
            continue;
        };
        let Some(mut kept) = store::read(&path).and_then(|b| serde_json::from_slice::<CollectorExport>(&b).ok()) else {
            continue;
        };
        let before = kept.events.len();
        kept.events.retain(|r| {
            r.split('|')
                .next()
                .and_then(|s| s.parse::<i64>().ok())
                .is_some_and(|s| s > sent.last_seq)
        });
        if kept.events.len() != before
            && let Ok(bytes) = serde_json::to_vec(&kept)
        {
            store::write(&path, bytes);
        }
    }
}

const MANUAL_BURST: usize = 3;

#[derive(Serialize)]
struct SendReport {
    status: &'static str,
    documents: usize,
    more: bool,
}

fn send_contribution() -> CmdResult<SendReport> {
    let _sending = SEND_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let edition = edition();
    let url = server_endpoint(&settings(), "contributions");
    let mut record = read_send_record(edition)?;
    if record.retry_after.is_some_and(|t| now().is_ok_and(|now| now < t)) {
        return Err("rate_limited".to_owned());
    }
    let (mut accepted, mut last_accepted, mut refused, mut more_left) = (0, None, None, false);
    for _ in 0..MANUAL_BURST {
        let Some((posted, sent, more)) = send_one(edition, &url, &record, false)? else {
            break;
        };
        record = sent;
        if !posted.accepted() {
            refused = Some(posted);
            more_left = true;
            break;
        }
        accepted += 1;
        last_accepted = Some(posted);
        more_left = more;
        if !more {
            break;
        }
    }
    match (last_accepted, refused) {
        (Some(last), _) => Ok(SendReport {
            status: last.status(),
            documents: accepted,
            more: more_left,
        }),
        (None, Some(Posted::RateLimited(_))) => Err("rate_limited".to_owned()),
        (None, Some(Posted::Failed(message))) => Err(message),
        (None, _) => Err("nothing_to_send".to_owned()),
    }
}

const AUTO_SEND_EVERY: u64 = 24 * 3600;
const AUTO_BACKLOG_EVERY: u64 = 3600;
const AUTO_CHECK_EVERY: u64 = 6 * 3600;

/// The automatic contributions, while the page is open: a check two minutes after it opens,
/// then every few hours (sooner for a backlog).
fn auto_contribute_loop() {
    let mut wait = 120;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(wait));
        wait = auto_contribute();
    }
}

fn auto_contribute() -> u64 {
    let mut next = AUTO_CHECK_EVERY;
    let settings = settings();
    let url = server_endpoint(&settings, "contributions");
    let Ok(now) = now() else {
        return next;
    };
    if !settings.auto_contribute {
        return next;
    }
    for edition in Edition::ALL {
        let _sending = SEND_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Ok(record) = read_send_record(edition) else {
            continue;
        };
        let every = if record.more {
            AUTO_BACKLOG_EVERY
        } else {
            AUTO_SEND_EVERY
        };
        let due = record.retry_after.is_none_or(|t| now >= t)
            && record.last_sent.is_none_or(|t| now.saturating_sub(t) >= every);
        let record = if due {
            match send_one(edition, &url, &record, true) {
                Ok(Some((_, sent, _))) => sent,
                Ok(None) => record,
                Err(e) => {
                    let mut record = record;
                    record.last_auto = Some(AutoSend {
                        time: now,
                        status: "error".to_owned(),
                        message: Some(e),
                    });
                    let _ = write_send_record(edition, &record);
                    record
                }
            }
        } else {
            record
        };
        let backlog = record.last_sent.filter(|_| record.more).map(|t| t + AUTO_BACKLOG_EVERY);
        if let Some(due_in) = [record.retry_after, backlog]
            .into_iter()
            .flatten()
            .filter(|&t| t > now)
            .map(|t| t - now)
            .min()
        {
            next = next.min(due_in);
        }
    }
    next.max(60)
}

// Reports ---------------------------------------------------------------------------------

#[derive(Deserialize)]
struct IssueDraft {
    kind: String,
    message: String,
    #[serde(default)]
    name: Option<String>,
    language: String,
    #[serde(default)]
    guide: Option<IssueGuide>,
    #[serde(default)]
    setup: bool,
}

#[derive(Deserialize)]
struct IssueGuide {
    guide: String,
    version: u32,
    #[serde(default)]
    steps: Vec<usize>,
    locale: String,
}

fn guide_report(g: &IssueGuide) -> CmdResult<(Edition, fg_contrib::issue::GuideReport)> {
    let v = version(edition(), &g.guide, g.version)?;
    let mut request = v.request;
    let edition = request_edition(&request);
    if request.params.is_none() {
        request.params = Some(overrides(edition)?.params);
    }
    request.name = None;
    if let Some(start) = &mut request.start {
        start.name.clear();
    }
    let route = render(edition, &v.outcome.route, Some(&g.locale))?;
    let mut steps = Vec::new();
    for &i in &g.steps {
        let step = serde_json::to_value(route.steps.get(i).ok_or("no such step")?).map_err(err)?;
        let map = &step["map"];
        steps.push(fg_contrib::issue::ReportedStep {
            index: i as u32,
            kind: step["kind"].as_str().unwrap_or_default().to_owned(),
            quest: step["quest"].as_u64().map(|q| q as u32),
            objective: step["objective"].as_u64().map(|o| o as u32),
            text: step["text"].as_str().unwrap_or_default().chars().take(500).collect(),
            zone: step["zone"].as_str().map(str::to_owned),
            level: step["level"].as_u64().unwrap_or_default() as u32,
            map: map["ui_map"]
                .as_u64()
                .zip(map["x"].as_f64().zip(map["y"].as_f64()))
                .map(|(m, (x, y))| (m as u32, x, y)),
        });
    }
    Ok((
        edition,
        fg_contrib::issue::GuideReport {
            request: serde_json::to_value(&request).map_err(err)?,
            plan: serde_json::to_value(&v.outcome.plan).map_err(err)?,
            data: v.meta.data,
            version: g.version,
            edited: v.meta.edited,
            imported: v.meta.imported,
            total_time: v.meta.total_time,
            steps,
        },
    ))
}

fn send_issue(draft: IssueDraft) -> CmdResult<&'static str> {
    let url = server_endpoint(&settings(), "issues");
    let (edition, guide) = match &draft.guide {
        Some(g) => {
            let (e, report) = guide_report(g)?;
            (e, Some(report))
        }
        None => (edition(), None),
    };
    let now = now()?;
    let issue = fg_contrib::issue::Issue {
        format: fg_contrib::issue::FORMAT.into(),
        version: fg_contrib::issue::VERSION,
        edition: edition.key().into(),
        app_version: APP_VERSION.into(),
        contributor: contributor_id()?,
        day: (now / 86_400) as u32,
        os: "web".into(),
        language: draft.language,
        kind: draft.kind,
        message: draft.message.trim().to_owned(),
        name: draft.name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()),
        guide,
        setup: draft.setup.then(setup_of),
    };
    fg_contrib::issue::validate(&issue, now).map_err(err)?;
    let send = |issue: &fg_contrib::issue::Issue| {
        let body = serde_json::to_vec(issue).map_err(err)?;
        if body.len() > fg_contrib::issue::MAX_BYTES {
            return Err("report too large".to_owned());
        }
        Ok(post(&url, &body))
    };
    let mut posted = send(&issue)?;
    if let Posted::Failed(message) = &posted
        && issue.setup.is_some()
        && message.contains("invalid report document")
    {
        posted = send(&fg_contrib::issue::Issue { setup: None, ..issue })?;
    }
    match posted {
        Posted::RateLimited(_) => Err("rate_limited".to_owned()),
        Posted::Failed(message) => Err(message),
        posted => Ok(posted.status()),
    }
}

fn need_help(locale: &str) -> CmdResult<Vec<fg_route::help::Check>> {
    let edition = edition();
    let defs = fg_route::help::load(override_file, edition).map_err(err)?;
    fg_route::help::resolve(&open_db(edition)?, &defs, edition, locale).map_err(err)
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
    static AUTO: std::sync::Once = std::sync::Once::new();
    AUTO.call_once(|| {
        std::thread::spawn(auto_contribute_loop);
    });
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
            if s.contributor.is_none() {
                s.contributor = before.contributor;
            }
            if s.addon_installed.is_none() {
                s.addon_installed = before.addon_installed;
            }
            to_json(write_settings(s))
        }
        "set_auto_contribute" => {
            #[derive(Deserialize)]
            struct A {
                on: bool,
            }
            let mut s = settings();
            s.auto_contribute = args::<A>(a)?.on;
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
        "addon_package" => Ok(addon_package()),
        "addon_synced" => Ok(Value::Bool(addon_synced())),
        "addon_health" => to_json(Ok(addon_health())),
        "setup_report" => to_json(Ok(setup_of())),
        // Polestar is installed in the game: the browser cannot tell, the player says.
        "polestar_installed" => Ok(Value::Bool(true)),
        "export_polestar" => {
            let g = args::<GuideArgs>(a)?;
            export_polestar(&g.guide, version_of(&g)?)
        }
        "wow_status" => Ok(json!({ "root": "", "clients": [], "problem": null })),
        "detect_wow" => Ok(json!([])),
        "contribution_preview" => to_json(preview()),
        "send_contribution" => to_json(send_contribution()),
        "send_issue" => {
            #[derive(Deserialize)]
            struct A {
                draft: IssueDraft,
            }
            to_json(send_issue(args::<A>(a)?.draft))
        }
        "need_help" => {
            #[derive(Deserialize)]
            struct A {
                locale: String,
            }
            to_json(need_help(&args::<A>(a)?.locale))
        }
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
