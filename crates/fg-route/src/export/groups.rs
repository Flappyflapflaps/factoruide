//! Steps done around the same place, one group in every addon: shown and passed together by
//! the guide's own addon, one step of Polestar's (open together, done in any order).

use super::types::{Step, StepKind};

/// How far from the first step of a group another still joins it (yards): the NPCs of a town,
/// the mobs of a keep (Northwatch Hold's captain, prisoner and cannoneers lie within 190
/// yards), not the camps of a region.
pub const NEAR: f64 = 200.0;
/// Steps of a group at most: a page the player reads at a glance.
pub const MAX: usize = 10;

/// A trip, or a grind held until a level: alone in its group.
fn alone(kind: StepKind) -> bool {
    matches!(
        kind,
        StepKind::Fly
            | StepKind::Link
            | StepKind::Hearth
            | StepKind::Teleport
            | StepKind::Grind
            | StepKind::DeathSkip
            | StepKind::Dungeon
    )
}

/// Whether each step joins the group of the step before: no trip between them, on the same
/// continent within `NEAR` of the group's first step (or at the same NPC, mob or object), the
/// group not full.
pub fn joins(steps: &[Step]) -> Vec<bool> {
    let mut out = Vec::with_capacity(steps.len());
    let (mut first, mut size): (Option<&Step>, usize) = (None, 0);
    for s in steps {
        let join = !alone(s.kind)
            && size < MAX
            && first.is_some_and(|f| {
                let near = match (f.world, s.world) {
                    (Some([c, x, y]), Some([sc, sx, sy])) => c as i64 == sc as i64 && (x - sx).hypot(y - sy) <= NEAR,
                    _ => false,
                };
                let same = f
                    .target
                    .as_ref()
                    .zip(s.target.as_ref())
                    .is_some_and(|(a, b)| a.id == b.id && a.kind == b.kind);
                near || same
            });
        if join {
            size += 1;
        } else {
            (first, size) = ((!alone(s.kind)).then_some(s), 1);
        }
        out.push(join);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn at(kind: &str, npc: i64, x: f64) -> Step {
        serde_json::from_value(json!({
            "kind": kind, "level": 10, "time": 0.0, "world": [1.0, x, 0.0],
            "target": { "kind": "npc", "id": npc, "name": "NPC" }
        }))
        .unwrap()
    }

    /// Within `NEAR` of the group's first step (not of the last one), the same NPC farther, a
    /// trip alone, at most `MAX` a group.
    #[test]
    fn steps_join_near_the_first_of_their_group() {
        let steps = [
            at("accept", 1, 0.0),
            at("accept", 2, 150.0),
            // 190 from the first, 40 from the one before.
            at("objective", 3, 190.0),
            // 40 from the one before, 230 from the first: another group.
            at("objective", 4, 230.0),
            // The first NPC of that group, far away.
            at("turnin", 4, 2000.0),
            at("fly", 5, 2000.0),
            at("accept", 6, 2000.0),
        ];
        assert_eq!(joins(&steps), [false, true, true, false, true, false, false]);
        let town: Vec<Step> = (0..MAX + 2).map(|i| at("accept", i as i64, 1.0)).collect();
        let joined = joins(&town);
        assert_eq!(joined.iter().filter(|j| !**j).count(), 2, "{joined:?}");
        assert!(!joined[MAX]);
    }
}
