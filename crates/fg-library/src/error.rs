use std::io;

/// Why the library could not do what was asked.
#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io { path: String, source: io::Error },
    /// A file does not hold what it should.
    #[error("{path}: {source}")]
    Corrupt { path: String, source: serde_json::Error },
    /// A guide computed by another version of the app that this one cannot read.
    #[error("{0:#}")]
    Outcome(anyhow::Error),
    /// An identifier that could point out of the guides' folder.
    #[error("not a guide identifier: {0:?}")]
    BadId(String),
    /// No such guide.
    #[error("no guide {0}")]
    NoGuide(String),
    /// A file received that is not a guide.
    #[error("not a Factoruide guide file{}", .0.as_deref().map(|e| format!(": {e}")).unwrap_or_default())]
    NotAGuideFile(Option<String>),
}

impl LibraryError {
    pub(crate) fn io(path: &str, source: io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            source,
        }
    }

    pub(crate) fn corrupt(path: &str, source: serde_json::Error) -> Self {
        Self::Corrupt {
            path: path.to_owned(),
            source,
        }
    }
}
