use crate::LibraryError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The identifier of a guide: the name of its folder. It comes from a guide's name (`from_name`)
/// or from the page (`TryFrom<String>`, so that serde checks it): one that could point out of
/// the guides' folder is refused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GuideId(String);

impl GuideId {
    /// The identifier of the guide of this name.
    pub fn from_name(name: &str) -> Self {
        Self(fg_route::job::slug(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GuideId {
    type Error = LibraryError;

    fn try_from(id: String) -> Result<Self, LibraryError> {
        let bad = id.is_empty()
            || id == "."
            || id == ".."
            || id
                .chars()
                .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control());
        if bad {
            Err(LibraryError::BadId(id))
        } else {
            Ok(Self(id))
        }
    }
}

impl From<GuideId> for String {
    fn from(id: GuideId) -> Self {
        id.0
    }
}

impl AsRef<str> for GuideId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GuideId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
