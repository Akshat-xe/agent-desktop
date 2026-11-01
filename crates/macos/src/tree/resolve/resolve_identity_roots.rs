//! Which live scope an identity count may search: the window recorded at capture, never the
//! surface that holds focus now, and a refusal when no window was recorded.

use agent_desktop_core::{AdapterError, RefEntry, SnapshotSurface};

use super::resolve_read_context::ResolveReadContext;
use super::resolve_roots::{CandidateRoots, candidate_roots, source_window_scoped_roots};
use crate::tree::properties::element::element_for_pid;

/// The live roots an identity count may search.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::tree) enum IdentityRootScope {
    RecordedWindow,
    CapturedSurface,
    RefuseMissingWindow,
}

pub(in crate::tree) fn identity_root_scope(entry: &RefEntry) -> IdentityRootScope {
    if entry.source.source_surface != SnapshotSurface::Focused {
        return IdentityRootScope::CapturedSurface;
    }
    if recorded_source_window(entry) {
        IdentityRootScope::RecordedWindow
    } else {
        IdentityRootScope::RefuseMissingWindow
    }
}

pub(in crate::tree) fn identity_scope_roots(
    entry: &RefEntry,
    context: &mut ResolveReadContext,
) -> Result<CandidateRoots, AdapterError> {
    match identity_root_scope(entry) {
        IdentityRootScope::CapturedSurface => candidate_roots(entry, context),
        IdentityRootScope::RefuseMissingWindow => {
            Err(AdapterError::element_not_found("saved source window"))
        }
        IdentityRootScope::RecordedWindow => {
            let pid = crate::system::process::process_identity::to_pid_t(entry.process.pid)?;
            let application = element_for_pid(pid);
            source_window_scoped_roots(&application, entry, context)
        }
    }
}

fn recorded_source_window(entry: &RefEntry) -> bool {
    entry.source.source_window_id.is_some()
        || entry
            .source
            .source_window_title
            .as_deref()
            .is_some_and(|title| !title.trim().is_empty())
}

#[cfg(test)]
#[path = "resolve_identity_roots_tests.rs"]
mod tests;
