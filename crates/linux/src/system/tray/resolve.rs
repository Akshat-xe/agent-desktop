use agent_desktop_core::refs::ref_identity::{identity_match, retained_object_match};
use agent_desktop_core::{
    AdapterError, Deadline, IdentifierKind, IdentityMatch, LocatorField, NativeHandle, RefEntry,
};

use super::{TrayTarget, identity, live};

fn malformed() -> AdapterError {
    AdapterError::stale_ref_because("The ref's tray identity could not be read")
}

pub(super) fn target_of_entry(entry: &RefEntry) -> Result<TrayTarget, AdapterError> {
    entry
        .identity
        .native_id
        .as_ref()
        .filter(|id| id.kind == IdentifierKind::RuntimeId)
        .and_then(|id| identity::decode(&id.value))
        .ok_or_else(malformed)
}

/// Re-identifies a stored tray ref against the live bus. The ref binds the
/// unique bus name, and the live item must still carry the recorded role and
/// identity, so a retitled item or a restarted service never resolves.
pub(crate) fn resolve(entry: &RefEntry, deadline: Deadline) -> Result<NativeHandle, AdapterError> {
    let target = target_of_entry(entry)?;
    let live = live::element(&target, deadline)?;
    let same = live.state.role == entry.identity.role
        && identity_match(
            entry,
            &live.identity.name,
            &LocatorField::Absent,
            &live.identity.description,
            &live.identity.identifiers,
        ) == IdentityMatch::Match
        && retained_object_match(entry, &live.identity.identifiers) == IdentityMatch::Match;
    if same {
        Ok(NativeHandle::new(target))
    } else {
        Err(AdapterError::stale_ref_because(
            "The tray item no longer matches the identity the ref was taken from",
        ))
    }
}
