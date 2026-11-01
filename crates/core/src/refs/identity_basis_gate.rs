//! Resolution rules that follow from a ref's recorded identity basis.

use crate::{AdapterError, IdentityBasis, RefEntryIdentity};

/// Refuses a ref before any live search when its basis cannot single out one element: a
/// basis-less entry is stale, and an ambiguous one never resolves.
pub fn basis_gate(identity: &RefEntryIdentity) -> Result<(), AdapterError> {
    match identity.basis {
        None => Err(AdapterError::stale_ref_because(
            "The saved ref was recorded before identity evidence was kept",
        )),
        Some(IdentityBasis::Ambiguous) => Err(AdapterError::ambiguous_target(
            "The saved ref has no identifier, name or description that was unique when it was captured",
        )
        .with_suggestion(
            "Use find with --name, --value or --nth to choose the element by its current content",
        )
        .with_details(serde_json::json!({
            "kind": "identity_not_unique",
            "role": identity.role,
        }))),
        Some(_) => Ok(()),
    }
}

/// Whether the ref resolves only when a complete live search finds exactly one
/// element matching its recorded identity.
pub fn requires_live_uniqueness(identity: &RefEntryIdentity) -> bool {
    matches!(
        identity.basis,
        Some(IdentityBasis::NativeId | IdentityBasis::Name | IdentityBasis::Description)
    )
}

/// Whether bounds may pick or confirm a candidate; only positional refs, which
/// carry no identity evidence, rely on geometry.
pub fn bounds_may_decide(identity: &RefEntryIdentity) -> bool {
    identity.basis == Some(IdentityBasis::Positional)
}
