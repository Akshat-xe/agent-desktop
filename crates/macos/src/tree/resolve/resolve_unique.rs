//! Live uniqueness for refs whose recorded identity singles them out.
//!
//! A ref that carries identity resolves only when a complete search of its
//! source scope finds exactly one element matching the recorded role and
//! identity evidence. Bounds never choose between candidates, and a stored
//! path is not evidence: it locates nothing on its own.

use agent_desktop_core::{AdapterError, IdentityMatch, LocatorEvidence, NativeHandle, RefEntry};
use rustc_hash::FxHashSet;

use super::resolve_classify::{ambiguous_target_error, retained_candidate_handle};
use super::resolve_errors::{identity_unknown, incomplete_traversal};
use super::resolve_read_context::ResolveReadContext;
use super::resolve_search::{identity_requirements, match_native_or_text_identity, read_node};
use crate::tree::AXElement;
use crate::tree::element_dedupe::push_unique;

/// What a live match count means for a ref that carries identity. A proven
/// duplicate never resolves, whether or not the search finished; an unfinished
/// search never resolves, even with one match already in hand.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::tree) enum UniqueOutcome {
    Resolved,
    Absent,
    Duplicate,
    Incomplete,
}

pub(in crate::tree) fn classify_unique_matches(
    match_count: usize,
    complete: bool,
) -> UniqueOutcome {
    if match_count > 1 {
        return UniqueOutcome::Duplicate;
    }
    if !complete {
        return UniqueOutcome::Incomplete;
    }
    if match_count == 1 {
        UniqueOutcome::Resolved
    } else {
        UniqueOutcome::Absent
    }
}

/// The verdict one live element earns against a ref's recorded identity. The
/// role must be equal before any identity evidence counts, and an unreadable
/// role is an unfinished count rather than an absence.
pub(in crate::tree) fn identity_candidate_verdict(
    entry: &RefEntry,
    evidence: &LocatorEvidence,
) -> IdentityMatch {
    match evidence.role.known() {
        Some(role) if role == &entry.identity.role => {
            match_native_or_text_identity(entry, evidence)
        }
        Some(_) => IdentityMatch::NoMatch,
        None if evidence.role.is_unknown() => IdentityMatch::Unknown,
        None => IdentityMatch::NoMatch,
    }
}

#[derive(Default)]
pub(in crate::tree) struct UniqueTally {
    matches: Vec<AXElement>,
    incomplete: Option<AdapterError>,
}

impl UniqueTally {
    pub(in crate::tree) fn record(&mut self, candidate: AXElement) {
        push_unique(&mut self.matches, candidate);
    }

    pub(in crate::tree) fn mark_incomplete(&mut self, error: AdapterError) {
        if self.incomplete.is_none() {
            self.incomplete = Some(error);
        }
    }

    fn duplicate_proven(&self) -> bool {
        self.matches.len() > 1
    }

    /// Answers every count-only outcome first, so a proven duplicate stays a
    /// duplicate without any candidate being inspected: no stored geometry, live
    /// bounds read or element field may turn two matches into one.
    pub(in crate::tree) fn into_resolution(
        self,
        entry: &RefEntry,
    ) -> Result<NativeHandle, AdapterError> {
        match classify_unique_matches(self.matches.len(), self.incomplete.is_none()) {
            UniqueOutcome::Duplicate => Err(ambiguous_target_error(&self.matches, entry)),
            UniqueOutcome::Incomplete => Err(self
                .incomplete
                .unwrap_or_else(|| incomplete_traversal("identity_uniqueness", 0))),
            UniqueOutcome::Absent => Err(AdapterError::element_not_found("element")),
            UniqueOutcome::Resolved => match self.matches.into_iter().next() {
                Some(candidate) => retained_candidate_handle(candidate),
                None => Err(AdapterError::element_not_found("element")),
            },
        }
    }
}

pub(in crate::tree) fn find_unique_entry_in_roots(
    roots: &[AXElement],
    entry: &RefEntry,
    resolve_depth: u8,
    read_context: &mut ResolveReadContext,
) -> Result<NativeHandle, AdapterError> {
    let mut tally = UniqueTally::default();
    for root in roots {
        if tally.duplicate_proven() {
            break;
        }
        let mut ancestors = FxHashSet::default();
        let mut context = UniqueCollect {
            entry,
            max_depth: resolve_depth,
            ancestors: &mut ancestors,
            tally: &mut tally,
            read_context,
        };
        collect_unique_recursive(root, 0, &mut context)?;
    }
    tally.into_resolution(entry)
}

struct UniqueCollect<'a> {
    entry: &'a RefEntry,
    max_depth: u8,
    ancestors: &'a mut FxHashSet<usize>,
    tally: &'a mut UniqueTally,
    read_context: &'a mut ResolveReadContext,
}

fn collect_unique_recursive(
    element: &AXElement,
    depth: u8,
    context: &mut UniqueCollect<'_>,
) -> Result<(), AdapterError> {
    if context.tally.duplicate_proven() {
        return Ok(());
    }
    crate::tree::locator_deadline::remaining(context.read_context.deadline)?;
    let pointer = element.0 as usize;
    if !context.ancestors.insert(pointer) {
        return Ok(());
    }
    let read = read_node(
        element,
        identity_requirements(context.entry),
        context.read_context,
    )?;
    if read.invalid_element {
        context
            .tally
            .mark_incomplete(incomplete_traversal("invalid_element", depth));
        context.ancestors.remove(&pointer);
        return Ok(());
    }

    match identity_candidate_verdict(context.entry, &read.evidence) {
        IdentityMatch::Match => context.tally.record(element.clone()),
        IdentityMatch::Unknown => {
            context
                .tally
                .mark_incomplete(identity_unknown(context.entry));
        }
        IdentityMatch::NoMatch => {}
    }

    if depth >= context.max_depth {
        if !read.child_read.elements.is_empty()
            || !read.child_read.complete
            || read.child_read.truncated()
        {
            context
                .tally
                .mark_incomplete(incomplete_traversal("depth_limit", depth));
        }
    } else {
        if !read.child_read.complete || read.child_read.truncated() {
            context
                .tally
                .mark_incomplete(incomplete_traversal("children", depth));
        }
        for child in &read.child_read.elements {
            collect_unique_recursive(child, depth.saturating_add(1), context)?;
            if context.tally.duplicate_proven() {
                break;
            }
        }
    }

    context.ancestors.remove(&pointer);
    Ok(())
}

#[cfg(test)]
#[path = "resolve_unique_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resolve_unique_more_tests.rs"]
mod tests_more;
