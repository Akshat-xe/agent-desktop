//! Resolution for a ref that carries a recorded identity basis.
//!
//! A ref whose basis is `NativeId`, `Name` or `Description` promises a unique
//! current match, not object continuity: it resolves only when a complete walk
//! of its source scope finds exactly one live element whose role and recorded
//! identity fields agree with it. The stored child-index path and the stored
//! rectangle have no say in that verdict, so this tier runs no path fast path
//! and no geometry promotion, and a walk that could not read part of the scope
//! never resolves - not even when it has already found one match.
//!
//! Split from `resolve_search.rs`, which is at the file-size cap. The search
//! there keeps the path and bounds tiers, which apply only to refs whose basis
//! lets bounds decide.

use agent_desktop_core::{AdapterError, Deadline, LocatorEvidence, NativeHandle, RefEntry};

use super::resolve_match::{
    Candidate, admitted_outcome, ambiguous_target_error, candidate_outcome, role_admission,
    stale_ref_error,
};
use super::resolve_search::{
    MAX_RESOLVE_DEPTH, NodeAdmission, SearchContext, build_candidate, enumerate_children,
    identity_unknown_error, resolve_walk_budget,
};
use super::{SearchVerdict, into_verified_handle, resolve_window_root};
use crate::tree::element::UIAElement;
use crate::tree::walk::walker::{TreeSource, WalkBudget};
use crate::tree::walk::walker_source::UiaTreeSource;

/// Resolves a ref that must find one live element: its recorded identity, read
/// completely, or nothing.
///
/// The window gate runs first and is unchanged - a ref never searches a window
/// another process now owns - and every other tier is deliberately absent. A
/// sole candidate found while any part of the scope stayed unread is answered
/// with the incomplete-traversal error, which the caller's retry loop replays
/// until the scope is read or the deadline expires.
pub(crate) fn resolve_unique_identity(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;
    let budget = resolve_walk_budget(deadline);
    let mut found = Vec::new();
    let mut unread_region = false;
    let ctx = SearchContext {
        source: &source,
        entry,
        budget: &budget,
    };
    search_source_scope(&ctx, &prepared, 0, &mut found, &mut unread_region)?;
    match classify_identity_search(found.len(), unread_region) {
        SearchVerdict::Incomplete => Err(identity_unknown_error(entry)),
        SearchVerdict::Stale => Err(stale_ref_error(entry)),
        SearchVerdict::Resolved(index) => found
            .into_iter()
            .nth(index)
            .map(|candidate| into_verified_handle(candidate.element, entry))
            .ok_or_else(|| stale_ref_error(entry)),
        SearchVerdict::Ambiguous => Err(ambiguous_target_error(entry, found.len())),
    }
}

/// What a live count decides.
///
/// Two matching live elements are conclusive on their own - one more candidate
/// cannot make them fewer - so an unread region elsewhere cannot turn that
/// answer into anything but ambiguous, the same rule the positional search's
/// own classification applies. Zero and one are the counts a match in the
/// unread region could still change, so both are withheld while anything is
/// left unread.
pub(super) fn classify_identity_search(count: usize, unread_region: bool) -> SearchVerdict {
    match count {
        0 if unread_region => SearchVerdict::Incomplete,
        0 => SearchVerdict::Stale,
        1 if unread_region => SearchVerdict::Incomplete,
        1 => SearchVerdict::Resolved(0),
        _ => SearchVerdict::Ambiguous,
    }
}

/// Whether the identity search counts this live candidate, refutes it, or could
/// not decide.
///
/// The role gate is [`role_admission`] and the identity rule is
/// [`candidate_outcome`], both shared with the positional search, so the
/// AutomationId-first stable-text corroboration that refutes a candidate whose
/// identifier matches but whose text drifted (A7-3) is applied here unchanged.
/// The one arm this tier does not inherit from `admit_node` is geometry
/// promotion: a ref with a recorded identity basis carries meaningful identity,
/// and its bounds must never confirm it.
pub(crate) fn admit_identity_candidate(
    entry: &RefEntry,
    evidence: &LocatorEvidence,
) -> NodeAdmission {
    match role_admission(entry, evidence) {
        Some(verdict) => verdict,
        None => admitted_outcome(candidate_outcome(entry, evidence)),
    }
}

/// Walks the source scope counting every candidate the recorded identity
/// admits, and records every part of the scope it could not read.
fn search_source_scope(
    ctx: &SearchContext<'_>,
    element: &UIAElement,
    depth: u8,
    found: &mut Vec<Candidate>,
    unread_region: &mut bool,
) -> Result<(), AdapterError> {
    if ctx.budget.deadline.is_expired() {
        *unread_region = true;
        return Ok(());
    }
    let (_, evidence, _) = ctx.source.evidence(element);
    match admit_identity_candidate(ctx.entry, &evidence) {
        NodeAdmission::Collect => found.push(build_candidate(element, &evidence)),
        NodeAdmission::Unread => *unread_region = true,
        NodeAdmission::Reject => {}
    }
    if depth + 1 >= MAX_RESOLVE_DEPTH {
        *unread_region |= boundary_hides_descendants(ctx.source, element);
        return Ok(());
    }
    let children = enumerate_children(ctx.source, element, ctx.budget, unread_region)?;
    *unread_region |= sibling_list_was_cut(ctx.source, &children, ctx.budget);
    for child in &children {
        search_source_scope(ctx, child, depth + 1, found, unread_region)?;
    }
    Ok(())
}

/// Whether a node at the depth cap has children the search never read.
///
/// One child probe answers it, and it is the only way to tell a subtree the
/// depth budget cut from one that ends there: the search deliberately stops
/// above this level rather than enumerate children it would discard, so no
/// enumeration ever reports the cut. A probe that fails is not emptiness - an
/// unreadable subtree is exactly what a uniqueness count must not resolve past.
fn boundary_hides_descendants<S: TreeSource>(source: &S, element: &S::Node) -> bool {
    !matches!(source.first_child(element), Err(failure) if failure.is_exhaustion())
}

/// Whether the sibling cap cut the child list short.
///
/// The shared enumeration reports a cap-hit as a whole list, because no stored
/// ref names an element past that point. A look-alike can still sit there, so
/// for a uniqueness count the cap is one more unread region. The probe costs a
/// single sibling call and runs only when the list reached the cap; a failing
/// probe is unreadable rather than empty.
fn sibling_list_was_cut<S: TreeSource>(
    source: &S,
    children: &[S::Node],
    budget: &WalkBudget,
) -> bool {
    if children.len() < budget.max_siblings {
        return false;
    }
    match children.last() {
        Some(last) => !matches!(source.next_sibling(last), Err(failure) if failure.is_exhaustion()),
        None => false,
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_identity_tests.rs"]
mod tests;
