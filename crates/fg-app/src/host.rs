use crate::CmdError;
use fg_library::Files;
use fg_route::job::Overrides;
use fg_route::xp::Edition;
use rusqlite::Connection;

/// What the app around gives the [`Studio`](crate::Studio): the desktop app's folders and
/// database files, or the browser's storage and downloads.
pub trait Host: Sync {
    type Files: Files;

    /// Where the guides are kept.
    fn files(&self) -> &Self::Files;
    /// The game version the app is set to.
    fn edition(&self) -> Edition;
    /// A read-only connection to the game data of an edition.
    fn open_db(&self, edition: Edition) -> Result<Connection, CmdError>;
    /// The corrections to the game data of an edition.
    fn overrides(&self, edition: Edition) -> Result<Overrides, CmdError>;
    /// A line of what a generation is doing (it may come from several threads).
    fn progress(&self, line: &str);
}
