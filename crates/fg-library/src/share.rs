use crate::{GuideVersion, LibraryError};
use fg_route::job::{PlanOutcome, PlanRequest};
use serde::{Deserialize, Serialize};

const SHARE_FORMAT: &str = "factoruide-guide";
/// Format of the files shared before the rename (BrouzQuest), still accepted.
const LEGACY_SHARE_FORMAT: &str = "brouzquest-result";

/// What a .fgguide (formerly .bqroute) file holds: a guide version (configuration and computed
/// guide) to share.
#[derive(Debug, Serialize, Deserialize)]
pub struct SharedGuide {
    format: String,
    version: u32,
    meta: serde_json::Value,
    pub request: PlanRequest,
    pub outcome: PlanOutcome,
}

impl SharedGuide {
    pub(crate) fn of(v: GuideVersion) -> Result<Self, LibraryError> {
        Ok(Self {
            format: SHARE_FORMAT.into(),
            version: 2,
            meta: serde_json::to_value(&v.meta).map_err(|e| LibraryError::corrupt("meta", e))?,
            request: v.request,
            outcome: v.outcome,
        })
    }

    /// The text of the file.
    ///
    /// # Errors
    ///
    /// [`LibraryError::NotAGuideFile`] when the text is not a guide of this app, [`LibraryError::Outcome`] when its computed guide cannot be read.
    pub fn to_json(&self) -> Result<String, LibraryError> {
        serde_json::to_string(self).map_err(|e| LibraryError::corrupt("guide", e))
    }

    /// The guide of a file received, whatever version of the app made it.
    ///
    /// # Errors
    ///
    /// [`LibraryError::NotAGuideFile`] when the text is not a guide of this app, [`LibraryError::Outcome`] when its computed guide cannot be read.
    pub fn parse(content: &str) -> Result<Self, LibraryError> {
        let not_a_guide = |e: serde_json::Error| LibraryError::NotAGuideFile(Some(e.to_string()));
        let mut shared: serde_json::Value = serde_json::from_str(content).map_err(not_a_guide)?;
        // A guide made by another version of the app, in the format of this one.
        if let Some(outcome) = shared.get_mut("outcome") {
            let o = PlanOutcome::from_value(outcome.take()).map_err(LibraryError::Outcome)?;
            *outcome = serde_json::to_value(o).map_err(not_a_guide)?;
        }
        let shared: Self = serde_json::from_value(shared).map_err(not_a_guide)?;
        if shared.format != SHARE_FORMAT && shared.format != LEGACY_SHARE_FORMAT {
            return Err(LibraryError::NotAGuideFile(None));
        }
        Ok(shared)
    }

    /// The game data the guide was made with.
    pub fn data(&self) -> Option<String> {
        self.meta.get("data").and_then(|d| d.as_str()).map(str::to_owned)
    }
}
