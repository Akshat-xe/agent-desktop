//! Resolution for a ref whose recorded identity must find one live match: a complete walk of the
//! source scope, no path or bounds shortcuts, and nothing resolved past an unread region.

use agent_desktop_core::{AdapterError, Deadline, LocatorEvidence, NativeHandle, RefEntry};

use super::resolve_match::{
    admitted_outcome, ambiguous_target_error, candidate_outcome, role_admission, stale_ref_error,
};
use super::resolve_search::descent::{DescentPolicy, DescentVerdict, ExpiryPolicy};
use super::resolve_search::{
    MAX_RESOLVE_DEPTH, NodeAdmission, SearchContext, enumerate_children, identity_unknown_error,
    resolve_walk_budget,
};
use super::{SearchVerdict, into_verified_handle, resolve_window_root};
use crate::system::runtime::hresult::ReadDisposition;
use crate::tree::automation::{UiaFailure, uia_failure_disposition};
use crate::tree::element::UIAElement;
use crate::tree::walk::walker::{TreeSource, WalkBudget};
use crate::tree::walk::walker_source::UiaTreeSource;

/// Resolves a ref that must find one live element by its recorded identity. The window gate runs
/// first, and a sole candidate beside an unread region is retried rather than resolved.
pub(crate) fn resolve_unique_identity(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;
    let budget = resolve_walk_budget(deadline);
    let mut found: Vec<UIAElement> = Vec::new();
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
            .map(|candidate| into_verified_handle(candidate, entry))
            .ok_or_else(|| stale_ref_error(entry)),
        SearchVerdict::Ambiguous => Err(ambiguous_target_error(entry, found.len())),
    }
}

/// What a live count decides: two matches are ambiguous whatever stays unread, while zero and one
/// are withheld until the whole scope has been read.
pub(super) fn classify_identity_search(count: usize, unread_region: bool) -> SearchVerdict {
    match count {
        0 if unread_region => SearchVerdict::Incomplete,
        0 => SearchVerdict::Stale,
        1 if unread_region => SearchVerdict::Incomplete,
        1 => SearchVerdict::Resolved(0),
        _ => SearchVerdict::Ambiguous,
    }
}

/// Whether the identity search counts this live candidate, refutes it, or could not decide. The
/// role and identity rules are shared with the positional search; bounds never confirm here.
pub(crate) fn admit_identity_candidate(
    entry: &RefEntry,
    evidence: &LocatorEvidence,
) -> NodeAdmission {
    match role_admission(entry, evidence) {
        Some(verdict) => verdict,
        None => admitted_outcome(candidate_outcome(entry, evidence)),
    }
}

/// The uniqueness search's descent policy: only the end-of-list signal establishes that a child
/// list was read whole. The observation walk that issued the ref settles completeness on
/// `is_exhaustion()` alone, so a list it counted as truncated must not read as complete here;
/// `E_POINTER`, `E_INVALIDARG` and `UIA_E_NOTSUPPORTED` are provider answers about one read, not
/// proof that a subtree is empty, and a look-alike may be sitting behind any of them.
pub(crate) const IDENTITY_SEARCH_DESCENT: DescentPolicy = DescentPolicy {
    classify: identity_descent_verdict,
    on_expiry: ExpiryPolicy::Unfinish,
    descend_context: "descend to a stored ref",
    sibling_context: "walk a stored ref's siblings",
};

/// Classifies a descent failure for the uniqueness count: the end-of-list signal ends the list,
/// a terminal failure is the attempt's answer, and every other failure leaves the region unread.
fn identity_descent_verdict(failure: UiaFailure) -> DescentVerdict {
    if failure.is_exhaustion() {
        return DescentVerdict::Settled;
    }
    match uia_failure_disposition(failure) {
        ReadDisposition::Terminal => DescentVerdict::Surfaced,
        ReadDisposition::SettledAbsence
        | ReadDisposition::Retryable
        | ReadDisposition::Unavailable => DescentVerdict::Unfinished,
    }
}

/// Counts every candidate the recorded identity admits and records every region it could not read.
/// A second match ends the walk, so the settled `AMBIGUOUS_TARGET` verdict is never overwritten.
fn search_source_scope<S: TreeSource>(
    ctx: &SearchContext<'_, S>,
    element: &S::Node,
    depth: u8,
    found: &mut Vec<S::Node>,
    unread_region: &mut bool,
) -> Result<(), AdapterError> {
    if match_count_is_conclusive(found.len()) {
        return Ok(());
    }
    if ctx.budget.deadline.is_expired() {
        *unread_region = true;
        return Ok(());
    }
    let (_, evidence, _) = ctx.source.evidence(element);
    match admit_identity_candidate(ctx.entry, &evidence) {
        NodeAdmission::Collect => found.push(element.clone()),
        NodeAdmission::Unread => *unread_region = true,
        NodeAdmission::Reject => {}
    }
    if match_count_is_conclusive(found.len()) {
        return Ok(());
    }
    if at_deepest_searched_level(depth) {
        *unread_region |= boundary_hides_descendants(ctx.source, element);
        return Ok(());
    }
    let children = enumerate_children(
        ctx.source,
        element,
        ctx.budget,
        unread_region,
        &IDENTITY_SEARCH_DESCENT,
    )?;
    *unread_region |= sibling_list_was_cut(ctx.source, &children, ctx.budget);
    for child in &children {
        if match_count_is_conclusive(found.len()) {
            break;
        }
        search_source_scope(ctx, child, depth + 1, found, unread_region)?;
    }
    Ok(())
}

/// Whether a node at `depth` is the deepest one this search evaluates. A ref can name an element
/// at exactly `MAX_RESOLVE_DEPTH`, so that node is counted and the search stops one level below it.
fn at_deepest_searched_level(depth: u8) -> bool {
    depth >= MAX_RESOLVE_DEPTH
}

/// Whether the collected count already settles the verdict: two matches are ambiguous whatever is
/// unread, and this tier has no bounds tie-break, so a third candidate can only cost time.
fn match_count_is_conclusive(match_count: usize) -> bool {
    match_count > 1
}

/// Whether a node at the depth cap has children the search never read. One child probe is the only
/// way to tell a cut subtree from a real end, and a failed probe is unreadable, not empty.
fn boundary_hides_descendants<S: TreeSource>(source: &S, element: &S::Node) -> bool {
    !matches!(source.first_child(element), Err(failure) if failure.is_exhaustion())
}

/// Whether the sibling cap cut the child list short: the shared enumeration reports a cap-hit as a
/// whole list, and a failing probe is unreadable rather than empty.
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

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_identity_count_tests.rs"]
mod count_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_identity_scope_tests.rs"]
mod scope_tests;

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_identity_enumeration_tests.rs"]
mod enumeration_tests;
