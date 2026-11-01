use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_ref: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub path_is_absolute: bool,
    /// The skeleton depth whose observed nodes counted this entry's look-alikes; the entry
    /// resolves only by a live count over that same depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub census_depth: Option<u8>,
    #[serde(default, skip_serializing_if = "smallvec::SmallVec::is_empty")]
    pub path: crate::refs::RefPath,
}

fn is_false(value: &bool) -> bool {
    !*value
}
