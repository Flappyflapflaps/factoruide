use fg_library::LibraryError;
use fg_route::job::{Cancelled, JobError, RequestError};
use serde::{Serialize, Serializer};

/// Why a command of the app failed. The page gets the text (it recognizes `cancelled` and
/// `superseded` by it).
#[derive(Debug, thiserror::Error)]
pub enum CmdError {
    /// The generation was given up (`Studio::cancel_plan`).
    #[error("cancelled")]
    Cancelled,
    /// A replay of the editor was asked for again meanwhile.
    #[error("superseded")]
    Superseded,
    #[error("no guide open in the editor")]
    NoEditor,
    #[error(transparent)]
    Library(#[from] LibraryError),
    /// What the player asked for does not hold together.
    #[error(transparent)]
    Request(RequestError),
    /// The settings leave no route that works.
    #[error("no valid route found ({0})")]
    NoRoute(String),
    /// The planner or the game data failed.
    #[error("{0:#}")]
    Planner(anyhow::Error),
    /// Something only the app knows: a file, a setting, the game folder...
    #[error("{0}")]
    App(String),
}

impl From<anyhow::Error> for CmdError {
    fn from(e: anyhow::Error) -> Self {
        if e.is::<Cancelled>() {
            Self::Cancelled
        } else {
            Self::Planner(e)
        }
    }
}

impl From<JobError> for CmdError {
    fn from(e: JobError) -> Self {
        match e {
            JobError::Cancelled => Self::Cancelled,
            JobError::Request(request) => Self::Request(request),
            JobError::NoRoute(why) => Self::NoRoute(why),
            JobError::Other(e) => Self::Planner(e),
        }
    }
}

impl From<rusqlite::Error> for CmdError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Planner(e.into())
    }
}

impl From<String> for CmdError {
    fn from(message: String) -> Self {
        Self::App(message)
    }
}

impl From<&str> for CmdError {
    fn from(message: &str) -> Self {
        Self::App(message.to_owned())
    }
}

impl From<CmdError> for String {
    fn from(e: CmdError) -> Self {
        e.to_string()
    }
}

impl Serialize for CmdError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
