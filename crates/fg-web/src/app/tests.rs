//! The web backend's calls, as the page makes them, on the game data of the repository.

use super::*;
use crate::host::native::{FILES, REPLIES};

/// A call as the page makes it: the answer's value, or its error.
fn call(cmd: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    let answer: serde_json::Value = serde_json::from_str(&dispatch(
        serde_json::json!({ "cmd": cmd, "args": args }).to_string().as_bytes(),
    ))
    .unwrap();
    match answer.get("err") {
        Some(e) => Err(e.as_str().unwrap_or_default().to_owned()),
        None => Ok(answer["ok"].clone()),
    }
}

/// The Classic database served as the site serves it.
fn serve_classic() {
    let db = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/classic.sqlite.zst");
    let mut files = FILES.lock().unwrap();
    if files.is_empty() {
        files.push(("data/classic.sqlite.zst".into(), std::fs::read(db).unwrap()));
    }
}

/// What `/fg export` gives for a dwarf warrior.
fn profile_export(name: &str, level: i64) -> String {
    let json = serde_json::json!({
        "format": "factoruide-profile", "version": 1, "edition": "classic", "addon": "0.13.0",
        "character": name,
        "profile": {
            "name": name, "race": 3, "class": "WARRIOR", "faction": "Alliance", "level": level,
            "xp": 1200, "xp_max": 2800, "rested": 0, "resting": false, "completed": [179, 233, 234],
            "log": [], "position": { "map": 1426, "x": 0.468, "y": 0.52 }, "instance": false,
            "bind": { "name": "Coldridge Valley" }, "flights": [], "professions": [], "riding": 0,
            "gear": [], "spells": [], "reputation": [], "money": 1234, "time": 1_760_800_000
        },
    });
    fgstring::encode(fgstring::PROFILE_PREFIX, json.to_string().as_bytes())
}

