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
            "If the elements differ in identifier, name or description, find one with --name and act on the new ref. If they are identical, no headless command can pick one; a headed run can read bounds with snapshot --include-bounds and use --headed mouse-click --xy",
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
    identity.basis.is_some_and(IdentityBasis::is_proven)
}
