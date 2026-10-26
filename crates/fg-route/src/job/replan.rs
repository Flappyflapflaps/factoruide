//! "Re-plan from here": a character asks in game (the addon's Replan.lua) for its route to be
//! computed again from where it stands. The app plans it with the settings of the guide the
//! character followed (or, for a guide it did not make, those of its "New guide" page), from the
//! character as the addon saved it at the reload, and installs it: the route carries who asked and
//! when (`Route::replan`), so that the addon makes that character follow it.

use super::{Options, PlanRequest, StartState};
use crate::export::Replanned;

/// The name of the route computed again for `character` (`Name-Realm`) from the guide `base`,
/// whose request started from `base_start`: the same guide when it was this character's already
/// (a new version of it), else a guide of its own, named after the character's first name, so
/// that the other characters following `base` keep it as it is.
pub fn replan_name(base: &str, base_start: Option<&str>, character: &str) -> String {
    if base_start == Some(character) {
        return base.to_owned();
    }
    let first = character.split('-').next().unwrap_or(character);
    let own = format!("{base} ({first})");
    // Already this character's own copy (its start dropped by an edit): the same one.
    if base.ends_with(&format!(" ({first})")) {
        return base.to_owned();
    }
    own
}

/// The request computing again, for `start`, the guide planned by `base`: the same settings (play
/// style, zones, dungeons, class quests, professions, language), from where the character stands
/// now; the other players of a group guide as last recorded (`profiles`, by name), or as they were.
pub fn replan_request(base: &PlanRequest, name: String, start: StartState, profiles: &[StartState]) -> PlanRequest {
    let mates = base
        .mates
        .iter()
        .map(|m| profiles.iter().find(|p| p.name == m.name).unwrap_or(m).clone())
        .collect();
    PlanRequest {
        from_level: start.level,
        name: Some(name),
        start: Some(start),
        mates,
        ..base.clone()
    }
}

/// The request for a guide the app did not make (a ready-made one, one imported): the "New
/// guide" page's settings for the character's race and class (`race`, `class`: keys), to the
/// level cap.
pub fn default_request(options: &Options, race: &str, class: &str, name: String, start: StartState) -> PlanRequest {
    let class_name = options
        .classes
        .iter()
        .find(|c| c.key == class)
        .map_or_else(|| class.to_owned(), |c| c.name.to_lowercase());
    PlanRequest {
        race: race.into(),
        class: class.into(),
        from_level: start.level,
        to_level: options.max_level,
        name: Some(name),
        params: Some(options.params.clone()),
        required_class_quests: Some(options.class_quests.get(&class_name).cloned().unwrap_or_default()),
        professions: Vec::new(),
        locale: None,
        start: Some(start),
        mates: Vec::new(),
    }
}

/// What the route computed again says of the request it answers.
pub fn replanned(character: &str, asked: i64, level: i64) -> Replanned {
    Replanned {
        character: character.to_owned(),
        asked,
        level,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character(name: &str, level: i64) -> StartState {
        StartState {
            name: name.into(),
            level,
            ..StartState::default()
        }
    }

    #[test]
    fn a_new_version_for_the_same_character_else_a_guide_of_its_own() {
        assert_eq!(
            replan_name("Gnome Mage", Some("Brouz-Realm"), "Brouz-Realm"),
            "Gnome Mage",
            "the character's own guide: a new version"
        );
        assert_eq!(
            replan_name("Ready-made Gnome Mage 1-60", None, "Brouz-Realm"),
            "Ready-made Gnome Mage 1-60 (Brouz)",
            "a guide for anyone: one of its own"
        );
        assert_eq!(
            replan_name("Gnome Mage (Brouz)", None, "Brouz-Realm"),
            "Gnome Mage (Brouz)",
            "its own copy again"
        );
        assert_eq!(
            replan_name("Gnome Mage", Some("Pata-Realm"), "Brouz-Realm"),
            "Gnome Mage (Brouz)",
            "another character's guide: one of its own"
        );
    }

    #[test]
    fn the_guides_settings_from_where_the_character_stands() {
        let base = PlanRequest {
            race: "gnome".into(),
            class: "mage".into(),
            from_level: 1,
            to_level: 60,
            name: Some("Gnome Mage".into()),
            params: None,
            required_class_quests: Some(vec!["Arcane Missiles".into()]),
            professions: Vec::new(),
            locale: Some("frFR".into()),
            start: Some(character("Brouz-Realm", 5)),
            mates: vec![character("Pata-Realm", 6), character("Gone-Realm", 7)],
        };
        let profiles = [character("Pata-Realm", 15), character("Other-Realm", 30)];
        let request = replan_request(&base, "Gnome Mage".into(), character("Brouz-Realm", 14), &profiles);
        assert_eq!(request.from_level, 14, "from the character's level");
        assert_eq!(request.start.as_ref().map(|s| s.level), Some(14));
        assert_eq!(
            (request.to_level, request.locale.as_deref()),
            (60, Some("frFR")),
            "same settings"
        );
        assert_eq!(request.required_class_quests, base.required_class_quests);
        let mates: Vec<(&str, i64)> = request.mates.iter().map(|m| (m.name.as_str(), m.level)).collect();
        assert_eq!(
            mates,
            [("Pata-Realm", 15), ("Gone-Realm", 7)],
            "the group as last recorded"
        );
    }
}