/// From a character pasted to the text the addon imports: the guide made from it, followed by
/// it, sent to the game; its recordings kept until sent; everything saved for the next visit.
#[test]
fn a_character_pasted_gets_its_guide_into_the_game() {
    serve_classic();
    store::clear();
    call("set_edition", &serde_json::json!({ "edition": "classic" })).unwrap();

    // A text that is not an export, or damaged, says so.
    assert_eq!(
        call("import_profile", &serde_json::json!({ "content": "hello" })).unwrap_err(),
        "not_a_profile"
    );
    let export = profile_export("Brouz-Realm", 5);
    let damaged = format!("{}x!", &export[..export.len() / 2]);
    assert_eq!(
        call("import_profile", &serde_json::json!({ "content": damaged })).unwrap_err(),
        "corrupt_profile"
    );

    let imported = call("import_profile", &serde_json::json!({ "content": export })).unwrap();
    assert_eq!(imported["character"], "Brouz-Realm");
    assert_eq!(imported["level"], 5);
    // Pasted again later: the profile replaced.
    let again = profile_export("Brouz-Realm", 6);
    assert_eq!(
        call("import_profile", &serde_json::json!({ "content": again })).unwrap()["level"],
        6
    );
    let characters = call("list_characters", &serde_json::json!({})).unwrap();
    assert_eq!(characters.as_array().unwrap().len(), 1);
    assert_eq!(characters[0]["race"], "dwarf");
    assert_eq!(characters[0]["class"], "warrior");

    // A guide from the character, a short one.
    let mut request: serde_json::Value =
        serde_json::from_str(r#"{"race":"dwarf","class":"warrior","from_level":6,"to_level":8,"name":"Brouz web"}"#)
            .unwrap();
    request["start"] = characters[0]["profile"].clone();
    let mut params = serde_json::to_value(overrides(Edition::Classic).unwrap().params).unwrap();
    params["effort"] = 0.0.into();
    request["params"] = params;
    let meta = call("plan_route", &serde_json::json!({ "request": request })).unwrap();
    assert_eq!(meta["character"], "Brouz-Realm");
    let guide = meta["guide"].as_str().unwrap().to_owned();
    assert_eq!(
        call("list_guides", &serde_json::json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Its text for the game: the guide, when it was computed and its version, the places.
    let export = call("guide_text", &serde_json::json!({ "guide": guide, "version": 1 })).unwrap();
    let lua =
        String::from_utf8(fgstring::decode(fgstring::GUIDE_PREFIX, export["text"].as_str().unwrap()).unwrap()).unwrap();
    assert!(lua.contains("FG.Routes[\"Brouz web\"] = {"), "the guide");
    assert!(
        lua.contains(&format!("FG.Routes[\"Brouz web\"].created = {}", meta["created"])),
        "when it was computed"
    );
    assert!(lua.contains("FG.Routes[\"Brouz web\"].version = 1"), "its version");
    assert!(lua.contains("FG.Services"), "the places");
    assert!(
        !lua.contains("FG.Assignments") && !lua.contains("FG.Config") && !lua.contains("FactoruideSent"),
        "no assignment, no settings, no sends"
    );
    // Every guide, as the data addon, zipped.
    let zip = call("data_zip", &serde_json::json!({})).unwrap();
    assert_eq!(zip["file"], "Factoruide_Data.zip");
    assert!(zip["base64"].as_str().unwrap().len() > 1000);

    // Watched, edited by hand, compared: the calls of the guide's page.
    let timeline = call(
        "guide_timeline",
        &serde_json::json!({ "guide": guide, "version": 1, "locale": "frFR" }),
    )
    .unwrap();
    assert_ne!(timeline["steps"].as_array().unwrap().len(), 0);
    let opened = call(
        "edit_open",
        &serde_json::json!({ "guide": guide, "version": 1, "locale": "enUS" }),
    )
    .unwrap();
    let mut steps = opened["steps"].as_array().unwrap().clone();
    assert!(steps.len() > 2);
    steps.pop();
    let view = call(
        "edit_simulate",
        &serde_json::json!({ "steps": steps, "locale": "enUS" }),
    )
    .unwrap();
    assert_eq!(view["report"]["steps"].as_array().unwrap().len(), steps.len());
    call(
        "edit_suggest",
        &serde_json::json!({ "steps": steps, "after": 0, "locale": "enUS" }),
    )
    .unwrap();
    let edited = call("edit_save", &serde_json::json!({ "steps": steps })).unwrap();
    assert_eq!(
        (edited["version"].as_u64(), edited["edited"].as_bool()),
        (Some(2), Some(true))
    );
    let compared = call(
        "compare_versions",
        &serde_json::json!({ "guide": guide, "a": 1, "b": 2, "locale": "enUS" }),
    )
    .unwrap();
    assert_ne!(compared["text"].as_str().unwrap().len(), 0);
    call("delete_version", &serde_json::json!({ "guide": guide, "version": 2 })).unwrap();
    // The other pages' data.
    assert_ne!(
        call("map_index", &serde_json::json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(call("get_options", &serde_json::json!({})).unwrap()["max_level"], 60);

    // A file to share comes back as a new version.
    let shared = call("export_version", &serde_json::json!({ "guide": guide, "version": 1 })).unwrap();
    assert_eq!(shared["file"], format!("{guide}-v1.fgguide"));
    let v2 = call("import_guide", &serde_json::json!({ "content": shared["text"] })).unwrap();
    assert_eq!(
        (v2["version"].as_u64(), v2["imported"].as_bool()),
        (Some(2), Some(true))
    );

    // Every file the page saved, given back at the next visit, gives the same guides.
    let saved: Vec<(String, Vec<u8>)> = ["settings.json", &format!("guides/classic/{guide}/guide.json")]
        .iter()
        .map(|p| ((*p).to_owned(), store::read(p).unwrap()))
        .collect();
    for (p, d) in saved {
        store::load(&p, d);
    }
    assert_eq!(
        call("get_guide", &serde_json::json!({ "id": guide })).unwrap()["versions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // Deleted: nothing left to send.
    call("delete_guide", &serde_json::json!({ "guide": guide })).unwrap();
    assert_eq!(call("data_zip", &serde_json::json!({})).unwrap_err(), "no guide yet");
    let _ = REPLIES.lock().unwrap().drain(..);
}
