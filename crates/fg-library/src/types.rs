use fg_route::export::diff::{Change, RouteDiff};
use fg_route::job::{PlanOutcome, PlanRequest};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuideInfo {
    pub name: String,
    /// Version the characters follow (None: the latest).
    #[serde(default)]
    pub installed: Option<u32>,
}

/// Summary of one version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMeta {
    /// Guide folder (slug of the name).
    pub guide: String,
    pub version: u32,
    pub name: String,
    /// Unix seconds.
    pub created: u64,
    pub race: String,
    pub class: String,
    #[serde(default)]
    pub group: Vec<String>,
    pub from_level: i64,
    pub to_level: i64,
    pub total_time: f64,
    pub total_time_text: String,
    pub quests: usize,
    /// Game data used (version of the quest data imported).
    pub data: Option<String>,
    /// Received from someone else (imported from a .fgguide file).
    #[serde(default)]
    pub imported: bool,
    /// Edited by hand (from the version before it).
    #[serde(default)]
    pub edited: bool,
    /// The character met in game the guide starts from ("Name-Realm").
    #[serde(default)]
    pub character: Option<String>,
}

/// A guide with its versions, newest first.
#[derive(Debug, Serialize)]
pub struct GuideSummary {
    pub id: String,
    pub name: String,
    pub installed: Option<u32>,
    pub versions: Vec<VersionMeta>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuideVersion {
    pub meta: VersionMeta,
    pub request: PlanRequest,
    pub outcome: PlanOutcome,
}

/// Where a version comes from, when not from a computation.
#[derive(Debug, Clone, Copy, Default)]
pub struct Origin {
    pub imported: bool,
    pub edited: bool,
}

impl Origin {
    pub const COMPUTED: Origin = Origin {
        imported: false,
        edited: false,
    };
    pub const IMPORTED: Origin = Origin {
        imported: true,
        edited: false,
    };
    pub const EDITED: Origin = Origin {
        imported: false,
        edited: true,
    };
}

/// Two versions of a guide compared: both, their differences, and the same as text.
#[derive(Serialize)]
pub struct Comparison {
    pub a: GuideVersion,
    pub b: GuideVersion,
    pub diff: RouteDiff,
    /// Settings that differ (the request, the game data).
    pub settings: Vec<Change>,
    /// The comparison as text (`fg diff`), to hand to someone looking for what went wrong.
    pub text: String,
}
