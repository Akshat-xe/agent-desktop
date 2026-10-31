use serde::{Deserialize, Serialize};

/// The snapshot evidence that singles a ref out from look-alikes. `None` on a
/// stored entry means it was written before bases were recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityBasis {
    NativeId,
    Name,
    Description,
    Ambiguous,
    Positional,
}
