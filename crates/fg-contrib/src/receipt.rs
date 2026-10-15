//! What the contribution server records of each contribution it accepts, one JSON line per
//! contribution in `index.jsonl` next to the files (`<edition>/<sha256>.json`). Nothing in it
//! leads back to a sender: no address, no time of day.

use serde::{Deserialize, Serialize};

pub const INDEX_FILE: &str = "index.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// SHA-256 of the stored document (hex): its file name, and what duplicates are recognized by.
    pub sha256: String,
    /// Day the server received it (days since 1970-01-01, UTC).
    pub received: u32,
    pub edition: String,
    /// The app install's random id (`Contribution::contributor`): pseudonymous, it tells how many
    /// different players back a fact.
    pub contributor: String,
    /// Keyed hash of the sender's network (its /24 for IPv4, its /48 for IPv6) cut to 16 bits
    /// (4 hex digits): tells contributions sent from independent places apart, while each value
    /// stands for hundreds of networks, so it does not lead back to an address.
    pub client: String,
    pub app_version: String,
    pub bytes: u64,
    pub quests: usize,
    pub events: usize,
}
