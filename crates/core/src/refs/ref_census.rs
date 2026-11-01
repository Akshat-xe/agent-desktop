use serde::{Deserialize, Serialize};

/// The depth and observation mode a depth-cut snapshot counted its look-alikes over. An entry
/// that records it resolves only by a live count that replays the same observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Census {
    pub depth: u8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skeleton: bool,
}
